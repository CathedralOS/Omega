use crate::control_flow::StateKey;
use omega_core::arena::Arena;
use omega_typed_program::expression::Expression;
use omega_typed_program::name::ProgramName;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateStoragePlan {
    pub locals: Arena<StateLocalStorage>,
    pub mutations: Arena<StateMutation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StateLocalStorage {
    pub source_key: StateKey,
    pub machine: ProgramName,
    pub state: ProgramName,
    pub statement_index: usize,
    pub name: ProgramName,
    pub type_name: String,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateMutation {
    pub source_key: StateKey,
    pub machine: ProgramName,
    pub state: ProgramName,
    pub statement_index: usize,
    pub target: Expression,
    pub value: Expression,
    pub mutation_kind: StateMutationKind,
    pub lowering: StateMutationLowering,
    pub required: bool,
}

impl Default for StateMutation {
    fn default() -> Self {
        Self {
            source_key: StateKey::default(),
            machine: ProgramName::default(),
            state: ProgramName::default(),
            statement_index: 0,
            target: Expression::Integer(0),
            value: Expression::Integer(0),
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
