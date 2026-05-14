use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::collections::{HashSet, VecDeque};

// --- Thresholds (tune here) ---
pub const VOLUME_WINDOW: usize = 20;    // candles for avg volume baseline
pub const MIN_CANDLES: usize = 5;       // minimum history before signaling
pub const WICK_THRESHOLD: f64 = 0.35;  // wick must be >= 35% of candle range
pub const VOLUME_THRESHOLD: f64 = 1.5; // volume must be >= 1.5x average
pub const MIN_RANGE_USD: f64 = 50.0;   // ignore candles narrower than $50

// Session resets at UTC midnight — same as Binance/TradingView daily CVD
const DAY_MS: i64 = 86_400_000;

fn day_start(ts_ms: i64) -> i64 {
    (ts_ms / DAY_MS) * DAY_MS
}

// Raw kline from Binance WebSocket
#[derive(Debug, Clone, Deserialize)]
pub struct RawKline {
    #[serde(rename = "t")] pub open_time: i64,
    #[serde(rename = "o")] pub open: String,
    #[serde(rename = "h")] pub high: String,
    #[serde(rename = "l")] pub low: String,
    #[serde(rename = "c")] pub close: String,
    #[serde(rename = "v")] pub volume: String,
    #[serde(rename = "x")] pub is_closed: bool,
    #[serde(rename = "V")] pub taker_buy_vol: String,
}

#[derive(Debug, Clone)]
pub struct Candle {
    pub open_time: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub taker_buy_vol: f64,
}

impl Candle {
    pub fn from_raw(r: &RawKline) -> Self {
        Self {
            open_time: r.open_time,
            open: r.open.parse().unwrap_or(0.0),
            high: r.high.parse().unwrap_or(0.0),
            low: r.low.parse().unwrap_or(0.0),
            close: r.close.parse().unwrap_or(0.0),
            volume: r.volume.parse().unwrap_or(0.0),
            taker_buy_vol: r.taker_buy_vol.parse().unwrap_or(0.0),
        }
    }

    // buy_vol - sell_vol (positive = net buying pressure)
    pub fn delta(&self) -> f64 {
        2.0 * self.taker_buy_vol - self.volume
    }

    pub fn range(&self) -> f64 { self.high - self.low }

    pub fn lower_wick(&self) -> f64 { self.open.min(self.close) - self.low }

    pub fn upper_wick(&self) -> f64 { self.high - self.open.max(self.close) }

    pub fn lower_wick_ratio(&self) -> f64 {
        let r = self.range();
        if r < 0.01 { 0.0 } else { self.lower_wick() / r }
    }

    pub fn upper_wick_ratio(&self) -> f64 {
        let r = self.range();
        if r < 0.01 { 0.0 } else { self.upper_wick() / r }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Direction { Long, Short }

impl std::fmt::Display for Direction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Direction::Long  => write!(f, "LONG ↑"),
            Direction::Short => write!(f, "SHORT ↓"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Signal {
    pub time: DateTime<Utc>,
    pub direction: Direction,
    pub price: f64,
    pub wick_ratio: f64,
    pub volume_ratio: f64,
    pub cvd: f64,
}

pub enum AppEvent {
    Kline(RawKline),
    Connected,
    Reconnecting,
}

pub struct AppState {
    pub candles: VecDeque<Candle>,
    pub current: Option<Candle>,
    pub signals: VecDeque<Signal>,
    pub status: String,
    pub connected: bool,
    pub alert_since: Option<std::time::Instant>,
    // Daily session CVD — resets at 00:00 UTC, matching Binance/TradingView
    pub session_cvd: f64,
    pub session_day_start: i64, // UTC midnight of current session (ms)
    seen: HashSet<i64>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            candles: VecDeque::with_capacity(120),
            current: None,
            signals: VecDeque::with_capacity(50),
            status: "Connecting...".to_string(),
            connected: false,
            alert_since: None,
            session_cvd: 0.0,
            session_day_start: 0,
            seen: HashSet::new(),
        }
    }

    pub fn load_history(&mut self, candles: Vec<Candle>) {
        for c in candles {
            self.seen.insert(c.open_time);
            let day = day_start(c.open_time);
            if day != self.session_day_start {
                self.session_day_start = day;
                self.session_cvd = 0.0;
            }
            self.session_cvd += c.delta();
            self.candles.push_back(c);
        }
        while self.candles.len() > 100 {
            self.candles.pop_front();
        }
        self.status = format!("Loaded {} historical candles", self.candles.len());
    }

    // Session CVD including the live (open) candle
    pub fn live_cvd(&self) -> f64 {
        self.session_cvd + self.current.as_ref().map_or(0.0, |c| c.delta())
    }

    pub fn avg_volume(&self) -> f64 {
        let v: Vec<f64> = self.candles.iter().rev().take(VOLUME_WINDOW).map(|c| c.volume).collect();
        if v.is_empty() { 1.0 } else { v.iter().sum::<f64>() / v.len() as f64 }
    }

    // Session CVD as it was when the candle `age` steps back closed (age=0 = most recent)
    pub fn historical_session_cvd(&self, age: usize) -> f64 {
        let n = self.candles.len();
        if age >= n { return 0.0; }
        let target_day = day_start(self.candles[n - 1 - age].open_time);
        self.candles.iter()
            .take(n - age)
            .filter(|c| day_start(c.open_time) == target_day)
            .map(|c| c.delta())
            .sum()
    }

    pub fn process(&mut self, event: AppEvent) -> Option<Signal> {
        match event {
            AppEvent::Connected => {
                self.connected = true;
                self.status = "Live".to_string();
                None
            }
            AppEvent::Reconnecting => {
                self.connected = false;
                self.status = "Reconnecting...".to_string();
                None
            }
            AppEvent::Kline(raw) => {
                let candle = Candle::from_raw(&raw);
                self.current = Some(candle.clone());
                self.connected = true;

                if !raw.is_closed || self.seen.contains(&raw.open_time) {
                    return None;
                }
                self.seen.insert(raw.open_time);

                // Session reset at UTC midnight
                let candle_day = day_start(candle.open_time);
                if candle_day != self.session_day_start {
                    self.session_day_start = candle_day;
                    self.session_cvd = 0.0;
                }

                let prev_cvd = self.session_cvd;
                let cvd = prev_cvd + candle.delta();

                let signal = if self.candles.len() >= MIN_CANDLES {
                    let avg_vol = self.avg_volume();
                    crate::signals::check(&candle, cvd, prev_cvd, avg_vol)
                } else {
                    None
                };

                self.session_cvd = cvd;
                if self.candles.len() >= 100 { self.candles.pop_front(); }
                self.candles.push_back(candle);
                self.status = "Live".to_string();

                signal
            }
        }
    }

    pub fn push_signal(&mut self, sig: Signal) {
        if self.signals.len() >= 50 { self.signals.pop_back(); }
        self.signals.push_front(sig);
        self.alert_since = Some(std::time::Instant::now());
        print!("\x07");
    }

    pub fn is_alerting(&self) -> bool {
        self.alert_since.map(|t| t.elapsed().as_secs() < 30).unwrap_or(false)
    }

    pub fn clear_alert(&mut self) {
        self.alert_since = None;
    }
}
