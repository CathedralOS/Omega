use super::*;

pub(super) struct FlowBuildContext {
    pub(super) state_mutation_summary_cache: StateMutationSummaryCache,
    pub(super) semantic_context_refs: omega_core::arena::Arena<FlowSemanticContextRef>,
    pub(super) constraint_refs: omega_core::arena::Arena<FlowConstraintRef>,
    pub(super) invalidation_segments: omega_core::arena::Arena<omega_facts::PlaceSegment>,
    pub(super) ownership_segments: omega_core::arena::Arena<omega_facts::PlaceSegment>,
    pub(super) invalidations: omega_core::arena::Arena<FlowInvalidationFact>,
    pub(super) borrow_activations: omega_core::arena::Arena<FlowBorrowActivationFact>,
    pub(super) borrow_weakenings: omega_core::arena::Arena<FlowBorrowWeakeningFact>,
    pub(super) moves: omega_core::arena::Arena<FlowMoveEventFact>,
    pub(super) drops: omega_core::arena::Arena<FlowDropEventFact>,
    pub(super) boundary_edges: omega_core::arena::Arena<FlowBoundaryEdgeFact>,
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
            ownership_segments: omega_core::arena::Arena::default(),
            invalidations: omega_core::arena::Arena::default(),
            borrow_activations: omega_core::arena::Arena::default(),
            borrow_weakenings: omega_core::arena::Arena::default(),
            moves: omega_core::arena::Arena::default(),
            drops: omega_core::arena::Arena::default(),
            boundary_edges: omega_core::arena::Arena::with_capacity(borrow.calls.len()),
            statements: omega_core::arena::Arena::with_capacity(borrow.calls.len()),
            calls: omega_core::arena::Arena::with_capacity(borrow.calls.len()),
            exits: omega_core::arena::Arena::with_capacity(proof.contract_exits.len()),
            states: omega_core::arena::Arena::with_capacity(borrow.states.len()),
        }
    }

    pub(super) fn finish(self) -> FlowFacts {
        FlowFacts {
            contexts: FlowContextFacts {
                semantic_context_refs: self.semantic_context_refs,
                constraint_refs: self.constraint_refs,
            },
            invalidations: FlowInvalidationFacts {
                segments: self.invalidation_segments,
                events: self.invalidations,
            },
            borrow_lifetimes: FlowBorrowLifetimeFacts {
                activations: self.borrow_activations,
                weakenings: self.borrow_weakenings,
            },
            ownership: FlowOwnershipFacts {
                segments: self.ownership_segments,
                moves: self.moves,
                drops: self.drops,
            },
            boundaries: FlowBoundaryFacts {
                edges: self.boundary_edges,
            },
            control: FlowControlFacts {
                statements: self.statements,
                calls: self.calls,
                exits: self.exits,
                states: self.states,
            },
        }
    }
}
