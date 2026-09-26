use arena::Handle;

use crate::checked_trees::{
    BorrowArgumentAccessFact, BorrowCallFact, BorrowLoanFact, BorrowWritableRootFact,
    StateBorrowFact,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FlowSemanticContextRef {
    pub context: crate::fact_plan::FactContextHandle,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FlowConstraintKind {
    #[default]
    Unknown,
    SemanticContext {
        context: crate::fact_plan::FactContextHandle,
    },
    BorrowState {
        state: Handle<StateBorrowFact>,
    },
    BorrowCall {
        call: Handle<BorrowCallFact>,
    },
    BorrowWritableRoot {
        root: Handle<BorrowWritableRootFact>,
    },
    BorrowAccess {
        access: Handle<BorrowArgumentAccessFact>,
    },
    BorrowLoan {
        loan: Handle<BorrowLoanFact>,
    },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FlowConstraintRef {
    pub kind: FlowConstraintKind,
}
