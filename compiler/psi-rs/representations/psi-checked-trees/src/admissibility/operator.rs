use crate::{
    AcceptanceSummary, AcceptanceVerdict, AcceptanceView, ContractOperatorUseFact,
    ContractProofFactRef, OperatorAcceptance,
};

impl<'facts> AcceptanceView for OperatorAcceptance<'facts> {
    fn summary(&self) -> AcceptanceSummary {
        AcceptanceSummary::accepted(
            0,
            self.operator_use.requires.len()
                + self.operator_use.ensures.len()
                + self.operator_use.boundary.len(),
            0,
            0,
            0,
            self.operator_use.boundary.len(),
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

    pub fn boundary(&self) -> &'facts [ContractProofFactRef] {
        self.facts
            .proof
            .contract_fact_refs
            .span_or_empty(self.operator_use.boundary)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use psi_arena::HandleSpan;

    #[test]
    fn operator_acceptance_counts_contract_evidence() {
        let facts = crate::CheckFacts::default();
        let operator_use = ContractOperatorUseFact {
            requires: HandleSpan::from_parts(psi_arena::Handle::from_arena_index(1), 2),
            ensures: HandleSpan::from_parts(psi_arena::Handle::from_arena_index(3), 1),
            boundary: HandleSpan::from_parts(psi_arena::Handle::from_arena_index(4), 1),
            ..Default::default()
        };
        let acceptance = OperatorAcceptance {
            facts: &facts,
            operator_use: &operator_use,
        };

        let summary = acceptance.summary();

        assert!(summary.is_accepted());
        assert_eq!(summary.proof.evidence_count, 4);
        assert_eq!(summary.boundaries.evidence_count, 1);
    }
}
