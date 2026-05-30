mod evidence;

use omega_core::symbols::SymbolHandle;

use crate::{
    AcceptanceSummary, AcceptanceVerdict, AcceptanceView, CallAcceptance, CheckedTrees,
    ExitAcceptance, FlowCallFact, FlowExitFact, FlowStateFact, FlowStatementFact, StateAcceptance,
    StatementAcceptance,
    admissibility::helpers::{effect_evidence_count, machine_decrease_count},
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

        AcceptanceSummary::accepted(
            state_borrow_evidence_count(&self.facts.flow, self.state, statements, calls, exits),
            state_proof_evidence_count(calls, exits),
            effect_evidence_count(self.state.transitive_effects),
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

    pub fn direct_effects(&self) -> omega_effects::EffectSet {
        self.state.direct_effects
    }

    pub fn transitive_effects(&self) -> omega_effects::EffectSet {
        self.state.transitive_effects
    }

    pub fn statements(&self) -> &'facts [FlowStatementFact] {
        self.facts
            .flow
            .control
            .statements
            .span_or_empty(self.state.statements)
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

    pub fn statement(&self, statement_index: usize) -> Option<StatementAcceptance<'facts>> {
        self.facts
            .flow
            .state_statement(self.state, statement_index)
            .map(|statement| StatementAcceptance {
                facts: self.facts,
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
}
