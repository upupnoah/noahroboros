use std::env;

pub struct Config {
    pub data_dir: String,
    pub initial_capital: f64,
    pub position_size_frac: f64,
    pub fee_bps: f64,
    pub slippage_bps: f64,
    pub scoring: ScoringConfig,
    pub binance: BinanceConfig,
    pub lighter: LighterConfig,
    pub download: DownloadConfig,
}

pub struct DownloadConfig {
    pub symbols: Vec<String>,
    pub interval: String,
    pub months: u32,
    pub market: String,
}

pub struct ScoringConfig {
    /// Bars per year for Sharpe annualization (8760 for 1h, 525600 for 1m)
    pub sharpe_annualize: f64,
    /// Drawdown below this % is free (no penalty)
    pub dd_free_pct: f64,
    /// Penalty per % of drawdown above dd_free_pct
    pub dd_penalty_mult: f64,
    /// Annual turnover below this is free
    pub turnover_free: f64,
    /// Penalty per unit of turnover above turnover_free
    pub turnover_penalty_mult: f64,
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
            initial_capital: env_parse("INITIAL_CAPITAL", 100_000.0),
            position_size_frac: env_parse("POSITION_SIZE_FRAC", 0.08),
            fee_bps: env_parse("FEE_BPS", 5.0),
            slippage_bps: env_parse("SLIPPAGE_BPS", 1.0),
            scoring: ScoringConfig {
                sharpe_annualize: env_parse("SHARPE_ANNUALIZE", 8760.0),
                dd_free_pct: env_parse("DD_FREE_PCT", 15.0),
                dd_penalty_mult: env_parse("DD_PENALTY_MULT", 0.05),
                turnover_free: env_parse("TURNOVER_FREE", 500.0),
                turnover_penalty_mult: env_parse("TURNOVER_PENALTY_MULT", 0.001),
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
                market: env_or("DOWNLOAD_MARKET", "spot"),
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
