use omega_control_flow::StateKey;
use omega_core::arena::{Arena, HandleSpan, PagedArena};
use omega_core::symbols::SymbolHandle;
use omega_checked_trees::name::ProgramName;
use omega_state_calls::StateCallLowering;
use omega_state_storage::{StateMutationKind, StateMutationLowering};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeDispatchBodyPlan {
    pub bodies: Arena<RuntimeDispatchBody>,
    pub operations: PagedArena<RuntimeDispatchBodyOperation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeDispatchBody {
    pub key: StateKey,
    pub dispatch_index: u32,
    pub operations: HandleSpan<RuntimeDispatchBodyOperation>,
}

impl Default for RuntimeDispatchBody {
    fn default() -> Self {
        Self {
            key: StateKey::default(),
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
        argument_count: usize,
    },
    InlineStateCall {
        target_key: StateKey,
        argument_count: usize,
        lowering: StateCallLowering,
    },
    StateCall {
        target_key: StateKey,
        argument_count: usize,
        lowering: StateCallLowering,
    },
    LocalStorage {
        symbol: SymbolHandle,
        name: ProgramName,
        type_symbol: SymbolHandle,
        type_name: String,
    },
    Mutation {
        mutation_kind: StateMutationKind,
        lowering: StateMutationLowering,
    },
    #[default]
    Other,
}
