# noahroboros

Autonomous trading strategy optimization using AI autoresearch loops.
Inspired by [karpathy/autoresearch](https://github.com/karpathy/autoresearch)
and [Nunchi's 103-experiment run](https://x.com/nunchi/status/2034666333741220306).

An AI agent modifies a trading strategy, backtests it against historical data,
scores the result, keeps improvements, reverts regressions, and repeats --
autonomously, indefinitely.

## Quick Start

```bash
# 1. Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 2. Install Cursor CLI (for autoresearch)
curl https://cursor.com/install -fsS | bash

# 3. Clone and build
git clone <repo-url> && cd noahroboros
cp .env.example .env    # optional: edit config
cargo build --release

# 4. Download historical data
cargo run --release -- download

# 5. Run backtest
cargo run --release -- backtest -d data/1h

# 6. Start autoresearch
./run.sh                    # interactive
./run.sh -n 100             # 100 autonomous experiments
./run.sh --cloud            # push to Cursor Cloud Agent
```

## Downloading Historical Data

Data is fetched from Binance's public API (no API key needed). Downloaded files
are saved to `data/{interval}/{symbol}.csv`.

```bash
# Default: 9 months of 1h BTC/ETH/SOL data
cargo run --release -- download

# Specific symbol and interval
cargo run --release -- download --symbols ETHUSDT --interval 1m

# Custom date range
cargo run --release -- download --symbols ETHUSDT --interval 5m \
  --start 2025-09-01 --end 2026-03-01

# Multiple symbols
cargo run --release -- download --symbols BTCUSDT,ETHUSDT,SOLUSDT --interval 15m
```

### Supported Intervals

| Interval | Flag | 6-month data size (per asset) | Download time |
|----------|------|-------------------------------|---------------|
| 1 second | `--interval 1s` | ~15M candles / 720 MB | ~90 min |
| 1 minute | `--interval 1m` | ~260K candles / 13 MB | ~2 min |
| 5 minutes | `--interval 5m` | ~52K candles / 3 MB | ~20 sec |
| 15 minutes | `--interval 15m` | ~17K candles / 936 KB | ~8 sec |
| 1 hour | `--interval 1h` | ~4.3K candles / 232 KB | ~3 sec |
| 4 hours | `--interval 4h` | ~1.1K candles / 56 KB | ~2 sec |
| 1 day | `--interval 1d` | ~180 candles / 10 KB | ~1 sec |

### Download Options

| Flag | Description | Default |
|------|-------------|---------|
| `--symbols`, `-s` | Comma-separated trading pairs | `BTCUSDT,ETHUSDT,SOLUSDT` |
| `--interval`, `-i` | Candle interval | `1h` |
| `--months`, `-m` | Months of history (from now) | `9` |
| `--start` | Start date (`YYYY-MM-DD`) | (auto from --months) |
| `--end` | End date (`YYYY-MM-DD`, exclusive) | now |
| `--output`, `-o` | Output base directory | `data` |

Defaults can be changed in `.env` (see `.env.example`).

## Running Backtests

```bash
# Backtest with 1-hour data (default)
cargo run --release -- backtest -d data/1h

# Backtest with 1-minute data
cargo run --release -- backtest -d data/1m

# Custom initial capital
cargo run --release -- backtest -d data/5m --capital 50000
```

Output:

```
---
composite_score:  0.492
sharpe_ratio:     0.537
max_drawdown_pct: 10.143
total_return_pct: 3.034
num_trades:       92
win_rate_pct:     54.348
avg_trade_pct:    0.046
turnover:         0.982
---
```

When multiple symbol CSVs exist in the data directory, the engine runs the
strategy independently per asset (capital split equally) and aggregates results.

## Autoresearch

The core loop: AI modifies `src/strategy/baseline.rs` -> builds -> backtests ->
keeps improvements / reverts regressions -> repeats forever.

```bash
# Interactive: you watch, can intervene
./run.sh

# Non-interactive: run N experiments autonomously
./run.sh -n 100

# Cloud: push to Cursor Cloud Agent, walk away
./run.sh --cloud

# Custom options
./run.sh -n 50 --model gpt-5.2 --tag experiment1
```

See `AGENTS.md` for the full agent instructions.

## Project Structure

```
AGENTS.md               AI agent instructions (= program.md)
.env.example             Configuration template
run.sh                   Automation script
src/
  main.rs                CLI entry point
  config.rs              Config loader (.env)
  strategy/
    mod.rs               Strategy trait
    baseline.rs           ** AI modifies this **
  backtest/mod.rs         Backtest engine
  scoring/mod.rs          Composite scoring
  market/
    mod.rs               CSV data loader
    download.rs           Binance data downloader
  trading/mod.rs          Exchange traits (Binance, Lighter.xyz)
data/
  1s/                    1-second candles
  1m/                    1-minute candles
  5m/                    5-minute candles
  15m/                   15-minute candles
  1h/                    1-hour candles
experiments/
  results.tsv            Experiment log
```

## Scoring

Composite score (higher = better), weights configurable in `.env`:

```
composite = sharpe * 0.4
          + (1 - max_drawdown / 100) * 0.3
          + tanh(total_return / 100) * 0.2
          + (1 - turnover) * 0.1
```

Penalizes high drawdown (risk) and high turnover (over-trading).
Weights are configurable via `SCORE_W_*` env vars.

## Configuration

Copy `.env.example` to `.env` to customize defaults:

```bash
cp .env.example .env
```

Key settings: `INITIAL_CAPITAL`, `POSITION_SIZE_FRAC`, `DOWNLOAD_SYMBOLS`,
`DOWNLOAD_INTERVAL`, scoring weights, exchange API keys. See `.env.example`
for the full list.

## License

MIT
