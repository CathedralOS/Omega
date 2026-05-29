use crate::{
    AcceptanceVerdict, ContractProofFactRef, ExitAcceptance, FlowConstraintRef, FlowExitFact,
    FlowSemanticContextRef,
    admissibility::helpers::{constraints, semantic_contexts},
};

impl<'facts> ExitAcceptance<'facts> {
    pub fn verdict(&self) -> AcceptanceVerdict {
        AcceptanceVerdict::Accepted
    }

    pub fn is_accepted(&self) -> bool {
        self.verdict() == AcceptanceVerdict::Accepted
    }

    pub fn exit(&self) -> &'facts FlowExitFact {
        self.exit
    }

    pub fn entry_semantic_contexts(&self) -> &'facts [FlowSemanticContextRef] {
        semantic_contexts(&self.facts.flow, self.exit.entry_semantic_contexts)
    }

    pub fn entry_constraints(&self) -> &'facts [FlowConstraintRef] {
        constraints(&self.facts.flow, self.exit.entry_constraints)
    }

    pub fn ensures_semantic_contexts(&self) -> &'facts [FlowSemanticContextRef] {
        semantic_contexts(&self.facts.flow, self.exit.ensures_contexts)
    }

    pub fn ensures_constraints(&self) -> &'facts [FlowConstraintRef] {
        constraints(&self.facts.flow, self.exit.ensures_constraints)
    }

    pub fn ensures(&self) -> &'facts [ContractProofFactRef] {
        self.facts
            .proof
            .contract_fact_refs
            .span_or_empty(self.exit.ensures)
    }
}
