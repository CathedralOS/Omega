//! Preserve authored state arrival predicates in their scalar parameter namespace.

use checked_trees::{CheckedOperatorFacts, ClosedScalarContractValue};
use typed_trees::{TypedTrees, machine::Machine, state::State};

pub(super) fn lower(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    machine: &Machine,
    state: &State,
) -> Option<Vec<Option<ClosedScalarContractValue>>> {
    // Named evidence bindings have their own contract owner. A predicate
    // projection cannot silently turn their bound output into a Boolean.
    if program
        .state_contracts(state)
        .iter()
        .any(|contract| contract.binding.is_some())
    {
        return None;
    }
    let Some(rows) = validation::structural_state_contract_scalar_predicates(program, state) else {
        // Scalar tags have a broader existing qualification-only route than
        // structural contracts. Preserve it without dropping any predicate.
        return validation::scalar_state_contracts_are_qualifications(program, state)
            .then(Vec::new);
    };
    rows.into_iter()
        .map(|row| {
            let predicate = match row {
                validation::StateScalarContractRow::Expression(expression) => {
                    crate::values::lower_state_scalar_contract_predicate(
                        program, operators, machine, state, expression, &mut 4096,
                    )?
                }
                validation::StateScalarContractRow::Interval {
                    parameter,
                    minimum,
                    maximum,
                } => crate::values::lower_state_interval_predicate(
                    program, state, parameter, minimum, maximum,
                )?,
                validation::StateScalarContractRow::Finite { parameter } => {
                    validation::state_finite_predicate(program, state, parameter)?
                }
            };
            Some(Some(ClosedScalarContractValue::Predicate(predicate)))
        })
        .collect()
}
