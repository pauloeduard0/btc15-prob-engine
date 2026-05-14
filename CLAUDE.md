# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

`btc15-prob-engine` is a Rust binary (edition 2024) — a real-time BTC/USDT signal bot for 15-minute candles on Binance, designed to identify high-probability reversal entries on Polymarket.

## Architecture

```
src/
  main.rs      — async event loop (tokio::select! over WebSocket events + keyboard)
  binance.rs   — Binance WebSocket (btcusdt@kline_15m) + REST history fetch
  types.rs     — Candle, Signal, AppState, AppEvent structs; CVD calculation; thresholds
  signals.rs   — pure signal detection logic
  ui.rs        — ratatui TUI (header, candle panel, CVD panel, signals table)
```

## Signal Logic

Identifies **rejection spikes** (large wick + high volume) in a divergent CVD regime:

- **LONG** → rolling CVD < 0 + lower wick ≥ 35% of range + volume ≥ 1.5× avg + lower wick > upper wick
- **SHORT** → rolling CVD > 0 + upper wick ≥ 35% of range + volume ≥ 1.5× avg + upper wick > lower wick

CVD per candle = `2 × taker_buy_vol − total_volume` (uses Binance kline field `V`).
Rolling CVD = sum of deltas over the last 20 closed candles + current candle delta.

## Thresholds (tune in `src/types.rs`)

| Constant | Default | Description |
|---|---|---|
| `WICK_THRESHOLD` | `0.35` | Min wick size as fraction of candle range |
| `VOLUME_THRESHOLD` | `1.5` | Min volume as multiple of 20-candle average |
| `MIN_RANGE_USD` | `50.0` | Min candle range in USD to consider |
| `CVD_WINDOW` | `20` | Candles for rolling CVD |
| `VOLUME_WINDOW` | `20` | Candles for average volume baseline |
| `MIN_CANDLES` | `5` | Min history before emitting signals |

## Commands

```bash
# Build
cargo build

# Build release
cargo build --release

# Run (starts TUI, connects to Binance WebSocket)
cargo run

# Lint
cargo clippy

# Format
cargo fmt
```

## TUI Keys

- `q` / Esc — quit
- `c` — clear signal alert
