use crate::control_flow::StateKey;
use crate::state_calls::StateCallLowering;
use crate::state_storage::{StateMutationKind, StateMutationLowering};
use omega_core::arena::{Arena, HandleSpan, PagedArena};
use omega_typed_program::name::ProgramName;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeDispatchBodyPlan {
    pub bodies: Arena<RuntimeDispatchBody>,
    pub operations: PagedArena<RuntimeDispatchBodyOperation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeDispatchBody {
    pub key: StateKey,
    pub machine: ProgramName,
    pub state: ProgramName,
    pub dispatch_index: u32,
    pub operations: HandleSpan<RuntimeDispatchBodyOperation>,
}

impl Default for RuntimeDispatchBody {
    fn default() -> Self {
        Self {
            key: StateKey::default(),
            machine: ProgramName::default(),
            state: ProgramName::default(),
            dispatch_index: 0,
            operations: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeDispatchBodyOperation {
    pub source_key: StateKey,
    pub statement_index: usize,
    pub kind: RuntimeDispatchBodyOperationKind,
}

impl Default for RuntimeDispatchBodyOperation {
    fn default() -> Self {
        Self {
            source_key: StateKey::default(),
            statement_index: 0,
            kind: RuntimeDispatchBodyOperationKind::Other,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum RuntimeDispatchBodyOperationKind {
    HostCall {
        platform_call: String,
    },
    InlineLeafStateCall {
        target_key: StateKey,
        target_machine: ProgramName,
        target_state: ProgramName,
        argument_count: usize,
    },
    InlineStateCall {
        target_key: StateKey,
        target_machine: ProgramName,
        target_state: ProgramName,
        argument_count: usize,
        lowering: StateCallLowering,
    },
    StateCall {
        target_key: StateKey,
        target_machine: ProgramName,
        target_state: ProgramName,
        argument_count: usize,
        lowering: StateCallLowering,
    },
    LocalStorage {
        name: ProgramName,
        type_name: String,
    },
    Mutation {
        mutation_kind: StateMutationKind,
        lowering: StateMutationLowering,
    },
    #[default]
    Other,
}
