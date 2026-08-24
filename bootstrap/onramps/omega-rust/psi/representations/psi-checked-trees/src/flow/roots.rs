use psi_arena::Arena;

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
    pub segments: Arena<psi_facts::PlaceSegment>,
    pub events: Arena<FlowInvalidationFact>,
}

impl FlowInvalidationFacts {
    pub fn with_roots(
        segments: Arena<psi_facts::PlaceSegment>,
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
    pub segments: Arena<psi_facts::PlaceSegment>,
    pub permissions: Arena<FlowPermissionEventFact>,
    pub claim_outcome_entries: Arena<FlowClaimOutcomeEntryFact>,
    pub claim_outcome_maps: Arena<FlowClaimOutcomeMapFact>,
}

impl FlowOwnershipFacts {
    pub fn with_roots(
        segments: Arena<psi_facts::PlaceSegment>,
        permissions: Arena<FlowPermissionEventFact>,
        claim_outcome_entries: Arena<FlowClaimOutcomeEntryFact>,
        claim_outcome_maps: Arena<FlowClaimOutcomeMapFact>,
    ) -> Self {
        Self {
            segments,
            permissions,
            claim_outcome_entries,
            claim_outcome_maps,
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
    pub statements: Arena<FlowStatementFact>,
    pub calls: Arena<FlowCallFact>,
    pub exits: Arena<FlowExitFact>,
    pub states: Arena<FlowStateFact>,
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
            calls,
            exits,
            states,
        }
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
    /// Source-handle-free control topology for the live terminal-Psi scalar
    /// producer. General terminal control will replace this bootstrap carrier.
    pub terminal_scalar_graphs: super::CheckedScalarGraphPlans,
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
    /// Complete checked input for the first claim-free affine structural Unit
    /// jump graph accepted by terminal production.
    pub terminal_structural_unit_controls: super::CheckedStructuralUnitControlPlans,
    /// Exact attached scalar-return plan for a claim-free affine structural
    /// entry frontier.
    pub terminal_structural_scalar_returns: super::CheckedStructuralScalarReturnPlans,
    /// One result-bearing bodyless boundary call whose successful completion
    /// consumes the exact structural claim frontier.
    pub terminal_boundary_scalar_returns: super::CheckedBoundaryScalarReturnPlans,
    /// Exact one-parameter whole-root structural result transfers.
    pub terminal_structural_returns: super::CheckedStructuralReturnPlans,
    /// Final direct internal calls whose exact whole-root structural result is
    /// returned immediately by the caller.
    pub terminal_structural_call_returns: super::CheckedStructuralCallReturnPlans,
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
            terminal_scalar_graphs: super::CheckedScalarGraphPlans::default(),
            terminal_machines: super::CheckedTerminalMachineSelections::default(),
            terminal_debug: super::CheckedTerminalDebugPlans::default(),
            terminal_unit_effects: super::CheckedUnitEffectPlans::default(),
            terminal_partial_affine_unit_cleanups:
                super::CheckedPartialAffineUnitCleanupPlans::default(),
            terminal_nominal_affine_unit_cleanups:
                super::CheckedNominalAffineUnitCleanupPlans::default(),
            terminal_structural_control_cleanups:
                super::CheckedStructuralControlCleanupPlans::default(),
            terminal_structural_unit_controls: super::CheckedStructuralUnitControlPlans::default(),
            terminal_structural_scalar_returns: super::CheckedStructuralScalarReturnPlans::default(
            ),
            terminal_boundary_scalar_returns: super::CheckedBoundaryScalarReturnPlans::default(),
            terminal_structural_returns: super::CheckedStructuralReturnPlans::default(),
            terminal_structural_call_returns: super::CheckedStructuralCallReturnPlans::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        FlowBorrowLifetimeFacts, FlowBoundaryFacts, FlowContextFacts, FlowControlFacts, FlowFacts,
        FlowInvalidationFacts, FlowOwnershipFacts,
    };

    #[test]
    fn flow_facts_constructor_keeps_flow_roots_explicit() {
        let contexts = FlowContextFacts::default();
        let invalidations = FlowInvalidationFacts::default();
        let borrow_lifetimes = FlowBorrowLifetimeFacts::default();
        let ownership = FlowOwnershipFacts::default();
        let boundaries = FlowBoundaryFacts::default();
        let control = FlowControlFacts::default();

        let facts = FlowFacts::with_roots(
            contexts.clone(),
            invalidations.clone(),
            borrow_lifetimes.clone(),
            ownership.clone(),
            boundaries.clone(),
            control.clone(),
        );

        assert_eq!(facts.contexts, contexts);
        assert_eq!(facts.invalidations, invalidations);
        assert_eq!(facts.borrow_lifetimes, borrow_lifetimes);
        assert_eq!(facts.ownership, ownership);
        assert_eq!(facts.boundaries, boundaries);
        assert_eq!(facts.control, control);
    }
}
