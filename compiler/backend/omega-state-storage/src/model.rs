use omega_checked_trees::expression::{ExpressionHandle, ExpressionTable, ExpressionTableCapacity};
use omega_checked_trees::name::Identifier;
use omega_checked_trees::types::{TypeReferenceHandle, TypeReferenceTable};
use omega_control_flow::StateKey;
use omega_core::arena::{Arena, HandleSpan};
use omega_core::symbols::SymbolHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateStoragePlan {
    pub expressions: ExpressionTable,
    pub invariant_names: Arena<Identifier>,
    pub locals: Arena<StateLocalStorage>,
    pub mutations: Arena<StateMutation>,
    pub type_references: TypeReferenceTable,
}

impl StateStoragePlan {
    pub(crate) fn with_capacities(
        local_capacity: usize,
        mutation_capacity: usize,
        expression_capacity: ExpressionTableCapacity,
    ) -> Self {
        Self {
            expressions: ExpressionTable::with_capacities(expression_capacity),
            invariant_names: Arena::new(),
            locals: Arena::with_capacity(local_capacity),
            mutations: Arena::with_capacity(mutation_capacity),
            type_references: TypeReferenceTable::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StateLocalStorage {
    pub source_key: StateKey,
    pub statement_index: usize,
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub type_symbol: SymbolHandle,
    pub type_reference: TypeReferenceHandle,
    pub invariant_names: HandleSpan<Identifier>,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateMutation {
    pub source_key: StateKey,
    pub statement_index: usize,
    pub target: ExpressionHandle,
    pub value: ExpressionHandle,
    pub mutation_kind: StateMutationKind,
    pub lowering: StateMutationLowering,
    pub required: bool,
}

impl Default for StateMutation {
    fn default() -> Self {
        Self {
            source_key: StateKey::default(),
            statement_index: 0,
            target: ExpressionHandle::invalid(),
            value: ExpressionHandle::invalid(),
            mutation_kind: StateMutationKind::Unknown,
            lowering: StateMutationLowering::Unknown,
            required: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StateMutationKind {
    Local,
    MachineOwned,
    ParameterOrAlias,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StateMutationLowering {
    AlreadyLowered,
    NeedsLocalWrite,
    NeedsMachineOwnedWrite,
    NeedsAliasWrite,
    #[default]
    Unknown,
}
