use super::*;

pub(super) struct FlowBuildContext {
    pub(super) state_mutation_summary_cache: StateMutationSummaryCache,
    pub(super) contexts: FlowContextFacts,
    pub(super) invalidations: FlowInvalidationFacts,
    pub(super) borrow_lifetimes: FlowBorrowLifetimeFacts,
    pub(super) ownership: FlowOwnershipFacts,
    pub(super) boundaries: FlowBoundaryFacts,
    pub(super) control: FlowControlFacts,
}

impl FlowBuildContext {
    pub(super) fn new(borrow: &BorrowFacts, proof: &ProofFacts, semantic: &FactPlan) -> Self {
        Self {
            state_mutation_summary_cache: StateMutationSummaryCache::default(),
            contexts: FlowContextFacts::with_roots(
                psi_arena::Arena::with_capacity(semantic.contexts.len().saturating_mul(2)),
                psi_arena::Arena::with_capacity(
                    semantic
                        .contexts
                        .len()
                        .saturating_mul(3)
                        .saturating_add(borrow.states.len())
                        .saturating_add(borrow.calls.len())
                        .saturating_add(borrow.loans.len()),
                ),
            ),
            invalidations: FlowInvalidationFacts::with_roots(
                psi_arena::Arena::default(),
                psi_arena::Arena::default(),
            ),
            borrow_lifetimes: FlowBorrowLifetimeFacts::with_roots(
                psi_arena::Arena::default(),
                psi_arena::Arena::default(),
            ),
            ownership: FlowOwnershipFacts::with_roots(
                psi_arena::Arena::default(),
                psi_arena::Arena::default(),
                psi_arena::Arena::default(),
                psi_arena::Arena::default(),
            ),
            boundaries: FlowBoundaryFacts::with_roots(psi_arena::Arena::with_capacity(
                borrow.calls.len(),
            )),
            control: FlowControlFacts::with_roots(
                psi_arena::Arena::with_capacity(borrow.calls.len()),
                psi_arena::Arena::with_capacity(borrow.calls.len()),
                psi_arena::Arena::with_capacity(proof.contract_exits.len()),
                psi_arena::Arena::with_capacity(borrow.states.len()),
            ),
        }
    }

    pub(super) fn finish(self) -> FlowFacts {
        FlowFacts::with_roots(
            self.contexts,
            self.invalidations,
            self.borrow_lifetimes,
            self.ownership,
            self.boundaries,
            self.control,
        )
    }
}
