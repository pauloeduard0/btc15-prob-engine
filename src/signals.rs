use chrono::Utc;

use crate::types::{Candle, Direction, Signal, MIN_RANGE_USD, VOLUME_THRESHOLD, WICK_THRESHOLD};

pub fn check(candle: &Candle, cvd: f64, prev_cvd: f64, avg_vol: f64) -> Option<Signal> {
    if candle.range() < MIN_RANGE_USD {
        return None;
    }

    let vol_ratio = if avg_vol > 0.0 { candle.volume / avg_vol } else { return None };
    if vol_ratio < VOLUME_THRESHOLD {
        return None;
    }

    let lower = candle.lower_wick_ratio();
    let upper = candle.upper_wick_ratio();

    // CVD em território bearish OU cruzou zero para baixo nesse candle
    let cvd_long = cvd < 0.0 || (prev_cvd > 0.0 && cvd < 0.0);
    // CVD em território bullish OU cruzou zero para cima nesse candle (ex: spike de compra rejeitado)
    let cvd_short = cvd > 0.0 || (prev_cvd < 0.0 && cvd > 0.0);

    // LONG: bearish close + sombra baixo dominante + CVD bearish
    if cvd_long && candle.close < candle.open && lower >= WICK_THRESHOLD && lower > upper {
        return Some(Signal {
            time: Utc::now(),
            direction: Direction::Long,
            price: candle.close,
            wick_ratio: lower,
            volume_ratio: vol_ratio,
            cvd,
        });
    }

    // SHORT: bullish close + sombra cima dominante + CVD bullish (inclui spike de compra que virou CVD positivo)
    if cvd_short && candle.close > candle.open && upper >= WICK_THRESHOLD && upper > lower {
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
