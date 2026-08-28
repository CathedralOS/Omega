use omega_control_flow::StateKey;
use omega_layout::TypeLayoutDescriptor;
use omega_state_calls::StateCallRole;
use omega_state_storage::{StateMutationKind, StateMutationLowering};
use psi_arena::Arena;
use psi_checked_trees::expression::{ExpressionHandle, ExpressionTable, ExpressionTableCapacity};
use psi_checked_trees::name::Identifier;
use psi_symbols::SymbolHandle;
use std::sync::Arc;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeStoragePlan {
    pub expressions: ExpressionTable,
    pub frame_slots: Arena<RuntimeFrameSlot>,
    pub writes: Arena<RuntimeStorageWrite>,
    /// Byte offset of a reserved frame SCRATCH region used to stage transition
    /// arguments when a same-call-context transition's source and target slots
    /// overlap (a parallel-copy cycle). 0 means no scratch reserved.
    pub frame_scratch_base: usize,
    /// Size of the reserved scratch region (0 if none). The scratch occupies
    /// `[frame_scratch_base, frame_scratch_base + frame_scratch_size)`.
    pub frame_scratch_size: usize,
    /// Byte offset of the reserved WIRE NESTED-MESSAGE scratch region
    /// (chapter 20): a `{ptr @ +0, len @ +8}` text descriptor followed by a
    /// staging buffer at +16 sized for the largest nested sub-message's
    /// worst-case body. The encoder stages a nested field's bytes here before
    /// replaying them (length varint + copy) into the caller's out buffer;
    /// the decoder reuses the first 8 bytes as the sub-region end-bound slot.
    /// Wire ops run strictly inside one statement, so a single region serves
    /// every encode/decode site. 0 means no wire scratch reserved.
    pub wire_scratch_base: usize,
    /// Size of the reserved wire scratch region (0 if none).
    pub wire_scratch_size: usize,
    /// Per-argument scalar scratch used to materialize computed host-call
    /// arguments before the platform marshaller reads them. Each slot is one
    /// native word and the region is reused across statements.
    pub host_argument_scratch_base: usize,
    pub host_argument_scratch_size: usize,
    /// Byte offset of the reserved ENTRY-ARGUMENT SPILL region: the entry
    /// prologue stores the platform's four incoming argument registers here,
    /// and the entry's `args: &[u8]` slice descriptor points at it (the bytes
    /// handoff -- `run(&self, args: &[u8])`). 0 means no spill reserved.
    pub entry_argument_spill_base: usize,
    /// Size of the reserved entry-argument spill (0 if none; 32 when present).
    pub entry_argument_spill_size: usize,
    /// Byte offset of the saved native indirect-result destination pointer.
    /// The entry prologue captures the ABI-selected register here before
    /// volatile work; a large terminal copies through it. Zero size means no
    /// pointer reserved.
    pub entry_indirect_result_pointer_base: usize,
    pub entry_indirect_result_pointer_size: usize,
    /// Frame scratch used to materialize an entry terminal before loading or
    /// copying its ABI-selected result placement. Zero size means no terminal
    /// expression needs a materialized place.
    pub entry_result_scratch_base: usize,
    pub entry_result_scratch_size: usize,
}

impl RuntimeStoragePlan {
    pub(crate) fn with_capacities(
        expression_capacity: ExpressionTableCapacity,
        frame_slot_capacity: usize,
        write_capacity: usize,
    ) -> Self {
        Self {
            expressions: ExpressionTable::with_capacities(expression_capacity),
            frame_slots: Arena::with_capacity(frame_slot_capacity),
            writes: Arena::with_capacity(write_capacity),
            frame_scratch_base: 0,
            frame_scratch_size: 0,
            wire_scratch_base: 0,
            wire_scratch_size: 0,
            host_argument_scratch_base: 0,
            host_argument_scratch_size: 0,
            entry_argument_spill_base: 0,
            entry_argument_spill_size: 0,
            entry_indirect_result_pointer_base: 0,
            entry_indirect_result_pointer_size: 0,
            entry_result_scratch_base: 0,
            entry_result_scratch_size: 0,
        }
    }

    pub(crate) fn source_matches(expected: StateKey, actual: StateKey) -> bool {
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

    /// Any-role sibling of `assignment_value_result_slot_any_dispatch`: the
    /// call-result slot for a statement REGARDLESS of the call's role
    /// (TransitionArgument / CallArgument / TransitionGuard results live in
    /// role-tagged slots the AssignmentValue-only lookup misses -- a
    /// dispatched callee's terminal return-write must still find them).
    /// AssignmentValue slots are preferred for determinism when a statement
    /// somehow carries both. New return-write paths use the ordinal-aware
    /// siblings below; this lookup remains for a bare single-call initializer
    /// whose result is represented by its LocalStorage slot.
    /// Dispatch-keyed sibling of `state_call_result_slot_any_role`: call-result
    /// slots are duplicated per dispatch context (a segmented caller has one
    /// per segment case), and a clone terminal's return-write must hit the
    /// slot of the CONTINUATION it returns into -- `continuation_dispatch_index`
    /// on the terminal edge -- or the writer and the reader (who resolves under
    /// the continuation's context) touch DIFFERENT slots (probed: write hit
    /// dispatch 1 offset 0, read used dispatch 4 offset 4; the transition-arg
    /// result arrived as ZII).
    pub fn state_call_result_slot_for_dispatch(
        &self,
        dispatch_index: u32,
        source_key: StateKey,
        statement_index: usize,
    ) -> Option<&RuntimeFrameSlot> {
        self.frame_slots.iter().find_map(|(_, slot)| {
            (slot.dispatch_index == dispatch_index
                && Self::source_matches(slot.source_key, source_key)
                && slot.statement_index == statement_index
                && matches!(slot.kind, RuntimeFrameSlotKind::StateCallResult { .. }))
            .then_some(slot)
        })
    }

    pub fn state_call_result_slot_for_dispatch_by_ordinal(
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
                        call_ordinal: slot_ordinal,
                        ..
                    } if slot_ordinal == call_ordinal
                ))
            .then_some(slot)
        })
    }

    pub fn state_call_result_slot_any_role(
        &self,
        source_key: StateKey,
        statement_index: usize,
    ) -> Option<&RuntimeFrameSlot> {
        self.assignment_value_result_slot_any_dispatch(source_key, statement_index)
            .or_else(|| {
                self.frame_slots.iter().find_map(|(_, slot)| {
                    (Self::source_matches(slot.source_key, source_key)
                        && slot.statement_index == statement_index
                        && matches!(slot.kind, RuntimeFrameSlotKind::StateCallResult { .. }))
                    .then_some(slot)
                })
            })
    }

    pub fn state_call_result_slot_any_role_by_ordinal(
        &self,
        source_key: StateKey,
        statement_index: usize,
        call_ordinal: usize,
    ) -> Option<&RuntimeFrameSlot> {
        self.frame_slots.iter().find_map(|(_, slot)| {
            (Self::source_matches(slot.source_key, source_key)
                && slot.statement_index == statement_index
                && matches!(
                    slot.kind,
                    RuntimeFrameSlotKind::StateCallResult {
                        call_ordinal: slot_ordinal,
                        ..
                    } if slot_ordinal == call_ordinal
                ))
            .then_some(slot)
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
    pub byte_offset: usize,
    pub byte_size: usize,
    pub alignment: usize,
    /// A statically selected boundary capability carries no runtime bytes in
    /// the current provider model. Its type/selection identity is sufficient
    /// for host-call lowering, so a zero-sized slot is intentional rather than
    /// evidence that layout planning failed.
    pub is_static_boundary_capability: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeFrameSlotKind {
    Parameter,
    DynamicReceiver {
        realization: StateKey,
    },
    DynamicResultScratch {
        realization: StateKey,
    },
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
