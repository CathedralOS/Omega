use omega_core::arena::HandleSpan;

use crate::{
    AcceptanceSummary, AcceptanceVerdict, BorrowArgumentAccessFact, CallAcceptance,
    ContractProofFactRef, FlowBoundaryEdgeFact, FlowCallFact, FlowConstraintRef,
    FlowInvalidationFact, FlowSemanticContextRef,
    admissibility::helpers::{
        borrow_constraint_count, constraints, effect_evidence_count, semantic_contexts,
    },
};

impl<'facts> CallAcceptance<'facts> {
    pub fn verdict(&self) -> AcceptanceVerdict {
        self.summary().verdict
    }

    pub fn is_accepted(&self) -> bool {
        self.summary().is_accepted()
    }

    pub fn summary(&self) -> AcceptanceSummary {
        AcceptanceSummary::accepted(
            self.call.accesses.len()
                + borrow_constraint_count(&self.facts.flow, self.call.entry_constraints)
                + borrow_constraint_count(&self.facts.flow, self.call.requires_constraints)
                + borrow_constraint_count(&self.facts.flow, self.call.exit_constraints),
            self.call.requires.len() + self.call.ensures.len(),
            effect_evidence_count(self.call.transitive_effects),
            self.call.boundary_edges.len(),
            0,
        )
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

    pub fn direct_effects(&self) -> omega_effects::EffectSet {
        self.call.direct_effects
    }

    pub fn transitive_effects(&self) -> omega_effects::EffectSet {
        self.call.transitive_effects
    }
}
