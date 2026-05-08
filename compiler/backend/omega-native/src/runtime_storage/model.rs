use crate::control_flow::StateKey;
use crate::state_storage::{StateMutationKind, StateMutationLowering};
use omega_core::arena::Arena;
use omega_typed_program::expression::Expression;
use omega_typed_program::name::ProgramName;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeStoragePlan {
    pub frame_slots: Arena<RuntimeFrameSlot>,
    pub writes: Arena<RuntimeStorageWrite>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuntimeFrameSlot {
    pub dispatch_index: u32,
    pub source_key: StateKey,
    pub source_machine: ProgramName,
    pub source_state: ProgramName,
    pub statement_index: usize,
    pub name: ProgramName,
    pub type_name: String,
    pub byte_offset: usize,
    pub byte_size: usize,
    pub alignment: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeStorageWrite {
    pub dispatch_index: u32,
    pub source_key: StateKey,
    pub source_machine: ProgramName,
    pub source_state: ProgramName,
    pub statement_index: usize,
    pub target: Expression,
    pub value: Expression,
    pub mutation_kind: StateMutationKind,
    pub lowering: StateMutationLowering,
}

impl Default for RuntimeStorageWrite {
    fn default() -> Self {
        Self {
            dispatch_index: 0,
            source_key: StateKey::default(),
            source_machine: ProgramName::default(),
            source_state: ProgramName::default(),
            statement_index: 0,
            target: Expression::Integer(0),
            value: Expression::Integer(0),
            mutation_kind: StateMutationKind::Unknown,
            lowering: StateMutationLowering::Unknown,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeStorageBodyInput {
    pub body: crate::runtime_dispatch::bodies::RuntimeDispatchBody,
    pub operations: Vec<crate::runtime_dispatch::bodies::RuntimeDispatchBodyOperation>,
}
