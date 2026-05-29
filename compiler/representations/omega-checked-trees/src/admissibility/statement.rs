use crate::{
    AcceptanceSummary, AcceptanceVerdict, FlowConstraintRef, FlowSemanticContextRef,
    FlowStatementFact, StatementAcceptance,
    admissibility::helpers::{borrow_constraint_count, constraints, semantic_contexts},
};

impl<'facts> StatementAcceptance<'facts> {
    pub fn verdict(&self) -> AcceptanceVerdict {
        self.summary().verdict
    }

    pub fn is_accepted(&self) -> bool {
        self.summary().is_accepted()
    }

    pub fn summary(&self) -> AcceptanceSummary {
        AcceptanceSummary::accepted(
            borrow_constraint_count(&self.facts.flow, self.statement.entry_constraints),
            0,
            0,
            0,
            0,
        )
    }

    pub fn statement(&self) -> &'facts FlowStatementFact {
        self.statement
    }

    pub fn entry_semantic_contexts(&self) -> &'facts [FlowSemanticContextRef] {
        semantic_contexts(&self.facts.flow, self.statement.entry_semantic_contexts)
    }

    pub fn entry_constraints(&self) -> &'facts [FlowConstraintRef] {
        constraints(&self.facts.flow, self.statement.entry_constraints)
    }
}
