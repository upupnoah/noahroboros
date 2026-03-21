pub mod baseline;

use crate::market::Candle;

/// Signal emitted by a strategy for each candle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Signal {
    /// Go long (or stay long)
    Long,
    /// Go short (or stay short)
    Short,
    /// No position / close existing
    Flat,
    /// No opinion, hold current state
    Hold,
}

/// A trading strategy processes candles and emits signals.
///
/// The backtest engine calls `on_candle` for each candle in sequence.
/// The strategy maintains its own internal state (indicators, etc).
pub trait Strategy {
    fn name(&self) -> &str;
    fn on_candle(&mut self, candle: &Candle) -> Signal;
    fn reset(&mut self);
}
