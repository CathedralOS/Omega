use crate::{
    AcceptanceSummary, AcceptanceVerdict, AcceptanceView, FlowConstraintRef,
    FlowSemanticContextRef, FlowStatementFact, StatementAcceptance,
    admissibility::helpers::{borrow_constraint_count, constraints, semantic_contexts},
};

impl<'facts> AcceptanceView for StatementAcceptance<'facts> {
    fn summary(&self) -> AcceptanceSummary {
        AcceptanceSummary::accepted(
            borrow_constraint_count(&self.facts.flow, self.statement.entry_constraints)
                + self.borrow_compatibility_certificates().count(),
            self.qualification_correspondences().count(),
            0,
            0,
            0,
            0,
            0,
        )
    }
}

impl<'facts> StatementAcceptance<'facts> {
    pub fn verdict(&self) -> AcceptanceVerdict {
        AcceptanceView::verdict(self)
    }

    pub fn is_accepted(&self) -> bool {
        AcceptanceView::is_accepted(self)
    }

    pub fn summary(&self) -> AcceptanceSummary {
        AcceptanceView::summary(self)
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

    /// Already-validated borrow-compatibility certificates formed by this
    /// exact state-owned statement.
    pub fn borrow_compatibility_certificates(
        &self,
    ) -> impl Iterator<Item = &'facts crate::CheckedBorrowCompatibilityCertificate> + '_ {
        self.facts
            .borrow
            .compatibility_certificates
            .iter()
            .filter_map(|(_, certificate)| {
                (certificate.formation.machine_symbol == self.state.machine_symbol
                    && certificate.formation.state_symbol == self.state.state_symbol
                    && certificate.formation.statement_index == self.statement.statement_index)
                    .then_some(certificate)
            })
    }

    /// Already-validated qualification correspondences formed by this exact
    /// state-owned statement. Checked progress remains their sole validator.
    pub fn qualification_correspondences(
        &self,
    ) -> impl Iterator<Item = &'facts psi_facts::QualificationCorrespondence> + '_ {
        self.facts
            .semantic
            .qualification_correspondences
            .iter()
            .filter_map(|(_, correspondence)| {
                matches!(
                    correspondence.formation,
                    psi_facts::ProgramPoint::Statement {
                        machine_symbol,
                        state_symbol,
                        statement_index,
                    } if machine_symbol == self.state.machine_symbol
                        && state_symbol == self.state.state_symbol
                        && statement_index == self.statement.statement_index
                )
                .then_some(correspondence)
            })
    }
}
