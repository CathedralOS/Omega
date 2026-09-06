//! Operation-local polarity facts from the validated scalar denotation.
//!
//! No goal, caller premise, or neighboring operation participates in this
//! projection. The original result equation remains the first emitted fact.

use std::collections::{BTreeMap, BTreeSet};

use proof_admission::check_predicate_denotations;
use semantic_vocabulary::{Proposition, PropositionContext, ScalarTerm, ScalarType, ValueId};
use terminal_semantics::{GoalFreeScalarLeafSemantics, ScalarLeafFactShape};

use crate::ModuleError;

pub(super) fn implications(
    semantics: &GoalFreeScalarLeafSemantics,
    value_types: &BTreeMap<ValueId, ScalarType>,
) -> Result<Vec<Proposition>, ModuleError> {
    match semantics.fact_shape() {
        ScalarLeafFactShape::ResultEquation => return Ok(Vec::new()),
        ScalarLeafFactShape::BooleanResultEquationAndPolarityImplications => {}
    }
    let equation = semantics.result_equation();
    let Proposition::Equal(result, denotation) = equation else {
        unreachable!("validated scalar leaf observation retains its result equation")
    };
    // A scalar leaf has only its result and direct operands. Do not rebuild a
    // whole-machine proposition context separately for every operation.
    let mut values = BTreeSet::new();
    equation.visit_value_ids(|value| {
        values.insert(value);
    });
    let context = PropositionContext::from_value_types(values.iter().filter_map(|value| {
        value_types
            .get(value)
            .map(|scalar_type| (*value, *scalar_type))
    }))
    .map_err(ModuleError::MalformedProposition)?;
    context
        .validate(equation)
        .map_err(ModuleError::MalformedProposition)?;

    [true, false]
        .into_iter()
        .map(|positive| {
            let denotation_polarity =
                Proposition::Equal(denotation.clone(), ScalarTerm::boolean(positive));
            let checked = check_predicate_denotations(&context, &denotation_polarity, &[], &[])
                .map_err(|error| ModuleError::OperationPredicateDenotation(Box::new(error)))?;
            Ok(Proposition::Implication {
                premise: Box::new(checked.goal().clone()),
                conclusion: Box::new(Proposition::Equal(
                    result.clone(),
                    ScalarTerm::boolean(positive),
                )),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests;
