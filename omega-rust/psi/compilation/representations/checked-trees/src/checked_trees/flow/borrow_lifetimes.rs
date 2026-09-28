use arena::Handle;

use crate::BorrowLoanFact;

use super::FlowInvalidationSource;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FlowBorrowWeakeningReason {
    #[default]
    LastUseExpired,
    StateExit,
    LocalReassigned,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowBorrowActivationFact {
    pub source: FlowInvalidationSource,
    pub loan: Handle<BorrowLoanFact>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowBorrowWeakeningFact {
    pub source: FlowInvalidationSource,
    pub loan: Handle<BorrowLoanFact>,
    pub reason: FlowBorrowWeakeningReason,
}
