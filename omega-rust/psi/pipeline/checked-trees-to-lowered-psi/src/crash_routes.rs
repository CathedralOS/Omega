//! Crash-route, structural requirement, and canonical proposition lowering.

use super::*;

pub(super) fn lower_checked_crash_frontier(
    frontier: &[PermissionClaimIdentity],
    source_claims: &[(PermissionClaimIdentity, ClaimId)],
) -> Result<Vec<ClaimId>, LoweringError> {
    let mut lowered = frontier
        .iter()
        .map(|identity| {
            source_claims
                .iter()
                .find_map(|(source, claim)| (source == identity).then_some(*claim))
                .ok_or(LoweringError::CrashFrontierClaimNotLowered(*identity))
        })
        .collect::<Result<Vec<_>, _>>()?;
    lowered.sort();
    lowered.dedup();
    Ok(lowered)
}

pub(super) fn lower_checked_crash_routes(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
) -> Result<Vec<checked_trees::CrashRouteBucket>, LoweringError> {
    checked
        .facts
        .contract_plans
        .for_machine(machine)
        .map(|contract| {
            contract
                .crash
                .published()
                .iter()
                .map(|bucket| {
                    if bucket.alternative_guards().iter().any(|guard| {
                        matches!(guard, checked_trees::CrashRouteGuard::Predicate(predicate)
                            if predicate.scalar_expression().is_none())
                    }) {
                        return unsupported(
                            "guarded crash route is outside structured scalar predicate lowering",
                        );
                    }
                    Ok(bucket.clone())
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .unwrap_or_else(|| Ok(Vec::new()))
}

pub(super) fn lower_checked_crash_exit(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement_ordinal: u32,
    source_claims: &[(PermissionClaimIdentity, ClaimId)],
) -> Result<LoweredCrashExit, LoweringError> {
    use checked_trees::statement::{
        StatementNode, TransitionExit, TransitionGuardNode, TransitionTargetNode,
    };

    let program = &checked.typed;
    let source_state = program
        .machines()
        .iter()
        .find(|candidate| candidate.symbol == machine && machine.is_valid())
        .and_then(|source_machine| {
            program
                .machine_states(source_machine)
                .iter()
                .find(|candidate| candidate.symbol == state && state.is_valid())
        })
        .ok_or(LoweringError::Unsupported(
            "explicit crash has no matching authored machine and state",
        ))?;
    let Some(StatementNode::Transition(transition)) = program
        .statement_table
        .statements(source_state.statement_nodes)
        .get(statement_ordinal as usize)
    else {
        return unsupported("explicit crash has no matching authored transition statement");
    };
    let TransitionExit::Crash(authored_cause) = transition.exit else {
        return unsupported("checked crash site names an ordinary authored transition");
    };
    // A nonzero stale target resolves through ZII to the dummy Terminal node.
    // Require arena membership before reading that node's semantic shape.
    if !program
        .statement_table
        .transition_target_is_valid(transition.target)
        || !matches!(
            program.statement_table.transition_target(transition.target),
            TransitionTargetNode::Terminal
        )
        || transition.continuation.is_valid()
        || transition.guard != TransitionGuardNode::Always
    {
        return unsupported(
            "explicit crash must retain an unconditional terminal target and no continuation",
        );
    }
    let Some(crash_plan) = checked
        .facts
        .contract_plans
        .for_machine(machine)
        .map(|contract| &contract.crash)
    else {
        return unsupported("explicit crash has no checked machine-contract plan");
    };
    let Some(checked_site) = crash_plan.checked_site_at(state, statement_ordinal) else {
        return unsupported("explicit crash has no body-derived checked crash-site row");
    };
    let authored_cause = match authored_cause {
        checked_trees::signature::CrashCause::Trap => checked_trees::CrashCause::Trap,
        checked_trees::signature::CrashCause::Abort => checked_trees::CrashCause::Abort,
    };
    if checked_site.cause() != authored_cause {
        return unsupported("checked crash cause disagrees with its authored transition");
    }
    let matching_contracts = crash_plan
        .covering_buckets_for_site(checked_site)
        .map(|(_, bucket)| bucket)
        .collect::<Vec<_>>();
    let [covering_bucket] = matching_contracts.as_slice() else {
        return unsupported(
            "an explicit crash in the terminal-Psi source slice requires exactly one prechecked covering route bucket",
        );
    };
    let site_identities = checked_site
        .path_guard_conjuncts()
        .iter()
        .chain(checked_site.path_guard_consequences())
        .collect::<BTreeSet<_>>();
    let site_guard = covering_bucket
        .alternative_guards()
        .iter()
        .filter_map(|guard| match guard {
            checked_trees::CrashRouteGuard::Truth => None,
            checked_trees::CrashRouteGuard::Predicate(predicate)
                if site_identities.contains(predicate) =>
            {
                Some(
                    predicate
                        .scalar_expression()
                        .cloned()
                        .ok_or(LoweringError::Unsupported(
                            "guarded crash site is outside structured scalar predicate lowering",
                        )),
                )
            }
            checked_trees::CrashRouteGuard::Predicate(_) => None,
        })
        .collect::<Result<Vec<_>, _>>()?;
    if !covering_bucket
        .alternative_guards()
        .contains(&checked_trees::CrashRouteGuard::Truth)
        && site_guard.is_empty()
    {
        return unsupported("guarded crash site has no structured covering predicate");
    }
    Ok(LoweredCrashExit {
        cause: match checked_site.cause() {
            checked_trees::CrashCause::Trap => TerminalCrashCause::Trap,
            checked_trees::CrashCause::Abort => TerminalCrashCause::Abort,
        },
        site_guard,
        frontier_lower_bound: lower_checked_crash_frontier(
            checked_site.frontier_lower_bound(),
            source_claims,
        )?,
    })
}

pub(super) fn lower_checked_crash_route_buckets(
    buckets: &[checked_trees::CrashRouteBucket],
    parameters: &[ValueDeclaration],
) -> Result<Vec<terminal_psi::CrashRouteBucket>, LoweringError> {
    buckets
        .iter()
        .map(|bucket| {
            let mut alternatives = bucket
                .alternative_guards()
                .iter()
                .map(|guard| match guard {
                    checked_trees::CrashRouteGuard::Truth => {
                        Ok(terminal_psi::CrashRouteGuard::Truth)
                    }
                    checked_trees::CrashRouteGuard::Predicate(predicate) => {
                        let expression = predicate.scalar_expression().ok_or(
                            LoweringError::Unsupported(
                                "guarded crash route is outside structured scalar predicate lowering",
                            ),
                        )?;
                        Ok(terminal_psi::CrashRouteGuard::Predicate(
                            terminal_psi::CrashPredicateTerm::new(
                                checked_boolean_proposition(expression, parameters)?,
                            ),
                        ))
                    }
                })
                .collect::<Result<Vec<_>, _>>()?;
            alternatives.sort();
            alternatives.dedup();
            Ok(terminal_psi::CrashRouteBucket {
                cause: match bucket.cause() {
                    checked_trees::CrashCause::Trap => TerminalCrashCause::Trap,
                    checked_trees::CrashCause::Abort => TerminalCrashCause::Abort,
                },
                alternatives,
            })
        })
        .collect()
}

pub(crate) fn lower_structural_member_path(
    parameter_position: u32,
    path: &[checked_trees::CheckedStructuralPredicatePathSegment],
    parameters: &[StructuralParameterDeclaration],
    structural_types: &[StructuralTypeDeclaration],
) -> Result<
    (
        PlaceId,
        Vec<CanonicalStructuralPathSegment>,
        StructuralFieldType,
    ),
    LoweringError,
> {
    if path.is_empty() {
        return unsupported("structural scalar contract has an empty member path");
    }
    let parameter = parameters
        .iter()
        .find(|parameter| parameter.position == parameter_position)
        .ok_or(LoweringError::Unsupported(
            "structural scalar contract names a non-structural parameter",
        ))?;
    let mut structural_type = parameter.structural_type;
    let mut terminal_path = Vec::with_capacity(path.len());
    let mut selected_case_fields = None;
    for (index, segment) in path.iter().enumerate() {
        let declaration = structural_types
            .iter()
            .find(|declaration| declaration.id == structural_type)
            .ok_or(LoweringError::Unsupported(
                "structural scalar contract path type is absent",
            ))?;
        if let checked_trees::CheckedStructuralPredicatePathSegment::Case(identity) = segment {
            if selected_case_fields.is_some() || index + 1 == path.len() {
                return unsupported("structural scalar contract has a malformed case path");
            }
            let cases = match &declaration.shape {
                StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } => {
                    cases
                }
                _ => {
                    return unsupported("structural scalar contract case receiver is not a sum");
                }
            };
            let case = cases
                .iter()
                .find(|candidate| candidate.identity == *identity)
                .ok_or(LoweringError::Unsupported(
                    "structural scalar contract case is absent",
                ))?;
            terminal_path.push(CanonicalStructuralPathSegment::Case(case.id));
            selected_case_fields = Some(&case.fields);
            continue;
        }
        let checked_trees::CheckedStructuralPredicatePathSegment::Field(identity) = segment else {
            unreachable!("case path handled above")
        };
        let fields = if let Some(fields) = selected_case_fields.take() {
            fields
        } else {
            match &declaration.shape {
                StructuralTypeShape::Record { fields }
                | StructuralTypeShape::Mixed { fields, .. } => fields,
                _ => {
                    return unsupported(
                        "structural scalar contract field receiver is not a record",
                    );
                }
            }
        };
        let field = fields
            .iter()
            .find(|candidate| candidate.identity == *identity)
            .filter(|field| !field.relevance.is_erased())
            .ok_or(LoweringError::Unsupported(
                "structural scalar contract path field is absent or erased",
            ))?;
        terminal_path.push(CanonicalStructuralPathSegment::Field(field.id));
        let is_last = index + 1 == path.len();
        match (&field.field_type, is_last) {
            (StructuralFieldType::Structural(next), false) => structural_type = *next,
            (_, true) => return Ok((parameter.place, terminal_path, field.field_type.clone())),
            _ => {
                return unsupported(
                    "structural scalar contract path does not end at a retained leaf",
                );
            }
        }
    }
    unreachable!("nonempty structural path returns at its final field")
}

fn lower_structural_member_term(
    parameter_position: u32,
    path: &[checked_trees::CheckedStructuralPredicatePathSegment],
    expected: ScalarType,
    parameters: &[StructuralParameterDeclaration],
    structural_types: &[StructuralTypeDeclaration],
) -> Result<ScalarTerm, LoweringError> {
    let (root, terminal_path, actual) =
        lower_structural_member_path(parameter_position, path, parameters, structural_types)?;
    if actual != StructuralFieldType::Scalar(expected) {
        return unsupported(
            "structural scalar contract path does not end at the retained scalar type",
        );
    }
    Ok(match expected {
        ScalarType::Boolean => ScalarTerm::boolean_field_path(root, terminal_path),
        ScalarType::Integer(integer_type) => {
            ScalarTerm::integer_field_path(root, terminal_path, integer_type)
        }
        ScalarType::IeeeFloat(_) => {
            return unsupported(
                "generic scalar crash terms do not carry IEEE float structural fields",
            );
        }
    })
}

fn lower_ieee_float_field(
    field: &checked_trees::CheckedStructuralParameterField,
    format: IeeeFloatFormat,
    parameters: &[StructuralParameterDeclaration],
    structural_types: &[StructuralTypeDeclaration],
) -> Result<IeeeFloatStructuralField, LoweringError> {
    let (root, path, actual) = lower_structural_member_path(
        field.parameter_position,
        &field.path,
        parameters,
        structural_types,
    )?;
    if actual != StructuralFieldType::IeeeFloat(format) {
        return unsupported("structural IEEE predicate leaf has the wrong retained format");
    }
    IeeeFloatStructuralField::new(root, path).map_err(LoweringError::InvalidCrashPredicate)
}

fn lower_byte_sequence_field(
    field: &checked_trees::CheckedStructuralParameterField,
    parameters: &[StructuralParameterDeclaration],
    structural_types: &[StructuralTypeDeclaration],
) -> Result<ByteSequenceStructuralField, LoweringError> {
    let (root, path, actual) = lower_structural_member_path(
        field.parameter_position,
        &field.path,
        parameters,
        structural_types,
    )?;
    if !matches!(actual, StructuralFieldType::ByteSequence(_)) {
        return unsupported("structural byte-sequence predicate leaf has the wrong retained type");
    }
    ByteSequenceStructuralField::new(root, path).map_err(LoweringError::InvalidCrashPredicate)
}

fn lower_structural_sum_subject(
    subject: &checked_trees::CheckedStructuralParameterField,
    parameters: &[StructuralParameterDeclaration],
    structural_types: &[StructuralTypeDeclaration],
) -> Result<(StructuralCaseSubject, StructuralTypeId), LoweringError> {
    let parameter = parameters
        .iter()
        .find(|parameter| parameter.position == subject.parameter_position)
        .ok_or(LoweringError::Unsupported(
            "structural sum predicate names a non-structural parameter",
        ))?;
    let mut structural_type = parameter.structural_type;
    let mut terminal_path = Vec::with_capacity(subject.path.len());
    let mut selected_case_fields = None;
    for segment in &subject.path {
        let declaration = structural_types
            .iter()
            .find(|declaration| declaration.id == structural_type)
            .ok_or(LoweringError::Unsupported(
                "structural sum predicate path type is absent",
            ))?;
        if let checked_trees::CheckedStructuralPredicatePathSegment::Case(identity) = segment {
            if selected_case_fields.is_some() {
                return unsupported("structural sum predicate has adjacent case selections");
            }
            let cases = match &declaration.shape {
                StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } => {
                    cases
                }
                _ => return unsupported("structural sum predicate case receiver is not a sum"),
            };
            let case = cases
                .iter()
                .find(|candidate| candidate.identity == *identity)
                .ok_or(LoweringError::Unsupported(
                    "structural sum predicate case is absent",
                ))?;
            terminal_path.push(CanonicalStructuralPathSegment::Case(case.id));
            selected_case_fields = Some(&case.fields);
            continue;
        }
        let checked_trees::CheckedStructuralPredicatePathSegment::Field(identity) = segment else {
            unreachable!("case path handled above")
        };
        let fields = if let Some(fields) = selected_case_fields.take() {
            fields
        } else {
            match &declaration.shape {
                StructuralTypeShape::Record { fields }
                | StructuralTypeShape::Mixed { fields, .. } => fields,
                _ => {
                    return unsupported("structural sum predicate path receiver is not a record");
                }
            }
        };
        let field = fields
            .iter()
            .find(|candidate| candidate.identity == *identity)
            .filter(|field| !field.relevance.is_erased())
            .ok_or(LoweringError::Unsupported(
                "structural sum predicate path field is absent or erased",
            ))?;
        let StructuralFieldType::Structural(next) = field.field_type else {
            return unsupported("structural sum predicate path does not reach a structural value");
        };
        terminal_path.push(CanonicalStructuralPathSegment::Field(field.id));
        structural_type = next;
    }
    if selected_case_fields.is_some() {
        return unsupported("structural sum predicate case selection has no payload field");
    }
    if !matches!(
        structural_types
            .iter()
            .find(|declaration| declaration.id == structural_type)
            .map(|declaration| &declaration.shape),
        Some(StructuralTypeShape::Sum { .. } | StructuralTypeShape::Mixed { .. })
    ) {
        return unsupported("structural sum predicate subject is not a sum");
    }
    Ok((
        StructuralCaseSubject::new(parameter.place, terminal_path),
        structural_type,
    ))
}

pub(super) fn lower_structural_runtime_requirement(
    expression: &CheckedBooleanExpression,
    parameters: &[StructuralParameterDeclaration],
    structural_types: &[StructuralTypeDeclaration],
) -> Result<Proposition, LoweringError> {
    fn integer_term(
        expression: &CheckedScalarExpression,
        parameters: &[StructuralParameterDeclaration],
        structural_types: &[StructuralTypeDeclaration],
    ) -> Result<ScalarTerm, LoweringError> {
        match expression {
            CheckedScalarExpression::StructuralParameterField {
                parameter_position,
                path,
                primitive_type,
            } => {
                let ScalarType::Integer(integer_type) = integer_scalar_type(*primitive_type)?
                else {
                    return unsupported("structural runtime requirement member is not an integer");
                };
                lower_structural_member_term(
                    *parameter_position,
                    path,
                    ScalarType::Integer(integer_type),
                    parameters,
                    structural_types,
                )
            }
            CheckedScalarExpression::IntegerLiteral { literal } => {
                let scalar_type = integer_landing_scalar_type(literal)?;
                let ScalarType::Integer(integer_type) = scalar_type else {
                    return unsupported("structural runtime requirement literal is not an integer");
                };
                ScalarTerm::integer(integer_type, integer_value(literal, scalar_type)?)
                    .map_err(LoweringError::InvalidCrashPredicate)
            }
            _ => unsupported(
                "structural runtime requirements currently admit only integer members and literals",
            ),
        }
    }

    let CheckedBooleanExpression::IntegerComparison { kind, left, right } = expression else {
        return unsupported(
            "structural runtime divisor evidence must be an integer comparison requirement",
        );
    };
    let left = integer_term(left, parameters, structural_types)?;
    let right = integer_term(right, parameters, structural_types)?;
    match kind {
        CheckedIntegerComparisonKind::Equal => Ok(Proposition::Equal(left, right)),
        CheckedIntegerComparisonKind::LessThan => Ok(Proposition::LessThan(left, right)),
        CheckedIntegerComparisonKind::LessOrEqual => Ok(Proposition::LessOrEqual(left, right)),
    }
}

fn safe_exact_structural_divisor(
    integer_type: IntegerType,
    dividend: &ScalarTerm,
    divisor: &ScalarTerm,
    requirements: &[Proposition],
) -> bool {
    match divisor {
        ScalarTerm::Integer {
            scalar_type,
            value: IntegerValue::Unsigned(value),
        } => return *scalar_type == integer_type && *value != 0,
        ScalarTerm::Integer {
            scalar_type,
            value: IntegerValue::Signed(value),
        } => return *scalar_type == integer_type && *value != 0 && *value != -1,
        _ => {}
    }

    let one = match integer_type.sign() {
        IntegerSign::Unsigned => IntegerValue::Unsigned(1),
        IntegerSign::Signed => IntegerValue::Signed(1),
    };
    if let Ok(one) = ScalarTerm::integer(integer_type, one)
        && requirements.contains(&Proposition::LessOrEqual(one, divisor.clone()))
    {
        return true;
    }
    if integer_type.sign() != IntegerSign::Signed {
        return false;
    }
    if let Ok(negative_two) = ScalarTerm::integer(integer_type, IntegerValue::Signed(-2))
        && requirements.contains(&Proposition::LessOrEqual(divisor.clone(), negative_two))
    {
        return true;
    }
    let Ok(negative_one) = ScalarTerm::integer(integer_type, IntegerValue::Signed(-1)) else {
        return false;
    };
    if !requirements.contains(&Proposition::LessOrEqual(divisor.clone(), negative_one)) {
        return false;
    }
    let IntegerValue::Signed(minimum) = integer_type.minimum_value() else {
        unreachable!("signed fixed integer has a signed minimum")
    };
    ScalarTerm::integer(
        integer_type,
        IntegerValue::Signed(minimum.checked_add(1).expect("minimum has a successor")),
    )
    .is_ok_and(|minimum_plus_one| {
        requirements.contains(&Proposition::LessOrEqual(
            minimum_plus_one,
            dividend.clone(),
        ))
    })
}

fn safe_policy_structural_divisor(
    integer_type: IntegerType,
    divisor: &ScalarTerm,
    requirements: &[Proposition],
) -> bool {
    match divisor {
        ScalarTerm::Integer {
            scalar_type,
            value: IntegerValue::Unsigned(value),
        } => return *scalar_type == integer_type && *value != 0,
        ScalarTerm::Integer {
            scalar_type,
            value: IntegerValue::Signed(value),
        } => return *scalar_type == integer_type && *value != 0,
        _ => {}
    }

    let one = match integer_type.sign() {
        IntegerSign::Unsigned => IntegerValue::Unsigned(1),
        IntegerSign::Signed => IntegerValue::Signed(1),
    };
    if ScalarTerm::integer(integer_type, one)
        .is_ok_and(|one| requirements.contains(&Proposition::LessOrEqual(one, divisor.clone())))
    {
        return true;
    }
    if integer_type.sign() != IntegerSign::Signed {
        return false;
    }
    [IntegerValue::Signed(-1), IntegerValue::Signed(-2)]
        .into_iter()
        .filter_map(|bound| ScalarTerm::integer(integer_type, bound).ok())
        .any(|bound| requirements.contains(&Proposition::LessOrEqual(divisor.clone(), bound)))
}

fn nonnegative_shift_count(value: IntegerValue) -> Option<u32> {
    match value {
        IntegerValue::Unsigned(value) => u32::try_from(value).ok(),
        IntegerValue::Signed(value) => u32::try_from(value).ok(),
    }
}

fn exact_structural_shift_maximum_count(
    value_type: IntegerType,
    count_type: IntegerType,
    count: &ScalarTerm,
    requirements: &[Proposition],
) -> Option<u32> {
    if count.scalar_type() != ScalarType::Integer(count_type) {
        return None;
    }
    if let Some((literal_type, literal)) = count.integer_value() {
        let literal = nonnegative_shift_count(literal)?;
        return (literal_type == count_type && literal < u32::from(value_type.bits()))
            .then_some(literal);
    }

    if count_type.sign() == IntegerSign::Signed {
        let zero = ScalarTerm::integer(count_type, IntegerValue::Signed(0)).ok()?;
        if !requirements.contains(&Proposition::LessOrEqual(zero, count.clone())) {
            return None;
        }
    }

    let width = u32::from(value_type.bits());
    let intrinsic_maximum = nonnegative_shift_count(count_type.maximum_value())?;
    if intrinsic_maximum < width {
        return Some(intrinsic_maximum);
    }
    requirements
        .iter()
        .filter_map(|requirement| match requirement {
            Proposition::LessOrEqual(left, right) if left == count => {
                let (right_type, right) = right.integer_value()?;
                let right = nonnegative_shift_count(right)?;
                (right_type == count_type && right < width).then_some(right)
            }
            Proposition::LessThan(left, right) if left == count => {
                let (right_type, right) = right.integer_value()?;
                let right = nonnegative_shift_count(right)?;
                (right_type == count_type && right > 0 && right <= width).then_some(right - 1)
            }
            _ => None,
        })
        .min()
}

fn safe_exact_structural_shift(
    left_shift: bool,
    value_type: IntegerType,
    count_type: IntegerType,
    value: &ScalarTerm,
    count: &ScalarTerm,
    requirements: &[Proposition],
) -> bool {
    if value.scalar_type() != ScalarType::Integer(value_type) {
        return false;
    }
    let Some(maximum_count) =
        exact_structural_shift_maximum_count(value_type, count_type, count, requirements)
    else {
        return false;
    };
    if !left_shift || maximum_count == 0 {
        return true;
    }
    if let Some((literal_type, literal)) = value.integer_value() {
        let maximum_count_value = match count_type.sign() {
            IntegerSign::Signed => IntegerValue::Signed(i128::from(maximum_count)),
            IntegerSign::Unsigned => IntegerValue::Unsigned(u128::from(maximum_count)),
        };
        return literal_type == value_type
            && value_type
                .exact_shift_left(literal, count_type, maximum_count_value)
                .is_some();
    }
    match value_type.sign() {
        IntegerSign::Unsigned => {
            let IntegerValue::Unsigned(maximum) = value_type.maximum_value() else {
                unreachable!("unsigned fixed integer has an unsigned maximum")
            };
            ScalarTerm::integer(value_type, IntegerValue::Unsigned(maximum >> maximum_count))
                .is_ok_and(|maximum| {
                    requirements.contains(&Proposition::LessOrEqual(value.clone(), maximum))
                })
        }
        IntegerSign::Signed => {
            let (IntegerValue::Signed(minimum), IntegerValue::Signed(maximum)) =
                (value_type.minimum_value(), value_type.maximum_value())
            else {
                unreachable!("signed fixed integer has signed bounds")
            };
            let minimum =
                ScalarTerm::integer(value_type, IntegerValue::Signed(minimum >> maximum_count));
            let maximum =
                ScalarTerm::integer(value_type, IntegerValue::Signed(maximum >> maximum_count));
            minimum.is_ok_and(|minimum| {
                requirements.contains(&Proposition::LessOrEqual(minimum, value.clone()))
            }) && maximum.is_ok_and(|maximum| {
                requirements.contains(&Proposition::LessOrEqual(value.clone(), maximum))
            })
        }
    }
}

pub(super) fn lower_structural_crash_route_buckets(
    buckets: &[checked_trees::CrashRouteBucket],
    parameters: &[StructuralParameterDeclaration],
    structural_types: &[StructuralTypeDeclaration],
    runtime_requirements: &[Proposition],
) -> Result<Vec<terminal_psi::CrashRouteBucket>, LoweringError> {
    fn checked_member_path(
        expression: &checked_trees::CrashPredicateExpression,
        path: &mut Vec<String>,
    ) -> Option<u32> {
        match expression {
            checked_trees::CrashPredicateExpression::Parameter(position) => Some(*position),
            checked_trees::CrashPredicateExpression::Member { receiver, member } => {
                let parameter = checked_member_path(receiver, path)?;
                path.push(member.clone());
                Some(parameter)
            }
            _ => None,
        }
    }

    fn lower_term(
        expression: &CheckedBooleanExpression,
        parameters: &[StructuralParameterDeclaration],
        structural_types: &[StructuralTypeDeclaration],
        runtime_requirements: &[Proposition],
    ) -> Result<ScalarTerm, LoweringError> {
        fn lower_integer_term(
            expression: &CheckedScalarExpression,
            parameters: &[StructuralParameterDeclaration],
            structural_types: &[StructuralTypeDeclaration],
            runtime_requirements: &[Proposition],
        ) -> Result<ScalarTerm, LoweringError> {
            match expression {
                CheckedScalarExpression::StructuralParameterField {
                    parameter_position,
                    path,
                    primitive_type,
                } => {
                    let ScalarType::Integer(integer_type) = integer_scalar_type(*primitive_type)?
                    else {
                        return unsupported(
                            "structural crash integer member has a non-integer type",
                        );
                    };
                    lower_structural_member_term(
                        *parameter_position,
                        path,
                        ScalarType::Integer(integer_type),
                        parameters,
                        structural_types,
                    )
                }
                CheckedScalarExpression::IntegerLiteral { literal } => {
                    let scalar_type = integer_landing_scalar_type(literal)?;
                    let ScalarType::Integer(integer_type) = scalar_type else {
                        return unsupported(
                            "structural crash integer literal is not fixed-integer",
                        );
                    };
                    ScalarTerm::integer(integer_type, integer_value(literal, scalar_type)?)
                        .map_err(LoweringError::InvalidCrashPredicate)
                }
                CheckedScalarExpression::IntegerBitwiseNot {
                    primitive_type,
                    operand,
                } => {
                    let ScalarType::Integer(integer_type) = integer_scalar_type(*primitive_type)?
                    else {
                        return unsupported("structural crash bitwise-not has a non-integer type");
                    };
                    let operand = lower_integer_term(
                        operand,
                        parameters,
                        structural_types,
                        runtime_requirements,
                    )?;
                    ScalarTerm::integer_bitwise_not(integer_type, operand)
                        .map_err(LoweringError::InvalidCrashPredicate)
                }
                CheckedScalarExpression::IntegerBinary {
                    kind,
                    primitive_type,
                    left,
                    right,
                } if matches!(
                    kind,
                    CheckedIntegerBinaryKind::BitwiseAnd
                        | CheckedIntegerBinaryKind::BitwiseOr
                        | CheckedIntegerBinaryKind::BitwiseXor
                ) =>
                {
                    let ScalarType::Integer(integer_type) = integer_scalar_type(*primitive_type)?
                    else {
                        return unsupported(
                            "structural crash bitwise expression has a non-integer type",
                        );
                    };
                    let left = lower_integer_term(
                        left,
                        parameters,
                        structural_types,
                        runtime_requirements,
                    )?;
                    let right = lower_integer_term(
                        right,
                        parameters,
                        structural_types,
                        runtime_requirements,
                    )?;
                    match kind {
                        CheckedIntegerBinaryKind::BitwiseAnd => {
                            ScalarTerm::integer_bitwise_and(integer_type, left, right)
                        }
                        CheckedIntegerBinaryKind::BitwiseOr => {
                            ScalarTerm::integer_bitwise_or(integer_type, left, right)
                        }
                        CheckedIntegerBinaryKind::BitwiseXor => {
                            ScalarTerm::integer_bitwise_xor(integer_type, left, right)
                        }
                        _ => unreachable!("guarded bitwise kind"),
                    }
                    .map_err(LoweringError::InvalidCrashPredicate)
                }
                CheckedScalarExpression::IntegerBinary {
                    kind,
                    primitive_type,
                    left,
                    right,
                } if matches!(
                    kind,
                    CheckedIntegerBinaryKind::WrappingShiftLeft
                        | CheckedIntegerBinaryKind::WrappingShiftRight
                        | CheckedIntegerBinaryKind::ExactShiftLeft
                        | CheckedIntegerBinaryKind::ExactShiftRight
                ) =>
                {
                    let ScalarType::Integer(value_type) = integer_scalar_type(*primitive_type)?
                    else {
                        return unsupported("structural crash shift has a non-integer value type");
                    };
                    let value = lower_integer_term(
                        left,
                        parameters,
                        structural_types,
                        runtime_requirements,
                    )?;
                    let count = lower_integer_term(
                        right,
                        parameters,
                        structural_types,
                        runtime_requirements,
                    )?;
                    if value.scalar_type() != ScalarType::Integer(value_type) {
                        return unsupported(
                            "structural crash shift value does not match its integer type",
                        );
                    }
                    let ScalarType::Integer(count_type) = count.scalar_type() else {
                        return unsupported("structural crash shift count is not an integer");
                    };
                    if matches!(
                        kind,
                        CheckedIntegerBinaryKind::ExactShiftLeft
                            | CheckedIntegerBinaryKind::ExactShiftRight
                    ) && !safe_exact_structural_shift(
                        matches!(kind, CheckedIntegerBinaryKind::ExactShiftLeft),
                        value_type,
                        count_type,
                        &value,
                        &count,
                        runtime_requirements,
                    ) {
                        return unsupported(
                            "structural crash Exact shift requires explicit terminal count and overflow safety evidence",
                        );
                    }
                    match kind {
                        CheckedIntegerBinaryKind::WrappingShiftLeft => {
                            ScalarTerm::wrapping_integer_shift_left(
                                value_type, count_type, value, count,
                            )
                        }
                        CheckedIntegerBinaryKind::WrappingShiftRight => {
                            ScalarTerm::wrapping_integer_shift_right(
                                value_type, count_type, value, count,
                            )
                        }
                        CheckedIntegerBinaryKind::ExactShiftLeft => {
                            ScalarTerm::exact_integer_shift_left(
                                value_type, count_type, value, count,
                            )
                        }
                        CheckedIntegerBinaryKind::ExactShiftRight => {
                            ScalarTerm::exact_integer_shift_right(
                                value_type, count_type, value, count,
                            )
                        }
                        _ => unreachable!("guarded structural shift kind"),
                    }
                    .map_err(LoweringError::InvalidCrashPredicate)
                }
                CheckedScalarExpression::IntegerBinary {
                    kind,
                    primitive_type,
                    left,
                    right,
                } if matches!(
                    kind,
                    CheckedIntegerBinaryKind::ExactAdd
                        | CheckedIntegerBinaryKind::ExactSubtract
                        | CheckedIntegerBinaryKind::ExactMultiply
                        | CheckedIntegerBinaryKind::ExactDivide
                        | CheckedIntegerBinaryKind::ExactRemainder
                        | CheckedIntegerBinaryKind::WrappingAdd
                        | CheckedIntegerBinaryKind::SaturatingAdd
                        | CheckedIntegerBinaryKind::WrappingSubtract
                        | CheckedIntegerBinaryKind::SaturatingSubtract
                        | CheckedIntegerBinaryKind::WrappingMultiply
                        | CheckedIntegerBinaryKind::SaturatingMultiply
                        | CheckedIntegerBinaryKind::WrappingDivide
                        | CheckedIntegerBinaryKind::WrappingRemainder
                        | CheckedIntegerBinaryKind::SaturatingDivide
                        | CheckedIntegerBinaryKind::SaturatingRemainder
                ) =>
                {
                    let ScalarType::Integer(integer_type) = integer_scalar_type(*primitive_type)?
                    else {
                        return unsupported("structural crash arithmetic has a non-integer type");
                    };
                    let left = Box::new(lower_integer_term(
                        left,
                        parameters,
                        structural_types,
                        runtime_requirements,
                    )?);
                    let right = Box::new(lower_integer_term(
                        right,
                        parameters,
                        structural_types,
                        runtime_requirements,
                    )?);
                    if left.scalar_type() != ScalarType::Integer(integer_type)
                        || right.scalar_type() != ScalarType::Integer(integer_type)
                    {
                        return unsupported(
                            "structural crash arithmetic operands do not match its integer type",
                        );
                    }
                    if matches!(
                        kind,
                        CheckedIntegerBinaryKind::ExactDivide
                            | CheckedIntegerBinaryKind::ExactRemainder
                    ) && !safe_exact_structural_divisor(
                        integer_type,
                        &left,
                        &right,
                        runtime_requirements,
                    ) {
                        return unsupported(
                            "structural crash exact division requires explicit terminal divisor safety evidence",
                        );
                    }
                    if matches!(
                        kind,
                        CheckedIntegerBinaryKind::WrappingDivide
                            | CheckedIntegerBinaryKind::WrappingRemainder
                            | CheckedIntegerBinaryKind::SaturatingDivide
                            | CheckedIntegerBinaryKind::SaturatingRemainder
                    ) && !safe_policy_structural_divisor(
                        integer_type,
                        &right,
                        runtime_requirements,
                    ) {
                        return unsupported(
                            "structural crash policy division requires explicit terminal nonzero-divisor evidence",
                        );
                    }
                    Ok(match kind {
                        CheckedIntegerBinaryKind::ExactAdd => ScalarTerm::ExactIntegerAdd {
                            scalar_type: integer_type,
                            left,
                            right,
                        },
                        CheckedIntegerBinaryKind::ExactSubtract => {
                            ScalarTerm::ExactIntegerSubtract {
                                scalar_type: integer_type,
                                left,
                                right,
                            }
                        }
                        CheckedIntegerBinaryKind::ExactMultiply => {
                            ScalarTerm::ExactIntegerMultiply {
                                scalar_type: integer_type,
                                left,
                                right,
                            }
                        }
                        CheckedIntegerBinaryKind::ExactDivide => ScalarTerm::ExactIntegerDivide {
                            scalar_type: integer_type,
                            left,
                            right,
                        },
                        CheckedIntegerBinaryKind::ExactRemainder => {
                            ScalarTerm::ExactIntegerRemainder {
                                scalar_type: integer_type,
                                left,
                                right,
                            }
                        }
                        CheckedIntegerBinaryKind::WrappingAdd => ScalarTerm::WrappingIntegerAdd {
                            scalar_type: integer_type,
                            left,
                            right,
                        },
                        CheckedIntegerBinaryKind::SaturatingAdd => {
                            ScalarTerm::SaturatingIntegerAdd {
                                scalar_type: integer_type,
                                left,
                                right,
                            }
                        }
                        CheckedIntegerBinaryKind::WrappingSubtract => {
                            ScalarTerm::WrappingIntegerSubtract {
                                scalar_type: integer_type,
                                left,
                                right,
                            }
                        }
                        CheckedIntegerBinaryKind::SaturatingSubtract => {
                            ScalarTerm::SaturatingIntegerSubtract {
                                scalar_type: integer_type,
                                left,
                                right,
                            }
                        }
                        CheckedIntegerBinaryKind::WrappingMultiply => {
                            ScalarTerm::WrappingIntegerMultiply {
                                scalar_type: integer_type,
                                left,
                                right,
                            }
                        }
                        CheckedIntegerBinaryKind::SaturatingMultiply => {
                            ScalarTerm::SaturatingIntegerMultiply {
                                scalar_type: integer_type,
                                left,
                                right,
                            }
                        }
                        CheckedIntegerBinaryKind::WrappingDivide => {
                            ScalarTerm::WrappingIntegerDivide {
                                scalar_type: integer_type,
                                left,
                                right,
                            }
                        }
                        CheckedIntegerBinaryKind::WrappingRemainder => {
                            ScalarTerm::WrappingIntegerRemainder {
                                scalar_type: integer_type,
                                left,
                                right,
                            }
                        }
                        CheckedIntegerBinaryKind::SaturatingDivide => {
                            ScalarTerm::SaturatingIntegerDivide {
                                scalar_type: integer_type,
                                left,
                                right,
                            }
                        }
                        CheckedIntegerBinaryKind::SaturatingRemainder => {
                            ScalarTerm::SaturatingIntegerRemainder {
                                scalar_type: integer_type,
                                left,
                                right,
                            }
                        }
                        _ => unreachable!("guarded structural arithmetic kind"),
                    })
                }
                _ => unsupported(
                    "structural crash integer predicate contains an unsupported operand",
                ),
            }
        }

        match expression {
            CheckedBooleanExpression::Constant(value) => Ok(ScalarTerm::boolean(*value)),
            CheckedBooleanExpression::StorageRead { .. } => {
                unsupported("crash predicate cannot reconstruct mutable storage")
            }
            CheckedBooleanExpression::StructuralParameterField {
                parameter_position,
                path,
            } => lower_structural_member_term(
                *parameter_position,
                path,
                ScalarType::Boolean,
                parameters,
                structural_types,
            ),
            CheckedBooleanExpression::Not(operand) => ScalarTerm::boolean_not(lower_term(
                operand,
                parameters,
                structural_types,
                runtime_requirements,
            )?)
            .map_err(LoweringError::InvalidCrashPredicate),
            CheckedBooleanExpression::Equal { left, right } => ScalarTerm::boolean_equal(
                lower_term(left, parameters, structural_types, runtime_requirements)?,
                lower_term(right, parameters, structural_types, runtime_requirements)?,
            )
            .map_err(LoweringError::InvalidCrashPredicate),
            CheckedBooleanExpression::IntegerComparison { kind, left, right } => {
                let left =
                    lower_integer_term(left, parameters, structural_types, runtime_requirements)?;
                let right =
                    lower_integer_term(right, parameters, structural_types, runtime_requirements)?;
                let ScalarType::Integer(integer_type) = left.scalar_type() else {
                    return unsupported("structural crash comparison operand is not an integer");
                };
                match kind {
                    CheckedIntegerComparisonKind::Equal => {
                        ScalarTerm::integer_equal(integer_type, left, right)
                    }
                    CheckedIntegerComparisonKind::LessThan => {
                        ScalarTerm::integer_less_than(integer_type, left, right)
                    }
                    CheckedIntegerComparisonKind::LessOrEqual => {
                        ScalarTerm::integer_less_or_equal(integer_type, left, right)
                    }
                }
                .map_err(LoweringError::InvalidCrashPredicate)
            }
            CheckedBooleanExpression::IeeeFloatComparison { .. } => {
                unsupported("IEEE equality lowers as an atomic proposition")
            }
            CheckedBooleanExpression::ByteSequenceEqual { .. } => {
                unsupported("byte-sequence equality lowers as an atomic proposition")
            }
            CheckedBooleanExpression::PayloadlessSumEqual { .. } => {
                unsupported("payload-less sum equality lowers through case-membership propositions")
            }
            CheckedBooleanExpression::StructuralCaseMembership { .. } => {
                unsupported("sum membership lowers as an atomic proposition")
            }
            CheckedBooleanExpression::Parameter { .. }
            | CheckedBooleanExpression::Local { .. }
            | CheckedBooleanExpression::And { .. }
            | CheckedBooleanExpression::Or { .. } => {
                unsupported("structural crash route contains an unsupported Boolean term")
            }
        }
    }

    fn lower_proposition(
        expression: &CheckedBooleanExpression,
        parameters: &[StructuralParameterDeclaration],
        structural_types: &[StructuralTypeDeclaration],
        runtime_requirements: &[Proposition],
    ) -> Result<Proposition, LoweringError> {
        fn contains_structural_atomic_proposition(expression: &CheckedBooleanExpression) -> bool {
            match expression {
                CheckedBooleanExpression::IeeeFloatComparison { .. }
                | CheckedBooleanExpression::ByteSequenceEqual { .. }
                | CheckedBooleanExpression::PayloadlessSumEqual { .. }
                | CheckedBooleanExpression::StructuralCaseMembership { .. } => true,
                CheckedBooleanExpression::Not(operand) => {
                    contains_structural_atomic_proposition(operand)
                }
                CheckedBooleanExpression::Equal { left, right }
                | CheckedBooleanExpression::And { left, right }
                | CheckedBooleanExpression::Or { left, right } => {
                    contains_structural_atomic_proposition(left)
                        || contains_structural_atomic_proposition(right)
                }
                CheckedBooleanExpression::Constant(_)
                | CheckedBooleanExpression::StorageRead { .. }
                | CheckedBooleanExpression::Parameter { .. }
                | CheckedBooleanExpression::Local { .. }
                | CheckedBooleanExpression::StructuralParameterField { .. }
                | CheckedBooleanExpression::IntegerComparison { .. } => false,
            }
        }

        if let CheckedBooleanExpression::Not(operand) = expression
            && contains_structural_atomic_proposition(operand)
        {
            return Ok(Proposition::Implication {
                premise: Box::new(lower_proposition(
                    operand,
                    parameters,
                    structural_types,
                    runtime_requirements,
                )?),
                conclusion: Box::new(Proposition::Falsehood),
            });
        }
        if let CheckedBooleanExpression::IeeeFloatComparison {
            kind,
            primitive_type,
            left,
            right,
        } = expression
        {
            let format = match primitive_type {
                PrimitiveType::F32 => IeeeFloatFormat::Binary32,
                PrimitiveType::F64 => IeeeFloatFormat::Binary64,
                _ => return unsupported("structural IEEE equality has a non-float format"),
            };
            let mut left = lower_ieee_float_field(left, format, parameters, structural_types)?;
            let mut right = lower_ieee_float_field(right, format, parameters, structural_types)?;
            if left > right {
                std::mem::swap(&mut left, &mut right);
            }
            return Ok(Proposition::IeeeFloatComparison {
                kind: match kind {
                    checked_trees::CheckedIeeeFloatComparisonKind::Equal => {
                        semantic_vocabulary::IeeeFloatComparisonKind::Equal
                    }
                    checked_trees::CheckedIeeeFloatComparisonKind::NotEqual => {
                        semantic_vocabulary::IeeeFloatComparisonKind::NotEqual
                    }
                },
                format,
                left,
                right,
            });
        }
        if let CheckedBooleanExpression::ByteSequenceEqual { left, right } = expression {
            let mut left = lower_byte_sequence_field(left, parameters, structural_types)?;
            let mut right = lower_byte_sequence_field(right, parameters, structural_types)?;
            if left > right {
                std::mem::swap(&mut left, &mut right);
            }
            return Ok(Proposition::ByteSequenceEqual { left, right });
        }
        if let CheckedBooleanExpression::StructuralCaseMembership { subject, case } = expression {
            let (subject, structural_type) =
                lower_structural_sum_subject(subject, parameters, structural_types)?;
            let cases = match &structural_types
                .iter()
                .find(|declaration| declaration.id == structural_type)
                .expect("sum subject type was resolved")
                .shape
            {
                StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } => {
                    cases
                }
                _ => unreachable!("sum subject resolver returned a sum"),
            };
            let case = cases
                .iter()
                .find(|candidate| candidate.identity == *case)
                .ok_or(LoweringError::Unsupported(
                    "structural sum membership case was redirected",
                ))?;
            return Ok(Proposition::StructuralCaseMembership {
                subject,
                case: case.id,
            });
        }
        if let CheckedBooleanExpression::PayloadlessSumEqual { left, right, cases } = expression {
            let (left, left_type) =
                lower_structural_sum_subject(left, parameters, structural_types)?;
            let (right, right_type) =
                lower_structural_sum_subject(right, parameters, structural_types)?;
            if left_type != right_type {
                return unsupported("payload-less sum equality operands have different types");
            }
            if left == right {
                return Ok(Proposition::Truth);
            }
            let StructuralTypeShape::Sum {
                cases: declared_cases,
            } = &structural_types
                .iter()
                .find(|declaration| declaration.id == left_type)
                .expect("sum subject type was resolved")
                .shape
            else {
                unreachable!("sum subject resolver returned a sum")
            };
            if cases.len() != declared_cases.len()
                || cases
                    .iter()
                    .zip(declared_cases)
                    .any(|(checked, declared)| checked != &declared.identity)
            {
                return unsupported("payload-less sum equality case roster was redirected");
            }
            let mut propositions = Vec::with_capacity(cases.len().saturating_mul(2));
            for declared in declared_cases {
                let left_membership = Proposition::StructuralCaseMembership {
                    subject: left.clone(),
                    case: declared.id,
                };
                let right_membership = Proposition::StructuralCaseMembership {
                    subject: right.clone(),
                    case: declared.id,
                };
                propositions.push(Proposition::Implication {
                    premise: Box::new(left_membership.clone()),
                    conclusion: Box::new(right_membership.clone()),
                });
                propositions.push(Proposition::Implication {
                    premise: Box::new(right_membership),
                    conclusion: Box::new(left_membership),
                });
            }
            let mut keyed = propositions
                .into_iter()
                .map(|proposition| {
                    terminal_codec::canonical_proposition_order_key(&proposition)
                        .map(|key| (key, proposition))
                        .map_err(|_| {
                            LoweringError::Unsupported(
                                "payload-less sum equality is not canonically encodable",
                            )
                        })
                })
                .collect::<Result<Vec<_>, _>>()?;
            keyed.sort_by(|left, right| left.0.cmp(&right.0));
            keyed.dedup_by(|left, right| left.0 == right.0);
            let mut propositions = keyed
                .into_iter()
                .map(|(_, proposition)| proposition)
                .collect::<Vec<_>>();
            return Ok(match propositions.len() {
                0 => Proposition::Truth,
                1 => propositions.pop().expect("one proposition"),
                _ => Proposition::Conjunction(propositions),
            });
        }
        if let CheckedBooleanExpression::And { left, right }
        | CheckedBooleanExpression::Or { left, right } = expression
        {
            let conjunction = matches!(expression, CheckedBooleanExpression::And { .. });
            let mut leaves = Vec::new();
            flatten_checked_boolean_connective(left, conjunction, &mut leaves);
            flatten_checked_boolean_connective(right, conjunction, &mut leaves);
            let propositions = leaves
                .into_iter()
                .map(|leaf| {
                    lower_proposition(leaf, parameters, structural_types, runtime_requirements)
                })
                .collect::<Result<Vec<_>, _>>()?;
            let mut flattened = Vec::new();
            for proposition in propositions {
                match proposition {
                    Proposition::Conjunction(nested) if conjunction => flattened.extend(nested),
                    Proposition::Disjunction(nested) if !conjunction => flattened.extend(nested),
                    proposition => flattened.push(proposition),
                }
            }
            let mut keyed = flattened
                .into_iter()
                .map(|proposition| {
                    terminal_codec::canonical_proposition_order_key(&proposition)
                        .map(|key| (key, proposition))
                        .map_err(|_| {
                            LoweringError::Unsupported(
                                "structural crash connective is not canonically encodable",
                            )
                        })
                })
                .collect::<Result<Vec<_>, _>>()?;
            keyed.sort_by(|left, right| left.0.cmp(&right.0));
            keyed.dedup_by(|left, right| left.0 == right.0);
            if keyed.len() < 2 {
                return unsupported(
                    "structural crash connective must retain at least two distinct predicates",
                );
            }
            let propositions = keyed
                .into_iter()
                .map(|(_, proposition)| proposition)
                .collect();
            return Ok(if conjunction {
                Proposition::Conjunction(propositions)
            } else {
                Proposition::Disjunction(propositions)
            });
        }
        Ok(Proposition::Equal(
            ScalarTerm::boolean(true),
            lower_term(
                expression,
                parameters,
                structural_types,
                runtime_requirements,
            )?,
        ))
    }

    buckets
        .iter()
        .map(|bucket| {
            let mut alternatives = bucket
                .alternative_guards()
                .iter()
                .map(|guard| match guard {
                    checked_trees::CrashRouteGuard::Truth => {
                        Ok(terminal_psi::CrashRouteGuard::Truth)
                    }
                    checked_trees::CrashRouteGuard::Predicate(predicate) => {
                        let proposition = if let Some(expression) = predicate.scalar_expression() {
                            lower_proposition(
                                expression,
                                parameters,
                                structural_types,
                                runtime_requirements,
                            )?
                        } else {
                            let mut path = Vec::new();
                            let parameter_position = predicate
                                .expression()
                                .and_then(|expression| checked_member_path(expression, &mut path))
                                .ok_or(LoweringError::Unsupported(
                                    "structural crash route is outside checked Boolean member lowering",
                                ))?;
                            Proposition::Equal(
                                ScalarTerm::boolean(true),
                                lower_structural_member_term(
                                    parameter_position,
                                    &path
                                        .into_iter()
                                        .map(
                                            checked_trees::CheckedStructuralPredicatePathSegment::Field,
                                        )
                                        .collect::<Vec<_>>(),
                                    ScalarType::Boolean,
                                    parameters,
                                    structural_types,
                                )?,
                            )
                        };
                        Ok(terminal_psi::CrashRouteGuard::Predicate(
                            terminal_psi::CrashPredicateTerm::new(proposition),
                        ))
                    }
                })
                .collect::<Result<Vec<_>, _>>()?;
            alternatives.sort();
            alternatives.dedup();
            Ok(terminal_psi::CrashRouteBucket {
                cause: match bucket.cause() {
                    checked_trees::CrashCause::Trap => TerminalCrashCause::Trap,
                    checked_trees::CrashCause::Abort => TerminalCrashCause::Abort,
                },
                alternatives,
            })
        })
        .collect()
}

pub(super) fn substitute_structural_crash_route_roots(
    buckets: &mut [terminal_psi::CrashRouteBucket],
    substitutions: &BTreeMap<PlaceId, (PlaceId, Vec<CanonicalStructuralPathSegment>)>,
) -> Result<(), LoweringError> {
    fn substitute_term(
        term: &mut ScalarTerm,
        substitutions: &BTreeMap<PlaceId, (PlaceId, Vec<CanonicalStructuralPathSegment>)>,
    ) -> Result<(), LoweringError> {
        match term {
            ScalarTerm::BooleanField { root, path }
            | ScalarTerm::IntegerField { root, path, .. } => {
                let Some((replacement, prefix)) = substitutions.get(root) else {
                    return Ok(());
                };
                *root = *replacement;
                if !prefix.is_empty() {
                    let mut rebased = Vec::with_capacity(prefix.len() + path.len());
                    rebased.extend(prefix);
                    rebased.append(path);
                    *path = rebased;
                }
            }
            ScalarTerm::BooleanNot { operand } => substitute_term(operand, substitutions)?,
            ScalarTerm::IntegerBitwiseNot { operand, .. } => {
                substitute_term(operand, substitutions)?
            }
            ScalarTerm::BooleanEqual { left, right }
            | ScalarTerm::IntegerEqual { left, right, .. }
            | ScalarTerm::IntegerLessThan { left, right, .. }
            | ScalarTerm::IntegerLessOrEqual { left, right, .. }
            | ScalarTerm::IntegerBitwiseAnd { left, right, .. }
            | ScalarTerm::IntegerBitwiseOr { left, right, .. }
            | ScalarTerm::IntegerBitwiseXor { left, right, .. }
            | ScalarTerm::ExactIntegerAdd { left, right, .. }
            | ScalarTerm::ExactIntegerSubtract { left, right, .. }
            | ScalarTerm::ExactIntegerMultiply { left, right, .. }
            | ScalarTerm::ExactIntegerDivide { left, right, .. }
            | ScalarTerm::ExactIntegerRemainder { left, right, .. }
            | ScalarTerm::WrappingIntegerAdd { left, right, .. }
            | ScalarTerm::SaturatingIntegerAdd { left, right, .. }
            | ScalarTerm::WrappingIntegerSubtract { left, right, .. }
            | ScalarTerm::SaturatingIntegerSubtract { left, right, .. }
            | ScalarTerm::WrappingIntegerMultiply { left, right, .. }
            | ScalarTerm::SaturatingIntegerMultiply { left, right, .. }
            | ScalarTerm::WrappingIntegerDivide { left, right, .. }
            | ScalarTerm::WrappingIntegerRemainder { left, right, .. }
            | ScalarTerm::SaturatingIntegerDivide { left, right, .. }
            | ScalarTerm::SaturatingIntegerRemainder { left, right, .. } => {
                substitute_term(left, substitutions)?;
                substitute_term(right, substitutions)?;
            }
            ScalarTerm::WrappingIntegerShiftLeft { value, count, .. }
            | ScalarTerm::WrappingIntegerShiftRight { value, count, .. }
            | ScalarTerm::ExactIntegerShiftLeft { value, count, .. }
            | ScalarTerm::ExactIntegerShiftRight { value, count, .. } => {
                substitute_term(value, substitutions)?;
                substitute_term(count, substitutions)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn substitute_proposition(
        proposition: &mut Proposition,
        substitutions: &BTreeMap<PlaceId, (PlaceId, Vec<CanonicalStructuralPathSegment>)>,
    ) -> Result<(), LoweringError> {
        match proposition {
            Proposition::Equal(left, right) => {
                substitute_term(left, substitutions)?;
                substitute_term(right, substitutions)?;
            }
            Proposition::Conjunction(propositions) | Proposition::Disjunction(propositions) => {
                for proposition in propositions.iter_mut() {
                    substitute_proposition(proposition, substitutions)?;
                }
            }
            Proposition::Implication {
                premise,
                conclusion,
            } => {
                substitute_proposition(premise, substitutions)?;
                substitute_proposition(conclusion, substitutions)?;
            }
            Proposition::IeeeFloatComparison { left, right, .. } => {
                for field in [left, right] {
                    if let Some((root, prefix)) = substitutions.get(&field.root()) {
                        *field = field.rebase(*root, prefix);
                    }
                }
            }
            Proposition::ByteSequenceEqual { left, right } => {
                for field in [left, right] {
                    if let Some((root, prefix)) = substitutions.get(&field.root()) {
                        *field = field.rebase(*root, prefix);
                    }
                }
            }
            Proposition::StructuralCaseMembership { subject, .. } => {
                if let Some((root, prefix)) = substitutions.get(&subject.root()) {
                    *subject = subject.rebase(*root, prefix);
                }
            }
            _ => {}
        }
        Ok(())
    }

    for bucket in buckets {
        for alternative in &mut bucket.alternatives {
            let terminal_psi::CrashRouteGuard::Predicate(predicate) = alternative else {
                continue;
            };
            let mut proposition = predicate.proposition().clone();
            substitute_proposition(&mut proposition, substitutions)?;
            *predicate = terminal_psi::CrashPredicateTerm::new(proposition);
        }
    }
    Ok(())
}

pub(super) fn substitute_structural_requirement_roots(
    proposition: &mut Proposition,
    substitutions: &BTreeMap<PlaceId, (PlaceId, Vec<CanonicalStructuralPathSegment>)>,
) -> Result<(), LoweringError> {
    fn substitute_term(
        term: &mut ScalarTerm,
        substitutions: &BTreeMap<PlaceId, (PlaceId, Vec<CanonicalStructuralPathSegment>)>,
    ) {
        match term {
            ScalarTerm::BooleanField { root, path }
            | ScalarTerm::IntegerField { root, path, .. } => {
                let Some((replacement, prefix)) = substitutions.get(root) else {
                    return;
                };
                *root = *replacement;
                if !prefix.is_empty() {
                    let mut rebased = Vec::with_capacity(prefix.len() + path.len());
                    rebased.extend(prefix);
                    rebased.append(path);
                    *path = rebased;
                }
            }
            ScalarTerm::BooleanNot { operand }
            | ScalarTerm::IntegerBitwiseNot { operand, .. }
            | ScalarTerm::IntegerWiden { operand, .. }
            | ScalarTerm::IntegerExactCast { operand, .. } => {
                substitute_term(operand, substitutions);
            }
            ScalarTerm::BooleanEqual { left, right }
            | ScalarTerm::IntegerEqual { left, right, .. }
            | ScalarTerm::IntegerLessThan { left, right, .. }
            | ScalarTerm::IntegerLessOrEqual { left, right, .. }
            | ScalarTerm::IntegerBitwiseAnd { left, right, .. }
            | ScalarTerm::IntegerBitwiseOr { left, right, .. }
            | ScalarTerm::IntegerBitwiseXor { left, right, .. }
            | ScalarTerm::ExactIntegerAdd { left, right, .. }
            | ScalarTerm::ExactIntegerSubtract { left, right, .. }
            | ScalarTerm::ExactIntegerMultiply { left, right, .. }
            | ScalarTerm::ExactIntegerDivide { left, right, .. }
            | ScalarTerm::ExactIntegerRemainder { left, right, .. }
            | ScalarTerm::WrappingIntegerDivide { left, right, .. }
            | ScalarTerm::WrappingIntegerRemainder { left, right, .. }
            | ScalarTerm::SaturatingIntegerDivide { left, right, .. }
            | ScalarTerm::SaturatingIntegerRemainder { left, right, .. }
            | ScalarTerm::WrappingIntegerAdd { left, right, .. }
            | ScalarTerm::SaturatingIntegerAdd { left, right, .. }
            | ScalarTerm::WrappingIntegerSubtract { left, right, .. }
            | ScalarTerm::SaturatingIntegerSubtract { left, right, .. }
            | ScalarTerm::WrappingIntegerMultiply { left, right, .. }
            | ScalarTerm::SaturatingIntegerMultiply { left, right, .. } => {
                substitute_term(left, substitutions);
                substitute_term(right, substitutions);
            }
            ScalarTerm::WrappingIntegerShiftLeft { value, count, .. }
            | ScalarTerm::WrappingIntegerShiftRight { value, count, .. }
            | ScalarTerm::ExactIntegerShiftLeft { value, count, .. }
            | ScalarTerm::ExactIntegerShiftRight { value, count, .. } => {
                substitute_term(value, substitutions);
                substitute_term(count, substitutions);
            }
            ScalarTerm::Value { .. } | ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } => {}
        }
    }

    match proposition {
        Proposition::Equal(left, right)
        | Proposition::LessThan(left, right)
        | Proposition::LessOrEqual(left, right) => {
            substitute_term(left, substitutions);
            substitute_term(right, substitutions);
        }
        Proposition::IeeeFloatComparison { left, right, .. } => {
            for field in [left, right] {
                if let Some((root, prefix)) = substitutions.get(&field.root()) {
                    *field = field.rebase(*root, prefix);
                }
            }
        }
        Proposition::ByteSequenceEqual { left, right } => {
            for field in [left, right] {
                if let Some((root, prefix)) = substitutions.get(&field.root()) {
                    *field = field.rebase(*root, prefix);
                }
            }
        }
        Proposition::StructuralCaseMembership { subject, .. } => {
            if let Some((root, prefix)) = substitutions.get(&subject.root()) {
                *subject = subject.rebase(*root, prefix);
            }
        }
        Proposition::Conjunction(propositions) | Proposition::Disjunction(propositions) => {
            for proposition in propositions {
                substitute_structural_requirement_roots(proposition, substitutions)?;
            }
        }
        Proposition::Implication {
            premise,
            conclusion,
        } => {
            substitute_structural_requirement_roots(premise, substitutions)?;
            substitute_structural_requirement_roots(conclusion, substitutions)?;
        }
        Proposition::Truth
        | Proposition::Falsehood
        | Proposition::Atom(_)
        | Proposition::IntegerMathEqual(_, _)
        | Proposition::IntegerMathLessThan(_, _)
        | Proposition::IntegerMathLessOrEqual(_, _) => {}
        Proposition::ContentConservation(_) => {
            return unsupported(
                "runtime structural requirements cannot carry content conservation",
            );
        }
    }
    Ok(())
}

pub(super) fn structural_crash_route_argument_prefix(
    argument: &StructuralArgument,
    parameters: &[StructuralParameterDeclaration],
    trivial_affine_locals: &[StructuralPlaceDeclaration],
    affine_scalar_record_locals: &[StructuralPlaceDeclaration],
    structural_types: &[StructuralTypeDeclaration],
) -> Result<Vec<CanonicalStructuralPathSegment>, LoweringError> {
    let mut structural_type = parameters
        .iter()
        .find(|parameter| parameter.place == argument.place)
        .map(|parameter| parameter.structural_type)
        .or_else(|| {
            trivial_affine_locals.iter().find_map(|local| {
                (local.id == argument.place)
                    .then_some(match local.kind {
                        StructuralPlaceKind::TrivialAffineLocal {
                            structural_type, ..
                        } => Some(structural_type),
                        _ => None,
                    })
                    .flatten()
            })
        })
        .or_else(|| {
            affine_scalar_record_locals.iter().find_map(|local| {
                (local.id == argument.place)
                    .then_some(match local.kind {
                        StructuralPlaceKind::OperationResult {
                            structural_type, ..
                        } => Some(structural_type),
                        _ => None,
                    })
                    .flatten()
            })
        })
        .ok_or(LoweringError::Unsupported(
            "structural crash route argument has no caller structural source",
        ))?;
    let mut prefix = Vec::with_capacity(argument.path.len());
    for segment in &argument.path {
        let declaration = structural_types
            .iter()
            .find(|declaration| declaration.id == structural_type)
            .ok_or(LoweringError::Unsupported(
                "structural crash route argument path type is absent",
            ))?;
        match segment {
            StructuralPathSegment::Field(identity) => {
                let fields = match &declaration.shape {
                    StructuralTypeShape::Record { fields }
                    | StructuralTypeShape::Mixed { fields, .. } => fields,
                    _ => {
                        return unsupported(
                            "structural crash route argument path receiver is not a record",
                        );
                    }
                };
                let field = fields
                    .iter()
                    .find(|field| field.identity == *identity)
                    .filter(|field| !field.relevance.is_erased())
                    .ok_or(LoweringError::Unsupported(
                        "structural crash route argument field is absent or erased",
                    ))?;
                let StructuralFieldType::Structural(next) = field.field_type else {
                    return unsupported("structural crash route argument field is not structural");
                };
                prefix.push(CanonicalStructuralPathSegment::Field(field.id));
                structural_type = next;
            }
            StructuralPathSegment::FixedIndex(index) => {
                let StructuralTypeShape::FixedArray { element, length } = declaration.shape else {
                    return unsupported(
                        "structural crash route argument fixed index receiver is not an array",
                    );
                };
                if *index >= length {
                    return unsupported(
                        "structural crash route argument fixed index is out of bounds",
                    );
                }
                prefix.push(CanonicalStructuralPathSegment::FixedIndex(*index));
                structural_type = element;
            }
        }
    }
    Ok(prefix)
}

pub(super) fn lower_checked_crash_predicates(
    predicates: &[CheckedBooleanExpression],
    values: &[ValueDeclaration],
) -> Result<Vec<terminal_psi::CrashPredicateTerm>, LoweringError> {
    let mut predicates = predicates
        .iter()
        .map(|predicate| {
            checked_boolean_proposition(predicate, values)
                .map(terminal_psi::CrashPredicateTerm::new)
        })
        .collect::<Result<Vec<_>, _>>()?;
    predicates.sort();
    predicates.dedup();
    Ok(predicates)
}

fn flatten_checked_boolean_connective<'expression>(
    expression: &'expression CheckedBooleanExpression,
    conjunction: bool,
    output: &mut Vec<&'expression CheckedBooleanExpression>,
) {
    match expression {
        CheckedBooleanExpression::And { left, right } if conjunction => {
            flatten_checked_boolean_connective(left, conjunction, output);
            flatten_checked_boolean_connective(right, conjunction, output);
        }
        CheckedBooleanExpression::Or { left, right } if !conjunction => {
            flatten_checked_boolean_connective(left, conjunction, output);
            flatten_checked_boolean_connective(right, conjunction, output);
        }
        expression => output.push(expression),
    }
}

pub(super) fn checked_boolean_proposition(
    expression: &CheckedBooleanExpression,
    values: &[ValueDeclaration],
) -> Result<Proposition, LoweringError> {
    match expression {
        CheckedBooleanExpression::Constant(_) => {
            unsupported("constant crash predicates must normalize before terminal lowering")
        }
        CheckedBooleanExpression::And { left, right }
        | CheckedBooleanExpression::Or { left, right } => {
            let conjunction = matches!(expression, CheckedBooleanExpression::And { .. });
            let mut leaves = Vec::new();
            flatten_checked_boolean_connective(left, conjunction, &mut leaves);
            flatten_checked_boolean_connective(right, conjunction, &mut leaves);
            let mut propositions = leaves
                .into_iter()
                .map(|leaf| checked_boolean_proposition(leaf, values))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .map(|proposition| {
                    terminal_codec::canonical_proposition_order_key(&proposition)
                        .map(|key| (key, proposition))
                        .map_err(|_| {
                            LoweringError::Unsupported(
                                "scalar crash connective is not canonically encodable",
                            )
                        })
                })
                .collect::<Result<Vec<_>, _>>()?;
            propositions.sort_by(|left, right| left.0.cmp(&right.0));
            propositions.dedup_by(|left, right| left.0 == right.0);
            if propositions.len() < 2 {
                return unsupported(
                    "scalar crash connective must retain at least two distinct predicates",
                );
            }
            let propositions = propositions
                .into_iter()
                .map(|(_, proposition)| proposition)
                .collect();
            Ok(if conjunction {
                Proposition::Conjunction(propositions)
            } else {
                Proposition::Disjunction(propositions)
            })
        }
        expression => {
            let mut left = checked_boolean_scalar_term(expression, values)?;
            let mut right = ScalarTerm::boolean(true);
            if left > right {
                std::mem::swap(&mut left, &mut right);
            }
            Ok(Proposition::Equal(left, right))
        }
    }
}

fn checked_boolean_scalar_term(
    expression: &CheckedBooleanExpression,
    values: &[ValueDeclaration],
) -> Result<ScalarTerm, LoweringError> {
    Ok(match expression {
        CheckedBooleanExpression::Constant(value) => ScalarTerm::boolean(*value),
        CheckedBooleanExpression::StorageRead { .. } => {
            return unsupported(
                "crash predicate requires an established scalar value, not mutable storage",
            );
        }
        CheckedBooleanExpression::Parameter { position }
        | CheckedBooleanExpression::Local { position } => {
            let value = values.get(*position).ok_or(LoweringError::Unsupported(
                "crash predicate value position is outside the selected scalar namespace",
            ))?;
            if value.scalar_type != ScalarType::Boolean {
                return unsupported("crash predicate Boolean value has a non-Boolean type");
            }
            ScalarTerm::value(value.id, value.scalar_type)
        }
        CheckedBooleanExpression::StructuralParameterField { .. } => {
            return unsupported(
                "structural crash predicates require structural signature lowering",
            );
        }
        CheckedBooleanExpression::Not(operand) => {
            ScalarTerm::boolean_not(checked_boolean_scalar_term(operand, values)?)
                .map_err(LoweringError::InvalidCrashPredicate)?
        }
        CheckedBooleanExpression::Equal { left, right } => ScalarTerm::boolean_equal(
            checked_boolean_scalar_term(left, values)?,
            checked_boolean_scalar_term(right, values)?,
        )
        .map_err(LoweringError::InvalidCrashPredicate)?,
        CheckedBooleanExpression::IntegerComparison { kind, left, right } => {
            let left = checked_scalar_term(left, values)?;
            let right = checked_scalar_term(right, values)?;
            let ScalarType::Integer(integer_type) = left.scalar_type() else {
                return unsupported("crash comparison operand is not an integer");
            };
            match kind {
                CheckedIntegerComparisonKind::Equal => {
                    ScalarTerm::integer_equal(integer_type, left, right)
                }
                CheckedIntegerComparisonKind::LessThan => {
                    ScalarTerm::integer_less_than(integer_type, left, right)
                }
                CheckedIntegerComparisonKind::LessOrEqual => {
                    ScalarTerm::integer_less_or_equal(integer_type, left, right)
                }
            }
            .map_err(LoweringError::InvalidCrashPredicate)?
        }
        CheckedBooleanExpression::IeeeFloatComparison { .. }
        | CheckedBooleanExpression::ByteSequenceEqual { .. }
        | CheckedBooleanExpression::PayloadlessSumEqual { .. }
        | CheckedBooleanExpression::StructuralCaseMembership { .. } => {
            return unsupported("structural equality requires structural signature lowering");
        }
        CheckedBooleanExpression::And { .. } | CheckedBooleanExpression::Or { .. } => {
            return unsupported(
                "short-circuit Boolean crash predicate is not one scalar terminal term",
            );
        }
    })
}

pub(super) fn checked_scalar_term(
    expression: &CheckedScalarExpression,
    values: &[ValueDeclaration],
) -> Result<ScalarTerm, LoweringError> {
    let expression = lower_checked_scalar_expression(expression)?;
    lowered_direct_scalar_term(&expression, values)
}

fn lowered_direct_scalar_term(
    expression: &LoweredDirectExpression,
    values: &[ValueDeclaration],
) -> Result<ScalarTerm, LoweringError> {
    Ok(match expression {
        LoweredDirectExpression::Parameter {
            position,
            scalar_type,
        }
        | LoweredDirectExpression::Local {
            position,
            scalar_type,
        } => {
            let value = values.get(*position).ok_or(LoweringError::Unsupported(
                "crash predicate value position is outside the selected scalar namespace",
            ))?;
            if value.scalar_type != *scalar_type {
                return unsupported("crash predicate value type does not match its checked plan");
            }
            ScalarTerm::value(value.id, *scalar_type)
        }
        LoweredDirectExpression::IntegerLiteral { value, scalar_type } => {
            let ScalarType::Integer(integer_type) = scalar_type else {
                return unsupported("crash predicate integer literal has a non-integer type");
            };
            ScalarTerm::integer(*integer_type, *value)
                .map_err(LoweringError::InvalidCrashPredicate)?
        }
        LoweredDirectExpression::IntegerBinary {
            kind,
            scalar_type,
            left,
            right,
        } => {
            let ScalarType::Integer(integer_type) = scalar_type else {
                return unsupported("crash predicate arithmetic has a non-integer type");
            };
            let left = Box::new(lowered_direct_scalar_term(left, values)?);
            let right = Box::new(lowered_direct_scalar_term(right, values)?);
            match kind {
                LoweredIntegerBinaryKind::BitwiseAnd => ScalarTerm::IntegerBitwiseAnd {
                    scalar_type: *integer_type,
                    left,
                    right,
                },
                LoweredIntegerBinaryKind::BitwiseOr => ScalarTerm::IntegerBitwiseOr {
                    scalar_type: *integer_type,
                    left,
                    right,
                },
                LoweredIntegerBinaryKind::BitwiseXor => ScalarTerm::IntegerBitwiseXor {
                    scalar_type: *integer_type,
                    left,
                    right,
                },
                LoweredIntegerBinaryKind::WrappingShiftLeft => {
                    let ScalarType::Integer(count_type) = right.scalar_type() else {
                        return unsupported("crash shift count is not an integer");
                    };
                    ScalarTerm::WrappingIntegerShiftLeft {
                        value_type: *integer_type,
                        count_type,
                        value: left,
                        count: right,
                    }
                }
                LoweredIntegerBinaryKind::WrappingShiftRight => {
                    let ScalarType::Integer(count_type) = right.scalar_type() else {
                        return unsupported("crash shift count is not an integer");
                    };
                    ScalarTerm::WrappingIntegerShiftRight {
                        value_type: *integer_type,
                        count_type,
                        value: left,
                        count: right,
                    }
                }
                LoweredIntegerBinaryKind::ExactShiftLeft => {
                    let ScalarType::Integer(count_type) = right.scalar_type() else {
                        return unsupported("crash shift count is not an integer");
                    };
                    ScalarTerm::ExactIntegerShiftLeft {
                        value_type: *integer_type,
                        count_type,
                        value: left,
                        count: right,
                    }
                }
                LoweredIntegerBinaryKind::ExactShiftRight => {
                    let ScalarType::Integer(count_type) = right.scalar_type() else {
                        return unsupported("crash shift count is not an integer");
                    };
                    ScalarTerm::ExactIntegerShiftRight {
                        value_type: *integer_type,
                        count_type,
                        value: left,
                        count: right,
                    }
                }
                LoweredIntegerBinaryKind::ExactAdd => ScalarTerm::ExactIntegerAdd {
                    scalar_type: *integer_type,
                    left,
                    right,
                },
                LoweredIntegerBinaryKind::ExactSubtract => ScalarTerm::ExactIntegerSubtract {
                    scalar_type: *integer_type,
                    left,
                    right,
                },
                LoweredIntegerBinaryKind::ExactMultiply => ScalarTerm::ExactIntegerMultiply {
                    scalar_type: *integer_type,
                    left,
                    right,
                },
                LoweredIntegerBinaryKind::ExactDivide => ScalarTerm::ExactIntegerDivide {
                    scalar_type: *integer_type,
                    left,
                    right,
                },
                LoweredIntegerBinaryKind::ExactRemainder => ScalarTerm::ExactIntegerRemainder {
                    scalar_type: *integer_type,
                    left,
                    right,
                },
                LoweredIntegerBinaryKind::WrappingDivide => ScalarTerm::WrappingIntegerDivide {
                    scalar_type: *integer_type,
                    left,
                    right,
                },
                LoweredIntegerBinaryKind::WrappingRemainder => {
                    ScalarTerm::WrappingIntegerRemainder {
                        scalar_type: *integer_type,
                        left,
                        right,
                    }
                }
                LoweredIntegerBinaryKind::SaturatingDivide => ScalarTerm::SaturatingIntegerDivide {
                    scalar_type: *integer_type,
                    left,
                    right,
                },
                LoweredIntegerBinaryKind::SaturatingRemainder => {
                    ScalarTerm::SaturatingIntegerRemainder {
                        scalar_type: *integer_type,
                        left,
                        right,
                    }
                }
                LoweredIntegerBinaryKind::WrappingAdd => ScalarTerm::WrappingIntegerAdd {
                    scalar_type: *integer_type,
                    left,
                    right,
                },
                LoweredIntegerBinaryKind::SaturatingAdd => ScalarTerm::SaturatingIntegerAdd {
                    scalar_type: *integer_type,
                    left,
                    right,
                },
                LoweredIntegerBinaryKind::WrappingSubtract => ScalarTerm::WrappingIntegerSubtract {
                    scalar_type: *integer_type,
                    left,
                    right,
                },
                LoweredIntegerBinaryKind::SaturatingSubtract => {
                    ScalarTerm::SaturatingIntegerSubtract {
                        scalar_type: *integer_type,
                        left,
                        right,
                    }
                }
                LoweredIntegerBinaryKind::WrappingMultiply => ScalarTerm::WrappingIntegerMultiply {
                    scalar_type: *integer_type,
                    left,
                    right,
                },
                LoweredIntegerBinaryKind::SaturatingMultiply => {
                    ScalarTerm::SaturatingIntegerMultiply {
                        scalar_type: *integer_type,
                        left,
                        right,
                    }
                }
            }
        }
        LoweredDirectExpression::IntegerBitwiseNot {
            scalar_type,
            operand,
        } => {
            let ScalarType::Integer(integer_type) = scalar_type else {
                return unsupported("crash predicate bitwise-not has a non-integer type");
            };
            ScalarTerm::IntegerBitwiseNot {
                scalar_type: *integer_type,
                operand: Box::new(lowered_direct_scalar_term(operand, values)?),
            }
        }
        LoweredDirectExpression::IntegerWiden {
            scalar_type,
            operand,
        } => {
            let ScalarType::Integer(target_type) = scalar_type else {
                return unsupported("crash predicate widen has a non-integer target");
            };
            let operand = lowered_direct_scalar_term(operand, values)?;
            let ScalarType::Integer(source_type) = operand.scalar_type() else {
                return unsupported("crash predicate widen has a non-integer operand");
            };
            ScalarTerm::IntegerWiden {
                source_type,
                target_type: *target_type,
                operand: Box::new(operand),
            }
        }
        LoweredDirectExpression::IntegerExactCast {
            scalar_type,
            operand,
        } => {
            let ScalarType::Integer(target_type) = scalar_type else {
                return unsupported("crash predicate cast has a non-integer target");
            };
            let operand = lowered_direct_scalar_term(operand, values)?;
            let ScalarType::Integer(source_type) = operand.scalar_type() else {
                return unsupported("crash predicate cast has a non-integer operand");
            };
            ScalarTerm::IntegerExactCast {
                source_type,
                target_type: *target_type,
                operand: Box::new(operand),
            }
        }
        LoweredDirectExpression::IeeeFloatLiteral { .. } => {
            return unsupported("generic scalar crash terms do not carry IEEE float literals");
        }
        LoweredDirectExpression::Boolean { expression } => {
            return checked_boolean_scalar_term_from_lowered(expression, values);
        }
    })
}

fn checked_boolean_scalar_term_from_lowered(
    expression: &LoweredBooleanReturnExpression,
    values: &[ValueDeclaration],
) -> Result<ScalarTerm, LoweringError> {
    match expression {
        LoweredBooleanReturnExpression::Constant { value } => Ok(ScalarTerm::boolean(*value)),
        LoweredBooleanReturnExpression::Parameter { position }
        | LoweredBooleanReturnExpression::Local { position } => {
            let value = values.get(*position).ok_or(LoweringError::Unsupported(
                "crash predicate value position is outside the selected scalar namespace",
            ))?;
            (value.scalar_type == ScalarType::Boolean)
                .then(|| ScalarTerm::value(value.id, value.scalar_type))
                .ok_or(LoweringError::Unsupported(
                    "crash predicate Boolean value has a non-Boolean type",
                ))
        }
        LoweredBooleanReturnExpression::StructuralField { source, field } => {
            Ok(ScalarTerm::boolean_field(*source, *field))
        }
        LoweredBooleanReturnExpression::UnresolvedStructuralParameterField { .. } => {
            unsupported("unresolved structural field crossed crash-predicate lowering")
        }
        LoweredBooleanReturnExpression::Not { operand } => {
            ScalarTerm::boolean_not(checked_boolean_scalar_term_from_lowered(operand, values)?)
                .map_err(LoweringError::InvalidCrashPredicate)
        }
        LoweredBooleanReturnExpression::Equal { left, right } => ScalarTerm::boolean_equal(
            checked_boolean_scalar_term_from_lowered(left, values)?,
            checked_boolean_scalar_term_from_lowered(right, values)?,
        )
        .map_err(LoweringError::InvalidCrashPredicate),
        LoweredBooleanReturnExpression::IntegerComparison { kind, left, right } => {
            let left = lowered_direct_scalar_term(left, values)?;
            let right = lowered_direct_scalar_term(right, values)?;
            let ScalarType::Integer(integer_type) = left.scalar_type() else {
                return unsupported("crash comparison operand is not an integer");
            };
            match kind {
                LoweredIntegerComparisonKind::Equal => {
                    ScalarTerm::integer_equal(integer_type, left, right)
                }
                LoweredIntegerComparisonKind::LessThan => {
                    ScalarTerm::integer_less_than(integer_type, left, right)
                }
                LoweredIntegerComparisonKind::LessOrEqual => {
                    ScalarTerm::integer_less_or_equal(integer_type, left, right)
                }
            }
            .map_err(LoweringError::InvalidCrashPredicate)
        }
        LoweredBooleanReturnExpression::And { .. } | LoweredBooleanReturnExpression::Or { .. } => {
            unsupported("short-circuit Boolean crash predicate is not one scalar terminal term")
        }
    }
}
