use omega_core::arena::HandleSpan;
use omega_core::symbols::SymbolHandle;

use crate::{
    BorrowArgumentAccessFact, CheckFacts, CheckedTrees, ContractProofFactRef, FlowBoundaryEdgeFact,
    FlowCallFact, FlowConstraintRef, FlowExitFact, FlowFacts, FlowInvalidationFact,
    FlowSemanticContextRef, FlowStateFact, FlowStatementFact,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AcceptanceVerdict {
    #[default]
    Accepted,
}

#[derive(Debug, Clone, Copy)]
pub struct StateAcceptance<'facts> {
    facts: &'facts CheckFacts,
    state: &'facts FlowStateFact,
}

#[derive(Debug, Clone, Copy)]
pub struct StatementAcceptance<'facts> {
    facts: &'facts CheckFacts,
    statement: &'facts FlowStatementFact,
}

#[derive(Debug, Clone, Copy)]
pub struct CallAcceptance<'facts> {
    facts: &'facts CheckFacts,
    call: &'facts FlowCallFact,
}

#[derive(Debug, Clone, Copy)]
pub struct ExitAcceptance<'facts> {
    facts: &'facts CheckFacts,
    exit: &'facts FlowExitFact,
}

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

impl<'facts> StateAcceptance<'facts> {
    pub fn verdict(&self) -> AcceptanceVerdict {
        AcceptanceVerdict::Accepted
    }

    pub fn is_accepted(&self) -> bool {
        self.verdict() == AcceptanceVerdict::Accepted
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

impl<'facts> CallAcceptance<'facts> {
    pub fn verdict(&self) -> AcceptanceVerdict {
        AcceptanceVerdict::Accepted
    }

    pub fn is_accepted(&self) -> bool {
        self.verdict() == AcceptanceVerdict::Accepted
    }

    pub fn call(&self) -> &'facts FlowCallFact {
        self.call
    }

    pub fn borrow_accesses(&self) -> HandleSpan<BorrowArgumentAccessFact> {
        self.call.accesses
    }

    pub fn entry_semantic_contexts(&self) -> &'facts [FlowSemanticContextRef] {
        semantic_contexts(&self.facts.flow, self.call.entry_semantic_contexts)
    }

    pub fn entry_constraints(&self) -> &'facts [FlowConstraintRef] {
        constraints(&self.facts.flow, self.call.entry_constraints)
    }

    pub fn requires_semantic_contexts(&self) -> &'facts [FlowSemanticContextRef] {
        semantic_contexts(&self.facts.flow, self.call.requires_contexts)
    }

    pub fn requires_constraints(&self) -> &'facts [FlowConstraintRef] {
        constraints(&self.facts.flow, self.call.requires_constraints)
    }

    pub fn exit_semantic_contexts(&self) -> &'facts [FlowSemanticContextRef] {
        semantic_contexts(&self.facts.flow, self.call.exit_semantic_contexts)
    }

    pub fn exit_constraints(&self) -> &'facts [FlowConstraintRef] {
        constraints(&self.facts.flow, self.call.exit_constraints)
    }

    pub fn invalidations(&self) -> &'facts [FlowInvalidationFact] {
        self.facts
            .flow
            .invalidations
            .events
            .span_or_empty(self.call.invalidations)
    }

    pub fn boundary_edges(&self) -> &'facts [FlowBoundaryEdgeFact] {
        self.facts
            .flow
            .boundaries
            .edges
            .span_or_empty(self.call.boundary_edges)
    }

    pub fn requires(&self) -> &'facts [ContractProofFactRef] {
        self.facts
            .proof
            .contract_fact_refs
            .span_or_empty(self.call.requires)
    }

    pub fn ensures(&self) -> &'facts [ContractProofFactRef] {
        self.facts
            .proof
            .contract_fact_refs
            .span_or_empty(self.call.ensures)
    }

    pub fn direct_effects(&self) -> omega_effects::EffectSet {
        self.call.direct_effects
    }

    pub fn transitive_effects(&self) -> omega_effects::EffectSet {
        self.call.transitive_effects
    }
}

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

fn semantic_contexts(
    flow: &FlowFacts,
    contexts: HandleSpan<FlowSemanticContextRef>,
) -> &[FlowSemanticContextRef] {
    flow.contexts.semantic_context_refs.span_or_empty(contexts)
}

fn constraints(
    flow: &FlowFacts,
    constraints: HandleSpan<FlowConstraintRef>,
) -> &[FlowConstraintRef] {
    flow.contexts.constraint_refs.span_or_empty(constraints)
}
