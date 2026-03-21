use crate::backtest::BacktestOutput;
use crate::config::ScoringConfig;

pub struct Scores {
    pub score: f64,
    pub sharpe: f64,
    pub max_drawdown_pct: f64,
    pub total_return_pct: f64,
    pub num_trades: usize,
    pub win_rate_pct: f64,
    pub profit_factor: f64,
    pub annual_turnover: f64,
}

/// Compute scoring metrics from backtest output.
///
/// Sharpe is computed from per-bar equity returns (like Nunchi), annualized
/// with sqrt(bars_per_year). This matches the standard finance convention
/// and produces comparable numbers to Nunchi's 21.4 Sharpe.
///
/// Score formula (Nunchi-style):
///   score = sharpe * sqrt(trade_count_factor) - drawdown_penalty - turnover_penalty
pub fn score(output: &BacktestOutput, initial_capital: f64, cfg: &ScoringConfig) -> Scores {
    let num_trades = output.trades.len();
    let eq = &output.equity_curve;

    if eq.len() < 2 || num_trades == 0 {
        return Scores {
            score: -999.0,
            sharpe: 0.0,
            max_drawdown_pct: 0.0,
            total_return_pct: 0.0,
            num_trades: 0,
            win_rate_pct: 0.0,
            profit_factor: 0.0,
            annual_turnover: 0.0,
        };
    }

    // Per-bar returns
    let returns: Vec<f64> = eq
        .windows(2)
        .map(|w| {
            if w[0] > 0.0 {
                (w[1] - w[0]) / w[0]
            } else {
                0.0
            }
        })
        .collect();

    let mean_ret = returns.iter().sum::<f64>() / returns.len() as f64;
    let variance =
        returns.iter().map(|r| (r - mean_ret).powi(2)).sum::<f64>() / returns.len() as f64;
    let std_ret = variance.sqrt();

    // Sharpe: annualized from bar-level returns
    let bars_per_year = cfg.sharpe_annualize;
    let sharpe = if std_ret > 1e-12 {
        (mean_ret / std_ret) * bars_per_year.sqrt()
    } else {
        0.0
    };

    // Total return
    let final_equity = *eq.last().unwrap_or(&initial_capital);
    let total_return_pct = (final_equity - initial_capital) / initial_capital * 100.0;

    // Max drawdown
    let mut peak = eq[0];
    let mut max_dd = 0.0_f64;
    for &e in eq {
        if e > peak {
            peak = e;
        }
        if peak > 0.0 {
            let dd = (peak - e) / peak * 100.0;
            max_dd = max_dd.max(dd);
        }
    }

    // Win rate and profit factor
    let trade_pnls: Vec<f64> = output.trades.iter().map(|t| t.pnl_abs).collect();
    let wins: Vec<f64> = trade_pnls.iter().filter(|p| **p > 0.0).copied().collect();
    let losses: Vec<f64> = trade_pnls.iter().filter(|p| **p < 0.0).copied().collect();
    let win_rate_pct = if !trade_pnls.is_empty() {
        wins.len() as f64 / trade_pnls.len() as f64 * 100.0
    } else {
        0.0
    };
    let gross_profit: f64 = wins.iter().sum();
    let gross_loss: f64 = losses.iter().map(|l| l.abs()).sum();
    let profit_factor = if gross_loss > 1e-10 {
        gross_profit / gross_loss
    } else {
        0.0
    };

    // Annual turnover
    let data_bars = output.total_bars;
    let annual_turnover = if data_bars > 0 && initial_capital > 0.0 {
        output.total_volume * (bars_per_year / data_bars as f64) / initial_capital
    } else {
        0.0
    };

    // Nunchi-style composite score
    // Hard cutoffs
    if num_trades < 10 || max_dd > 50.0 || final_equity < initial_capital * 0.5 {
        return Scores {
            score: -999.0,
            sharpe,
            max_drawdown_pct: max_dd,
            total_return_pct,
            num_trades,
            win_rate_pct,
            profit_factor,
            annual_turnover,
        };
    }

    let trade_count_factor = (num_trades as f64 / 50.0).min(1.0);
    let drawdown_penalty = (max_dd - cfg.dd_free_pct).max(0.0) * cfg.dd_penalty_mult;
    let turnover_penalty = (annual_turnover - cfg.turnover_free).max(0.0) * cfg.turnover_penalty_mult;

    let score = sharpe * trade_count_factor.sqrt() - drawdown_penalty - turnover_penalty;

    Scores {
        score,
        sharpe,
        max_drawdown_pct: max_dd,
        total_return_pct,
        num_trades,
        win_rate_pct,
        profit_factor,
        annual_turnover,
    }
}
