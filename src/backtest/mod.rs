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
    pub direction: i8,
    pub entry_price: f64,
    pub exit_price: f64,
    pub size_usd: f64,
    pub pnl_abs: f64,
    pub pnl_pct: f64,
}

/// Full backtest output including equity curve for Sharpe calculation.
pub struct BacktestOutput {
    pub trades: Vec<TradeResult>,
    pub equity_curve: Vec<f64>,
    pub total_bars: usize,
    pub total_volume: f64,
}

pub struct BacktestEngine {
    capital: f64,
    position_size_frac: f64,
    fee_bps: f64,
    slippage_bps: f64,
}

impl BacktestEngine {
    pub fn new(capital: f64, position_size_frac: f64, fee_bps: f64, slippage_bps: f64) -> Self {
        Self {
            capital,
            position_size_frac,
            fee_bps,
            slippage_bps,
        }
    }

    pub fn run_with(
        &mut self,
        strategy: &mut dyn Strategy,
        candles: &[Candle],
    ) -> BacktestOutput {
        let by_symbol = group_by_symbol(candles);
        let num_assets = by_symbol.len().max(1);
        let capital_per_asset = self.capital / num_assets as f64;

        let mut all_trades = Vec::new();
        let mut symbol_equities: Vec<Vec<(i64, f64)>> = Vec::new();
        let mut total_volume = 0.0_f64;

        for (_symbol, symbol_candles) in &by_symbol {
            strategy.reset();
            let (trades, bar_equities, volume) =
                self.run_single(strategy, symbol_candles, capital_per_asset);
            all_trades.extend(trades);
            symbol_equities.push(bar_equities);
            total_volume += volume;
        }

        all_trades.sort_by_key(|t| t.entry_time);

        let equity_curve = merge_equity_curves(&symbol_equities, self.capital);
        let total_bars = equity_curve.len();

        BacktestOutput {
            trades: all_trades,
            equity_curve,
            total_bars,
            total_volume,
        }
    }

    fn run_single(
        &self,
        strategy: &mut dyn Strategy,
        candles: &[Candle],
        capital: f64,
    ) -> (Vec<TradeResult>, Vec<(i64, f64)>, f64) {
        let mut trades = Vec::new();
        let mut position = Position::Flat;
        let mut entry_price = 0.0;
        let mut entry_time = 0_i64;
        let mut equity = capital;
        let mut total_volume = 0.0_f64;

        let symbol = candles
            .first()
            .map(|c| c.symbol.clone())
            .unwrap_or_default();

        let fee_mult = self.fee_bps / 10_000.0;
        let slip_mult = self.slippage_bps / 10_000.0;

        let mut bar_equities: Vec<(i64, f64)> = Vec::with_capacity(candles.len());

        for candle in candles {
            let signal = strategy.on_candle(candle);
            let close = candle.close;

            match (position, signal) {
                (Position::Flat, Signal::Long) => {
                    let exec_price = close * (1.0 + slip_mult);
                    let size = equity * self.position_size_frac;
                    let fee = size * fee_mult;
                    equity -= fee;
                    total_volume += size;
                    position = Position::Long;
                    entry_price = exec_price;
                    entry_time = candle.timestamp;
                }
                (Position::Flat, Signal::Short) => {
                    let exec_price = close * (1.0 - slip_mult);
                    let size = equity * self.position_size_frac;
                    let fee = size * fee_mult;
                    equity -= fee;
                    total_volume += size;
                    position = Position::Short;
                    entry_price = exec_price;
                    entry_time = candle.timestamp;
                }
                (Position::Long, Signal::Short | Signal::Flat) => {
                    let exec_price = close * (1.0 - slip_mult);
                    let size_usd = equity * self.position_size_frac;
                    let pnl_pct = (exec_price - entry_price) / entry_price * 100.0;
                    let pnl_abs = size_usd * pnl_pct / 100.0;
                    let fee = size_usd * fee_mult;
                    equity += pnl_abs - fee;
                    total_volume += size_usd;

                    trades.push(TradeResult {
                        symbol: symbol.clone(),
                        entry_time,
                        exit_time: candle.timestamp,
                        direction: 1,
                        entry_price,
                        exit_price: exec_price,
                        size_usd,
                        pnl_abs: pnl_abs - fee,
                        pnl_pct,
                    });

                    if signal == Signal::Short {
                        let exec_price2 = close * (1.0 - slip_mult);
                        let fee2 = equity * self.position_size_frac * fee_mult;
                        equity -= fee2;
                        total_volume += equity * self.position_size_frac;
                        position = Position::Short;
                        entry_price = exec_price2;
                        entry_time = candle.timestamp;
                    } else {
                        position = Position::Flat;
                    }
                }
                (Position::Short, Signal::Long | Signal::Flat) => {
                    let exec_price = close * (1.0 + slip_mult);
                    let size_usd = equity * self.position_size_frac;
                    let pnl_pct = (entry_price - exec_price) / entry_price * 100.0;
                    let pnl_abs = size_usd * pnl_pct / 100.0;
                    let fee = size_usd * fee_mult;
                    equity += pnl_abs - fee;
                    total_volume += size_usd;

                    trades.push(TradeResult {
                        symbol: symbol.clone(),
                        entry_time,
                        exit_time: candle.timestamp,
                        direction: -1,
                        entry_price,
                        exit_price: exec_price,
                        size_usd,
                        pnl_abs: pnl_abs - fee,
                        pnl_pct,
                    });

                    if signal == Signal::Long {
                        let exec_price2 = close * (1.0 + slip_mult);
                        let fee2 = equity * self.position_size_frac * fee_mult;
                        equity -= fee2;
                        total_volume += equity * self.position_size_frac;
                        position = Position::Long;
                        entry_price = exec_price2;
                        entry_time = candle.timestamp;
                    } else {
                        position = Position::Flat;
                    }
                }
                _ => {
                    // Mark-to-market: update equity for open position
                    if position == Position::Long {
                        let unrealized = equity * self.position_size_frac
                            * (close - entry_price)
                            / entry_price;
                        bar_equities.push((candle.timestamp, equity + unrealized));
                        continue;
                    } else if position == Position::Short {
                        let unrealized = equity * self.position_size_frac
                            * (entry_price - close)
                            / entry_price;
                        bar_equities.push((candle.timestamp, equity + unrealized));
                        continue;
                    }
                }
            }

            bar_equities.push((candle.timestamp, equity));
        }

        // Close open position at end
        if position != Position::Flat {
            if let Some(last) = candles.last() {
                let close = last.close;
                let size_usd = equity * self.position_size_frac;
                let dir = if position == Position::Long { 1 } else { -1 };
                let (exec_price, pnl_pct) = if dir == 1 {
                    let ep = close * (1.0 - slip_mult);
                    (ep, (ep - entry_price) / entry_price * 100.0)
                } else {
                    let ep = close * (1.0 + slip_mult);
                    (ep, (entry_price - ep) / entry_price * 100.0)
                };
                let pnl_abs = size_usd * pnl_pct / 100.0;
                let fee = size_usd * fee_mult;
                equity += pnl_abs - fee;
                total_volume += size_usd;

                trades.push(TradeResult {
                    symbol: symbol.clone(),
                    entry_time,
                    exit_time: last.timestamp,
                    direction: dir,
                    entry_price,
                    exit_price: exec_price,
                    size_usd,
                    pnl_abs: pnl_abs - fee,
                    pnl_pct,
                });
            }
        }

        (trades, bar_equities, total_volume)
    }
}

/// Merge per-symbol equity curves into one total equity curve.
fn merge_equity_curves(
    symbol_equities: &[Vec<(i64, f64)>],
    _total_capital: f64,
) -> Vec<f64> {
    if symbol_equities.len() == 1 {
        return symbol_equities[0].iter().map(|(_, e)| *e).collect();
    }

    let mut all_ts: Vec<i64> = symbol_equities
        .iter()
        .flat_map(|v| v.iter().map(|(t, _)| *t))
        .collect();
    all_ts.sort();
    all_ts.dedup();

    let mut result = Vec::with_capacity(all_ts.len());
    let mut last_vals: Vec<f64> = symbol_equities
        .iter()
        .map(|v| v.first().map_or(0.0, |(_, e)| *e))
        .collect();

    for ts in &all_ts {
        for (i, curve) in symbol_equities.iter().enumerate() {
            if let Ok(idx) = curve.binary_search_by_key(ts, |(t, _)| *t) {
                last_vals[i] = curve[idx].1;
            }
        }
        result.push(last_vals.iter().sum());
    }

    result
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
