use arena::Arena;
use symbols::SymbolHandle;

use super::{
    FlowBorrowActivationFact, FlowBorrowWeakeningFact, FlowBoundaryEdgeFact, FlowCallFact,
    FlowClaimOutcomeEntryFact, FlowClaimOutcomeMapFact, FlowConstraintRef, FlowExitFact,
    FlowInvalidationFact, FlowPermissionEventFact, FlowSemanticContextRef, FlowStateFact,
    FlowStatementFact,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowOwnershipFacts {
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
    pub owned_selections: Arena<super::FlowOwnedSelectionReceipt>,
    pub selection_sources: Arena<super::FlowOwnedSelectionSource>,
    pub selection_transfers: Arena<super::FlowOwnedSelectionTransfer>,
    pub selection_transfer_claims: Arena<super::FlowOwnedSelectionClaim>,
}

impl FlowOwnershipFacts {
    pub fn with_roots(
        segments: Arena<facts::PlaceSegment>,
        permissions: Arena<FlowPermissionEventFact>,
        claim_outcome_entries: Arena<FlowClaimOutcomeEntryFact>,
        claim_outcome_maps: Arena<FlowClaimOutcomeMapFact>,
    ) -> Self {
        Self {
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
            owned_selections: Arena::default(),
            selection_sources: Arena::default(),
            selection_transfers: Arena::default(),
            selection_transfer_claims: Arena::default(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowBoundaryFacts {
    pub edges: Arena<FlowBoundaryEdgeFact>,
}

impl FlowBoundaryFacts {
    pub fn with_roots(edges: Arena<FlowBoundaryEdgeFact>) -> Self {
        Self { edges }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowControlFacts {
    pub case_accesses: Arena<super::FlowCaseAccessFact>,
    pub operator_invocations: Arena<super::FlowOperatorInvocationFact>,
    pub operands: Arena<super::FlowOperandFact>,
    pub operand_children: Arena<super::FlowOperandChild>,
    pub operand_child_paths: Arena<facts::PlaceSegment>,
    /// Exact storage an operand's evaluated view carrier denotes, one row per
    /// resolvable referent; segment payloads live in `operand_referent_segments`
    /// in the semantic `Place` vocabulary.
    pub operand_origin_writes: Arena<super::FlowOperandReferent>,
    pub operand_referents: Arena<super::FlowOperandReferent>,
    pub operand_referent_segments: Arena<facts::PlaceSegment>,
    pub statements: Arena<FlowStatementFact>,
    pub calls: Arena<FlowCallFact>,
    /// Authored argument expressions, spanned by [`FlowCallFact::authored_arguments`].
    pub call_arguments: Arena<typed_trees::expression::ExpressionHandle>,
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
    pub fn with_roots(
        statements: Arena<FlowStatementFact>,
        calls: Arena<FlowCallFact>,
        exits: Arena<FlowExitFact>,
        states: Arena<FlowStateFact>,
    ) -> Self {
        Self {
            statements,
            case_accesses: Arena::default(),
            operator_invocations: Arena::default(),
            operands: Arena::default(),
            operand_children: Arena::default(),
            operand_child_paths: Arena::default(),
            operand_origin_writes: Arena::default(),
            operand_referents: Arena::default(),
            operand_referent_segments: Arena::default(),
            calls,
            call_arguments: Arena::default(),
            exits,
            outgoing: Arena::default(),
            exit_parameter_origins: Arena::default(),
            states: states.into(),
            retired_calls: Vec::new(),
        }
    }

    /// The call row of `state` at an authored statement and call ordinal.
    pub fn call_at(
        &self,
        state: SymbolHandle,
        statement_index: usize,
        call_ordinal: usize,
    ) -> Option<&FlowCallFact> {
        let (_, state) = self
            .states
            .iter()
            .find(|(_, candidate)| candidate.state_symbol == state)?;
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
        let (_, state) = self
            .states
            .iter()
            .find(|(_, candidate)| candidate.state_symbol == state)?;
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
        let Some(span) = self.states.iter().find_map(|(_, candidate)| {
            (candidate.machine_symbol == machine && candidate.state_symbol == state)
                .then_some(candidate.calls)
        }) else {
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
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
