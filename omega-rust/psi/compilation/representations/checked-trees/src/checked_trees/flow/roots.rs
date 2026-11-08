type Arena<T> = arena::PagedArenaSet<T>;
use symbols::SymbolHandle;

use crate::checked_trees::keyed_rows::KeyedRows;

use super::{
    FlowBorrowActivationFact, FlowBorrowWeakeningFact, FlowBoundaryEdgeFact, FlowCallFact,
    FlowClaimOutcomeEntryFact, FlowClaimOutcomeMapFact, FlowConstraintRef, FlowExitFact,
    FlowInvalidationFact, FlowPermissionEventFact, FlowSemanticContextRef, FlowStateFact,
    FlowStatementFact,
};

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct FlowContextFacts {
    pub semantic_context_refs: Arena<FlowSemanticContextRef>,
    pub constraint_refs: Arena<FlowConstraintRef>,
}

impl FlowContextFacts {
    pub fn with_roots(
        semantic_context_refs: Arena<FlowSemanticContextRef>,
        constraint_refs: Arena<FlowConstraintRef>,
    ) -> Self {
        Self {
            semantic_context_refs,
            constraint_refs,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct FlowInvalidationFacts {
    pub segments: Arena<facts::PlaceSegment>,
    pub events: Arena<FlowInvalidationFact>,
}

impl FlowInvalidationFacts {
    pub fn with_roots(
        segments: Arena<facts::PlaceSegment>,
        events: Arena<FlowInvalidationFact>,
    ) -> Self {
        Self { segments, events }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct FlowBorrowLifetimeFacts {
    pub activations: Arena<FlowBorrowActivationFact>,
    pub weakenings: Arena<FlowBorrowWeakeningFact>,
}

impl FlowBorrowLifetimeFacts {
    pub fn with_roots(
        activations: Arena<FlowBorrowActivationFact>,
        weakenings: Arena<FlowBorrowWeakeningFact>,
    ) -> Self {
        Self {
            activations,
            weakenings,
        }
    }
}

/// The states a worker's copy of the ownership tables is walking, and
/// whether any read on that copy asked for another state's per-state rows.
///
/// The permission walk can run each state on its own checking-pool worker,
/// against a copy of these tables that shares every existing page and writes
/// only the state's own page. That equals one serial walk only if no state
/// reads a row another state writes during the walk. Every per-state lookup
/// the walk can reach notes the state it reads here; a walk that read outside
/// its own state is discarded and the whole walk runs serially instead. The
/// guard travels with the copy, so reads made on any pool thread are noted.
/// An ordinary table carries no guard and notes nothing.
#[derive(Debug, Clone, Default)]
pub struct WalkGuard(Option<std::sync::Arc<WalkGuardState>>);

#[derive(Debug)]
struct WalkGuardState {
    /// The walked states, sorted by arena position for lookup.
    states: Vec<SymbolHandle>,
    crossed: std::sync::atomic::AtomicBool,
}

impl WalkGuard {
    pub fn for_states(mut states: Vec<SymbolHandle>) -> Self {
        states.sort_unstable_by_key(|state| (state.arena_index(), state.generation()));
        Self(Some(std::sync::Arc::new(WalkGuardState {
            states,
            crossed: std::sync::atomic::AtomicBool::new(false),
        })))
    }

    /// Note a read of `state`'s per-state rows.
    pub fn note(&self, state: SymbolHandle) {
        let Some(guard) = &self.0 else {
            return;
        };
        let owned = guard
            .states
            .binary_search_by_key(&(state.arena_index(), state.generation()), |owned| {
                (owned.arena_index(), owned.generation())
            })
            .is_ok();
        if !owned {
            guard
                .crossed
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }

    /// Whether any read on this copy asked for a state it does not own.
    pub fn crossed(&self) -> bool {
        self.0
            .as_ref()
            .is_some_and(|guard| guard.crossed.load(std::sync::atomic::Ordering::Relaxed))
    }
}

// Derived, nonsemantic state: two tables are equal whatever guards they carry.
impl PartialEq for WalkGuard {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Eq for WalkGuard {}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct FlowOwnershipFacts {
    pub instruction_effect_receipts: Arena<super::FlowInstructionEffectReceiptFact>,
    pub instruction_dispositions: Arena<super::FlowInstructionDispositionFact>,
    pub segments: Arena<facts::PlaceSegment>,
    pub storage_transfers: Arena<super::FlowStorageTransferFact>,
    pub permissions: Arena<FlowPermissionEventFact>,
    pub borrow_claims: Arena<super::FlowBorrowClaimFact>,
    /// Candidate normal deaths, separate from the unconditional replay ledger.
    pub selected_cleanup_permissions: Arena<FlowPermissionEventFact>,
    pub loan_completions: Arena<super::FlowLoanCompletionFact>,
    pub claim_outcome_entries: Arena<FlowClaimOutcomeEntryFact>,
    pub claim_outcome_maps: Arena<FlowClaimOutcomeMapFact>,
    pub claim_join_receipts: Arena<super::FlowClaimJoinReceipt>,
    pub claim_join_alternatives: Arena<super::FlowClaimJoinAlternative>,
    pub claim_join_exits: Arena<super::FlowClaimJoinExit>,
    pub claim_join_captures: Arena<super::FlowClaimJoinCapture>,
    pub owned_selections: Arena<super::FlowOwnedSelectionReceipt>,
    pub selection_sources: Arena<super::FlowOwnedSelectionSource>,
    pub selection_transfers: Arena<super::FlowOwnedSelectionTransfer>,
    pub selection_transfer_claims: Arena<super::FlowOwnedSelectionClaim>,
    pub selection_place_bindings: Arena<super::FlowSelectionPlaceBinding>,
    /// Final placement of each live owned linear transfer within its operand.
    pub owned_transfer_paths: Arena<super::FlowOwnedTransferPath>,
    pub owned_transfer_path_segments: Arena<super::CheckedUnitStructuralPathSegment>,
    pub permissions_by_state: KeyedRows<SymbolHandle, FlowPermissionEventFact>,
    pub owned_selections_by_state: KeyedRows<SymbolHandle, super::FlowOwnedSelectionReceipt>,
    pub selection_place_bindings_by_state:
        KeyedRows<SymbolHandle, super::FlowSelectionPlaceBinding>,
    /// Present only on a worker's copy that walks one state's permissions; see
    /// [`WalkGuard`]. Derived, nonsemantic state: never recorded or compared.
    #[record(skip)]
    pub walk_guard: WalkGuard,
}

impl FlowOwnershipFacts {
    pub fn with_roots(
        segments: Arena<facts::PlaceSegment>,
        permissions: Arena<FlowPermissionEventFact>,
        claim_outcome_entries: Arena<FlowClaimOutcomeEntryFact>,
        claim_outcome_maps: Arena<FlowClaimOutcomeMapFact>,
    ) -> Self {
        Self {
            instruction_effect_receipts: Arena::default(),
            instruction_dispositions: Arena::default(),
            segments,
            permissions,
            storage_transfers: Arena::default(),
            borrow_claims: Arena::default(),
            selected_cleanup_permissions: Arena::default(),
            loan_completions: Arena::default(),
            claim_outcome_entries,
            claim_outcome_maps,
            claim_join_receipts: Arena::default(),
            claim_join_alternatives: Arena::default(),
            claim_join_exits: Arena::default(),
            claim_join_captures: Arena::default(),
            owned_selections: Arena::default(),
            selection_sources: Arena::default(),
            selection_transfers: Arena::default(),
            selection_transfer_claims: Arena::default(),
            selection_place_bindings: Arena::default(),
            owned_transfer_paths: Arena::default(),
            owned_transfer_path_segments: Arena::default(),
            permissions_by_state: KeyedRows::default(),
            owned_selections_by_state: KeyedRows::default(),
            selection_place_bindings_by_state: KeyedRows::default(),
            walk_guard: WalkGuard::default(),
        }
    }

    pub fn with_state_permissions<R>(
        &self,
        state: SymbolHandle,
        run: impl FnOnce(&[arena::Handle<FlowPermissionEventFact>]) -> R,
    ) -> R {
        self.walk_guard.note(state);
        self.permissions_by_state.with_rows(
            &self.permissions,
            |event| event.state_symbol,
            &state,
            run,
        )
    }

    pub fn with_state_owned_selections<R>(
        &self,
        state: SymbolHandle,
        run: impl FnOnce(&[arena::Handle<super::FlowOwnedSelectionReceipt>]) -> R,
    ) -> R {
        self.walk_guard.note(state);
        self.owned_selections_by_state.with_rows(
            &self.owned_selections,
            |receipt| receipt.state,
            &state,
            run,
        )
    }

    pub fn with_state_selection_place_bindings<R>(
        &self,
        state: SymbolHandle,
        run: impl FnOnce(&[arena::Handle<super::FlowSelectionPlaceBinding>]) -> R,
    ) -> R {
        self.walk_guard.note(state);
        self.selection_place_bindings_by_state.with_rows(
            &self.selection_place_bindings,
            |binding| binding.state,
            &state,
            run,
        )
    }

    pub fn seal_indexes(&mut self) {
        self.permissions_by_state
            .seal(&self.permissions, |event| event.state_symbol);
        self.owned_selections_by_state
            .seal(&self.owned_selections, |receipt| receipt.state);
        self.selection_place_bindings_by_state
            .seal(&self.selection_place_bindings, |binding| binding.state);
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct FlowBoundaryFacts {
    pub edges: Arena<FlowBoundaryEdgeFact>,
}

impl FlowBoundaryFacts {
    pub fn with_roots(edges: Arena<FlowBoundaryEdgeFact>) -> Self {
        Self { edges }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct FlowControlFacts {
    pub storage_snapshots: Arena<super::FlowStorageSnapshot>,
    pub storage_predecessors: Arena<arena::Handle<super::FlowStorageSnapshot>>,
    pub value_observations: Arena<super::FlowValueObservationFact>,
    pub descriptor_context_observations: super::FlowDescriptorObservationTable,
    pub atomic_executions: Arena<super::FlowAtomicExecutionFact>,
    pub case_accesses: Arena<super::FlowCaseAccessFact>,
    pub owned_case_selections: Arena<super::FlowOwnedCaseSelection>,
    pub owned_cases_by_expression:
        KeyedRows<typed_trees::expression::ExpressionHandle, super::FlowOwnedCaseSelection>,
    pub operator_invocations: Arena<super::FlowOperatorInvocationFact>,
    pub operands: Arena<super::FlowOperandFact>,
    pub copied_values: Arena<super::FlowCopiedValue>,
    pub copied_value_uses: Arena<super::FlowCopiedValueUse>,
    pub captured_reference_leaves: Arena<super::FlowCapturedReferenceLeaf>,
    /// Captured occurrences grouped by their actual evaluated expression.
    /// The shared index follows the operand arena's exact mutation revision.
    pub operands_by_expression:
        KeyedRows<typed_trees::expression::ExpressionHandle, super::FlowOperandFact>,
    pub operand_children: Arena<super::FlowOperandChild>,
    pub operand_alternatives: Arena<super::FlowOperandAlternative>,
    pub operand_child_paths: Arena<facts::PlaceSegment>,
    /// Exact storage an operand's evaluated view carrier denotes, one row per
    /// resolvable referent; segment payloads live in `operand_referent_segments`
    /// in the semantic `Place` vocabulary.
    pub operand_origin_writes: Arena<super::FlowOperandReferent>,
    pub operand_referents: Arena<super::FlowOperandReferent>,
    pub referent_formations: Arena<super::FlowReferentFormation>,
    pub operand_referent_segments: Arena<facts::PlaceSegment>,
    pub operand_retentions: Arena<super::FlowOperandRetention>,
    pub operand_retained_roots: Arena<facts::PlaceRoot>,
    pub operand_retained_leaves: Arena<super::FlowOperandRetainedLeaf>,
    pub operand_retained_suppliers: Arena<super::FlowOperandRetainedSupplier>,
    pub operand_stored_sources: Arena<super::FlowOperandStoredSource>,
    pub operand_reborrow_sources: Arena<super::FlowOperandReborrowSource>,
    pub output_lifetime_sources: Arena<super::FlowOutputLifetimeSources>,
    pub operand_retained_regions: Arena<super::FlowOperandRetainedRegion>,
    pub operand_retained_paths: Arena<facts::PlaceSegment>,
    pub statements: Arena<FlowStatementFact>,
    pub calls: Arena<FlowCallFact>,
    /// Authored argument expressions, spanned by [`FlowCallFact::authored_arguments`].
    pub call_arguments: Arena<typed_trees::expression::ExpressionHandle>,
    pub call_argument_positions: Arena<u32>,
    pub exits: Arena<FlowExitFact>,
    pub outgoing: Arena<super::FlowStateOutgoingFact>,
    pub exit_parameter_origins: Arena<super::FlowExitParameterOrigin>,
    pub states: super::FlowStateTable,
    /// Call rows whose authored call selected execution replaced in the typed
    /// body after checking. The rows stay in `calls` because borrow
    /// certificates and computation roots hold their handles; execution
    /// planning consults this set and skips a retired row.
    pub retired_calls: Vec<super::RetiredFlowCall>,
}

impl FlowControlFacts {
    pub fn with_owned_case_selections<R>(
        &self,
        expression: typed_trees::expression::ExpressionHandle,
        run: impl FnOnce(&[arena::Handle<super::FlowOwnedCaseSelection>]) -> R,
    ) -> R {
        self.owned_cases_by_expression.with_rows(
            &self.owned_case_selections,
            |selection| selection.expression,
            &expression,
            run,
        )
    }

    pub fn with_expression_operands<R>(
        &self,
        expression: typed_trees::expression::ExpressionHandle,
        run: impl FnOnce(&[arena::Handle<super::FlowOperandFact>]) -> R,
    ) -> R {
        self.operands_by_expression.with_rows(
            &self.operands,
            |operand| operand.expression,
            &expression,
            run,
        )
    }

    pub fn seal_indexes(&mut self) {
        self.owned_cases_by_expression
            .seal(&self.owned_case_selections, |selection| {
                selection.expression
            });
        self.operands_by_expression
            .seal(&self.operands, |operand| operand.expression);
    }

    /// Complete captured lifetime dependencies, never exact address evidence.
    pub fn retained_roots(
        &self,
        retention: arena::Handle<super::FlowOperandRetention>,
    ) -> Option<&[facts::PlaceRoot]> {
        if !self.operand_retentions.is_valid(retention) {
            return None;
        }
        let profile = self.operand_retentions.get(retention);
        // A projected snapshot must first resolve its selected value. Its
        // empty local roster is not a proof of absent lifetime dependencies.
        if !profile.stored_projection.is_empty() {
            return None;
        }
        self.operand_retained_roots.span(profile.roots)
    }

    pub fn with_roots(
        statements: Arena<FlowStatementFact>,
        calls: Arena<FlowCallFact>,
        exits: Arena<FlowExitFact>,
        states: Arena<FlowStateFact>,
    ) -> Self {
        Self {
            storage_snapshots: Arena::default(),
            storage_predecessors: Arena::default(),
            statements,
            value_observations: Arena::default(),
            descriptor_context_observations: super::FlowDescriptorObservationTable::default(),
            atomic_executions: Arena::default(),
            case_accesses: Arena::default(),
            owned_case_selections: Arena::default(),
            owned_cases_by_expression: KeyedRows::default(),
            operator_invocations: Arena::default(),
            operands: Arena::default(),
            copied_values: Arena::default(),
            copied_value_uses: Arena::default(),
            captured_reference_leaves: Arena::default(),
            operands_by_expression: KeyedRows::default(),
            operand_children: Arena::default(),
            operand_alternatives: Arena::default(),
            operand_child_paths: Arena::default(),
            operand_origin_writes: Arena::default(),
            operand_referents: Arena::default(),
            referent_formations: Arena::default(),
            operand_referent_segments: Arena::default(),
            operand_retentions: Arena::default(),
            operand_retained_roots: Arena::default(),
            operand_retained_leaves: Arena::default(),
            operand_retained_suppliers: Arena::default(),
            operand_stored_sources: Arena::default(),
            operand_reborrow_sources: Arena::default(),
            output_lifetime_sources: Arena::default(),
            operand_retained_regions: Arena::default(),
            operand_retained_paths: Arena::default(),
            calls,
            call_arguments: Arena::default(),
            call_argument_positions: Arena::default(),
            exits,
            outgoing: Arena::default(),
            exit_parameter_origins: Arena::default(),
            states: states.into(),
            retired_calls: Vec::new(),
        }
    }

    /// The checking owner has already formed this complete source permutation.
    /// An absent span is natural order; an unavailable nonempty span contributes
    /// no argument observations rather than reconstructing source evaluation.
    pub fn call_argument_evaluation_order(&self, call: &FlowCallFact) -> Vec<usize> {
        if call.argument_evaluation_order.is_empty() {
            return (0..call.authored_arguments.len()).collect();
        }
        self.call_argument_positions
            .span_or_empty(call.argument_evaluation_order)
            .iter()
            .map(|position| *position as usize)
            .collect()
    }

    /// The call row of `state` at an authored statement and call ordinal.
    pub fn call_at(
        &self,
        state: SymbolHandle,
        statement_index: usize,
        call_ordinal: usize,
    ) -> Option<&FlowCallFact> {
        let state = self.state_by_symbol(state)?;
        self.calls.span_or_empty(state.calls).iter().find(|call| {
            call.statement_index == statement_index && call.call_ordinal == call_ordinal
        })
    }

    /// The evaluation position of a call among `state`'s rows: the flow walk
    /// appends each row after its evaluated operands, so the position orders
    /// one statement's calls as they execute.
    pub fn call_position(
        &self,
        state: SymbolHandle,
        statement_index: usize,
        call_ordinal: usize,
    ) -> Option<usize> {
        let state = self.state_by_symbol(state)?;
        self.calls
            .span_or_empty(state.calls)
            .iter()
            .position(|call| {
                call.statement_index == statement_index && call.call_ordinal == call_ordinal
            })
    }

    /// Whether `call`, a row of `state`, was retired by selected execution.
    pub fn is_retired(&self, state: SymbolHandle, call: &FlowCallFact) -> bool {
        self.retired_calls.iter().any(|retired| {
            retired.state_symbol == state
                && retired.statement_index == call.statement_index
                && retired.call_ordinal == call.call_ordinal
        })
    }

    /// Retire every call row of `state` at `statement_index` whose authored
    /// expression is `expression`; with `statement_root`, also the
    /// statement-level row (ordinal zero, which captures no expression).
    /// Returns the number of rows newly retired.
    pub fn retire_calls(
        &mut self,
        machine: SymbolHandle,
        state: SymbolHandle,
        statement_index: usize,
        expression: typed_trees::expression::ExpressionHandle,
        statement_root: bool,
    ) -> usize {
        let Some(span) = self.state(machine, state).map(|candidate| candidate.calls) else {
            return 0;
        };
        let mut retired = 0;
        for call in self.calls.span_or_empty(span) {
            if call.statement_index != statement_index {
                continue;
            }
            let matches = if call.authored_expression.is_valid() {
                call.authored_expression == expression
            } else {
                statement_root && call.call_ordinal == 0
            };
            if !matches || self.is_retired(state, call) {
                continue;
            }
            self.retired_calls.push(super::RetiredFlowCall {
                state_symbol: state,
                statement_index: call.statement_index,
                call_ordinal: call.call_ordinal,
            });
            retired += 1;
        }
        retired
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct FlowFacts {
    pub contexts: FlowContextFacts,
    pub invalidations: FlowInvalidationFacts,
    pub borrow_lifetimes: FlowBorrowLifetimeFacts,
    pub ownership: FlowOwnershipFacts,
    pub boundaries: FlowBoundaryFacts,
    pub control: FlowControlFacts,
    /// Exact declaration-level nominal, representation, ownership, and
    /// compiler-selected cleanup dependencies derived from checked value flow.
    /// These rows affect artifact/API identity but grant no source authority.
    pub semantic_dependencies: super::CheckedSemanticDependencies,
    /// Every state's ordered guarded exits and fallback, planned for every
    /// machine. Unit and composed bodies name their arms by span into this
    /// arena, so guard order and destinations have one owner.
    pub guarded_exits: super::CheckedGuardedExitPlans,
    /// Stable machine selection and signature-eligibility rows for terminal
    /// production.
    pub terminal_machines: super::CheckedTerminalMachineSelections,
    /// Optional source presentation retained independently from terminal
    /// semantic and proof plans.
    pub terminal_debug: super::CheckedTerminalDebugPlans,
    /// General source-handle-free structural/Unit effect plans. These are
    /// populated after checked ownership and carry recording succeeds.
    pub terminal_unit_effects: super::CheckedUnitEffectPlans,
    /// Checked-only direct-record-field transfer plus residual Unit-return
    /// cleanup plans. Terminal consumers intentionally ignore this lane until
    /// they gain a path-sensitive ownership frontier.
    pub terminal_partial_affine_unit_cleanups: super::CheckedPartialAffineUnitCleanupPlans,
    /// Exact whole-root affine returns that require one checked empty nominal
    /// cleanup machine. This lane stays distinct from no-code disposal.
    pub terminal_nominal_affine_unit_cleanups: super::CheckedNominalAffineUnitCleanupPlans,
    /// Whole-parameter no-code cleanup rows for supported ordinary structural
    /// transitions. These are populated only after multiplicity checking has
    /// recorded the authoritative state-exit permission events.
    pub terminal_structural_control_cleanups: super::CheckedStructuralControlCleanupPlans,
}

impl FlowFacts {
    pub fn with_roots(
        contexts: FlowContextFacts,
        invalidations: FlowInvalidationFacts,
        borrow_lifetimes: FlowBorrowLifetimeFacts,
        ownership: FlowOwnershipFacts,
        boundaries: FlowBoundaryFacts,
        control: FlowControlFacts,
    ) -> Self {
        Self {
            contexts,
            invalidations,
            borrow_lifetimes,
            ownership,
            boundaries,
            control,
            semantic_dependencies: super::CheckedSemanticDependencies::default(),
            guarded_exits: super::CheckedGuardedExitPlans::default(),
            terminal_machines: super::CheckedTerminalMachineSelections::default(),
            terminal_debug: super::CheckedTerminalDebugPlans::default(),
            terminal_unit_effects: super::CheckedUnitEffectPlans::default(),
            terminal_partial_affine_unit_cleanups:
                super::CheckedPartialAffineUnitCleanupPlans::default(),
            terminal_nominal_affine_unit_cleanups:
                super::CheckedNominalAffineUnitCleanupPlans::default(),
            terminal_structural_control_cleanups:
                super::CheckedStructuralControlCleanupPlans::default(),
        }
    }
}

/// Page partitioning for flow producers that build disjoint machine groups
/// on separate workers: each worker writes every machine on its own page,
/// hands back those pages, and the owner adopts them without remapping a
/// handle. `begin_page` names every field, so a new one must be classified
/// as a paged arena, a derived index rebuilt from revisions, or a carried
/// list before it compiles.
macro_rules! paged_flow_roots {
    (
        $name:ident {
            arenas: [$($arena:ident),* $(,)?],
            indexes: [$($index:ident),* $(,)?],
            lists: [$($list:ident),* $(,)?] $(,)?
        }
    ) => {
        impl $name {
            /// Freeze every arena's pages so a clone shares them; see
            /// `PagedArenaSet::share_pages`.
            pub fn share_pages(&mut self) {
                $(self.$arena.share_pages();)*
            }

            /// Empty every arena, keeping each first page's allocation.
            pub fn reset_pages(&mut self) {
                $(self.$arena.reset_retain_capacity();)*
                $(self.$list.clear();)*
            }

            pub fn begin_page(&mut self, generation: u32) {
                let Self {
                    $($arena,)*
                    $($index: _,)*
                    $($list: _,)*
                } = self;
                $($arena.begin_page(generation);)*
            }

            pub fn take_additional_pages(&mut self) -> Self {
                let mut taken = Self::default();
                $(taken.$arena.adopt_additional_pages(self.$arena.take_additional_pages());)*
                $(taken.$list = std::mem::take(&mut self.$list);)*
                taken
            }

            pub fn adopt_additional_pages(&mut self, mut other: Self) {
                $(self.$arena.adopt_additional_pages(other.$arena.take_additional_pages());)*
                $(self.$list.append(&mut other.$list);)*
            }

            /// The highest page generation any arena holds or has opened. A
            /// fork that writes on pages above it cannot collide with a page
            /// the arenas already hold.
            pub fn latest_page_generation(&self) -> u32 {
                let mut latest = 1;
                $(latest = latest.max(self.$arena.latest_generation());)*
                latest
            }

            /// Hand back every arena's pages from `generation` up, restoring
            /// each arena's append generation to `baseline`'s. A fork of
            /// `baseline` that wrote only on those pages equals `baseline`
            /// again afterwards. Lists have no pages and move whole.
            pub fn take_pages_from(&mut self, generation: u32, baseline: &Self) -> Self {
                let mut taken = Self::default();
                $(taken.$arena.adopt_additional_pages(
                    self.$arena
                        .take_pages_from(generation, baseline.$arena.current_generation()),
                );)*
                $(taken.$list = std::mem::take(&mut self.$list);)*
                taken
            }
        }
    };
}

paged_flow_roots!(FlowContextFacts {
    arenas: [semantic_context_refs, constraint_refs],
    indexes: [],
    lists: [],
});
paged_flow_roots!(FlowInvalidationFacts {
    arenas: [segments, events],
    indexes: [],
    lists: [],
});
paged_flow_roots!(FlowBorrowLifetimeFacts {
    arenas: [activations, weakenings],
    indexes: [],
    lists: [],
});
paged_flow_roots!(FlowOwnershipFacts {
    arenas: [
        instruction_effect_receipts,
        instruction_dispositions,
        segments,
        storage_transfers,
        permissions,
        borrow_claims,
        selected_cleanup_permissions,
        loan_completions,
        claim_outcome_entries,
        claim_outcome_maps,
        claim_join_receipts,
        claim_join_alternatives,
        claim_join_exits,
        claim_join_captures,
        owned_selections,
        selection_sources,
        selection_transfers,
        selection_transfer_claims,
        selection_place_bindings,
        owned_transfer_paths,
        owned_transfer_path_segments,
    ],
    indexes: [
        permissions_by_state,
        owned_selections_by_state,
        selection_place_bindings_by_state,
        // Not an index, but likewise not paged: the guard a worker's copy carries.
        walk_guard,
    ],
    lists: [],
});
paged_flow_roots!(FlowBoundaryFacts {
    arenas: [edges],
    indexes: [],
    lists: [],
});
paged_flow_roots!(FlowControlFacts {
    arenas: [
        storage_snapshots,
        storage_predecessors,
        value_observations,
        descriptor_context_observations,
        atomic_executions,
        case_accesses,
        owned_case_selections,
        operator_invocations,
        operands,
        copied_values,
        copied_value_uses,
        captured_reference_leaves,
        operand_children,
        operand_alternatives,
        operand_child_paths,
        operand_origin_writes,
        operand_referents,
        referent_formations,
        operand_referent_segments,
        operand_retentions,
        operand_retained_roots,
        operand_retained_leaves,
        operand_retained_suppliers,
        operand_stored_sources,
        operand_reborrow_sources,
        output_lifetime_sources,
        operand_retained_regions,
        operand_retained_paths,
        statements,
        calls,
        call_arguments,
        call_argument_positions,
        exits,
        outgoing,
        exit_parameter_origins,
        states,
    ],
    indexes: [operands_by_expression, owned_cases_by_expression],
    lists: [retired_calls],
});
