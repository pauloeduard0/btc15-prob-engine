use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio::sync::mpsc::UnboundedSender;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::types::{AppEvent, Candle, RawKline};

const WS_URL: &str = "wss://stream.binance.com:9443/ws/btcusdt@kline_15m";
const REST_URL: &str = "https://api.binance.com/api/v3/klines";

pub async fn fetch_history(client: &reqwest::Client) -> Vec<Candle> {
    let result = client
        .get(REST_URL)
        .query(&[("symbol", "BTCUSDT"), ("interval", "15m"), ("limit", "50")])
        .send()
        .await;

    let data: Vec<Vec<Value>> = match result {
        Ok(r) => r.json().await.unwrap_or_default(),
        Err(_) => return vec![],
    };

    // Skip the last entry which is the current (still-open) candle
    data.iter()
        .take(data.len().saturating_sub(1))
        .filter_map(parse_rest_candle)
        .collect()
}

fn parse_rest_candle(row: &Vec<Value>) -> Option<Candle> {
    Some(Candle {
        open_time: row.first()?.as_i64()?,
        open: row.get(1)?.as_str()?.parse().ok()?,
        high: row.get(2)?.as_str()?.parse().ok()?,
        low: row.get(3)?.as_str()?.parse().ok()?,
        close: row.get(4)?.as_str()?.parse().ok()?,
        volume: row.get(5)?.as_str()?.parse().ok()?,
        taker_buy_vol: row.get(9)?.as_str()?.parse().ok()?,
    })
}

pub async fn run_ws(tx: UnboundedSender<AppEvent>) {
    loop {
        match connect_async(WS_URL).await {
            Ok((stream, _)) => {
                let _ = tx.send(AppEvent::Connected);
                let (mut write, mut read) = stream.split();

                while let Some(Ok(msg)) = read.next().await {
                    match msg {
                        Message::Text(text) => {
                            if let Ok(kline) = parse_ws_kline(&text) {
                                if tx.send(AppEvent::Kline(kline)).is_err() {
                                    return;
                                }
                            }
                        }
                        Message::Ping(data) => {
                            let _ = write.send(Message::Pong(data)).await;
                        }
                        Message::Close(_) => break,
                        _ => {}
                    }
                }
            }
            Err(_) => {}
        }

        let _ = tx.send(AppEvent::Reconnecting);
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

fn parse_ws_kline(text: &str) -> Result<RawKline, serde_json::Error> {
    #[derive(serde::Deserialize)]
    struct Msg {
        k: RawKline,
    }
    let m: Msg = serde_json::from_str(text)?;
    Ok(m.k)
}
