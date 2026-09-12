//! Ordered scalar bodies reuse the invocation requirement package, but that
//! package is not source authority. Rejoin every authored fact and implicit
//! parameter range before emission. Read custody checks declaration identity;
//! value correspondence separately checks operators and literals. Neither alone
//! prevents a same-typed predicate substitution. Source order and duplicates are
//! checked before canonical publication; callers then owe each published slot
//! before importing guarantees, even if actual substitution makes slots equal.

use super::*;
use checked_trees::domain::ProofFact;
use checked_trees::signature::SignatureContractKind;
use checked_trees::types::{TypeConstraintNode, TypeReferenceNode};

pub(crate) fn validate_scalar_source(
    checked: &CheckedTrees,
    machine: &checked_trees::machine::Machine,
    state: &checked_trees::state::State,
) -> Result<(), LoweringError> {
    let requirements = checked
        .facts
        .contract_plans
        .for_machine(machine.symbol)
        .and_then(|contract| contract.crash.structural_runtime_requirements())
        .ok_or(LoweringError::Unsupported(
            "scalar completion lacks complete entry requirements",
        ))?;
    let mut retained = requirements.iter();
    for contract in checked
        .machine_contracts(machine)
        .iter()
        .chain(checked.state_contracts(state))
    {
        if contract.kind != SignatureContractKind::Requires {
            continue;
        }
        if contract.binding.is_some() {
            return unsupported("scalar entry requirement cannot erase an evidence binding");
        }
        let facts = checked
            .proof_facts
            .span(contract.facts)
            .ok_or(LoweringError::Unsupported(
                "scalar entry requirement has an invalid source fact span",
            ))?;
        if facts.is_empty() {
            return unsupported("scalar entry requirement has no authored predicate");
        }
        for fact in facts {
            let ProofFact::Expression(expression) = fact else {
                return unsupported("scalar entry requirement needs its predicate evidence owner");
            };
            let predicate = retained.next().ok_or(LoweringError::Unsupported(
                "scalar entry requirement lost an authored fact",
            ))?;
            validate_scalar_namespace(predicate)?;
            let scalar = CheckedScalarExpression::Boolean(Box::new(predicate.clone()));
            crate::scalar_source_custody::validate_entry_read_expression(
                checked,
                state.symbol,
                *expression,
                &scalar,
            )?;
            validate_value(
                checked,
                state.symbol,
                *expression,
                PrimitiveType::Bool,
                scalar,
            )?;
        }
    }
    let mut scalar_position = 0;
    for parameter in checked.state_parameters(state) {
        let primitive = checked.primitive_type_reference(parameter.type_reference);
        let position = scalar_position;
        scalar_position += usize::from(primitive.is_some());
        let mut reference = parameter.type_reference;
        loop {
            match checked.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Reference { referee, .. } => reference = *referee,
                TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                } => {
                    for constraint in checked.type_reference_table.constraints(*constraints) {
                        let TypeConstraintNode::Range { minimum, maximum } = constraint else {
                            continue;
                        };
                        let primitive = primitive.ok_or(LoweringError::Unsupported(
                            "scalar range has no primitive carrier",
                        ))?;
                        if parameter.is_self
                            || parameter.is_const
                            || checked
                                .arithmetic_domain_for_type_reference(parameter.type_reference)
                                != numerics::arithmetic::ArithmeticDomain::Exact
                            || validation::closed_integer_range_bound(&checked.typed, *minimum)
                                .zip(validation::closed_integer_range_bound(
                                    &checked.typed,
                                    *maximum,
                                ))
                                .is_none_or(|(low, high)| low > high)
                        {
                            return unsupported(
                                "scalar entry range differs from its exact source bounds",
                            );
                        }
                        integer_scalar_type(primitive)?;
                        let Some(CheckedBooleanExpression::And { left, right }) = retained.next()
                        else {
                            return unsupported("scalar entry range lost its two ordered bounds");
                        };
                        let (
                            CheckedBooleanExpression::IntegerComparison {
                                kind: CheckedIntegerComparisonKind::LessOrEqual,
                                left: low,
                                right: low_subject,
                            },
                            CheckedBooleanExpression::IntegerComparison {
                                kind: CheckedIntegerComparisonKind::LessOrEqual,
                                left: high_subject,
                                right: high,
                            },
                        ) = (left.as_ref(), right.as_ref())
                        else {
                            return unsupported(
                                "scalar entry range changes its comparison meaning",
                            );
                        };
                        let subject = CheckedScalarExpression::Parameter {
                            position,
                            primitive_type: primitive,
                        };
                        if low_subject.as_ref() != &subject || high_subject.as_ref() != &subject {
                            return unsupported("scalar entry range belongs to another parameter");
                        }
                        // Bounds are closed literals in this runtime vocabulary.
                        // Value correspondence is not parameter-read custody:
                        // a name-shaped endpoint must not supply a runtime read.
                        if !matches!(low.as_ref(), CheckedScalarExpression::IntegerLiteral { .. })
                            || !matches!(
                                high.as_ref(),
                                CheckedScalarExpression::IntegerLiteral { .. }
                            )
                        {
                            return unsupported(
                                "scalar entry range endpoint is not a closed literal",
                            );
                        }
                        validate_value(
                            checked,
                            state.symbol,
                            *minimum,
                            primitive,
                            low.as_ref().clone(),
                        )?;
                        validate_value(
                            checked,
                            state.symbol,
                            *maximum,
                            primitive,
                            high.as_ref().clone(),
                        )?;
                    }
                    reference = *base_type;
                }
                _ => break,
            }
        }
    }
    if retained.next().is_some() {
        return unsupported("scalar entry requirements contain an unauthored clause");
    }
    Ok(())
}

fn validate_value(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    expression: checked_trees::expression::ExpressionHandle,
    primitive: PrimitiveType,
    value: CheckedScalarExpression,
) -> Result<(), LoweringError> {
    crate::scalar_source_custody::value_correspondence::validate(
        checked,
        state,
        0,
        expression,
        primitive,
        &checked_trees::CheckedCallScalarArgument::Pure(value),
    )
}

fn validate_scalar_namespace(predicate: &CheckedBooleanExpression) -> Result<(), LoweringError> {
    match predicate {
        CheckedBooleanExpression::Constant(_) | CheckedBooleanExpression::Parameter { .. } => {
            Ok(())
        }
        CheckedBooleanExpression::Not(operand) => validate_scalar_namespace(operand),
        CheckedBooleanExpression::Equal { left, right }
        | CheckedBooleanExpression::And { left, right }
        | CheckedBooleanExpression::Or { left, right } => {
            validate_scalar_namespace(left)?;
            validate_scalar_namespace(right)
        }
        CheckedBooleanExpression::IntegerComparison { left, right, .. } => {
            for operand in [left, right] {
                match operand.as_ref() {
                    CheckedScalarExpression::Parameter { primitive_type, .. }
                        if *primitive_type != PrimitiveType::Addr =>
                    {
                        integer_scalar_type(*primitive_type)?;
                    }
                    CheckedScalarExpression::IntegerLiteral { .. } => {}
                    _ => {
                        return unsupported(
                            "scalar entry predicate needs a separate field or arithmetic evidence owner",
                        );
                    }
                }
            }
            Ok(())
        }
        _ => unsupported("scalar entry predicate is outside its invocation parameter namespace"),
    }
}
