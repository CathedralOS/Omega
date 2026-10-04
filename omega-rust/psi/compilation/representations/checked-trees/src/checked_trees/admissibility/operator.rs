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
    pub fn verdict(&self) -> AcceptanceVerdict {
        AcceptanceView::verdict(self)
    }

    pub fn is_accepted(&self) -> bool {
        AcceptanceView::is_accepted(self)
    }

    pub fn summary(&self) -> AcceptanceSummary {
        AcceptanceView::summary(self)
    }

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
