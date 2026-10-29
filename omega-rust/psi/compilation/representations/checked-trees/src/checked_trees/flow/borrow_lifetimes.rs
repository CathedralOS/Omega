use arena::Handle;

use crate::BorrowLoanFact;

use super::FlowInvalidationSource;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub enum FlowBorrowWeakeningReason {
    #[default]
    LastUseExpired,
    StateExit,
    LocalReassigned,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct FlowBorrowActivationFact {
    pub source: FlowInvalidationSource,
    pub loan: Handle<BorrowLoanFact>,
    /// The completed occurrence that formed this loan. Zero means no unique
    /// operand capture; raw loan geometry still supplies static coordinates,
    /// never a reread of a dynamic selector or initializer.
    pub formation_value: Handle<super::FlowOperandFact>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct FlowBorrowWeakeningFact {
    pub source: FlowInvalidationSource,
    pub loan: Handle<BorrowLoanFact>,
    pub reason: FlowBorrowWeakeningReason,
}
