use omega_control_flow::StateKey;
use omega_core::arena::{Arena, HandleSpan};
use omega_core::symbols::SymbolHandle;
use omega_runtime_bodies::RuntimeDispatchBody;
use omega_state_calls::StateCallRole;
use omega_state_storage::{StateMutationKind, StateMutationLowering};
use omega_checked_trees::expression::{ExpressionHandle, ExpressionTable};
use omega_checked_trees::name::ProgramName;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeStoragePlan {
    pub expressions: ExpressionTable,
    pub invariant_names: Arena<ProgramName>,
    pub frame_slots: Arena<RuntimeFrameSlot>,
    pub writes: Arena<RuntimeStorageWrite>,
}

impl RuntimeStoragePlan {
    fn source_matches(expected: StateKey, actual: StateKey) -> bool {
        expected == actual
            || (expected.machine == actual.machine && expected.state == actual.state)
    }

    pub fn call_result_slot(
        &self,
        dispatch_index: u32,
        source_key: StateKey,
        statement_index: usize,
        role: StateCallRole,
    ) -> Option<&RuntimeFrameSlot> {
        self.frame_slots.iter().find_map(|(_, slot)| {
            (slot.dispatch_index == dispatch_index
                && Self::source_matches(slot.source_key, source_key)
                && slot.statement_index == statement_index
                && matches!(
                    slot.kind,
                    RuntimeFrameSlotKind::StateCallResult {
                        role: slot_role,
                        ..
                    } if slot_role == role
                ))
            .then_some(slot)
        })
    }

    pub fn assignment_value_result_slot(
        &self,
        dispatch_index: u32,
        source_key: StateKey,
        statement_index: usize,
    ) -> Option<&RuntimeFrameSlot> {
        self.call_result_slot(
            dispatch_index,
            source_key,
            statement_index,
            StateCallRole::AssignmentValue,
        )
    }

    pub fn assignment_value_result_slot_by_ordinal(
        &self,
        dispatch_index: u32,
        source_key: StateKey,
        statement_index: usize,
        call_ordinal: usize,
    ) -> Option<&RuntimeFrameSlot> {
        self.frame_slots.iter().find_map(|(_, slot)| {
            (slot.dispatch_index == dispatch_index
                && Self::source_matches(slot.source_key, source_key)
                && slot.statement_index == statement_index
                && matches!(
                    slot.kind,
                    RuntimeFrameSlotKind::StateCallResult {
                        role: StateCallRole::AssignmentValue,
                        call_ordinal: slot_call_ordinal,
                        ..
                    } if slot_call_ordinal == call_ordinal
                ))
            .then_some(slot)
        })
    }

    pub fn transition_guard_result_slot(
        &self,
        dispatch_index: u32,
        source_key: StateKey,
        statement_index: usize,
    ) -> Option<&RuntimeFrameSlot> {
        self.call_result_slot(
            dispatch_index,
            source_key,
            statement_index,
            StateCallRole::TransitionGuard,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuntimeFrameSlot {
    pub dispatch_index: u32,
    pub source_key: StateKey,
    pub statement_index: usize,
    pub kind: RuntimeFrameSlotKind,
    pub symbol: SymbolHandle,
    pub name: ProgramName,
    pub type_symbol: SymbolHandle,
    pub type_name: String,
    pub invariant_names: HandleSpan<ProgramName>,
    pub byte_offset: usize,
    pub byte_size: usize,
    pub alignment: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeFrameSlotKind {
    Parameter,
    LocalStorage,
    StateCallResult {
        role: StateCallRole,
        call_ordinal: usize,
        target_key: StateKey,
    },
}

impl Default for RuntimeFrameSlotKind {
    fn default() -> Self {
        Self::LocalStorage
    }
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
