//! Preserve authored state arrival predicates in their scalar parameter namespace.

use checked_trees::{
    CheckedBooleanExpression, CheckedOperatorFacts, CheckedScalarExpression,
    ClosedScalarContractValue,
};
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

/// Literal equality intentionally compares spelling; source custody must
/// additionally preserve the selected integer carrier and arithmetic landing.
pub(super) fn matches(
    expected: &[Option<ClosedScalarContractValue>],
    retained: &[Option<ClosedScalarContractValue>],
) -> bool {
    expected == retained
        && expected
            .iter()
            .zip(retained)
            .all(|(expected, retained)| match (expected, retained) {
                (
                    Some(ClosedScalarContractValue::Predicate(expected)),
                    Some(ClosedScalarContractValue::Predicate(retained)),
                ) => predicate_landings(expected, retained),
                _ => false,
            })
}

fn predicate_landings(
    expected: &CheckedBooleanExpression,
    retained: &CheckedBooleanExpression,
) -> bool {
    use CheckedBooleanExpression as Boolean;
    match (expected, retained) {
        (Boolean::Not(left), Boolean::Not(right)) => predicate_landings(left, right),
        (
            Boolean::Equal { left, right },
            Boolean::Equal {
                left: other_left,
                right: other_right,
            },
        )
        | (
            Boolean::And { left, right },
            Boolean::And {
                left: other_left,
                right: other_right,
            },
        )
        | (
            Boolean::Or { left, right },
            Boolean::Or {
                left: other_left,
                right: other_right,
            },
        ) => predicate_landings(left, other_left) && predicate_landings(right, other_right),
        (
            Boolean::IntegerComparison { left, right, .. },
            Boolean::IntegerComparison {
                left: other_left,
                right: other_right,
                ..
            },
        )
        | (
            Boolean::ScalarIeeeFloatComparison { left, right, .. },
            Boolean::ScalarIeeeFloatComparison {
                left: other_left,
                right: other_right,
                ..
            },
        ) => scalar_landings(left, other_left) && scalar_landings(right, other_right),
        _ => true,
    }
}

fn scalar_landings(expected: &CheckedScalarExpression, retained: &CheckedScalarExpression) -> bool {
    use CheckedScalarExpression as Scalar;
    match (expected, retained) {
        (Scalar::IntegerLiteral { literal }, Scalar::IntegerLiteral { literal: other }) => {
            literal.landing() == other.landing()
        }
        (Scalar::Boolean(left), Scalar::Boolean(right)) => predicate_landings(left, right),
        (
            Scalar::IntegerBinary { left, right, .. },
            Scalar::IntegerBinary {
                left: other_left,
                right: other_right,
                ..
            },
        ) => scalar_landings(left, other_left) && scalar_landings(right, other_right),
        (
            Scalar::IntegerBitwiseNot { operand, .. },
            Scalar::IntegerBitwiseNot { operand: other, .. },
        )
        | (Scalar::IntegerWiden { operand, .. }, Scalar::IntegerWiden { operand: other, .. })
        | (
            Scalar::IntegerExactCast { operand, .. },
            Scalar::IntegerExactCast { operand: other, .. },
        )
        | (
            Scalar::IntegerTrappingCast { operand, .. },
            Scalar::IntegerTrappingCast { operand: other, .. },
        )
        | (
            Scalar::IntegerWrappingCast { operand, .. },
            Scalar::IntegerWrappingCast { operand: other, .. },
        )
        | (
            Scalar::IntegerSaturatingCast { operand, .. },
            Scalar::IntegerSaturatingCast { operand: other, .. },
        )
        | (
            Scalar::RepresentationView { operand, .. },
            Scalar::RepresentationView { operand: other, .. },
        ) => scalar_landings(operand, other),
        (
            Scalar::StructuralParameterIndexedRead {
                index,
                inner_indexes,
                ..
            },
            Scalar::StructuralParameterIndexedRead {
                index: other_index,
                inner_indexes: other_indexes,
                ..
            },
        ) => {
            scalar_landings(index, other_index)
                && inner_indexes
                    .iter()
                    .zip(other_indexes)
                    .all(|(left, right)| scalar_landings(left, right))
        }
        _ => true,
    }
}
