# noahroboros

Autonomous trading strategy optimization using AI autoresearch loops.
Inspired by [karpathy/autoresearch](https://github.com/karpathy/autoresearch)
and [Nunchi's 103-experiment run](https://x.com/nunchi/status/2034666333741220306).

An AI agent modifies a trading strategy, backtests it against historical data,
scores the result, keeps improvements, reverts regressions, and repeats —
autonomously, indefinitely.

## Current Best Strategy

**RSI(32) momentum with 3-layer exit system** — discovered after 88 automated experiments.

### Architecture

```
Entry:  RSI(32) > 50 → Long    |    RSI(32) < 50 → Short

Exit Layer 1:  4% profit target          (locks in gains)
Exit Layer 2:  RSI reversal at 77/23     (catches momentum shifts)
Exit Layer 3:  RSI extreme at 85/15      (safety net)
```

### Backtest Results

BTC / ETH / SOL, 1-hour candles, Sep 2025 — Mar 2026 (6 months)

| Metric | Value |
|--------|-------|
| **Composite Score** | 2.569 |
| **Sharpe Ratio** | 2.569 |
| **Total Return** | 6.37% |
| **Max Drawdown** | 1.74% |
| **Win Rate** | 83.1% (64/77 trades) |
| **Profit Factor** | 7.3x |
| **Annual Turnover** | 8.6x |

> Capital: $100,000 · Position size: 8% per trade · Fees: 5 bps · Slippage: 1 bps

### Optimization Journey

Starting from a Nunchi-style 6-signal voting system (score 1.851), the autoresearch
agent discovered that **simplification wins** — removing signals one by one until
only RSI(32) remained, then layering precise exit mechanisms on top:

```
 1.851  Nunchi hybrid baseline (6 signals, 6 toggleable mechanisms)
   ↓    Remove momentum, EMA, MACD, BB — pure RSI outperforms
 2.316  RSI(32) with 80/20 exits
   ↓    Add 4% profit target
 2.418  RSI(32) + profit target
   ↓    Widen RSI exits to 85/15 (PT handles most exits now)
 2.483  RSI(32) + PT + wider exits
   ↓    Add RSI reversal exit at 77/23
 2.569  Current best ✓
```

88 experiments tested, 85 discarded. Full log in `experiments/results.tsv`.

## Quick Start

```bash
# 1. Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 2. Clone and build
git clone https://github.com/upupnoah/noahroboros.git && cd noahroboros
cp .env.example .env
cargo build --release

# 3. Download historical data
cargo run --release -- download

# 4. Run backtest
cargo run --release -- backtest -d data/1h

# 5. Start autoresearch (requires Cursor CLI)
./run.sh -n 100
```

## Downloading Historical Data

Data is fetched from Binance's public klines API (no API key needed for spot).
Files are saved to `data/{interval}/{SYMBOL}.csv`.

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

## Running Backtests

```bash
# Backtest on 1-hour data (default)
cargo run --release -- backtest -d data/1h

# Backtest on higher-frequency data
cargo run --release -- backtest -d data/1m
```

Example output:

```
Backtesting 13035 candles, capital=$100000, position=8%, fee=5bps, slip=1bps
---
score:              2.569
sharpe:             2.569
total_return_pct:   6.369
max_drawdown_pct:   1.735
num_trades:         77
win_rate_pct:       83.117
profit_factor:      7.295
annual_turnover:    8.6
---
```

When multiple symbol CSVs exist in the data directory, the engine splits capital
equally and runs the strategy independently per asset, then aggregates results.

## Autoresearch

The core loop: AI modifies `src/strategy/baseline.rs` → builds → backtests →
keeps improvements / reverts regressions → repeats.

```bash
# Run N experiments autonomously
./run.sh -n 100

# Interactive mode
./run.sh

# Cloud: push to Cursor Cloud Agent
./run.sh --cloud
```

See `AGENTS.md` for the full agent instructions.

## Scoring

Composite score (Nunchi-style, higher = better):

```
score = sharpe × √(trade_count_factor) − drawdown_penalty − turnover_penalty
```

Where:
- `trade_count_factor` = min(num_trades / 50, 1.0) — penalizes fewer than 50 trades
- `drawdown_penalty` = max(max_dd% − 15, 0) × 0.05 — free below 15% DD
- `turnover_penalty` = max(annual_turnover − 500, 0) × 0.001 — free below 500x

Hard cutoffs: score = −999 if trades < 10, max DD > 50%, or equity drops below 50%.

## Project Structure

```
AGENTS.md                AI agent instructions (= program.md)
.env.example             Configuration template
run.sh                   Automation script
src/
  main.rs                CLI entry point
  config.rs              Config loader (.env)
  strategy/
    mod.rs               Strategy trait (Signal: Long/Short/Flat/Hold)
    baseline.rs          ** AI modifies this file **
  backtest/mod.rs        Backtest engine (per-bar equity, fees, slippage)
  scoring/mod.rs         Composite scoring (Sharpe, DD, turnover)
  market/
    mod.rs               CSV data loader
    download.rs          Binance klines downloader
  trading/mod.rs         Exchange traits (Binance, Lighter.xyz)
data/
  1h/                    1-hour candles (BTC, ETH, SOL)
experiments/
  results.tsv            Experiment log (88 experiments)
```

## Configuration

Copy `.env.example` to `.env` to customize:

```bash
cp .env.example .env
```

Key settings: `INITIAL_CAPITAL`, `POSITION_SIZE_FRAC`, `FEE_BPS`,
`DOWNLOAD_SYMBOLS`, `DOWNLOAD_INTERVAL`, scoring parameters, exchange API keys.
See `.env.example` for the full list.

## License

MIT
