pub mod download;

use anyhow::Result;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct Candle {
    pub timestamp: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    /// Which asset this candle belongs to (e.g. "BTCUSDT")
    #[serde(default)]
    pub symbol: String,
}

pub struct CsvLoader;

impl CsvLoader {
    /// Load all CSV files from a directory, sorted by timestamp.
    /// Each CSV must have columns: timestamp, open, high, low, close, volume
    /// Optionally: symbol (otherwise inferred from filename)
    pub fn load_dir(dir: &str) -> Result<Vec<Candle>> {
        let path = Path::new(dir);
        if !path.is_dir() {
            anyhow::bail!("{dir} is not a directory");
        }

        let mut all_candles = Vec::new();

        let mut entries: Vec<_> = std::fs::read_dir(path)?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .map_or(false, |ext| ext == "csv")
            })
            .collect();
        entries.sort_by_key(|e| e.file_name());

        for entry in entries {
            let file_path = entry.path();
            let symbol = file_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("UNKNOWN")
                .to_uppercase();

            let mut rdr = csv::Reader::from_path(&file_path)?;
            for result in rdr.deserialize() {
                let mut candle: Candle = result?;
                if candle.symbol.is_empty() {
                    candle.symbol = symbol.clone();
                }
                all_candles.push(candle);
            }
        }

        all_candles.sort_by_key(|c| c.timestamp);
        Ok(all_candles)
    }
}

/// Trait for real-time market data feeds (not used in autoresearch, but defines the interface)
#[allow(async_fn_in_trait)]
pub trait MarketFeed {
    async fn historical_candles(
        &self,
        symbol: &str,
        interval: &str,
        start: i64,
        end: i64,
    ) -> Result<Vec<Candle>>;
}
