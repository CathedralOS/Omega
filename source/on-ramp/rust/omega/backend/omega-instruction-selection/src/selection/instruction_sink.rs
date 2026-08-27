use omega_abstract_operations::{
    CheckedNoCodePermissionReason, PermissionRealizationCandidate,
    PermissionRealizationCandidateKind, SelectedInstruction,
};
use omega_control_flow::{ControlFlowPlan, StateKey};
use psi_arena::{Arena, Handle, HandleSpan};
use psi_symbols::SymbolHandle;

pub(super) struct SelectedInstructionSink<'arena, 'plan> {
    instructions: &'arena mut Arena<SelectedInstruction>,
    control_flow: &'plan ControlFlowPlan,
    start: Handle<SelectedInstruction>,
    count: u32,
    permission_realization_candidates: Vec<PermissionRealizationCandidate>,
    active_permission_events: Vec<u32>,
    active_instruction_count: u32,
    active_empty_selection_reason: Option<CheckedNoCodePermissionReason>,
}

impl<'arena, 'plan> SelectedInstructionSink<'arena, 'plan> {
    pub(super) fn new(
        instructions: &'arena mut Arena<SelectedInstruction>,
        control_flow: &'plan ControlFlowPlan,
    ) -> Self {
        Self {
            instructions,
            control_flow,
            start: Handle::invalid(),
            count: 0,
            permission_realization_candidates: Vec::new(),
            active_permission_events: Vec::new(),
            active_instruction_count: 0,
            active_empty_selection_reason: None,
        }
    }

    pub(super) fn push(&mut self, instruction: SelectedInstruction) {
        let handle = self.instructions.append(instruction);
        if self.count == 0 {
            self.start = handle;
        }
        self.count = self
            .count
            .checked_add(1)
            .expect("selected instruction span overflow");
        for source_event_index in &self.active_permission_events {
            self.permission_realization_candidates
                .push(PermissionRealizationCandidate {
                    source_event_index: *source_event_index,
                    kind: PermissionRealizationCandidateKind::SelectedInstruction {
                        instruction_index: handle.arena_index(),
                    },
                });
        }
    }

    pub(super) const fn len(&self) -> usize {
        self.count as usize
    }

    pub(super) fn pop(&mut self) -> Option<SelectedInstruction> {
        if self.count == 0 {
            return None;
        }

        let last_index = self
            .start
            .arena_index()
            .checked_add(self.count.checked_sub(1)?)
            .expect("selected instruction span overflow");
        let handle = Handle::from_parts(last_index, self.start.generation());
        let instruction = self.instructions.pop_last_appended(handle)?;
        self.permission_realization_candidates.retain(|candidate| {
            !matches!(
                candidate.kind,
                PermissionRealizationCandidateKind::SelectedInstruction { instruction_index }
                    if instruction_index == handle.arena_index()
            )
        });

        self.count -= 1;
        if self.count == 0 {
            self.start = Handle::invalid();
        }

        Some(instruction)
    }

    pub(super) fn begin_permission_site(
        &mut self,
        source_key: StateKey,
        statement_index: usize,
        call_ordinal: Option<Option<usize>>,
        call_target: Option<SymbolHandle>,
    ) {
        self.end_permission_site();
        self.active_instruction_count = self.count;
        self.active_empty_selection_reason =
            Some(CheckedNoCodePermissionReason::ExplicitZeroCodeConsume);

        self.include_permission_events_for_site(
            source_key,
            statement_index,
            call_ordinal,
            call_target,
        );
    }

    /// Add another semantic site to the currently selected instruction span.
    /// Inline branching calls emit their caller handoff, callee entry, and
    /// callee terminal action together, so all three canonical event sets must
    /// join the same concrete selection rather than competing for one active
    /// site slot.
    pub(super) fn include_permission_events_for_site(
        &mut self,
        source_key: StateKey,
        statement_index: usize,
        call_ordinal: Option<Option<usize>>,
        call_target: Option<SymbolHandle>,
    ) {
        let mut matching_events = Vec::new();
        for (_, state) in self.control_flow.states.iter().filter(|(_, state)| {
            state.key == source_key
                || (state.key.machine == source_key.machine && state.key.state == source_key.state)
        }) {
            let permission_span = state.ownership.permissions;
            let events = self
                .control_flow
                .semantics
                .ownership
                .permissions
                .span_or_empty(permission_span);
            let unique_call_ordinal = call_ordinal.and_then(|requested| {
                requested.or_else(|| {
                    let mut ordinals = events.iter().filter_map(|event| match event.source {
                        psi_language_semantics::PermissionEventSource::Call {
                            statement_index: event_statement,
                            call_ordinal,
                            target_symbol,
                            ..
                        } if event_statement == statement_index
                            && call_target.is_none_or(|target| target == target_symbol) =>
                        {
                            Some(call_ordinal)
                        }
                        _ => None,
                    });
                    let first = ordinals.next()?;
                    ordinals.all(|ordinal| ordinal == first).then_some(first)
                })
            });

            let mut matched_origins = Vec::new();
            for (event_offset, event) in events.iter().enumerate() {
                let matches = match event.source {
                    psi_language_semantics::PermissionEventSource::Statement {
                        statement_index: event_statement,
                    } => event_statement == statement_index,
                    psi_language_semantics::PermissionEventSource::Call {
                        statement_index: event_statement,
                        call_ordinal: event_ordinal,
                        target_symbol,
                        ..
                    } => {
                        event_statement == statement_index
                            && unique_call_ordinal == Some(event_ordinal)
                            && call_target.is_none_or(|target| target == target_symbol)
                    }
                    psi_language_semantics::PermissionEventSource::StateEntry
                    | psi_language_semantics::PermissionEventSource::StateExit => false,
                };
                if matches {
                    if let psi_language_semantics::PermissionProvenance::Established {
                        machine_symbol,
                        state_symbol,
                        source,
                    } = event.provenance
                    {
                        matched_origins.push((machine_symbol, state_symbol, source));
                    }
                    matching_events.push(
                        permission_span
                            .start()
                            .arena_index()
                            .checked_add(
                                u32::try_from(event_offset)
                                    .expect("permission event offset overflow"),
                            )
                            .expect("permission event index overflow"),
                    );
                }
            }
            for (event_offset, event) in events.iter().enumerate() {
                let source_not_future = match event.source {
                    // Entry establishment has its own concrete handoff site:
                    // an incoming platform write for the program entry, or the
                    // explicit state/transition handoff hook for an internal
                    // target. A later transfer/consume cannot retroactively
                    // prove that the value arrived.
                    psi_language_semantics::PermissionEventSource::StateEntry => false,
                    psi_language_semantics::PermissionEventSource::Statement {
                        statement_index: event_statement,
                    }
                    | psi_language_semantics::PermissionEventSource::Call {
                        statement_index: event_statement,
                        ..
                    } => event_statement <= statement_index,
                    psi_language_semantics::PermissionEventSource::StateExit => false,
                };
                let shares_origin = match event.provenance {
                    psi_language_semantics::PermissionProvenance::Established {
                        machine_symbol,
                        state_symbol,
                        source,
                    } => matched_origins.contains(&(machine_symbol, state_symbol, source)),
                    psi_language_semantics::PermissionProvenance::Unknown => false,
                };
                let establishes_origin = matches!(
                    event.kind,
                    psi_language_semantics::PermissionEventKind::Establish
                ) && matched_origins.iter().any(
                    |(machine_symbol, state_symbol, source)| {
                        state.key.machine == *machine_symbol
                            && state.key.state == *state_symbol
                            && event.source == *source
                    },
                );
                if !source_not_future || (!shares_origin && !establishes_origin) {
                    continue;
                }
                matching_events.push(
                    permission_span
                        .start()
                        .arena_index()
                        .checked_add(
                            u32::try_from(event_offset).expect("permission event offset overflow"),
                        )
                        .expect("permission event index overflow"),
                );
            }
        }
        matching_events.sort_unstable();
        matching_events.dedup();
        self.active_permission_events.extend(matching_events);
        self.active_permission_events.sort_unstable();
        self.active_permission_events.dedup();
    }

    pub(super) fn include_state_entry_permission_events(&mut self, target_key: StateKey) {
        for (_, state) in self.control_flow.states.iter().filter(|(_, state)| {
            state.key == target_key
                || (state.key.machine == target_key.machine && state.key.state == target_key.state)
        }) {
            let permission_span = state.ownership.permissions;
            for (event_offset, event) in self
                .control_flow
                .semantics
                .ownership
                .permissions
                .span_or_empty(permission_span)
                .iter()
                .enumerate()
            {
                if !matches!(
                    event.source,
                    psi_language_semantics::PermissionEventSource::StateEntry
                ) {
                    continue;
                }
                self.active_permission_events.push(
                    permission_span
                        .start()
                        .arena_index()
                        .checked_add(
                            u32::try_from(event_offset).expect("permission event offset overflow"),
                        )
                        .expect("permission event index overflow"),
                );
            }
        }
        self.active_permission_events.sort_unstable();
        self.active_permission_events.dedup();
    }

    pub(super) fn begin_state_entry_permission_site(&mut self, target_key: StateKey) {
        self.end_permission_site();
        self.active_instruction_count = self.count;
        // A live boundary input with no selected prologue write is missing
        // establishment evidence, not an exact no-code site. Leave it without
        // a candidate so ledger normalization fails closed. Zero-width entry
        // capabilities need a future explicit boundary-proof reason rather
        // than inheriting storage initialization.
        self.active_empty_selection_reason = None;
        self.include_state_entry_permission_events(target_key);
    }

    pub(super) fn end_permission_site(&mut self) {
        if !self.active_permission_events.is_empty()
            && self.count == self.active_instruction_count
            && let Some(reason) = self.active_empty_selection_reason
        {
            let explicit_actions = self
                .active_permission_events
                .iter()
                .copied()
                .filter(|source_event_index| {
                    self.control_flow
                        .semantics
                        .ownership
                        .permissions
                        .iter()
                        .find_map(|(handle, event)| {
                            (handle.arena_index() == *source_event_index).then_some(event)
                        })
                        .is_some_and(|event| {
                            matches!(
                                event.kind,
                                psi_language_semantics::PermissionEventKind::Consume
                            ) && event.access == psi_language_semantics::PermissionAccess::Owned
                                && event.obligation_live
                        })
                })
                .collect::<Vec<_>>();
            for source_event_index in explicit_actions {
                self.permission_realization_candidates
                    .push(PermissionRealizationCandidate {
                        source_event_index,
                        kind: PermissionRealizationCandidateKind::CheckedNoCode { reason },
                    });
            }
        }
        self.active_permission_events.clear();
        self.active_instruction_count = self.count;
        self.active_empty_selection_reason = None;
    }

    pub(super) fn finish(
        mut self,
    ) -> (
        HandleSpan<SelectedInstruction>,
        Vec<PermissionRealizationCandidate>,
    ) {
        self.end_permission_site();
        (
            HandleSpan::from_parts(self.start, self.count),
            self.permission_realization_candidates,
        )
    }
}
