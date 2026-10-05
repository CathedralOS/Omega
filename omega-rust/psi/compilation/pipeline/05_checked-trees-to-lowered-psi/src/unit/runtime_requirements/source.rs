//! Ordered scalar bodies reuse the invocation requirement package, but that
//! package is not source authority. Rejoin every authored fact and implicit
//! parameter range before emission. Read custody checks declaration identity;
//! value correspondence separately checks operators and literals. Neither alone
//! prevents a same-typed predicate substitution. Source order and duplicates are
//! checked before canonical publication; callers then owe each published slot
//! before importing guarantees, even if actual substitution makes slots equal.
//! The invocation namespace is shared with runtime requirement conversion: a
//! scalar return does not restrict which parameter observations its entry
//! contract may name. Keeping a second expression roster here rejected view
//! extents already supported by that converter.
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
    let crash = &checked
        .facts
        .contract_plans
        .for_machine(machine.symbol)
        .ok_or(LoweringError::Unsupported(
            "scalar completion lacks a contract plan",
        ))?
        .crash;
    let requirements =
        crash
            .structural_runtime_requirements()
            .ok_or(LoweringError::Unsupported(
                "scalar completion lacks complete entry requirements",
            ))?;
    let mut membership_subjects = crash.runtime_membership_subjects().iter();
    let mut fact_ordinal = 0u32;
    // The package closes with the declaration predicate roster. Rederive it
    // from the entry parameters' declared storage and require that exact
    // tail, so authored clauses and ranges rejoin only the prefix before it.
    let derived = validation::machine_declaration_predicates(
        &checked.typed,
        &checked.facts.operators,
        machine,
    );
    let authored_end =
        requirements
            .len()
            .checked_sub(derived.len())
            .ok_or(LoweringError::Unsupported(
                "scalar entry requirements lost their declaration predicate rows",
            ))?;
    if !requirements[authored_end..]
        .iter()
        .zip(&derived)
        .all(|(retained, row)| *retained == row.predicate)
    {
        return unsupported("scalar entry requirements differ from their declaration predicates");
    }
    let mut retained = requirements[..authored_end].iter();
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
            let ordinal = fact_ordinal;
            fact_ordinal = fact_ordinal
                .checked_add(1)
                .ok_or(LoweringError::Unsupported(
                    "runtime requirement fact ordinal overflow",
                ))?;
            // The declaration carries a structural parameter's restated
            // qualification; it occupies no runtime requirement slot.
            if validation::restates_structural_parameter_qualification(&checked.typed, state, fact)
            {
                continue;
            }
            if let ProofFact::Membership(membership) = fact {
                validate_membership_source(
                    checked,
                    membership,
                    ordinal,
                    &mut membership_subjects,
                    &mut retained,
                )?;
                continue;
            }
            let ProofFact::Expression(_) = fact else {
                return unsupported("scalar entry requirement needs its predicate evidence owner");
            };
            let predicate = retained.next().ok_or(LoweringError::Unsupported(
                "scalar entry requirement lost an authored fact",
            ))?;
            super::validate_namespace(predicate)?;
            let _ = CheckedScalarExpression::Boolean(Box::new(predicate.clone()));
        }
    }
    if membership_subjects.next().is_some() {
        return unsupported("scalar entry requirements retain an unauthored membership subject");
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

/// Rejoin membership rows at every runtime-contract publisher. Ordinary
/// expression requirements retain their existing source validators; this
/// walk counts their slots so a membership cannot substitute another clause.
pub(crate) fn validate_runtime_memberships(
    checked: &CheckedTrees,
    machine_symbol: symbols::SymbolHandle,
) -> Result<(), LoweringError> {
    let machine = checked
        .machines()
        .iter()
        .find(|row| row.symbol == machine_symbol)
        .ok_or(LoweringError::Unsupported(
            "runtime membership has no source machine",
        ))?;
    let state = checked
        .machine_states(machine)
        .first()
        .ok_or(LoweringError::Unsupported(
            "runtime membership has no source entry",
        ))?;
    let crash = &checked
        .facts
        .contract_plans
        .for_machine(machine_symbol)
        .ok_or(LoweringError::Unsupported(
            "runtime membership has no contract plan",
        ))?
        .crash;
    let mut retained = crash
        .structural_runtime_requirements()
        .ok_or(LoweringError::Unsupported(
            "runtime membership has no complete requirement package",
        ))?
        .iter();
    let mut subjects = crash.runtime_membership_subjects().iter();
    if checked
        .machine_contracts(machine)
        .iter()
        .chain(checked.state_contracts(state))
        .any(|contract| {
            contract.kind == SignatureContractKind::Requires && contract.binding.is_some()
        })
    {
        return unsupported("runtime entry requirement cannot erase an evidence binding");
    }
    let facts = checked
        .machine_contracts(machine)
        .iter()
        .chain(checked.state_contracts(state))
        .filter(|contract| contract.kind == SignatureContractKind::Requires)
        .flat_map(|contract| checked.proof_facts.span_or_empty(contract.facts));
    for (ordinal, fact) in facts.enumerate() {
        if validation::restates_structural_parameter_qualification(&checked.typed, state, fact) {
            continue;
        }
        match fact {
            ProofFact::Expression(_) => {
                retained.next().ok_or(LoweringError::Unsupported(
                    "runtime requirement lost an authored expression",
                ))?;
            }
            ProofFact::Membership(membership) => validate_membership_source(
                checked,
                membership,
                u32::try_from(ordinal).map_err(|_| {
                    LoweringError::Unsupported("runtime requirement fact ordinal overflow")
                })?,
                &mut subjects,
                &mut retained,
            )?,
            ProofFact::Proposition(_) => {
                return unsupported("runtime membership predicate needs its evidence owner");
            }
        }
    }
    if subjects.next().is_some() {
        return unsupported("runtime requirements retain an unauthored membership subject");
    }
    Ok(())
}

fn validate_membership_source<'clause>(
    checked: &CheckedTrees,
    membership: &checked_trees::domain::ProofMembershipFact,
    ordinal: u32,
    membership_subjects: &mut std::slice::Iter<
        'clause,
        checked_trees::CheckedRuntimeMembershipSubject,
    >,
    retained: &mut std::slice::Iter<'clause, CheckedBooleanExpression>,
) -> Result<(), LoweringError> {
    let witness = membership_subjects
        .next()
        .filter(|row| row.fact_ordinal == ordinal && row.membership == *membership)
        .ok_or(LoweringError::Unsupported(
            "scalar entry membership lost its exact subject row",
        ))?;
    let _ = validation::scalar_membership_carrier(&checked.typed, membership).ok_or(
        LoweringError::Unsupported(
            "scalar entry membership needs its declared theory evidence owner",
        ),
    )?;
    let predicates = validation::scalar_membership_predicates(
        &checked.typed,
        &checked.facts.operators,
        membership,
        &witness.subject,
    )
    .ok_or(LoweringError::Unsupported(
        "scalar entry membership has no complete predicate projection",
    ))?;
    for _ in predicates {
        let actual = retained.next().ok_or(LoweringError::Unsupported(
            "scalar entry membership lost a declared predicate",
        ))?;
        super::validate_namespace(actual)?;
    }
    Ok(())
}

/// One requires-tail row during the range rejoin. Integer bounds are checked
/// comparison predicates; a floating entry range occupies its own
/// `FloatRange` row.
enum RetainedClause<'clause> {
    Predicate(&'clause CheckedBooleanExpression),
    FloatRange,
}

/// Scalar graphs retain authored clauses separately from their appended range
/// clauses. Check those exact retained rows, not a different contract
/// capsule. A floating range occupies its requires-tail `FloatRange` row; its
/// authored source constraint must resolve and the retained entry roster must
/// carry one evidence row for it.
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
            Some(ClosedScalarContractValue::FloatRange(_)) => Some(RetainedClause::FloatRange),
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
                        if let TypeConstraintNode::Domain(domain) = constraint
                            && let checked_trees::types::DomainConstraintSubject::Value(domain) =
                                domain.subject
                        {
                            let expected = primitive
                                .and_then(|primitive_type| {
                                    validation::scalar_value_domain_predicate(
                                        domain,
                                        CheckedScalarExpression::Parameter {
                                            position,
                                            primitive_type,
                                        },
                                    )
                                })
                                .ok_or(LoweringError::Unsupported(
                                    "scalar value domain has no predicate meaning",
                                ))?;
                            if !matches!(retained.next(), Some(Some(RetainedClause::Predicate(actual))) if *actual == expected)
                            {
                                return unsupported(
                                    "scalar entry value domain lost its exact predicate",
                                );
                            }
                            continue;
                        }
                        // An exactly-interval domain on an integer carrier
                        // retains the same closed entry range a bracketed
                        // range does; any other domain carries no range row.
                        let exact_domain = match constraint {
                            TypeConstraintNode::Domain(domain) => {
                                let Some(bounds) = primitive
                                    .filter(|primitive| integer_scalar_type(*primitive).is_ok())
                                    .and_then(|primitive| {
                                        validation::exact_declared_domain_carrier_interval(
                                            &checked.typed,
                                            primitive,
                                            domain,
                                        )
                                        .filter(|(minimum, maximum)| minimum <= maximum)
                                    })
                                else {
                                    continue;
                                };
                                Some(bounds)
                            }
                            TypeConstraintNode::Range { .. } => None,
                            _ => continue,
                        };
                        let primitive = primitive.ok_or(LoweringError::Unsupported(
                            "scalar range has no primitive carrier",
                        ))?;
                        if let TypeConstraintNode::Range {
                            minimum, maximum, ..
                        } = constraint
                            && matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64)
                        {
                            // A floating range is a `FloatRange` clause in
                            // the requires tail: consume that row; its
                            // authored source bounds must resolve at the
                            // declared carrier in IEEE order.
                            let Some(RetainedClause::FloatRange) = retained.next().flatten() else {
                                return unsupported(
                                    "scalar float entry range lost its requires clause",
                                );
                            };
                            if validation::closed_float_range_endpoint(
                                &checked.typed,
                                *minimum,
                                primitive,
                            )
                            .zip(validation::closed_float_range_endpoint(
                                &checked.typed,
                                *maximum,
                                primitive,
                            ))
                            .filter(|(low, high)| validation::ieee_float_range_ordered(*low, *high))
                            .is_none()
                            {
                                return unsupported(
                                    "scalar float entry range has unavailable or unordered source bounds",
                                );
                            }
                            // The retained roster delivers this clause's
                            // endpoint evidence.
                            float_ranges.as_mut().and_then(Iterator::next).ok_or(
                                LoweringError::Unsupported(
                                    "scalar float entry range lost its retained endpoint evidence",
                                ),
                            )?;
                            continue;
                        }
                        let (minimum_value, maximum_value) = match (exact_domain, constraint) {
                            (Some(bounds), _) => bounds,
                            (
                                None,
                                TypeConstraintNode::Range {
                                    minimum,
                                    maximum,
                                    end_inclusive,
                                },
                            ) => validation::closed_integer_range_bound(&checked.typed, *minimum)
                                .zip(validation::closed_integer_range_maximum(
                                    &checked.typed,
                                    *maximum,
                                    *end_inclusive,
                                ))
                                .filter(|(low, high)| low <= high)
                                .ok_or(LoweringError::Unsupported(
                                    "scalar entry range has unavailable or empty source bounds",
                                ))?,
                            (None, _) => {
                                return unsupported("scalar entry range lost its constraint");
                            }
                        };
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
                        // The retained integer roster delivers this
                        // interval's endpoint evidence.
                        if let Some(roster) = integer_ranges.as_mut() {
                            roster.next().ok_or(LoweringError::Unsupported(
                                "scalar integer entry range lost its retained endpoint evidence",
                            ))?;
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
