//! Scalar state plans: the scalar bindings a state evaluates before its
//! terminator, the terminator and its successors, and every state's ordered
//! guarded exits. Unit planning, composed admission and scalar completion
//! read them; no separate scalar graph carries them.

use symbols::SymbolHandle;
use typed_trees::types::PrimitiveType;

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

/// One primitive computation evaluated in source order before the terminator.
/// Unborrowed mutable bindings resolve to the current scalar value; borrowed
/// locals retain their referents through the state's primitive-local roster.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedScalarBinding {
    pub statement_ordinal: u32,
    pub destination: CheckedScalarBindingDestination,
    pub primitive_type: PrimitiveType,
    pub value: CheckedScalarBindingValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedScalarBindingDestination {
    Immutable,
    StorageInitialize { symbol: SymbolHandle },
    StorageAssign { symbol: SymbolHandle },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedScalarBindingValue {
    Expression,
    /// The binding value's exact role names its checked computation root.
    Computation,
    /// One closed, receiver-free call whose result initializes this binding.
    /// The call coordinate joins directly to the checked crash-call row; its
    /// arguments live in `CheckedScalarExpressionPlans` under the same binding
    /// ordinal, so terminal production does not rediscover source expressions.
    DirectCall {
        target_machine: SymbolHandle,
        target_state: SymbolHandle,
        call_ordinal: u32,
        argument_count: u32,
    },
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
    Jump(CheckedScalarSuccessor),
    Conditional {
        guard_statement_ordinal: u32,
        when_true: CheckedScalarBranchDestination,
        when_false: CheckedScalarBranchDestination,
    },
}

/// A selected scalar arm transfers into a state, returns, or crashes without a successor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedScalarBranchDestination {
    Jump(CheckedScalarSuccessor),
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedScalarSuccessor {
    pub statement_ordinal: u32,
    pub is_continuation: bool,
    pub target: SymbolHandle,
    /// Complete authored arity, before partitioning the target namespaces.
    pub argument_count: u32,
}
