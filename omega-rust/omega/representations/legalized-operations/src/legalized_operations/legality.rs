//! legality in the legalized operations program.

/// Checked exact arithmetic. The instruction retains its semantic integer type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegalizedExactIntegerOperator {
    Add,
    Subtract,
}
