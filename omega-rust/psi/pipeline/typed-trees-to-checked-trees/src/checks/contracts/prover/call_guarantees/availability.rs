//! Contract and borrow consumers share call establishment, source identity,
//! and capture preservation. Availability comes from the exact use's live
//! flow contexts, never a certificate's assertion of dominance.

use super::{
    Invocation, actual_projection, builtin_predicate, capture_preserved, captured_place,
    invocation, stable_arguments,
};
use crate::flow::CanonicalPlace;
use crate::semantic_calls::CallSite;
use checked_trees::FlowStateFact;
use facts::{ContractFactKind, FactOrigin, FactPayload, FactPlan, PlaceRoot, ProgramPoint};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::state::State;

pub(in crate::checks) struct AvailableGuarantee<'program> {
    pub(in crate::checks) expression: ExpressionHandle,
    pub(in crate::checks) fact: arena::Handle<typed_trees::domain::ProofFact>,
    pub(super) invocation: Invocation<'program>,
}

impl AvailableGuarantee<'_> {
    pub(in crate::checks) fn coordinates(&self) -> (usize, usize) {
        (self.invocation.statement, self.invocation.ordinal)
    }

    pub(in crate::checks) fn has_builtin_meaning(
        &self,
        program: &TypedTrees,
        expression: ExpressionHandle,
    ) -> bool {
        validation::has_builtin_decomposed_guard_meaning(
            program,
            self.invocation.machine,
            Some(self.invocation.state),
            expression,
        )
    }

    pub(in crate::checks) fn is_result(
        &self,
        program: &TypedTrees,
        expression: ExpressionHandle,
    ) -> bool {
        validation::reserved_result_place(program, expression).is_some_and(|result| {
            result.machine_symbol == self.invocation.machine.symbol && result.segments.is_empty()
        })
    }

    pub(in crate::checks) fn actual(
        &self,
        program: &TypedTrees,
        expression: ExpressionHandle,
    ) -> Option<ExpressionHandle> {
        if matches!(
            program.expression_table.expression(expression),
            ExpressionNode::Integer(_)
        ) {
            return Some(expression);
        }
        let (actual, remaining) = actual_projection(program, &self.invocation, expression)?;
        remaining.is_empty().then_some(actual)
    }

    /// The initial assignment names an occurrence; it is never evaluated.
    /// Live provenance must independently join that immutable binding to
    /// this exact call. Copies normalize through the existing bound reader.
    pub(in crate::checks) fn immutable_result_binding(
        &self,
        program: &TypedTrees,
        semantic: &FactPlan,
        contexts: &[facts::FactContextHandle],
        state: &State,
    ) -> Option<SymbolHandle> {
        if state.symbol != self.invocation.caller_state {
            return None;
        }
        let typed_trees::statement::StatementNode::LocalData(local) = program
            .statement_table
            .statements(state.statement_nodes)
            .get(self.invocation.statement)?
        else {
            return None;
        };
        let CallSite::Expression { expression, .. } = self.invocation.site else {
            return None;
        };
        if local.is_mutable || !local.symbol.is_valid() || local.initial_value != expression {
            return None;
        }
        let place = captured_place(
            program,
            semantic,
            contexts,
            CanonicalPlace {
                root: PlaceRoot::Symbol(local.symbol),
                segments: Vec::new(),
            },
        )?;
        (place.root == PlaceRoot::Expression(expression) && place.segments.is_empty())
            .then_some(local.symbol)
    }
}

pub(in crate::checks) fn available<'program>(
    program: &'program TypedTrees,
    facts: &checked_trees::CheckFacts,
    caller: &FlowStateFact,
    statement: usize,
    contexts: &[facts::FactContextHandle],
    frames: &validation::CallFrameResolver<'_>,
) -> Vec<AvailableGuarantee<'program>> {
    let Some(caller_machine) = crate::lookup::machine_by_symbol(program, caller.machine_symbol)
    else {
        return Vec::new();
    };
    let mut guarantees = Vec::new();
    for fact in contexts.iter().flat_map(|context| {
        facts
            .semantic
            .context_view(facts.semantic.contexts.get(*context))
            .facts()
    }) {
        let FactPayload::ContractBooleanExpression {
            kind: ContractFactKind::Ensures,
            fact: source,
            expression,
            ..
        } = fact.payload
        else {
            continue;
        };
        let ProgramPoint::CallEnsures {
            machine_symbol,
            state_symbol,
            statement_index,
            call_ordinal,
        } = fact.point
        else {
            continue;
        };
        if fact.origin != FactOrigin::CallEnsures
            || machine_symbol != caller.machine_symbol
            || state_symbol != caller.state_symbol
            || statement_index >= statement
        {
            continue;
        }
        let Some(supplied) = invocation(program, caller, statement_index, call_ordinal) else {
            continue;
        };
        let CallSite::Expression {
            expression: produced,
            ..
        } = supplied.site
        else {
            continue;
        };
        if !owns_guarantee(program, &supplied, source, expression)
            || !stable_arguments(program, &supplied)
            || !capture_preserved(
                program,
                &facts.borrow,
                caller_machine,
                &supplied,
                produced,
                expression,
                frames,
            )
            || !builtin_predicate(program, &facts.operators, &supplied, expression)
        {
            continue;
        }
        if guarantees.iter().any(|prior: &AvailableGuarantee<'_>| {
            prior.fact == source && prior.coordinates() == (statement_index, call_ordinal)
        }) {
            continue;
        }
        guarantees.push(AvailableGuarantee {
            expression,
            fact: source,
            invocation: supplied,
        });
    }
    guarantees
}

fn owns_guarantee(
    program: &TypedTrees,
    supplied: &Invocation<'_>,
    source: arena::Handle<typed_trees::domain::ProofFact>,
    expression: ExpressionHandle,
) -> bool {
    source.is_valid()
        && matches!(program.proof_facts.get(source), typed_trees::domain::ProofFact::Expression(actual) if *actual == expression)
        && program
            .machine_contracts(supplied.machine)
            .iter()
            .chain(program.state_contracts(supplied.state))
            .filter(|contract| {
                contract.kind == typed_trees::signature::SignatureContractKind::Ensures
            })
            .any(|contract| {
                (0..contract.facts.count()).any(|offset| {
                    contract
                        .facts
                        .start()
                        .arena_index()
                        .checked_add(offset)
                        .is_some_and(|index| {
                            source
                                == arena::Handle::from_parts(
                                    index,
                                    contract.facts.start().generation(),
                                )
                        })
                })
            })
}
