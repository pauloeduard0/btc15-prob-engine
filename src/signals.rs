use chrono::Utc;

use crate::types::{Candle, Direction, Signal, MIN_RANGE_USD, VOLUME_THRESHOLD, WICK_THRESHOLD};

pub fn check(candle: &Candle, cvd: f64, avg_vol: f64) -> Option<Signal> {
    // Ignore tiny candles
    if candle.range() < MIN_RANGE_USD {
        return None;
    }

    // Volume must be above threshold
    let vol_ratio = if avg_vol > 0.0 { candle.volume / avg_vol } else { return None };
    if vol_ratio < VOLUME_THRESHOLD {
        return None;
    }

    let lower = candle.lower_wick_ratio();
    let upper = candle.upper_wick_ratio();

    // LONG signal: CVD negative + dominant lower wick (rejection of selling)
    if cvd < 0.0 && lower >= WICK_THRESHOLD && lower > upper {
        return Some(Signal {
            time: Utc::now(),
            direction: Direction::Long,
            price: candle.close,
            wick_ratio: lower,
            volume_ratio: vol_ratio,
            cvd,
        });
    }

    // SHORT signal: CVD positive + dominant upper wick (rejection of buying)
    if cvd > 0.0 && upper >= WICK_THRESHOLD && upper > lower {
        return Some(Signal {
            time: Utc::now(),
            direction: Direction::Short,
            price: candle.close,
            wick_ratio: upper,
            volume_ratio: vol_ratio,
            cvd,
        });
    }

    None
}
