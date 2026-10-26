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
    /// An existing invocation permission, not a local borrow activation.
    BorrowIncomingReference {
        identity: Handle<crate::IncomingReferenceIdentity>,
    },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FlowConstraintRef {
    pub kind: FlowConstraintKind,
}

#[macro_export]
macro_rules! borrow_resource_pattern {
    ($($fields:tt)*) => {
        $crate::FlowConstraintKind::BorrowAccess { $($fields)* }
            | $crate::FlowConstraintKind::BorrowCall { $($fields)* }
            | $crate::FlowConstraintKind::BorrowLoan { $($fields)* }
            | $crate::FlowConstraintKind::BorrowIncomingReference { $($fields)* }
            | $crate::FlowConstraintKind::BorrowState { $($fields)* }
            | $crate::FlowConstraintKind::BorrowWritableRoot { $($fields)* }
    };
}

#[macro_export]
macro_rules! nonloan_constraint_pattern {
    ($($fields:tt)*) => {
        $crate::FlowConstraintKind::BorrowAccess { $($fields)* }
            | $crate::FlowConstraintKind::BorrowCall { $($fields)* }
            | $crate::FlowConstraintKind::BorrowState { $($fields)* }
            | $crate::FlowConstraintKind::BorrowWritableRoot { $($fields)* }
            | $crate::FlowConstraintKind::SemanticContext { $($fields)* }
            | $crate::FlowConstraintKind::BorrowIncomingReference { $($fields)* }
    };
}
