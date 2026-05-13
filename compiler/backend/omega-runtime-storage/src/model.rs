use omega_control_flow::StateKey;
use omega_core::arena::Arena;
use omega_core::symbols::SymbolHandle;
use omega_runtime_bodies::RuntimeDispatchBody;
use omega_state_storage::{StateMutationKind, StateMutationLowering};
use omega_checked_trees::expression::{ExpressionHandle, ExpressionTable};
use omega_checked_trees::name::ProgramName;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeStoragePlan {
    pub expressions: ExpressionTable,
    pub frame_slots: Arena<RuntimeFrameSlot>,
    pub writes: Arena<RuntimeStorageWrite>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuntimeFrameSlot {
    pub dispatch_index: u32,
    pub source_key: StateKey,
    pub statement_index: usize,
    pub symbol: SymbolHandle,
    pub name: ProgramName,
    pub type_symbol: SymbolHandle,
    pub type_name: String,
    pub invariant_names: Vec<ProgramName>,
    pub byte_offset: usize,
    pub byte_size: usize,
    pub alignment: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeStorageWrite {
    pub dispatch_index: u32,
    pub source_key: StateKey,
    pub statement_index: usize,
    pub target: ExpressionHandle,
    pub value: ExpressionHandle,
    pub mutation_kind: StateMutationKind,
    pub lowering: StateMutationLowering,
}

impl Default for RuntimeStorageWrite {
    fn default() -> Self {
        Self {
            dispatch_index: 0,
            source_key: StateKey::default(),
            statement_index: 0,
            target: ExpressionHandle::invalid(),
            value: ExpressionHandle::invalid(),
            mutation_kind: StateMutationKind::Unknown,
            lowering: StateMutationLowering::Unknown,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeStorageBodyInput {
    pub body: RuntimeDispatchBody,
}
