use omega_checked_trees::expression::{ExpressionHandle, ExpressionTable, ExpressionTableCapacity};
use omega_checked_trees::name::Identifier;
use omega_control_flow::StateKey;
use omega_core::arena::{Arena, HandleSpan};
use omega_core::symbols::SymbolHandle;
use omega_layout::TypeLayoutDescriptor;
use omega_state_calls::StateCallRole;
use omega_state_storage::{StateMutationKind, StateMutationLowering};
use std::sync::Arc;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeStoragePlan {
    pub expressions: ExpressionTable,
    pub invariant_names: Arena<Identifier>,
    pub frame_slots: Arena<RuntimeFrameSlot>,
    pub writes: Arena<RuntimeStorageWrite>,
    /// Byte offset of a reserved frame SCRATCH region used to stage transition
    /// arguments when a same-call-context transition's source and target slots
    /// overlap (a parallel-copy cycle). 0 means no scratch reserved.
    pub frame_scratch_base: usize,
    /// Size of the reserved scratch region (0 if none). The scratch occupies
    /// `[frame_scratch_base, frame_scratch_base + frame_scratch_size)`.
    pub frame_scratch_size: usize,
}

impl RuntimeStoragePlan {
    pub(crate) fn with_capacities(
        expression_capacity: ExpressionTableCapacity,
        invariant_name_capacity: usize,
        frame_slot_capacity: usize,
        write_capacity: usize,
    ) -> Self {
        Self {
            expressions: ExpressionTable::with_capacities(expression_capacity),
            invariant_names: Arena::with_capacity(invariant_name_capacity),
            frame_slots: Arena::with_capacity(frame_slot_capacity),
            writes: Arena::with_capacity(write_capacity),
            frame_scratch_base: 0,
            frame_scratch_size: 0,
        }
    }

    fn source_matches(expected: StateKey, actual: StateKey) -> bool {
        expected == actual || (expected.machine == actual.machine && expected.state == actual.state)
    }

    pub fn call_result_slot(
        &self,
        dispatch_index: u32,
        source_key: StateKey,
        statement_index: usize,
        role: StateCallRole,
    ) -> Option<&RuntimeFrameSlot> {
        self.frame_slots
            .iter()
            .find_map(|(_, slot)| {
                (slot.dispatch_index == dispatch_index
                    && slot.source_key == source_key
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
            .or_else(|| {
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
        self.call_result_slot_by_ordinal(
            dispatch_index,
            source_key,
            statement_index,
            StateCallRole::AssignmentValue,
            call_ordinal,
        )
    }

    /// The caller's value-call-result slot for `(source_key, statement_index)`,
    /// searched across ALL dispatch indices. A dispatched value call's terminal
    /// write happens in the CALLEE's context (a different dispatch index than the
    /// caller's), but every context shares one frame region, so the slot's
    /// byte_offset is valid from the callee too. Prefers the StateCallResult slot
    /// (the `let n` the call feeds, which guards read); falls back to the matching
    /// LocalStorage slot when the result was allocated as a plain local.
    pub fn assignment_value_result_slot_any_dispatch(
        &self,
        source_key: StateKey,
        statement_index: usize,
    ) -> Option<&RuntimeFrameSlot> {
        self.frame_slots
            .iter()
            .find_map(|(_, slot)| {
                (Self::source_matches(slot.source_key, source_key)
                    && slot.statement_index == statement_index
                    && matches!(
                        slot.kind,
                        RuntimeFrameSlotKind::StateCallResult {
                            role: StateCallRole::AssignmentValue,
                            ..
                        }
                    ))
                .then_some(slot)
            })
            .or_else(|| {
                self.frame_slots.iter().find_map(|(_, slot)| {
                    (Self::source_matches(slot.source_key, source_key)
                        && slot.statement_index == statement_index
                        && matches!(slot.kind, RuntimeFrameSlotKind::LocalStorage))
                    .then_some(slot)
                })
            })
    }

    pub fn call_result_slot_by_ordinal(
        &self,
        dispatch_index: u32,
        source_key: StateKey,
        statement_index: usize,
        role: StateCallRole,
        call_ordinal: usize,
    ) -> Option<&RuntimeFrameSlot> {
        self.frame_slots
            .iter()
            .find_map(|(_, slot)| {
                (slot.dispatch_index == dispatch_index
                    && slot.source_key == source_key
                    && slot.statement_index == statement_index
                    && matches!(
                        slot.kind,
                        RuntimeFrameSlotKind::StateCallResult {
                            role: slot_role,
                            call_ordinal: slot_call_ordinal,
                            ..
                        } if slot_role == role && slot_call_ordinal == call_ordinal
                    ))
                .then_some(slot)
            })
            .or_else(|| {
                self.frame_slots.iter().find_map(|(_, slot)| {
                    (slot.dispatch_index == dispatch_index
                        && Self::source_matches(slot.source_key, source_key)
                        && slot.statement_index == statement_index
                        && matches!(
                            slot.kind,
                            RuntimeFrameSlotKind::StateCallResult {
                                role: slot_role,
                                call_ordinal: slot_call_ordinal,
                                ..
                            } if slot_role == role && slot_call_ordinal == call_ordinal
                        ))
                    .then_some(slot)
                })
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
    pub name: Identifier,
    pub type_symbol: SymbolHandle,
    pub type_name: Arc<str>,
    pub type_descriptor: TypeLayoutDescriptor,
    pub invariant_names: HandleSpan<Identifier>,
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
