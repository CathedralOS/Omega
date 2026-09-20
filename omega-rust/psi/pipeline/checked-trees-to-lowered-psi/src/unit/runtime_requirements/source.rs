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
    // Ordered bodies keep no floating or integer range roster: the
    // structural runtime requirement capsule already fails closed when an
    // authored range cannot be retained, so no authored floating range can
    // reach this call, and integer bounds ride their predicate clauses.
    validate_parameter_ranges(
        checked,
        state,
        retained.map(|predicate| Some(RetainedClause::Predicate(predicate))),
        None,
        None,
    )
}

/// One requires-tail row during the range rejoin. Integer bounds are checked
/// comparison predicates; a floating entry range carries its retained IEEE
/// endpoints directly on the `FloatRange` clause.
enum RetainedClause<'clause> {
    Predicate(&'clause CheckedBooleanExpression),
    FloatRange(&'clause checked_trees::ClosedFloatRangeRequirement),
}

/// Scalar graphs retain authored clauses separately from their appended range
/// clauses. Check those exact retained rows, not a different contract
/// capsule. A floating range keeps its IEEE endpoints on the requires-tail
/// `FloatRange` row; rejoin that row against the authored source constraint
/// and require the retained entry roster to carry it verbatim.
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
            Some(ClosedScalarContractValue::Predicate(predicate)) => {
                Some(RetainedClause::Predicate(predicate))
            }
            Some(ClosedScalarContractValue::FloatRange(range)) => {
                Some(RetainedClause::FloatRange(range))
            }
            _ => None,
        }),
        plan.float_entry_ranges().map(<[_]>::iter),
        plan.integer_entry_ranges().map(<[_]>::iter),
    )
}

fn validate_parameter_ranges<'clause>(
    checked: &CheckedTrees,
    state: &checked_trees::state::State,
    mut retained: impl Iterator<Item = Option<RetainedClause<'clause>>>,
    mut float_ranges: Option<std::slice::Iter<'_, checked_trees::ClosedFloatRangeRequirement>>,
    mut integer_ranges: Option<std::slice::Iter<'_, checked_trees::ClosedIntegerRangeRequirement>>,
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
                            // A floating range is a `FloatRange` clause in
                            // the requires tail: consume that row, then
                            // rejoin it bit-for-bit against the authored
                            // source bounds — endpoints at the declared
                            // carrier, IEEE order, and the authored boundary
                            // kind verbatim.
                            let Some(RetainedClause::FloatRange(range)) = retained.next().flatten()
                            else {
                                return unsupported(
                                    "scalar float entry range lost its requires clause",
                                );
                            };
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
                            if range.position != position
                                || range.primitive_type != primitive
                                || range.minimum != minimum_value
                                || range.maximum != maximum_value
                                || range.maximum_inclusive != *end_inclusive
                            {
                                return unsupported(
                                    "scalar float entry range differs from its authored bounds",
                                );
                            }
                            // The retained roster delivers this exact clause;
                            // an absent or drifted evidence row fails closed.
                            let evidence = float_ranges.as_mut().and_then(Iterator::next).ok_or(
                                LoweringError::Unsupported(
                                    "scalar float entry range lost its retained endpoint evidence",
                                ),
                            )?;
                            if evidence != range {
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
                        let Some(RetainedClause::Predicate(CheckedBooleanExpression::And {
                            left,
                            right,
                        })) = retained.next().flatten()
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
                        let mut expected_endpoints = Vec::with_capacity(2);
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
                            expected_endpoints.push(expected);
                        }
                        // The retained integer roster delivers this exact
                        // interval; an absent or drifted evidence row fails
                        // closed. `IntegerLiteral` equality is text-only, so
                        // the landing rides the comparison explicitly.
                        if let Some(roster) = integer_ranges.as_mut() {
                            let [minimum, maximum] = expected_endpoints.as_slice() else {
                                unreachable!("two normalized endpoints were checked above")
                            };
                            let evidence = roster.next().ok_or(LoweringError::Unsupported(
                                "scalar integer entry range lost its retained endpoint evidence",
                            ))?;
                            if evidence.position != position
                                || evidence.primitive_type != primitive
                                || evidence.minimum != *minimum
                                || evidence.minimum.landing() != minimum.landing()
                                || evidence.maximum != *maximum
                                || evidence.maximum.landing() != maximum.landing()
                            {
                                return unsupported(
                                    "scalar integer entry range differs from its retained evidence",
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
    if integer_ranges
        .and_then(|mut ranges| ranges.next())
        .is_some()
    {
        return unsupported("scalar contract retains an unauthored integer entry range");
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
