use crate::backtest::TradeResult;
use crate::config::ScoringConfig;

pub struct Scores {
    pub composite: f64,
    pub sharpe: f64,
    pub max_drawdown_pct: f64,
    pub total_return_pct: f64,
    pub num_trades: usize,
    pub win_rate_pct: f64,
    pub avg_trade_pct: f64,
    pub turnover: f64,
}

/// Compute all scoring metrics from backtest trade results.
///
/// Composite score formula (weights from config):
///   sharpe * w_sharpe
/// + (1.0 - max_dd / 100.0) * w_drawdown
/// + tanh(total_return / 100.0) * w_return
/// + (1.0 - turnover) * w_turnover
pub fn score(trades: &[TradeResult], initial_capital: f64, cfg: &ScoringConfig) -> Scores {
    let num_trades = trades.len();

    if num_trades == 0 {
        return Scores {
            composite: 0.0,
            sharpe: 0.0,
            max_drawdown_pct: 0.0,
            total_return_pct: 0.0,
            num_trades: 0,
            win_rate_pct: 0.0,
            avg_trade_pct: 0.0,
            turnover: 0.0,
        };
    }

    let returns: Vec<f64> = trades.iter().map(|t| t.pnl_pct).collect();

    let total_return_pct = returns.iter().fold(1.0, |acc, r| acc * (1.0 + r / 100.0));
    let total_return_pct = (total_return_pct - 1.0) * 100.0;

    let mean_return = returns.iter().sum::<f64>() / returns.len() as f64;
    let variance = returns.iter().map(|r| (r - mean_return).powi(2)).sum::<f64>()
        / returns.len() as f64;
    let std_dev = variance.sqrt();
    let sharpe = if std_dev > 1e-10 {
        (mean_return / std_dev) * cfg.sharpe_annualize.sqrt()
    } else {
        0.0
    };

    let mut equity = initial_capital;
    let mut peak = equity;
    let mut max_dd = 0.0_f64;
    for trade in trades {
        equity += trade.pnl_abs;
        if equity > peak {
            peak = equity;
        }
        let dd = (peak - equity) / peak * 100.0;
        max_dd = max_dd.max(dd);
    }

    let wins = returns.iter().filter(|r| **r > 0.0).count();
    let win_rate_pct = (wins as f64 / num_trades as f64) * 100.0;

    let avg_trade_pct = mean_return;

    let total_volume: f64 = trades.iter().map(|t| t.size_usd).sum();
    let avg_equity = (initial_capital + equity) / 2.0;
    let turnover = (total_volume / (avg_equity * num_trades as f64)).min(1.0);

    let composite = sharpe * cfg.w_sharpe
        + (1.0 - max_dd / 100.0) * cfg.w_drawdown
        + (total_return_pct / 100.0).tanh() * cfg.w_return
        + (1.0 - turnover) * cfg.w_turnover;

    Scores {
        composite,
        sharpe,
        max_drawdown_pct: max_dd,
        total_return_pct,
        num_trades,
        win_rate_pct,
        avg_trade_pct,
        turnover,
    }
}
