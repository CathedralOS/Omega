use crate::{
    AcceptanceSummary, AcceptanceVerdict, AcceptanceView, ContractOperatorUseFact,
    ContractProofFactRef, OperatorAcceptance,
};

impl<'facts> AcceptanceView for OperatorAcceptance<'facts> {
    fn summary(&self) -> AcceptanceSummary {
        AcceptanceSummary::accepted(
            0,
            self.operator_use.requires.len() + self.operator_use.ensures.len(),
            0,
            0,
            0,
            0,
            0,
        )
    }
}

impl<'facts> OperatorAcceptance<'facts> {
    super::acceptance_accessors!();

    pub fn operator_use(&self) -> &'facts ContractOperatorUseFact {
        self.operator_use
    }

    pub fn requires(&self) -> &'facts [ContractProofFactRef] {
        self.facts
            .proof
            .contract_fact_refs
            .span_or_empty(self.operator_use.requires)
    }

    pub fn ensures(&self) -> &'facts [ContractProofFactRef] {
        self.facts
            .proof
            .contract_fact_refs
            .span_or_empty(self.operator_use.ensures)
    }
}
