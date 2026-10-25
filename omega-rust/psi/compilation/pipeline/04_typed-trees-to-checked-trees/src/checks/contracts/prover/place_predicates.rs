//! Semantic observations over one exact place. The caller supplies the
//! observation's value epoch; these queries never select or merge epochs.

use checked_trees::CheckFacts;
use typed_trees::expression::ExpressionHandle;

pub(in crate::checks::contracts) enum PlacePredicate {
    Bytes(validation::field_domains::ByteSequencePredicate),
    Case(symbols::SymbolHandle),
}

pub(in crate::checks::contracts) fn place_predicate(
    program: &dyn ::validation::CheckingInput,
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
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        point: facts::ProgramPoint,
        contexts: &[facts::FactContextHandle],
        subject: &validation::CanonicalPlace,
    ) -> bool {
        match self {
            Self::Bytes(predicate) => super::byte_predicate_proven_at_place(
                program,
                &facts.semantic,
                contexts,
                subject,
                *predicate,
            ),
            Self::Case(case) => {
                let facts::ProgramPoint::Statement {
                    machine_symbol,
                    state_symbol,
                    statement_index,
                } = point
                else {
                    return false;
                };
                crate::flow::place_case_has_value(
                    program,
                    &facts.semantic,
                    contexts,
                    machine_symbol,
                    state_symbol,
                    statement_index,
                    subject,
                    *case,
                    true,
                )
            }
        }
    }
}
