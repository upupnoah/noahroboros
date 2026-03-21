use std::collections::HashMap;

use crate::market::Candle;
use crate::strategy::{Signal, Strategy};

#[derive(Debug, Clone, Copy, PartialEq)]
enum Position {
    Flat,
    Long,
    Short,
}

#[derive(Debug, Clone)]
pub struct TradeResult {
    pub symbol: String,
    pub entry_time: i64,
    pub exit_time: i64,
    pub direction: i8, // 1 = long, -1 = short
    pub entry_price: f64,
    pub exit_price: f64,
    pub size_usd: f64,
    pub pnl_abs: f64,
    pub pnl_pct: f64,
}

pub struct BacktestEngine {
    capital: f64,
    position_size_frac: f64,
}

impl BacktestEngine {
    pub fn new(capital: f64, position_size_frac: f64) -> Self {
        Self {
            capital,
            position_size_frac,
        }
    }

    /// Run backtest across all symbols. Capital is split equally per asset.
    /// Strategy is reset and run independently for each symbol.
    pub fn run_with(
        &mut self,
        strategy: &mut dyn Strategy,
        candles: &[Candle],
    ) -> Vec<TradeResult> {
        let by_symbol = group_by_symbol(candles);
        let num_assets = by_symbol.len().max(1);
        let capital_per_asset = self.capital / num_assets as f64;

        let mut all_trades = Vec::new();

        for (_symbol, symbol_candles) in &by_symbol {
            strategy.reset();
            let trades = self.run_single(strategy, symbol_candles, capital_per_asset);
            all_trades.extend(trades);
        }

        all_trades.sort_by_key(|t| t.entry_time);
        all_trades
    }

    fn run_single(
        &self,
        strategy: &mut dyn Strategy,
        candles: &[Candle],
        capital: f64,
    ) -> Vec<TradeResult> {
        let mut trades = Vec::new();
        let mut position = Position::Flat;
        let mut entry_price = 0.0;
        let mut entry_time = 0_i64;
        let mut equity = capital;

        let symbol = candles
            .first()
            .map(|c| c.symbol.clone())
            .unwrap_or_default();

        for candle in candles {
            let signal = strategy.on_candle(candle);
            let close = candle.close;

            match (position, signal) {
                (Position::Flat, Signal::Long) => {
                    position = Position::Long;
                    entry_price = close;
                    entry_time = candle.timestamp;
                }
                (Position::Flat, Signal::Short) => {
                    position = Position::Short;
                    entry_price = close;
                    entry_time = candle.timestamp;
                }
                (Position::Long, Signal::Short | Signal::Flat) => {
                    let size_usd = equity * self.position_size_frac;
                    let pnl_pct = (close - entry_price) / entry_price * 100.0;
                    let pnl_abs = size_usd * pnl_pct / 100.0;
                    equity += pnl_abs;

                    trades.push(TradeResult {
                        symbol: symbol.clone(),
                        entry_time,
                        exit_time: candle.timestamp,
                        direction: 1,
                        entry_price,
                        exit_price: close,
                        size_usd,
                        pnl_abs,
                        pnl_pct,
                    });

                    if signal == Signal::Short {
                        position = Position::Short;
                        entry_price = close;
                        entry_time = candle.timestamp;
                    } else {
                        position = Position::Flat;
                    }
                }
                (Position::Short, Signal::Long | Signal::Flat) => {
                    let size_usd = equity * self.position_size_frac;
                    let pnl_pct = (entry_price - close) / entry_price * 100.0;
                    let pnl_abs = size_usd * pnl_pct / 100.0;
                    equity += pnl_abs;

                    trades.push(TradeResult {
                        symbol: symbol.clone(),
                        entry_time,
                        exit_time: candle.timestamp,
                        direction: -1,
                        entry_price,
                        exit_price: close,
                        size_usd,
                        pnl_abs,
                        pnl_pct,
                    });

                    if signal == Signal::Long {
                        position = Position::Long;
                        entry_price = close;
                        entry_time = candle.timestamp;
                    } else {
                        position = Position::Flat;
                    }
                }
                _ => {}
            }
        }

        // Close any open position at end
        if position != Position::Flat {
            if let Some(last) = candles.last() {
                let close = last.close;
                let size_usd = equity * self.position_size_frac;
                let dir = if position == Position::Long { 1 } else { -1 };
                let pnl_pct = if dir == 1 {
                    (close - entry_price) / entry_price * 100.0
                } else {
                    (entry_price - close) / entry_price * 100.0
                };
                let pnl_abs = size_usd * pnl_pct / 100.0;

                trades.push(TradeResult {
                    symbol: symbol.clone(),
                    entry_time,
                    exit_time: last.timestamp,
                    direction: dir,
                    entry_price,
                    exit_price: close,
                    size_usd,
                    pnl_abs,
                    pnl_pct,
                });
            }
        }

        trades
    }
}

fn group_by_symbol(candles: &[Candle]) -> Vec<(String, Vec<Candle>)> {
    let mut map: HashMap<String, Vec<Candle>> = HashMap::new();
    for candle in candles {
        map.entry(candle.symbol.clone())
            .or_default()
            .push(candle.clone());
    }
    let mut result: Vec<_> = map.into_iter().collect();
    result.sort_by(|a, b| a.0.cmp(&b.0));
    result
}
