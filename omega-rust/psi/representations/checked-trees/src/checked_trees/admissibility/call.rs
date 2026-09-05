use arena::HandleSpan;

use crate::{
    AcceptanceSummary, AcceptanceVerdict, AcceptanceView, BorrowArgumentAccessFact, CallAcceptance,
    ContractProofFactRef, FlowBoundaryEdgeFact, FlowCallFact, FlowConstraintRef,
    FlowInvalidationFact, FlowSemanticContextRef,
    admissibility::helpers::{
        blocking_evidence_count, borrow_constraint_count, constraints, semantic_contexts,
        service_reach_evidence_count, suspension_evidence_count,
    },
};

impl<'facts> AcceptanceView for CallAcceptance<'facts> {
    fn summary(&self) -> AcceptanceSummary {
        AcceptanceSummary::accepted(
            self.call.accesses.len()
                + borrow_constraint_count(&self.facts.flow, self.call.entry_constraints)
                + borrow_constraint_count(&self.facts.flow, self.call.requires_constraints)
                + borrow_constraint_count(&self.facts.flow, self.call.exit_constraints),
            self.call.requires.len() + self.call.ensures.len(),
            service_reach_evidence_count(self.facts, self.call.service_reach),
            suspension_evidence_count(self.call.suspension),
            blocking_evidence_count(self.call.blocking),
            self.call.boundary_edges.len(),
            0,
        )
    }
}

impl<'facts> CallAcceptance<'facts> {
    pub fn verdict(&self) -> AcceptanceVerdict {
        AcceptanceView::verdict(self)
    }

    pub fn is_accepted(&self) -> bool {
        AcceptanceView::is_accepted(self)
    }

    pub fn summary(&self) -> AcceptanceSummary {
        AcceptanceView::summary(self)
    }

    pub fn call(&self) -> &'facts FlowCallFact {
        self.call
    }

    pub fn borrow_accesses(&self) -> HandleSpan<BorrowArgumentAccessFact> {
        self.call.accesses
    }

    pub fn entry_semantic_contexts(&self) -> &'facts [FlowSemanticContextRef] {
        semantic_contexts(&self.facts.flow, self.call.entry_semantic_contexts)
    }

    pub fn entry_constraints(&self) -> &'facts [FlowConstraintRef] {
        constraints(&self.facts.flow, self.call.entry_constraints)
    }

    pub fn requires_semantic_contexts(&self) -> &'facts [FlowSemanticContextRef] {
        semantic_contexts(&self.facts.flow, self.call.requires_contexts)
    }

    pub fn requires_constraints(&self) -> &'facts [FlowConstraintRef] {
        constraints(&self.facts.flow, self.call.requires_constraints)
    }

    pub fn exit_semantic_contexts(&self) -> &'facts [FlowSemanticContextRef] {
        semantic_contexts(&self.facts.flow, self.call.exit_semantic_contexts)
    }

    pub fn exit_constraints(&self) -> &'facts [FlowConstraintRef] {
        constraints(&self.facts.flow, self.call.exit_constraints)
    }

    pub fn invalidations(&self) -> &'facts [FlowInvalidationFact] {
        self.facts
            .flow
            .invalidations
            .events
            .span_or_empty(self.call.invalidations)
    }

    pub fn boundary_edges(&self) -> &'facts [FlowBoundaryEdgeFact] {
        self.facts
            .flow
            .boundaries
            .edges
            .span_or_empty(self.call.boundary_edges)
    }

    pub fn requires(&self) -> &'facts [ContractProofFactRef] {
        self.facts
            .proof
            .contract_fact_refs
            .span_or_empty(self.call.requires)
    }

    pub fn ensures(&self) -> &'facts [ContractProofFactRef] {
        self.facts
            .proof
            .contract_fact_refs
            .span_or_empty(self.call.ensures)
    }

    pub fn service_reach(&self) -> language_semantics::ServiceReachSummary {
        self.call.service_reach
    }

    pub fn suspension(&self) -> language_semantics::SuspensionSummary {
        self.call.suspension
    }

    pub fn blocking(&self) -> language_semantics::BlockingSummary {
        self.call.blocking
    }
}
