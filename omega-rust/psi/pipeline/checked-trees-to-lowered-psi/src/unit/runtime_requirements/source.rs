//! Ordered scalar bodies reuse the invocation requirement package, but that
//! package is not source authority. Rejoin every authored fact and implicit
//! parameter range before emission. Read custody checks declaration identity;
//! value correspondence separately checks operators and literals. Neither alone
//! prevents a same-typed predicate substitution. Source order and duplicates are
//! checked before canonical publication; callers then owe each published slot
//! before importing guarantees, even if actual substitution makes slots equal.
use super::super::{CheckedIntegerComparisonKind, CheckedTrees, ClosedScalarContractValue};
use super::{
    CheckedBooleanExpression, CheckedScalarExpression, LoweringError, PrimitiveType,
    integer_scalar_type, unsupported,
};
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
            crate::expression_preparation::source_custody::validate_entry_read_expression(
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
    // Ordered bodies keep no floating range roster: the structural runtime
    // requirement capsule already fails closed when an authored range cannot
    // be retained, so no authored floating range can reach this call.
    validate_parameter_ranges(checked, state, retained.map(Some), None)
}

/// Scalar graphs retain authored clauses separately from their appended range
/// predicates. Check those exact retained rows, not a different contract
/// capsule. Floating ranges keep an explicit placeholder row in the requires
/// tail while their exact IEEE endpoints ride the retained entry roster;
/// rejoin both against the authored source constraints.
pub(crate) fn validate_graph_parameter_ranges(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    plan: &checked_trees::ClosedScalarValueContractPlan,
) -> Result<(), LoweringError> {
    let source = checked
        .machines()
        .iter()
        .find(|source| source.symbol == machine)
        .ok_or(LoweringError::Unsupported(
            "scalar range has no source machine",
        ))?;
    let state = checked
        .machine_states(source)
        .first()
        .ok_or(LoweringError::Unsupported(
            "scalar range has no source entry",
        ))?;
    let authored_count = checked
        .machine_contracts(source)
        .iter()
        .filter(|contract| {
            contract.kind == SignatureContractKind::Requires && contract.binding.is_none()
        })
        .count();
    let ranges = plan
        .requires()
        .get(authored_count..)
        .ok_or(LoweringError::Unsupported(
            "scalar contract lost authored requirements",
        ))?;
    validate_parameter_ranges(
        checked,
        state,
        ranges.iter().map(|clause| match clause {
            Some(ClosedScalarContractValue::Predicate(predicate)) => Some(predicate),
            _ => None,
        }),
        plan.float_entry_ranges().map(<[_]>::iter),
    )
}

fn validate_parameter_ranges<'predicate>(
    checked: &CheckedTrees,
    state: &checked_trees::state::State,
    mut retained: impl Iterator<Item = Option<&'predicate CheckedBooleanExpression>>,
    mut float_ranges: Option<std::slice::Iter<'_, checked_trees::ClosedFloatRangeRequirement>>,
) -> Result<(), LoweringError> {
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
                        let TypeConstraintNode::Range {
                            minimum,
                            maximum,
                            end_inclusive,
                        } = constraint
                        else {
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
                        {
                            return unsupported(
                                "scalar entry range differs from its exact source bounds",
                            );
                        }
                        if matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64) {
                            // The closed scalar predicate language cannot
                            // spell IEEE membership, so a floating range keeps
                            // an explicit placeholder requires row while its
                            // evidence rides the retained roster. Consume that
                            // row, then rejoin the roster entry bit-for-bit:
                            // authored endpoints at the declared carrier, IEEE
                            // order, and the authored boundary kind verbatim.
                            if !matches!(retained.next(), Some(None)) {
                                return unsupported(
                                    "scalar float entry range lost its explicit requires row",
                                );
                            }
                            let (minimum_value, maximum_value) = validation::closed_float_range_endpoint(
                                    &checked.typed,
                                    *minimum,
                                    primitive,
                                )
                                .zip(validation::closed_float_range_endpoint(
                                    &checked.typed,
                                    *maximum,
                                    primitive,
                                ))
                                .filter(|(low, high)| {
                                    validation::ieee_float_range_ordered(*low, *high)
                                })
                                .ok_or(LoweringError::Unsupported(
                                    "scalar float entry range has unavailable or unordered source bounds",
                                ))?;
                            let range = float_ranges.as_mut().and_then(Iterator::next).ok_or(
                                LoweringError::Unsupported(
                                    "scalar float entry range lost its retained endpoint evidence",
                                ),
                            )?;
                            if range.position != position
                                || range.primitive_type != primitive
                                || range.minimum != minimum_value
                                || range.maximum != maximum_value
                                || range.maximum_inclusive != *end_inclusive
                            {
                                return unsupported(
                                    "scalar float entry range differs from its retained evidence",
                                );
                            }
                            continue;
                        }
                        let (minimum_value, maximum_value) =
                            validation::closed_integer_range_bound(&checked.typed, *minimum)
                                .zip(validation::closed_integer_range_maximum(
                                    &checked.typed,
                                    *maximum,
                                    *end_inclusive,
                                ))
                                .filter(|(low, high)| low <= high)
                                .ok_or(LoweringError::Unsupported(
                                    "scalar entry range has unavailable or empty source bounds",
                                ))?;
                        integer_scalar_type(primitive)?;
                        let Some(CheckedBooleanExpression::And { left, right }) =
                            retained.next().flatten()
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
                        // Recompute the interval from the authored endpoints.
                        // These literals represent normalized bounds, not raw
                        // endpoint reads or executable predecessor expressions.
                        for (retained, expected) in [
                            (low.as_ref(), minimum_value),
                            (high.as_ref(), maximum_value),
                        ] {
                            let CheckedScalarExpression::IntegerLiteral { literal } = retained
                            else {
                                return unsupported(
                                    "scalar entry range endpoint is not a closed literal",
                                );
                            };
                            let expected = validation::land_integer_value(&expected, primitive)
                                .ok_or(LoweringError::Unsupported(
                                    "scalar range bound does not fit its carrier",
                                ))?;
                            if literal != &expected || literal.landing() != expected.landing() {
                                return unsupported(
                                    "scalar entry range changes its normalized bound or carrier",
                                );
                            }
                        }
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
    if float_ranges.and_then(|mut ranges| ranges.next()).is_some() {
        return unsupported("scalar contract retains an unauthored floating entry range");
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
    crate::expression_preparation::source_custody::value_correspondence::validate(
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
