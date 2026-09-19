//! Scalar graph plans: the machine and state graphs, primitive locals,
//! bindings, terminators and successors of scalar-only units.

use crate::checked_trees::flow::terminal::{
    CheckedStructuralControlTransferPlan, CheckedStructuralRankedSccPlan,
    CheckedStructuralScalarArgumentPlan, CheckedStructuralScalarParameterPlan,
    CheckedUnitEffectOperationPlan, CheckedUnitStructuralParameterPlan,
    CheckedUnitStructuralTypePlan,
};
use symbols::SymbolHandle;
use typed_trees::types::PrimitiveType;

/// Source-handle-free control plans accepted by the bootstrap terminal-Psi
/// scalar producer.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedScalarGraphPlans {
    pub machines: Vec<CheckedScalarMachineGraph>,
    pub parameter_storage: arena::Arena<CheckedScalarParameterStorage>,
    pub structural_transfers: arena::Arena<CheckedStructuralControlTransferPlan>,
    pub scalar_arguments: arena::Arena<CheckedStructuralScalarArgumentPlan>,
    pub structural_types: Vec<CheckedUnitStructuralTypePlan>,
    /// Ordered exits survive graph-body admission so ordinary Unit bodies use
    /// the same guard and selected-destination correspondence.
    pub guarded_exits: arena::Arena<CheckedScalarGuardedExit>,
    pub guarded_tails: Vec<CheckedScalarGuardedTail>,
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

/// One owned mutable primitive seeded from the current state's incoming value.
/// The ordinal retains the authored parameter slot, including immutable peers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedScalarParameterStorage {
    pub parameter_ordinal: u32,
    pub symbol: SymbolHandle,
    pub primitive_type: PrimitiveType,
}

impl Default for CheckedScalarParameterStorage {
    fn default() -> Self {
        Self {
            parameter_ordinal: 0,
            symbol: SymbolHandle::invalid(),
            primitive_type: PrimitiveType::Bool,
        }
    }
}

impl CheckedScalarGraphPlans {
    pub fn for_machine(&self, machine: SymbolHandle) -> Option<&CheckedScalarMachineGraph> {
        self.machines.iter().find(|plan| plan.machine == machine)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedScalarMachineGraph {
    pub machine: SymbolHandle,
    pub states: Vec<CheckedScalarStateGraph>,
    /// Exact source Nat countdown judgment, before private evaluation blocks
    /// are introduced. Absence does not authorize a termination guarantee.
    pub ranked_scc: Option<CheckedStructuralRankedSccPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedScalarStateGraph {
    pub state: SymbolHandle,
    /// Structural formals retain their complete authored parameter positions.
    pub structural_parameters: Vec<CheckedUnitStructuralParameterPlan>,
    /// Dense scalar formals retain the corresponding authored positions.
    pub scalar_parameters: Vec<CheckedStructuralScalarParameterPlan>,
    /// Proof-only erased scalar formals in authored order, retaining their
    /// authored parameter positions. They own no runtime argument lane.
    pub erased_scalar_parameters: Vec<CheckedStructuralScalarParameterPlan>,
    pub parameter_types: Vec<PrimitiveType>,
    pub parameter_storage: arena::HandleSpan<CheckedScalarParameterStorage>,
    /// Authored mutable locals borrowed by retained statements or computations.
    pub primitive_locals: Vec<CheckedScalarPrimitiveLocalPlan>,
    pub bindings: Vec<CheckedScalarBinding>,
    /// Ordered Unit effects retain authored coordinates independently of scalar slots.
    pub unit_operations: Vec<CheckedUnitEffectOperationPlan>,
    pub result_type: PrimitiveType,
    pub terminator: CheckedScalarStateTerminator,
}

/// A primitive local place required by this state's retained calls.
/// The binding at `statement_ordinal` owns the initializer and its source custody.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedScalarPrimitiveLocalPlan {
    pub statement_ordinal: u32,
    pub symbol: SymbolHandle,
    pub primitive_type: PrimitiveType,
    pub type_identity: String,
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
    pub structural_transfers: arena::HandleSpan<CheckedStructuralControlTransferPlan>,
    pub scalar_arguments: arena::HandleSpan<CheckedStructuralScalarArgumentPlan>,
    /// Proof-only erased actuals in erased-roster order. Each row's
    /// `target_scalar_parameter_index` indexes the target state's
    /// `erased_scalar_parameters` roster, not its dense scalar roster.
    pub erased_arguments: arena::HandleSpan<CheckedStructuralScalarArgumentPlan>,
}
