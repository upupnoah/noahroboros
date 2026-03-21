use std::env;

pub struct Config {
    pub data_dir: String,
    pub initial_capital: f64,
    pub position_size_frac: f64,
    pub scoring: ScoringConfig,
    pub binance: BinanceConfig,
    pub lighter: LighterConfig,
    pub download: DownloadConfig,
}

pub struct DownloadConfig {
    pub symbols: Vec<String>,
    pub interval: String,
    pub months: u32,
}

pub struct ScoringConfig {
    pub w_sharpe: f64,
    pub w_drawdown: f64,
    pub w_return: f64,
    pub w_turnover: f64,
    pub sharpe_annualize: f64,
}

pub struct BinanceConfig {
    pub api_key: String,
    pub api_secret: String,
    pub base_url: String,
}

pub struct LighterConfig {
    pub api_url: String,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            data_dir: env_or("DATA_DIR", "data"),
            initial_capital: env_parse("INITIAL_CAPITAL", 10_000.0),
            position_size_frac: env_parse("POSITION_SIZE_FRAC", 1.0),
            scoring: ScoringConfig {
                w_sharpe: env_parse("SCORE_W_SHARPE", 0.4),
                w_drawdown: env_parse("SCORE_W_DRAWDOWN", 0.3),
                w_return: env_parse("SCORE_W_RETURN", 0.2),
                w_turnover: env_parse("SCORE_W_TURNOVER", 0.1),
                sharpe_annualize: env_parse("SHARPE_ANNUALIZE", 365.0),
            },
            binance: BinanceConfig {
                api_key: env_or("BINANCE_API_KEY", ""),
                api_secret: env_or("BINANCE_API_SECRET", ""),
                base_url: env_or("BINANCE_BASE_URL", "https://api.binance.com"),
            },
            lighter: LighterConfig {
                api_url: env_or(
                    "LIGHTER_API_URL",
                    "https://mainnet.zklighter.elliot.ai/api/v1",
                ),
            },
            download: DownloadConfig {
                symbols: env_or("DOWNLOAD_SYMBOLS", "BTCUSDT,ETHUSDT,SOLUSDT")
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect(),
                interval: env_or("DOWNLOAD_INTERVAL", "1h"),
                months: env_parse("DOWNLOAD_MONTHS", 9),
            },
        }
    }
}

fn env_or(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

fn env_parse<T: std::str::FromStr>(key: &str, default: T) -> T {
    env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}
