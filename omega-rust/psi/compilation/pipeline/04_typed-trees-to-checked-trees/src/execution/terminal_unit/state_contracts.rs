//! The complete original predicate roster at one state arrival.
//!
//! Graph headers and cleanup preservation consume this same declaration-owned
//! projection. Parameter domain windows are part of the receiving contract;
//! reading only authored Requires would omit their live observations.
use checked_trees::{CheckFacts, CheckedBooleanExpression};

pub(crate) fn arrival_predicates(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    owner: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
) -> Option<Vec<CheckedBooleanExpression>> {
    let rows = validation::structural_state_contract_scalar_predicates(program, state)?;
    let mut predicates = Vec::with_capacity(rows.len());
    for row in rows {
        let predicate = match row {
            validation::StateScalarContractRow::Expression(expression) => {
                crate::values::lower_state_scalar_contract_predicate(
                    program,
                    &facts.operators,
                    owner,
                    state,
                    expression,
                    &mut 4096,
                )
            }
            validation::StateScalarContractRow::DeclaredRange {
                parameter,
                minimum,
                maximum,
                end_inclusive,
            } => crate::values::lower_state_declared_range_predicate(
                program,
                &facts.operators,
                owner,
                state,
                parameter,
                minimum,
                maximum,
                end_inclusive,
            ),
            validation::StateScalarContractRow::Interval {
                parameter,
                minimum,
                maximum,
            } => crate::values::lower_state_interval_predicate(
                program, state, parameter, minimum, maximum,
            ),
            validation::StateScalarContractRow::Finite { parameter } => {
                validation::state_finite_predicate(program, state, parameter)
            }
        }?;
        predicates.push(predicate);
    }
    predicates.extend(
        validation::state_declaration_predicates(program, &facts.operators, state)
            .into_iter()
            .map(|row| row.predicate),
    );
    Some(predicates)
}
