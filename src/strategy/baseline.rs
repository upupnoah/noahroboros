use std::collections::VecDeque;

use crate::market::Candle;
use crate::strategy::{Signal, Strategy};
use ta::indicators::{ExponentialMovingAverage, RelativeStrengthIndex};
use ta::Next;

/// Hybrid strategy: proven +2.033 baseline + Nunchi mechanisms as opt-in toggles.
///
/// When all USE_* toggles are false, this produces the exact same signals as the
/// commit a1374a8 strategy (score +2.033 on ETH 1h, +1.851 on BTC/ETH/SOL 1h).
///
/// Autoresearch can enable individual mechanisms and tune parameters.
///
/// This is the file the AI agent modifies during autoresearch.

// === CORE PARAMETERS (proven values, score +2.033) ===

const MOM_LONG_BARS: usize = 12;
const MOM_SHORT_BARS: usize = 6;

const EMA_FAST_PERIOD: usize = 5;
const EMA_SLOW_PERIOD: usize = 21;

const RSI_PERIOD: usize = 24;
const RSI_EXIT_LONG: f64 = 80.0;
const RSI_EXIT_SHORT: f64 = 20.0;

const VOTE_THRESHOLD: usize = 3;

const WARMUP_BARS: usize = 120;

// === NUNCHI MECHANISM TOGGLES (all off = exact old behavior) ===

const USE_DYNAMIC_THRESHOLD: bool = false;
const USE_MACD: bool = false;
const USE_BB: bool = false;
const USE_ATR_STOP: bool = false;
const USE_COOLDOWN: bool = false;
const USE_SIGNAL_FLIP: bool = false;

// === MECHANISM PARAMETERS (Nunchi defaults, tunable) ===

// Dynamic momentum threshold
const BASE_THRESHOLD: f64 = 0.012;
const VOL_LOOKBACK: usize = 36;
const TARGET_VOL: f64 = 0.015;
const VSHORT_THRESHOLD_MULT: f64 = 0.7;

// MACD
const MACD_FAST: usize = 14;
const MACD_SLOW: usize = 23;
const MACD_SIGNAL: usize = 9;

// Bollinger Band width percentile
const BB_PERIOD: usize = 7;
const BB_COMPRESS_PCTILE: f64 = 90.0;

// ATR trailing stop
const ATR_LOOKBACK: usize = 24;
const ATR_STOP_MULT: f64 = 5.5;

// Cooldown
const COOLDOWN_BARS: usize = 2;

pub struct BaselineStrategy {
    closes: VecDeque<f64>,
    highs: VecDeque<f64>,
    lows: VecDeque<f64>,

    ema_fast: ExponentialMovingAverage,
    ema_slow: ExponentialMovingAverage,

    rsi: RelativeStrengthIndex,

    macd_fast_ema: ExponentialMovingAverage,
    macd_slow_ema: ExponentialMovingAverage,
    macd_signal_ema: ExponentialMovingAverage,

    position: i8,
    entry_price: f64,
    peak_price: f64,
    atr_at_entry: f64,
    bars_since_exit: usize,

    count: usize,
}

impl BaselineStrategy {
    pub fn new() -> Self {
        Self {
            closes: VecDeque::with_capacity(WARMUP_BARS + 1),
            highs: VecDeque::with_capacity(ATR_LOOKBACK + 2),
            lows: VecDeque::with_capacity(ATR_LOOKBACK + 2),

            ema_fast: ExponentialMovingAverage::new(EMA_FAST_PERIOD).unwrap(),
            ema_slow: ExponentialMovingAverage::new(EMA_SLOW_PERIOD).unwrap(),

            rsi: RelativeStrengthIndex::new(RSI_PERIOD).unwrap(),

            macd_fast_ema: ExponentialMovingAverage::new(MACD_FAST).unwrap(),
            macd_slow_ema: ExponentialMovingAverage::new(MACD_SLOW).unwrap(),
            macd_signal_ema: ExponentialMovingAverage::new(MACD_SIGNAL).unwrap(),

            position: 0,
            entry_price: 0.0,
            peak_price: 0.0,
            atr_at_entry: 0.0,
            bars_since_exit: 999,

            count: 0,
        }
    }

    fn calc_atr(&self) -> f64 {
        let n = self.highs.len();
        if n < 2 {
            return 0.0;
        }
        let lookback = ATR_LOOKBACK.min(n - 1);
        let closes: Vec<f64> = self.closes.iter().copied().collect();
        let highs: Vec<f64> = self.highs.iter().copied().collect();
        let lows: Vec<f64> = self.lows.iter().copied().collect();

        let h_len = highs.len();
        let c_len = closes.len();
        let mut sum_tr = 0.0;
        for i in 0..lookback {
            let hi = highs[h_len - 1 - i];
            let lo = lows[h_len - 1 - i];
            let prev_c = if c_len > i + 1 {
                closes[c_len - 2 - i]
            } else {
                lo
            };
            let tr = (hi - lo)
                .max((hi - prev_c).abs())
                .max((lo - prev_c).abs());
            sum_tr += tr;
        }
        sum_tr / lookback as f64
    }

    fn calc_realized_vol(&self) -> f64 {
        let n = self.closes.len();
        if n < VOL_LOOKBACK + 1 {
            return TARGET_VOL;
        }
        let start = n - VOL_LOOKBACK;
        let closes: Vec<f64> = self.closes.iter().copied().collect();
        let mut sum = 0.0;
        let mut sum_sq = 0.0;
        let count = VOL_LOOKBACK - 1;
        for i in start..(n - 1) {
            let log_ret = (closes[i + 1] / closes[i]).ln();
            sum += log_ret;
            sum_sq += log_ret * log_ret;
        }
        let mean = sum / count as f64;
        let var = sum_sq / count as f64 - mean * mean;
        var.max(0.0).sqrt().max(1e-6)
    }

    fn get_momentum_threshold(&self) -> f64 {
        if !USE_DYNAMIC_THRESHOLD {
            return 0.0;
        }
        let vol = self.calc_realized_vol();
        let vol_ratio = vol / TARGET_VOL;
        let dt = BASE_THRESHOLD * (0.3 + vol_ratio * 0.7);
        dt.max(0.005).min(0.020)
    }

    fn calc_bb_width_percentile(&self) -> f64 {
        let n = self.closes.len();
        if n < BB_PERIOD * 3 {
            return 50.0;
        }
        let closes: Vec<f64> = self.closes.iter().copied().collect();
        let mut widths = Vec::new();
        for i in (BB_PERIOD * 2)..n {
            let window = &closes[(i - BB_PERIOD)..i];
            let mean: f64 = window.iter().sum::<f64>() / window.len() as f64;
            if mean <= 0.0 {
                continue;
            }
            let var: f64 = window.iter().map(|x| (x - mean).powi(2)).sum::<f64>()
                / window.len() as f64;
            let std = var.sqrt();
            widths.push(2.0 * std / mean);
        }
        if widths.len() < 2 {
            return 50.0;
        }
        let current = *widths.last().unwrap();
        let below = widths.iter().filter(|w| **w <= current).count();
        100.0 * below as f64 / widths.len() as f64
    }
}

impl Strategy for BaselineStrategy {
    fn name(&self) -> &str {
        "nunchi_hybrid_v3"
    }

    fn on_candle(&mut self, candle: &Candle) -> Signal {
        let close = candle.close;

        self.closes.push_back(close);
        if self.closes.len() > WARMUP_BARS + 1 {
            self.closes.pop_front();
        }

        if USE_ATR_STOP {
            self.highs.push_back(candle.high);
            self.lows.push_back(candle.low);
            if self.highs.len() > ATR_LOOKBACK + 2 {
                self.highs.pop_front();
            }
            if self.lows.len() > ATR_LOOKBACK + 2 {
                self.lows.pop_front();
            }
        }

        let ema_fast_val = self.ema_fast.next(close);
        let ema_slow_val = self.ema_slow.next(close);
        let rsi_val = self.rsi.next(close);

        let macd_fast_val = self.macd_fast_ema.next(close);
        let macd_slow_val = self.macd_slow_ema.next(close);
        let macd_line = macd_fast_val - macd_slow_val;
        let _macd_signal_val = self.macd_signal_ema.next(macd_line);
        let macd_hist = macd_line - _macd_signal_val;

        self.count += 1;
        self.bars_since_exit += 1;

        if self.count <= WARMUP_BARS {
            return Signal::Hold;
        }

        let dyn_threshold = self.get_momentum_threshold();

        // === EXIT LOGIC ===
        if self.position != 0 {
            // ATR trailing stop (if enabled)
            if USE_ATR_STOP {
                let atr = self.calc_atr();
                let stop_atr = if self.atr_at_entry > 0.0 {
                    self.atr_at_entry
                } else {
                    atr
                };

                if self.position == 1 {
                    if close > self.peak_price {
                        self.peak_price = close;
                    }
                    if close < self.peak_price - ATR_STOP_MULT * stop_atr {
                        self.position = 0;
                        self.bars_since_exit = 0;
                        return Signal::Flat;
                    }
                } else {
                    if close < self.peak_price {
                        self.peak_price = close;
                    }
                    if close > self.peak_price + ATR_STOP_MULT * stop_atr {
                        self.position = 0;
                        self.bars_since_exit = 0;
                        return Signal::Flat;
                    }
                }
            }

            // RSI mean-reversion exit (always active)
            if self.position == 1 && rsi_val >= RSI_EXIT_LONG {
                self.position = 0;
                self.bars_since_exit = 0;
                return Signal::Flat;
            }
            if self.position == -1 && rsi_val <= RSI_EXIT_SHORT {
                self.position = 0;
                self.bars_since_exit = 0;
                return Signal::Flat;
            }
        }

        // === VOTING ===
        let mut bull_votes = 0_usize;
        let mut bear_votes = 0_usize;

        // 1. 12h Momentum
        if self.closes.len() > MOM_LONG_BARS {
            let past = self.closes[self.closes.len() - 1 - MOM_LONG_BARS];
            let ret = (close - past) / past;
            if ret > dyn_threshold {
                bull_votes += 1;
            } else if ret < -dyn_threshold {
                bear_votes += 1;
            }
        }

        // 2. 6h V-Short Momentum
        if self.closes.len() > MOM_SHORT_BARS {
            let past = self.closes[self.closes.len() - 1 - MOM_SHORT_BARS];
            let ret = (close - past) / past;
            let vshort_thresh = if USE_DYNAMIC_THRESHOLD {
                dyn_threshold * VSHORT_THRESHOLD_MULT
            } else {
                0.0
            };
            if ret > vshort_thresh {
                bull_votes += 1;
            } else if ret < -vshort_thresh {
                bear_votes += 1;
            }
        }

        // 3. EMA Crossover
        if ema_fast_val > ema_slow_val {
            bull_votes += 1;
        } else if ema_fast_val < ema_slow_val {
            bear_votes += 1;
        }

        // 4. RSI above/below 50
        if rsi_val > 50.0 {
            bull_votes += 1;
        } else if rsi_val < 50.0 {
            bear_votes += 1;
        }

        // 5. MACD histogram (if enabled)
        if USE_MACD {
            if macd_hist > 0.0 {
                bull_votes += 1;
            } else if macd_hist < 0.0 {
                bear_votes += 1;
            }
        }

        // 6. BB compression (if enabled)
        if USE_BB {
            let bb_pctile = self.calc_bb_width_percentile();
            if bb_pctile < BB_COMPRESS_PCTILE {
                bull_votes += 1;
                bear_votes += 1;
            }
        }

        let bullish = bull_votes >= VOTE_THRESHOLD;
        let bearish = bear_votes >= VOTE_THRESHOLD;
        let in_cooldown = USE_COOLDOWN && self.bars_since_exit < COOLDOWN_BARS;

        // Signal flip (if enabled)
        if USE_SIGNAL_FLIP && self.position != 0 && !in_cooldown {
            if self.position == 1 && bearish {
                self.position = -1;
                self.entry_price = close;
                self.peak_price = close;
                self.atr_at_entry = self.calc_atr();
                self.bars_since_exit = 999;
                return Signal::Short;
            }
            if self.position == -1 && bullish {
                self.position = 1;
                self.entry_price = close;
                self.peak_price = close;
                self.atr_at_entry = self.calc_atr();
                self.bars_since_exit = 999;
                return Signal::Long;
            }
        }

        // If in position and no flip, hold
        if self.position != 0 {
            return Signal::Hold;
        }

        // === NEW ENTRY ===
        if in_cooldown {
            return Signal::Hold;
        }

        if bullish {
            self.position = 1;
            self.entry_price = close;
            self.peak_price = close;
            self.atr_at_entry = self.calc_atr();
            return Signal::Long;
        }
        if bearish {
            self.position = -1;
            self.entry_price = close;
            self.peak_price = close;
            self.atr_at_entry = self.calc_atr();
            return Signal::Short;
        }

        Signal::Hold
    }

    fn reset(&mut self) {
        *self = Self::new();
    }
}
