use crate::{
    AcceptanceSummary, AcceptanceVerdict, AcceptanceView, ContractProofFactRef, ExitAcceptance,
    FlowConstraintRef, FlowExitFact, FlowSemanticContextRef,
    admissibility::evidence_counts::{borrow_constraint_count, constraints, semantic_contexts},
};

impl<'facts> AcceptanceView for ExitAcceptance<'facts> {
    fn summary(&self) -> AcceptanceSummary {
        AcceptanceSummary::accepted(
            borrow_constraint_count(&self.facts.flow, self.exit.entry_constraints)
                + borrow_constraint_count(&self.facts.flow, self.exit.ensures_constraints),
            self.exit.ensures.len(),
            0,
            0,
            0,
            0,
            0,
        )
    }
}

impl<'facts> ExitAcceptance<'facts> {
    super::acceptance_accessors!();

    pub fn exit(&self) -> &'facts FlowExitFact {
        self.exit
    }

    pub fn entry_semantic_contexts(&self) -> &'facts [FlowSemanticContextRef] {
        semantic_contexts(&self.facts.flow, self.exit.entry_semantic_contexts)
    }

    pub fn entry_constraints(&self) -> &'facts [FlowConstraintRef] {
        constraints(&self.facts.flow, self.exit.entry_constraints)
    }

    pub fn ensures_constraints(&self) -> &'facts [FlowConstraintRef] {
        constraints(&self.facts.flow, self.exit.ensures_constraints)
    }

    pub fn ensures(&self) -> &'facts [ContractProofFactRef] {
        self.facts.contract_fact_refs(self.exit.ensures)
    }
}
