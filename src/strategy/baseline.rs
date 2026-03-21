use std::collections::VecDeque;

use crate::market::Candle;
use crate::strategy::{Signal, Strategy};
use ta::indicators::{ExponentialMovingAverage, RelativeStrengthIndex};
use ta::Next;

/// Nunchi-style 6-signal voting strategy.
///
/// This is the file the AI agent modifies during autoresearch.
///
/// Six signals vote on each entry:
///   1. 12h Momentum     — close > close[N bars ago]
///   2. 6h V-Short Mom   — close > close[N bars ago]
///   3. EMA Crossover    — fast EMA vs slow EMA
///   4. RSI(8)           — above/below 50
///   5. MACD             — MACD line vs signal line
///   6. BB Compress      — Bollinger Band squeeze + direction
///
/// Entry: >= VOTE_THRESHOLD signals agree (4 of 6)
/// Exit:  RSI hits 69 (long) or 31 (short), or ATR trailing stop

// --- Tunable Parameters ---

// Momentum lookback (in bars — for 1h data: 12 bars = 12h, 6 bars = 6h)
const MOM_LONG_BARS: usize = 12;
const MOM_SHORT_BARS: usize = 6;

// EMA crossover
const EMA_FAST_PERIOD: usize = 5;
const EMA_SLOW_PERIOD: usize = 21;

// RSI
const RSI_PERIOD: usize = 14;
const RSI_EXIT_LONG: f64 = 80.0;
const RSI_EXIT_SHORT: f64 = 20.0;

// MACD (built from EMAs)
const MACD_FAST: usize = 12;
const MACD_SLOW: usize = 26;
const MACD_SIGNAL: usize = 9;

// Bollinger Bands
const BB_PERIOD: usize = 20;
const BB_STD_MULT: f64 = 2.0;
const BB_SQUEEZE_LOOKBACK: usize = 120;

// ATR (kept for potential future use, but trailing stop removed)
const ATR_PERIOD: usize = 14;

// Voting
const VOTE_THRESHOLD: usize = 3;

// Warmup: needs max lookback across all indicators
const WARMUP_BARS: usize = 120;

pub struct BaselineStrategy {
    // Momentum ring buffers
    closes: VecDeque<f64>,

    // EMA crossover
    ema_fast: ExponentialMovingAverage,
    ema_slow: ExponentialMovingAverage,

    // RSI
    rsi: RelativeStrengthIndex,

    // MACD (three EMAs)
    macd_fast_ema: ExponentialMovingAverage,
    macd_slow_ema: ExponentialMovingAverage,
    macd_signal_ema: ExponentialMovingAverage,

    // Bollinger Bands
    bb_prices: VecDeque<f64>,
    bb_widths: VecDeque<f64>,

    // Position tracking (no trailing stop — RSI exits only)
    position: i8, // 0=flat, 1=long, -1=short

    count: usize,
}

impl BaselineStrategy {
    pub fn new() -> Self {
        Self {
            closes: VecDeque::with_capacity(WARMUP_BARS + 1),

            ema_fast: ExponentialMovingAverage::new(EMA_FAST_PERIOD).unwrap(),
            ema_slow: ExponentialMovingAverage::new(EMA_SLOW_PERIOD).unwrap(),

            rsi: RelativeStrengthIndex::new(RSI_PERIOD).unwrap(),

            macd_fast_ema: ExponentialMovingAverage::new(MACD_FAST).unwrap(),
            macd_slow_ema: ExponentialMovingAverage::new(MACD_SLOW).unwrap(),
            macd_signal_ema: ExponentialMovingAverage::new(MACD_SIGNAL).unwrap(),

            bb_prices: VecDeque::with_capacity(BB_PERIOD + 1),
            bb_widths: VecDeque::with_capacity(BB_SQUEEZE_LOOKBACK + 1),

            position: 0,

            count: 0,
        }
    }

    fn bb_stats(&self) -> (f64, f64) {
        // Returns (middle, width_pct)
        if self.bb_prices.len() < BB_PERIOD {
            return (0.0, 0.0);
        }
        let prices: Vec<f64> = self.bb_prices.iter().copied().collect();
        let n = prices.len() as f64;
        let mean = prices.iter().sum::<f64>() / n;
        let variance = prices.iter().map(|p| (p - mean).powi(2)).sum::<f64>() / n;
        let std = variance.sqrt();
        let width = (BB_STD_MULT * 2.0 * std) / mean * 100.0;
        (mean, width)
    }

    fn is_squeezed(&self, current_width: f64) -> bool {
        if self.bb_widths.len() < 2 {
            return false;
        }
        let avg_width: f64 =
            self.bb_widths.iter().sum::<f64>() / self.bb_widths.len() as f64;
        current_width < avg_width
    }
}

impl Strategy for BaselineStrategy {
    fn name(&self) -> &str {
        "nunchi_6signal_voting"
    }

    fn on_candle(&mut self, candle: &Candle) -> Signal {
        let close = candle.close;

        // Update ring buffers
        self.closes.push_back(close);
        if self.closes.len() > WARMUP_BARS + 1 {
            self.closes.pop_front();
        }

        // Update indicators
        let ema_fast_val = self.ema_fast.next(close);
        let ema_slow_val = self.ema_slow.next(close);
        let rsi_val = self.rsi.next(close);

        let macd_fast_val = self.macd_fast_ema.next(close);
        let macd_slow_val = self.macd_slow_ema.next(close);
        let macd_line = macd_fast_val - macd_slow_val;
        let macd_signal_val = self.macd_signal_ema.next(macd_line);

        // Bollinger Bands
        self.bb_prices.push_back(close);
        if self.bb_prices.len() > BB_PERIOD {
            self.bb_prices.pop_front();
        }
        let (bb_mid, bb_width) = self.bb_stats();
        self.bb_widths.push_back(bb_width);
        if self.bb_widths.len() > BB_SQUEEZE_LOOKBACK {
            self.bb_widths.pop_front();
        }

        self.count += 1;

        if self.count <= WARMUP_BARS {
            return Signal::Hold;
        }

        // --- Exit checks first (before voting) ---

        // RSI exit (sole exit mechanism — trailing stop removed)
        if self.position == 1 && rsi_val >= RSI_EXIT_LONG {
            self.position = 0;
            return Signal::Flat;
        }
        if self.position == -1 && rsi_val <= RSI_EXIT_SHORT {
            self.position = 0;
            return Signal::Flat;
        }

        // If already in a position, hold
        if self.position != 0 {
            return Signal::Hold;
        }

        // --- 6-signal voting for entry ---

        let mut bullish = 0_usize;
        let mut bearish = 0_usize;

        // 1. 12h Momentum
        if self.closes.len() > MOM_LONG_BARS {
            let past = self.closes[self.closes.len() - 1 - MOM_LONG_BARS];
            if close > past {
                bullish += 1;
            } else if close < past {
                bearish += 1;
            }
        }

        // 2. 6h V-Short Momentum
        if self.closes.len() > MOM_SHORT_BARS {
            let past = self.closes[self.closes.len() - 1 - MOM_SHORT_BARS];
            if close > past {
                bullish += 1;
            } else if close < past {
                bearish += 1;
            }
        }

        // 3. EMA Crossover
        if ema_fast_val > ema_slow_val {
            bullish += 1;
        } else if ema_fast_val < ema_slow_val {
            bearish += 1;
        }

        // 4. RSI(8)
        if rsi_val > 50.0 {
            bullish += 1;
        } else if rsi_val < 50.0 {
            bearish += 1;
        }

        // 5. MACD — REMOVED (simplification experiment)

        // 6. BB Compress — REMOVED (simplification experiment)
        // let squeezed = self.is_squeezed(bb_width);
        // if squeezed && bb_mid > 0.0 { ... }

        // Vote (now 5 signals, threshold still 4 = stricter filter)
        if bullish >= VOTE_THRESHOLD {
            self.position = 1;
            return Signal::Long;
        }
        if bearish >= VOTE_THRESHOLD {
            self.position = -1;
            return Signal::Short;
        }

        Signal::Hold
    }

    fn reset(&mut self) {
        *self = Self::new();
    }
}
