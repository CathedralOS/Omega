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
            contexts: FlowContextFacts {
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
            },
            invalidations: FlowInvalidationFacts {
                segments: omega_core::arena::Arena::default(),
                events: omega_core::arena::Arena::default(),
            },
            borrow_lifetimes: FlowBorrowLifetimeFacts {
                activations: omega_core::arena::Arena::default(),
                weakenings: omega_core::arena::Arena::default(),
            },
            ownership: FlowOwnershipFacts {
                segments: omega_core::arena::Arena::default(),
                moves: omega_core::arena::Arena::default(),
                drops: omega_core::arena::Arena::default(),
            },
            boundaries: FlowBoundaryFacts {
                edges: omega_core::arena::Arena::with_capacity(borrow.calls.len()),
            },
            control: FlowControlFacts {
                statements: omega_core::arena::Arena::with_capacity(borrow.calls.len()),
                calls: omega_core::arena::Arena::with_capacity(borrow.calls.len()),
                exits: omega_core::arena::Arena::with_capacity(proof.contract_exits.len()),
                states: omega_core::arena::Arena::with_capacity(borrow.states.len()),
            },
        }
    }

    pub(super) fn finish(self) -> FlowFacts {
        FlowFacts {
            contexts: self.contexts,
            invalidations: self.invalidations,
            borrow_lifetimes: self.borrow_lifetimes,
            ownership: self.ownership,
            boundaries: self.boundaries,
            control: self.control,
        }
    }
}
