use super::*;

pub(super) struct FlowBuildContext {
    pub(super) state_mutation_summary_cache: StateMutationSummaryCache,
    pub(super) semantic_context_refs: omega_core::arena::Arena<FlowSemanticContextRef>,
    pub(super) constraint_refs: omega_core::arena::Arena<FlowConstraintRef>,
    pub(super) invalidation_segments: omega_core::arena::Arena<omega_facts::PlaceSegment>,
    pub(super) invalidations: omega_core::arena::Arena<FlowInvalidationFact>,
    pub(super) borrow_activations: omega_core::arena::Arena<FlowBorrowActivationFact>,
    pub(super) borrow_weakenings: omega_core::arena::Arena<FlowBorrowWeakeningFact>,
    pub(super) statements: omega_core::arena::Arena<FlowStatementFact>,
    pub(super) calls: omega_core::arena::Arena<FlowCallFact>,
    pub(super) exits: omega_core::arena::Arena<FlowExitFact>,
    pub(super) states: omega_core::arena::Arena<FlowStateFact>,
}

impl FlowBuildContext {
    pub(super) fn new(borrow: &BorrowFacts, proof: &ProofFacts, semantic: &FactPlan) -> Self {
        Self {
            state_mutation_summary_cache: StateMutationSummaryCache::default(),
            semantic_context_refs: omega_core::arena::Arena::with_capacity(
                semantic.contexts.len().saturating_mul(2),
            ),
            constraint_refs: omega_core::arena::Arena::with_capacity(
                semantic
                    .contexts
                    .len()
                    .saturating_mul(3)
                    .saturating_add(borrow.states.len())
                    .saturating_add(borrow.calls.len())
                    .saturating_add(borrow.loans.len()),
            ),
            invalidation_segments: omega_core::arena::Arena::default(),
            invalidations: omega_core::arena::Arena::default(),
            borrow_activations: omega_core::arena::Arena::default(),
            borrow_weakenings: omega_core::arena::Arena::default(),
            statements: omega_core::arena::Arena::with_capacity(borrow.calls.len()),
            calls: omega_core::arena::Arena::with_capacity(borrow.calls.len()),
            exits: omega_core::arena::Arena::with_capacity(proof.contract_exits.len()),
            states: omega_core::arena::Arena::with_capacity(borrow.states.len()),
        }
    }

    pub(super) fn finish(self) -> FlowFacts {
        FlowFacts {
            semantic_context_refs: self.semantic_context_refs,
            constraint_refs: self.constraint_refs,
            invalidation_segments: self.invalidation_segments,
            invalidations: self.invalidations,
            borrow_activations: self.borrow_activations,
            borrow_weakenings: self.borrow_weakenings,
            statements: self.statements,
            calls: self.calls,
            exits: self.exits,
            states: self.states,
        }
    }
}
