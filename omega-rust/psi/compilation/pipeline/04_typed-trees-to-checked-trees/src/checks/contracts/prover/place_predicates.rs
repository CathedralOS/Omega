//! Semantic observations over one exact place. The caller supplies the
//! observation's value epoch; these queries never select or merge epochs.

use checked_trees::{CheckFacts, FlowCallFact, FlowStateFact};
use typed_trees::{TypedTrees, expression::ExpressionHandle};

pub(in crate::checks::contracts) enum PlacePredicate {
    Bytes(crate::facts::field_domain::ByteSequencePredicate),
    Case(symbols::SymbolHandle),
}

pub(in crate::checks::contracts) fn place_predicate(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<(PlacePredicate, ExpressionHandle)> {
    if let Some((predicate, subject)) =
        typed_trees::byte_predicates::byte_predicate_call(program, expression)
    {
        return Some((PlacePredicate::Bytes(predicate), subject));
    }
    crate::proof::exact_outcome_case_test(program, expression)
        .map(|(subject, case)| (PlacePredicate::Case(case), subject))
}

impl PlacePredicate {
    pub(in crate::checks::contracts) fn proven(
        &self,
        program: &TypedTrees,
        facts: &CheckFacts,
        state: &FlowStateFact,
        call: &FlowCallFact,
        contexts: &[facts::FactContextHandle],
        subject: &crate::flow::CanonicalPlace,
    ) -> bool {
        match self {
            Self::Bytes(predicate) => super::byte_predicate_proven_at_place(
                program,
                &facts.semantic,
                contexts,
                subject,
                *predicate,
            ),
            Self::Case(case) => crate::flow::place_case_has_value(
                program,
                &facts.semantic,
                contexts,
                state.machine_symbol,
                state.state_symbol,
                call.statement_index,
                subject,
                *case,
                true,
            ),
        }
    }
}
