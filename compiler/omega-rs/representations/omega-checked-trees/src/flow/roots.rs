use omega_core::arena::Arena;

use super::{
    FlowBorrowActivationFact, FlowBorrowWeakeningFact, FlowBoundaryEdgeFact, FlowCallFact,
    FlowConstraintRef, FlowDropEventFact, FlowExitFact, FlowInvalidationFact, FlowMoveEventFact,
    FlowSemanticContextRef, FlowStateFact, FlowStatementFact,
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
    pub segments: Arena<omega_facts::PlaceSegment>,
    pub events: Arena<FlowInvalidationFact>,
}

impl FlowInvalidationFacts {
    pub fn with_roots(
        segments: Arena<omega_facts::PlaceSegment>,
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
    pub segments: Arena<omega_facts::PlaceSegment>,
    pub moves: Arena<FlowMoveEventFact>,
    pub drops: Arena<FlowDropEventFact>,
}

impl FlowOwnershipFacts {
    pub fn with_roots(
        segments: Arena<omega_facts::PlaceSegment>,
        moves: Arena<FlowMoveEventFact>,
        drops: Arena<FlowDropEventFact>,
    ) -> Self {
        Self {
            segments,
            moves,
            drops,
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
