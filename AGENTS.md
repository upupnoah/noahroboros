# noahroboros — autoresearch for trading

This is an autonomous research loop for crypto trading strategy optimization.
An AI agent modifies the strategy code, backtests it, scores the result,
keeps improvements, and discards regressions. Inspired by
[karpathy/autoresearch](https://github.com/karpathy/autoresearch) and
[Nunchi's 103-experiment run](https://github.com/Nunchi-trade/auto-researchtrading).

## Setup

To set up a new experiment run, work with the user to:

1. **Agree on a run tag**: propose a tag based on today's date (e.g. `mar21`).
   The branch `autoresearch/<tag>` must not already exist.
2. **Create the branch**: `git checkout -b autoresearch/<tag>` from current main.
3. **Read the in-scope files**:
   - This file (`AGENTS.md`) — your operating instructions.
   - `src/strategy/mod.rs` — the Strategy trait. Understand the interface.
   - `src/strategy/baseline.rs` — the starting strategy. This is what you improve.
   - `src/backtest/mod.rs` — the backtest engine. Read-only. Understand its output format.
   - `src/scoring/mod.rs` — the scoring function. Read-only. Understand the composite metric.
4. **Verify data exists**: Check that `data/1h/` contains `.csv` files
   (BTCUSDT.csv, ETHUSDT.csv, SOLUSDT.csv). If not, tell the human to run
   `cargo run -- download`.
5. **Build**: `cargo build --release`. Fix any compilation errors before proceeding.
6. **Establish baseline**: Run the backtest with the unmodified strategy:
   ```
   cargo run --release -- backtest -d data/1h > run.log 2>&1
   ```
   Record the baseline score in `experiments/results.tsv`.
7. **Initialize results.tsv**: If it doesn't exist, create it with the header row only.
   The baseline will be the first data row.
8. **Confirm and go**: Confirm setup looks good, then begin the experiment loop.

## Project Structure

```
AGENTS.md               ← you are here (your instructions)
.env                    ← infrastructure config (read-only, see .env.example)
Cargo.toml              ← dependencies (read-only)
run.sh                  ← automation script (read-only)
src/
  main.rs               ← CLI entry point (read-only)
  lib.rs                ← module re-exports (read-only)
  config.rs             ← config loader from .env (read-only)
  strategy/
    mod.rs              ← Strategy trait definition (read-only)
    baseline.rs         ← *** YOU EDIT THIS *** (the starting strategy)
  backtest/
    mod.rs              ← backtest engine (read-only)
  scoring/
    mod.rs              ← scoring metrics (read-only)
  trading/
    mod.rs              ← Exchange trait + stubs (read-only)
  market/
    mod.rs              ← MarketFeed trait + CSV loader (read-only)
data/
  1h/                   ← 1-hour OHLCV candle data (read-only)
    BTCUSDT.csv
    ETHUSDT.csv
    SOLUSDT.csv
experiments/
  results.tsv           ← experiment log (you append to this)
```

**Note on configuration:** Infrastructure settings (backtest capital, scoring weights,
exchange URLs) are loaded from `.env` via `src/config.rs`. Strategy parameters (indicator
periods, thresholds, etc.) live in `src/strategy/baseline.rs` as code constants — these
are what you optimize. Do NOT modify `.env` or `config.rs`.

## Scoring

The backtest outputs a structured summary. The primary optimization target is
`score` — higher is better.

```
---
score:              1.851
sharpe:             1.851
total_return_pct:   4.328
max_drawdown_pct:   1.577
num_trades:         309
win_rate_pct:       71.845
profit_factor:      1.661
annual_turnover:    33.9
---
```

**Scoring formula (Nunchi-style):**

```
score = sharpe * sqrt(trade_count_factor) - drawdown_penalty - turnover_penalty

trade_count_factor = min(num_trades / 50, 1.0)
drawdown_penalty   = max(0, max_drawdown_pct - 15) * 0.05
turnover_penalty   = max(0, annual_turnover - 500) * 0.001

Hard cutoffs: <10 trades -> -999, >50% drawdown -> -999, lost >50% -> -999
```

Sharpe is computed from **per-bar equity returns** (mark-to-market each candle),
annualized with sqrt(bars_per_year). For 1h data, bars_per_year = 8760.

**Backtest parameters:** capital $100K, position size 8%, fee 5bps, slippage 1bps.
These are fixed in .env — do NOT change them.

**Multi-asset:** The backtest runs on BTC, ETH, and SOL in parallel (capital split
equally). Each asset has independent strategy state.

**Current baseline score: +1.851** (score on BTC/ETH/SOL 1h data)

## Output Extraction

After each run, extract the key metrics:

```
grep "^score:\|^sharpe:\|^max_drawdown_pct:" run.log
```

If grep returns empty, the run crashed. Run `tail -n 50 run.log` to see the error.

## Simplicity Criterion

All else being equal, simpler is better. Nunchi's biggest finding: the biggest gains
came from REMOVING features, not adding them. Nine features were built and
subsequently removed — every removal improved performance.

- Equal score but simpler code? Keep.
- Small improvement from deleting code? Definitely keep.
- Small improvement that adds ugly complexity? Skip.

## Logging Results

When an experiment finishes, append a row to `experiments/results.tsv`.

The TSV has a header row and 6 columns (tab-separated, NOT commas):

```
commit	score	sharpe	max_dd	status	description
```

1. git commit hash (short, 7 chars)
2. score (e.g. 2.500) — use 0.000 for crashes
3. sharpe (e.g. 2.500) — use 0.000 for crashes
4. max_drawdown_pct (e.g. 7.600) — use 0.000 for crashes
5. status: `keep`, `discard`, or `crash`
6. short text description of what this experiment tried

Do NOT git-commit `experiments/results.tsv`. Leave it untracked.

## The Experiment Loop

The experiment runs on a dedicated branch (e.g. `autoresearch/mar21`).

LOOP FOREVER:

1. **Review state**: Read the current strategy code and recent experiment history.
   Look at what worked and what didn't. Think about what to try next.
2. **Form hypothesis**: Decide on ONE change to test. Write it down mentally.
   Good ideas: adjust a parameter, toggle a feature, modify entry/exit thresholds.
   One change at a time.
3. **Implement**: Edit `src/strategy/baseline.rs` with the change.
4. **Commit**: `git add -A && git commit -m "experiment: <brief description>"`
5. **Build**: `cargo build --release 2>&1 | tail -n 20`. If it fails, fix and retry.
   If you can't fix after 2 attempts, revert and move on.
6. **Run backtest**: `cargo run --release -- backtest -d data/1h > run.log 2>&1`
7. **Read results**: `grep "^score:\|^sharpe:\|^max_drawdown_pct:" run.log`
8. **Evaluate**:
   - If score IMPROVED (higher) → keep the commit. Log status `keep`.
   - If score is EQUAL or WORSE → revert: `git reset --hard HEAD~1`. Log status `discard`.
   - If the run CRASHED → check `tail -n 50 run.log`. If it's a trivial fix, fix and
     re-run. If fundamental, revert and log status `crash`.
9. **Log**: Append result to `experiments/results.tsv`.
10. **Repeat from step 1.**

## Strategy Architecture

The strategy has feature toggles for Nunchi's six mechanisms. Currently:

```
4 ACTIVE signals:    momentum (12h/6h), EMA crossover, RSI
6 TOGGLEABLE mechanisms: USE_DYNAMIC_THRESHOLD, USE_MACD, USE_BB,
                         USE_ATR_STOP, USE_COOLDOWN, USE_SIGNAL_FLIP
```

Each toggle is a `const bool` that enables a mechanism. When false, the behavior
is exactly the proven +1.851 baseline.

## What Nunchi Discovered (103 experiments)

These are the key findings from Nunchi's successful run that achieved Sharpe 21.4.
Their data was different (BTC/ETH/SOL from Hyperliquid, Jul 2024 - Mar 2025), so
parameters won't transfer directly, but the structural insights are valuable:

1. **RSI period 8** was their biggest single improvement (+5.0 Sharpe)
2. **Removing features** was their most consistent improvement source
3. **ATR trailing stop at 5.5x** (wide) outperformed tight stops
4. **Cooldown of 2 bars** prevented whipsaw re-entries
5. **Dynamic vol-adjusted momentum threshold** adapted to regime changes
6. **BB width percentile < 90** as quality filter for entries
7. **Position size 0.08** (8%) eliminated turnover penalty
8. **Signal flip on opposing consensus** for faster position reversal

## Strategy Ideas to Explore (Prioritized)

**TIER 1 — Toggle experiments (fastest, safest):**
- Turn on USE_MACD (adds 5th signal). May need to raise VOTE_THRESHOLD.
- Turn on USE_BB (adds 6th signal). May need to raise VOTE_THRESHOLD.
- Turn on USE_ATR_STOP. Try ATR_STOP_MULT values: 4.0, 5.5, 7.0, 10.0.
- Turn on USE_COOLDOWN. Try COOLDOWN_BARS: 1, 2, 3.
- Turn on USE_SIGNAL_FLIP.
- Turn on USE_DYNAMIC_THRESHOLD.
- Combine: enable multiple toggles together.

**TIER 2 — Parameter tuning:**
- RSI period: 6, 8, 10, 12, 14, 20
- RSI exit levels: 69/31, 70/30, 75/25, 80/20, 85/15
- EMA periods: (5/21), (7/26), (9/21), (3/15)
- Momentum lookback: (6/3), (12/6), (24/12), (48/24)
- Vote threshold: 2, 3, 4

**TIER 3 — Structural changes:**
- Replace binary momentum with Z-score momentum (return / rolling_std)
- Adaptive RSI exits (tighter when profitable, wider when losing)
- Asymmetric long/short parameters (crypto trends differently up vs down)
- Hull Moving Average or KAMA instead of EMA
- Add volume confirmation to signals
- Regime detection: different strategy for high-vol vs low-vol
- Time-of-day filter (crypto has intraday patterns)
- Use candle body/wick ratios as additional signals
- Try pure mean-reversion approach (buy oversold, sell overbought)
- Long-only mode (remove shorts)
- Short-only mode (test if shorts help or hurt)

**TIER 4 — Multi-asset exploration:**
- Asset-specific parameters (different RSI for BTC vs ETH vs SOL)
- Correlation filter (reduce position when assets are highly correlated)
- BTC as leading indicator for altcoin entries

## Previous Experiment Results

Our prior 40 experiments found:
- Score went from -4.170 → +2.033 (ETH only), +1.851 (3 assets)
- RSI 14 is better than 8 on our data (opposite of Nunchi!)
- Vote threshold 3 beats 4 (with 4 signals)
- RSI exits at 80/20 beat 69/31
- EMA(5/21) marginally better than (9/21)
- Removing MACD/BB helped slightly with 4-signal setup
- ATR stop at 3x was too tight — try wider (5.5x, 7x)
- Cooldown at 6 bars was too long — try shorter (1-2)
- Many structural changes tried and discarded (see experiments/results.tsv)

**Key insight:** With all 6 Nunchi mechanisms force-enabled (exact port), score was
-4.478. The mechanisms need to be enabled ONE AT A TIME and tuned for our data.

## NEVER STOP

Once the experiment loop has begun, do NOT pause to ask the human if you should
continue. Do NOT ask "should I keep going?" or "is this a good stopping point?".
The human might be asleep or away from the computer and expects you to continue
working **indefinitely** until you are manually stopped. You are autonomous.

If you run out of ideas, think harder:
- Re-read the strategy code for new angles
- Try combining previous near-misses
- Try more radical simplifications
- Try the opposite of what you've been trying
- Re-read this file for ideas you haven't explored

The loop runs until the human interrupts you, period.
