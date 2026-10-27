//! Scalar state plans: a state's scalar terminator and its successors, and
//! every state's ordered guarded exits. Unit planning, composed admission and
//! scalar completion read them.

use crate::checked_trees::flow::terminal::CheckedStructuralControlSuccessorPlan;
use symbols::SymbolHandle;

/// Every state's ordered guarded exits:
/// `tails` holds one row per state whose authored transition guards an exit,
/// and its `arms` span `exits`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedGuardedExitPlans {
    pub exits: arena::Arena<CheckedScalarGuardedExit>,
    pub tails: Vec<CheckedScalarGuardedTail>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedScalarGuardedTail {
    pub state: SymbolHandle,
    pub arms: arena::HandleSpan<CheckedScalarGuardedExit>,
    pub fallback: Option<CheckedScalarBranchDestination>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedScalarGuardedExit {
    pub guard_statement_ordinal: u32,
    pub destination: CheckedScalarBranchDestination,
}

impl Default for CheckedScalarGuardedExit {
    fn default() -> Self {
        Self {
            guard_statement_ordinal: 0,
            destination: CheckedScalarBranchDestination::Return {
                statement_ordinal: 0,
                is_continuation: false,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedScalarStateTerminator {
    /// Guards are tested in authored order. No fallback requires independently
    /// checked exhaustive coverage; the final guarded row remains explicit.
    Guarded {
        arms: arena::HandleSpan<CheckedScalarGuardedExit>,
        fallback: Option<CheckedScalarBranchDestination>,
    },
    Return {
        statement_ordinal: u32,
    },
    Crash {
        statement_ordinal: u32,
    },
    Jump(CheckedStructuralControlSuccessorPlan),
    Conditional {
        guard_statement_ordinal: u32,
        when_true: CheckedScalarBranchDestination,
        when_false: CheckedScalarBranchDestination,
    },
}

/// A selected scalar arm transfers into a state, returns, or crashes without a successor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedScalarBranchDestination {
    /// An exact checked module invocation with no normal result or successor.
    /// Its ordinary entry obligations and argument evaluation remain live.
    InvokeNoNormal {
        statement_ordinal: u32,
        is_continuation: bool,
    },
    Jump(CheckedStructuralControlSuccessorPlan),
    /// The exact standalone source crash retains its cause and site custody.
    Crash {
        statement_ordinal: u32,
    },
    Return {
        statement_ordinal: u32,
        /// The false sibling of a combined transition has its own value role.
        is_continuation: bool,
    },
}
