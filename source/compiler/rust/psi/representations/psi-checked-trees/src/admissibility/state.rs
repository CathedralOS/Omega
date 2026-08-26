mod evidence;

use psi_symbols::SymbolHandle;

use crate::{
    AcceptanceSummary, AcceptanceVerdict, AcceptanceView, CallAcceptance, CheckedTrees,
    ContractOperatorUseFact, ExitAcceptance, FlowCallFact, FlowExitFact, FlowStateFact,
    FlowStatementFact, OperatorAcceptance, StateAcceptance, StateOperationAcceptance,
    StatementAcceptance,
    admissibility::helpers::{
        blocking_evidence_count, machine_decrease_count, service_reach_evidence_count,
        suspension_evidence_count,
    },
};

use evidence::{
    state_borrow_evidence_count, state_boundary_evidence_count, state_proof_evidence_count,
};

impl CheckedTrees {
    pub fn state_acceptance(
        &self,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
    ) -> Option<StateAcceptance<'_>> {
        self.facts
            .flow
            .control
            .states
            .iter()
            .map(|(_, state)| state)
            .find(|state| {
                state.machine_symbol == machine_symbol && state.state_symbol == state_symbol
            })
            .map(|state| StateAcceptance {
                facts: &self.facts,
                state,
            })
    }
}

impl<'facts> AcceptanceView for StateAcceptance<'facts> {
    fn summary(&self) -> AcceptanceSummary {
        let statements = self.statements();
        let calls = self.calls();
        let exits = self.exits();
        let operator_proof_evidence = self
            .operator_uses()
            .map(|operator_use| operator_use.requires.len() + operator_use.ensures.len())
            .sum::<usize>();

        AcceptanceSummary::accepted(
            state_borrow_evidence_count(&self.facts.flow, self.state, statements, calls, exits)
                + self.borrow_compatibility_certificates().count(),
            state_proof_evidence_count(calls, exits) + operator_proof_evidence,
            service_reach_evidence_count(self.facts, self.state.service_reach),
            suspension_evidence_count(self.state.suspension),
            blocking_evidence_count(self.state.blocking),
            state_boundary_evidence_count(self.state, calls),
            machine_decrease_count(self.facts, self.state.machine_symbol),
        )
    }
}

impl<'facts> StateAcceptance<'facts> {
    pub fn verdict(&self) -> AcceptanceVerdict {
        AcceptanceView::verdict(self)
    }

    pub fn is_accepted(&self) -> bool {
        AcceptanceView::is_accepted(self)
    }

    pub fn summary(&self) -> AcceptanceSummary {
        AcceptanceView::summary(self)
    }

    pub fn state(&self) -> &'facts FlowStateFact {
        self.state
    }

    pub fn service_reach(&self) -> psi_language_semantics::ServiceReachSummary {
        self.state.service_reach
    }

    pub fn suspension(&self) -> psi_language_semantics::SuspensionSummary {
        self.state.suspension
    }

    pub fn blocking(&self) -> psi_language_semantics::BlockingSummary {
        self.state.blocking
    }

    pub fn statements(&self) -> &'facts [FlowStatementFact] {
        self.facts
            .flow
            .control
            .statements
            .span_or_empty(self.state.statements)
    }

    /// Already-validated borrow-compatibility certificates formed in this
    /// exact state. The checked borrow pass remains their sole validator; this
    /// acceptance view only publishes its retained evidence.
    pub fn borrow_compatibility_certificates(
        &self,
    ) -> impl Iterator<Item = &'facts crate::CheckedBorrowCompatibilityCertificate> + '_ {
        self.facts
            .borrow
            .compatibility_certificates
            .iter()
            .filter_map(|(_, certificate)| {
                (certificate.formation.machine_symbol == self.state.machine_symbol
                    && certificate.formation.state_symbol == self.state.state_symbol)
                    .then_some(certificate)
            })
    }

    pub fn calls(&self) -> &'facts [FlowCallFact] {
        self.facts
            .flow
            .control
            .calls
            .span_or_empty(self.state.calls)
    }

    pub fn exits(&self) -> &'facts [FlowExitFact] {
        self.facts
            .flow
            .control
            .exits
            .span_or_empty(self.state.exits)
    }

    pub fn operator_uses(&self) -> impl Iterator<Item = &'facts ContractOperatorUseFact> + '_ {
        self.facts
            .proof
            .contract_operator_uses
            .iter()
            .filter_map(|(_, operator_use)| {
                matches!(
                    operator_use.origin,
                    crate::CheckedValueOrigin::StateStatement {
                        machine_symbol,
                        state_symbol,
                        ..
                    } if machine_symbol == self.state.machine_symbol
                        && state_symbol == self.state.state_symbol
                )
                .then_some(operator_use)
            })
    }

    pub fn statement(&self, statement_index: usize) -> Option<StatementAcceptance<'facts>> {
        self.facts
            .flow
            .state_statement(self.state, statement_index)
            .map(|statement| StatementAcceptance {
                facts: self.facts,
                state: self.state,
                statement,
            })
    }

    pub fn call(
        &self,
        statement_index: usize,
        call_ordinal: usize,
        target_symbol: SymbolHandle,
        receiver_symbol: SymbolHandle,
    ) -> Option<CallAcceptance<'facts>> {
        self.facts
            .flow
            .state_call(
                self.state,
                statement_index,
                call_ordinal,
                target_symbol,
                receiver_symbol,
            )
            .map(|call| CallAcceptance {
                facts: self.facts,
                call,
            })
    }

    pub fn exit(&self, statement_index: usize) -> Option<ExitAcceptance<'facts>> {
        self.exits()
            .iter()
            .find(|exit| exit.statement_index == statement_index)
            .map(|exit| ExitAcceptance {
                facts: self.facts,
                exit,
            })
    }

    pub fn operations(&self) -> impl Iterator<Item = StateOperationAcceptance<'facts>> + '_ {
        let facts = self.facts;
        let state = self.state;
        let statements = self.statements().iter().map(move |statement| {
            StateOperationAcceptance::Statement(StatementAcceptance {
                facts,
                state,
                statement,
            })
        });
        let calls = self
            .calls()
            .iter()
            .map(move |call| StateOperationAcceptance::Call(CallAcceptance { facts, call }));
        let exits = self
            .exits()
            .iter()
            .map(move |exit| StateOperationAcceptance::Exit(ExitAcceptance { facts, exit }));
        let operator_uses = self.operator_uses().map(move |operator_use| {
            StateOperationAcceptance::Operator(OperatorAcceptance {
                facts,
                operator_use,
            })
        });

        statements.chain(calls).chain(exits).chain(operator_uses)
    }
}
