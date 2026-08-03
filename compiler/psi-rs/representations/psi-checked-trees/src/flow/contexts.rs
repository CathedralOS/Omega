use psi_arena::Handle;

use crate::{
    BorrowArgumentAccessFact, BorrowCallFact, BorrowLoanFact, BorrowWritableRootFact,
    StateBorrowFact,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FlowSemanticContextRef {
    pub context: psi_facts::FactContextHandle,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FlowConstraintKind {
    #[default]
    Unknown,
    SemanticContext {
        context: psi_facts::FactContextHandle,
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
