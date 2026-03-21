use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::config::{BinanceConfig, LighterConfig};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub symbol: String,
    pub side: OrderSide,
    pub price: Option<f64>,
    pub size: f64,
    pub order_type: OrderType,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum OrderType {
    Market,
    Limit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderResponse {
    pub id: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Balance {
    pub asset: String,
    pub free: f64,
    pub locked: f64,
}

/// Exchange connector trait.
/// Not used during autoresearch (backtest-only), but defines the interface
/// for live trading with Binance and Lighter.xyz.
#[allow(async_fn_in_trait)]
pub trait Exchange {
    fn name(&self) -> &str;
    async fn place_order(&self, order: &Order) -> Result<OrderResponse>;
    async fn cancel_order(&self, order_id: &str) -> Result<()>;
    async fn get_balances(&self) -> Result<Vec<Balance>>;
}

// --- Binance ---

pub struct BinanceExchange {
    pub api_key: String,
    pub api_secret: String,
    pub base_url: String,
}

impl BinanceExchange {
    pub fn from_config(cfg: &BinanceConfig) -> Self {
        Self {
            api_key: cfg.api_key.clone(),
            api_secret: cfg.api_secret.clone(),
            base_url: cfg.base_url.clone(),
        }
    }
}

impl Exchange for BinanceExchange {
    fn name(&self) -> &str {
        "binance"
    }

    async fn place_order(&self, _order: &Order) -> Result<OrderResponse> {
        todo!("Binance order placement not implemented")
    }

    async fn cancel_order(&self, _order_id: &str) -> Result<()> {
        todo!("Binance order cancellation not implemented")
    }

    async fn get_balances(&self) -> Result<Vec<Balance>> {
        todo!("Binance balance query not implemented")
    }
}

// --- Lighter.xyz ---

pub struct LighterExchange {
    pub api_url: String,
}

impl LighterExchange {
    pub fn from_config(cfg: &LighterConfig) -> Self {
        Self {
            api_url: cfg.api_url.clone(),
        }
    }
}

impl Exchange for LighterExchange {
    fn name(&self) -> &str {
        "lighter"
    }

    async fn place_order(&self, _order: &Order) -> Result<OrderResponse> {
        todo!("Lighter order placement not implemented")
    }

    async fn cancel_order(&self, _order_id: &str) -> Result<()> {
        todo!("Lighter order cancellation not implemented")
    }

    async fn get_balances(&self) -> Result<Vec<Balance>> {
        todo!("Lighter balance query not implemented")
    }
}
