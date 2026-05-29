use crate::{
    AcceptanceVerdict, FlowConstraintRef, FlowSemanticContextRef, FlowStatementFact,
    StatementAcceptance,
    admissibility::helpers::{constraints, semantic_contexts},
};

impl<'facts> StatementAcceptance<'facts> {
    pub fn verdict(&self) -> AcceptanceVerdict {
        AcceptanceVerdict::Accepted
    }

    pub fn is_accepted(&self) -> bool {
        self.verdict() == AcceptanceVerdict::Accepted
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
