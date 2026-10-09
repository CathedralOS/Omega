use arena::Handle;

use crate::{
    BorrowArgumentAccessFact, BorrowCallFact, BorrowLoanFact, BorrowWritableRootFact,
    StateBorrowFact,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FlowSemanticContextRef {
    pub context: facts::FactContextHandle,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FlowConstraintKind {
    #[default]
    Unknown,
    SemanticContext {
        context: facts::FactContextHandle,
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

#[macro_export]
macro_rules! borrow_set_quint_pattern {
    ($($fields:tt)*) => {
        $crate::FlowConstraintKind::BorrowAccess { $($fields)* }
            | $crate::FlowConstraintKind::BorrowCall { $($fields)* }
            | $crate::FlowConstraintKind::BorrowLoan { $($fields)* }
            | $crate::FlowConstraintKind::BorrowState { $($fields)* }
            | $crate::FlowConstraintKind::BorrowWritableRoot { $($fields)* }
    };
}
