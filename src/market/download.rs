use anyhow::{Context, Result};
use serde::Deserialize;
use std::fmt;
use std::io::Write;
use std::path::Path;

const KLINE_LIMIT: u32 = 1000;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Market {
    Spot,
    Futures,
}

impl Market {
    pub fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "spot" => Ok(Market::Spot),
            "futures" | "perp" | "perpetual" | "usdt-m" => Ok(Market::Futures),
            _ => anyhow::bail!("Unknown market type: {s}. Use 'spot' or 'futures'"),
        }
    }

    fn base_url(&self) -> &str {
        match self {
            Market::Spot => "https://api.binance.com",
            Market::Futures => "https://fapi.binance.com",
        }
    }

    fn kline_path(&self) -> &str {
        match self {
            Market::Spot => "/api/v3/klines",
            Market::Futures => "/fapi/v1/klines",
        }
    }
}

impl fmt::Display for Market {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Market::Spot => write!(f, "spot"),
            Market::Futures => write!(f, "futures"),
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawKline(Vec<serde_json::Value>);

pub struct Downloader {
    base_url: String,
    kline_path: String,
    client: reqwest::blocking::Client,
}

impl Downloader {
    pub fn new(market: Market) -> Self {
        Self {
            base_url: market.base_url().to_string(),
            kline_path: market.kline_path().to_string(),
            client: reqwest::blocking::Client::new(),
        }
    }

    /// Download historical klines for a symbol and write to CSV.
    pub fn download(
        &self,
        symbol: &str,
        interval: &str,
        start_ms: i64,
        end_ms: i64,
        out_dir: &str,
    ) -> Result<usize> {
        let dir = Path::new(out_dir).join(interval);
        std::fs::create_dir_all(&dir)?;

        let path = dir.join(format!("{symbol}.csv"));
        let mut file = std::fs::File::create(&path)
            .with_context(|| format!("Failed to create {}", path.display()))?;

        writeln!(file, "timestamp,open,high,low,close,volume")?;

        let mut current_start = start_ms;
        let mut total_rows = 0_usize;

        loop {
            if current_start >= end_ms {
                break;
            }

            let url = format!(
                "{}{}?symbol={}&interval={}&startTime={}&endTime={}&limit={}",
                self.base_url, self.kline_path, symbol, interval, current_start, end_ms, KLINE_LIMIT
            );

            let resp: Vec<RawKline> = self
                .client
                .get(&url)
                .send()
                .with_context(|| format!("Request failed for {symbol}"))?
                .json()
                .with_context(|| format!("Failed to parse kline response for {symbol}"))?;

            if resp.is_empty() {
                break;
            }

            for kline in &resp {
                let vals = &kline.0;
                if vals.len() < 6 {
                    continue;
                }
                let ts = vals[0].as_i64().unwrap_or(0) / 1000;
                let open = val_as_f64(&vals[1]);
                let high = val_as_f64(&vals[2]);
                let low = val_as_f64(&vals[3]);
                let close = val_as_f64(&vals[4]);
                let volume = val_as_f64(&vals[5]);

                writeln!(file, "{ts},{open},{high},{low},{close},{volume}")?;
                total_rows += 1;
            }

            let last_ts = resp.last().unwrap().0[0].as_i64().unwrap_or(end_ms);
            current_start = last_ts + 1;

            if resp.len() == KLINE_LIMIT as usize {
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }

        Ok(total_rows)
    }
}

fn val_as_f64(v: &serde_json::Value) -> f64 {
    match v {
        serde_json::Value::String(s) => s.parse().unwrap_or(0.0),
        serde_json::Value::Number(n) => n.as_f64().unwrap_or(0.0),
        _ => 0.0,
    }
}
