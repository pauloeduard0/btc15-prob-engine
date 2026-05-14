use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::collections::{HashSet, VecDeque};

// --- Thresholds (tune here) ---
pub const CVD_WINDOW: usize = 20;       // candles for rolling CVD
pub const VOLUME_WINDOW: usize = 20;    // candles for avg volume baseline
pub const MIN_CANDLES: usize = 5;       // minimum history before signaling
pub const WICK_THRESHOLD: f64 = 0.35;  // wick must be >= 35% of candle range
pub const VOLUME_THRESHOLD: f64 = 1.5; // volume must be >= 1.5x average
pub const MIN_RANGE_USD: f64 = 50.0;   // ignore candles narrower than $50

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

    pub fn range(&self) -> f64 {
        self.high - self.low
    }

    pub fn lower_wick(&self) -> f64 {
        self.open.min(self.close) - self.low
    }

    pub fn upper_wick(&self) -> f64 {
        self.high - self.open.max(self.close)
    }

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
pub enum Direction {
    Long,
    Short,
}

impl std::fmt::Display for Direction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Direction::Long => write!(f, "LONG ↑"),
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
            seen: HashSet::new(),
        }
    }

    pub fn load_history(&mut self, candles: Vec<Candle>) {
        for c in candles {
            self.seen.insert(c.open_time);
            self.candles.push_back(c);
        }
        while self.candles.len() > 100 {
            self.candles.pop_front();
        }
        self.status = format!("Loaded {} historical candles", self.candles.len());
    }

    pub fn rolling_cvd(&self) -> f64 {
        self.candles.iter().rev().take(CVD_WINDOW).map(|c| c.delta()).sum()
    }

    pub fn live_cvd(&self) -> f64 {
        let base = self.rolling_cvd();
        self.current.as_ref().map_or(base, |c| base + c.delta())
    }

    pub fn avg_volume(&self) -> f64 {
        let v: Vec<f64> = self.candles.iter().rev().take(VOLUME_WINDOW).map(|c| c.volume).collect();
        if v.is_empty() { 1.0 } else { v.iter().sum::<f64>() / v.len() as f64 }
    }

    // Returns a signal if the closed candle triggers one
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

                let signal = if self.candles.len() >= MIN_CANDLES {
                    let prev_cvd = self.rolling_cvd();
                    let cvd = prev_cvd + candle.delta();
                    let avg_vol = self.avg_volume();
                    crate::signals::check(&candle, cvd, prev_cvd, avg_vol)
                } else {
                    None
                };

                if self.candles.len() >= 100 {
                    self.candles.pop_front();
                }
                self.candles.push_back(candle);
                self.status = "Live".to_string();

                signal
            }
        }
    }

    pub fn push_signal(&mut self, sig: Signal) {
        if self.signals.len() >= 50 {
            self.signals.pop_back();
        }
        self.signals.push_front(sig);
        self.alert_since = Some(std::time::Instant::now());
        print!("\x07"); // terminal bell
    }

    pub fn is_alerting(&self) -> bool {
        self.alert_since
            .map(|t| t.elapsed().as_secs() < 30)
            .unwrap_or(false)
    }

    pub fn clear_alert(&mut self) {
        self.alert_since = None;
    }
}
