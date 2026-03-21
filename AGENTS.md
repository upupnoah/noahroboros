# noahroboros — autoresearch for trading

This is an autonomous research loop for crypto trading strategy optimization.
An AI agent modifies the strategy code, backtests it, scores the result,
keeps improvements, and discards regressions. Inspired by
[karpathy/autoresearch](https://github.com/karpathy/autoresearch) and
[Nunchi's 103-experiment run](https://x.com/nunchi/status/2034666333741220306).

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
4. **Verify data exists**: Check that `data/` contains subdirectories with `.csv` files
   (e.g. `data/1m/ETHUSDT.csv`). If not, tell the human to run `cargo run -- download`.
5. **Build**: `cargo build --release`. Fix any compilation errors before proceeding.
6. **Establish baseline**: Run the backtest with the unmodified strategy.
   Use the data directory specified by the human (default `data/1m`):
   ```
   cargo run --release -- backtest -d data/1m > run.log 2>&1
   ```
   Record the baseline score in `experiments/results.tsv`.
7. **Initialize results.tsv**: If it doesn't exist, create it with the header row only.
   The baseline will be the first data row.
8. **Confirm and go**: Confirm setup looks good, then begin the experiment loop.

## Project Structure

```
AGENTS.md               ← you are here (your instructions)
.env                    ← infrastructure config (read-only, see .env.example)
.env.example            ← config template with defaults (read-only)
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
    mod.rs              ← Exchange trait + stubs (read-only, not used in autoresearch)
  market/
    mod.rs              ← MarketFeed trait + CSV loader (read-only)
data/
  *.csv                 ← historical OHLCV candle data (read-only)
experiments/
  results.tsv           ← experiment log (you append to this)
```

**Note on configuration:** Infrastructure settings (backtest capital, scoring weights,
exchange URLs) are loaded from `.env` via `src/config.rs`. Strategy parameters (indicator
periods, thresholds, etc.) live in `src/strategy/baseline.rs` as code constants — these
are what you optimize. Do NOT modify `.env` or `config.rs`.

## Scope Rules

**What you CAN do:**
- Modify `src/strategy/baseline.rs` — this is the only file you edit. Everything about
  the strategy is fair game: signal logic, indicators, parameters, entry/exit rules,
  position sizing, risk management, filters.
- Create NEW files under `src/strategy/` if the strategy grows complex enough to warrant
  splitting (e.g. `src/strategy/signals.rs`, `src/strategy/filters.rs`). If you do,
  update `src/strategy/mod.rs` to include them.
- Modify `src/strategy/mod.rs` ONLY to add `mod` declarations for new strategy files.

**What you CANNOT do:**
- Modify any file outside `src/strategy/`.
- Modify `Cargo.toml` or add dependencies.
- Modify the backtest engine, scoring function, market data loader, or exchange connectors.
- Modify `AGENTS.md`, `run.sh`, `main.rs`, or `lib.rs`.
- Change the output format of the backtest. The scoring is the ground truth.

## Scoring

The backtest outputs a structured summary. The primary optimization target is
`composite_score` — higher is better.

```
---
composite_score:  8.500
sharpe_ratio:     2.710
max_drawdown_pct: 7.600
total_return_pct: 45.200
num_trades:       127
win_rate_pct:     58.300
avg_trade_pct:    0.356
turnover:         0.450
---
```

**Composite score formula:**

```
composite = sharpe * 0.4
          + (1.0 - max_drawdown_pct / 100.0) * 0.3
          + tanh(total_return_pct / 100.0) * 0.2
          + (1.0 - turnover) * 0.1
```

The metric penalizes:
- High drawdown (risk)
- High turnover (over-trading)
- Uses tanh on returns to diminish marginal value of extreme returns

**The goal: maximize composite_score.**

## Output Extraction

After each run, extract the key metrics:

```
grep "^composite_score:\|^sharpe_ratio:\|^max_drawdown_pct:" run.log
```

If grep returns empty, the run crashed. Run `tail -n 50 run.log` to see the error.

## Simplicity Criterion

All else being equal, simpler is better.

- A small improvement that adds ugly complexity is NOT worth it.
- Removing a feature and getting equal or better results IS worth it — that's a
  simplification win.
- A 0.01 composite_score improvement that adds 50 lines of convoluted logic? Skip.
- A 0.01 composite_score improvement from DELETING code? Definitely keep.
- Equal score but simpler code? Keep.

Nunchi's key finding: the biggest gains came from REMOVING features, not adding them.
Nine features were built and subsequently removed — every removal improved performance.
Keep this in mind.

## Logging Results

When an experiment finishes, append a row to `experiments/results.tsv`.

The TSV has a header row and 6 columns (tab-separated, NOT commas):

```
commit	composite	sharpe	max_dd	status	description
```

1. git commit hash (short, 7 chars)
2. composite_score (e.g. 8.500) — use 0.000 for crashes
3. sharpe_ratio (e.g. 2.710) — use 0.000 for crashes
4. max_drawdown_pct (e.g. 7.600) — use 0.000 for crashes
5. status: `keep`, `discard`, or `crash`
6. short text description of what this experiment tried

Example:

```
commit	composite	sharpe	max_dd	status	description
a1b2c3d	8.500	2.710	7.600	keep	baseline
b2c3d4e	9.120	3.450	5.200	keep	RSI lookback 14 -> 8
c3d4e5f	7.800	2.500	9.100	discard	added pyramiding
d4e5f6g	0.000	0.000	0.000	crash	OOM on signal matrix
```

Do NOT git-commit `experiments/results.tsv`. Leave it untracked.

## The Experiment Loop

The experiment runs on a dedicated branch (e.g. `autoresearch/mar21`).

LOOP FOREVER:

1. **Review state**: Read the current strategy code and recent experiment history.
   Look at what worked and what didn't. Think about what to try next.
2. **Form hypothesis**: Decide on ONE change to test. Write it down mentally.
   Good ideas: adjust a parameter, remove a feature, simplify a condition, change
   an indicator period, modify entry/exit thresholds. One change at a time.
3. **Implement**: Edit `src/strategy/baseline.rs` with the change.
4. **Commit**: `git add -A && git commit -m "experiment: <brief description>"`
5. **Build**: `cargo build --release 2>&1 | tail -n 20`. If it fails, fix and retry.
   If you can't fix after 2 attempts, revert and move on.
6. **Run backtest**: `cargo run --release -- backtest -d data/1m > run.log 2>&1`
7. **Read results**: `grep "^composite_score:\|^sharpe_ratio:\|^max_drawdown_pct:" run.log`
8. **Evaluate**:
   - If composite_score IMPROVED → keep the commit. Log status `keep`.
   - If composite_score is EQUAL or WORSE → revert: `git reset --hard HEAD~1`. Log status `discard`.
   - If the run CRASHED → check `tail -n 50 run.log`. If it's a trivial fix, fix and
     re-run. If fundamental, revert and log status `crash`.
9. **Log**: Append result to `experiments/results.tsv`.
10. **Repeat from step 1.**

## Timeout

Each backtest should complete in seconds (Rust is fast). If a run exceeds 60 seconds,
kill it (`kill` the process) and treat it as a crash.

## Crashes

Use your judgment:
- Typo or missing import → fix and re-run.
- Strategy logic produces NaN/infinity → fix the math and re-run.
- Fundamentally broken idea → revert, log `crash`, move on.

Don't spend more than 2 attempts fixing a single crash. If it doesn't work, skip it.

## Strategy Ideas to Explore

These are starting points, not an exhaustive list. Use your judgment and build on
what the data tells you.

**Parameter tuning:**
- RSI period (default 14 — try 6, 8, 10, 12, 20)
- EMA periods (fast/slow crossover)
- ATR multiplier for stops
- Entry/exit thresholds
- Minimum signal agreement count

**Feature additions (test carefully — they often hurt):**
- Bollinger Band squeeze detection
- Volume confirmation
- MACD divergence
- Multi-timeframe analysis
- Volatility regime detection

**Feature removals (often the biggest wins):**
- Remove strength scaling
- Remove pyramiding
- Remove correlation filters
- Remove any feature that "never triggers"
- Simplify position sizing to fixed fraction

**Structural changes:**
- Signal voting threshold (require N of M signals to agree)
- Trailing stop vs fixed stop
- Asymmetric entry/exit conditions
- Per-asset parameter tuning vs universal parameters

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
