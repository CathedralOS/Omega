//! Validates scalar crash routes, live frontiers, and Boolean field predicates.

use numerics::{
    arithmetic::ArithmeticDomain,
    integer_policy::{IntegerFormationCondition, IntegerPolicyPrimitive, integer_policy_bridge},
};

use super::*;

pub(super) fn substitute_crash_routes(
    routes: &[CrashRouteBucket],
    substitutions: &BTreeMap<ValueId, ScalarTerm>,
) -> Vec<CrashRouteBucket> {
    routes
        .iter()
        .filter_map(|bucket| {
            let mut alternatives = bucket
                .alternatives
                .iter()
                .filter_map(|guard| match guard {
                    CrashRouteGuard::Truth => Some(CrashRouteGuard::Truth),
                    CrashRouteGuard::Predicate(predicate) => {
                        match substitute_proposition_values(predicate.proposition(), substitutions)
                        {
                            Proposition::Truth => Some(CrashRouteGuard::Truth),
                            Proposition::Falsehood => None,
                            proposition => Some(CrashRouteGuard::Predicate(
                                CrashPredicateTerm::new(proposition),
                            )),
                        }
                    }
                })
                .collect::<Vec<_>>();
            alternatives.sort();
            alternatives.dedup();
            if alternatives.contains(&CrashRouteGuard::Truth) {
                alternatives = vec![CrashRouteGuard::Truth];
            }
            (!alternatives.is_empty()).then_some(CrashRouteBucket {
                cause: bucket.cause,
                alternatives,
            })
        })
        .collect()
}

pub(super) fn validate_crash_frontiers(
    module: &TerminalModule,
    machine: &TerminalMachine,
    context: &PropositionContext,
    contract_values: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    if machine
        .contract
        .crash_routes
        .windows(2)
        .any(|pair| pair[0].cause >= pair[1].cause)
    {
        return Err(ModuleError::NonCanonicalCrashRoutes(machine.id));
    }
    for bucket in &machine.contract.crash_routes {
        if bucket.alternatives.is_empty() {
            return Err(ModuleError::EmptyCrashRouteBucket {
                machine: machine.id,
                cause: bucket.cause,
            });
        }
        if bucket
            .alternatives
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
            || (bucket.alternatives.contains(&CrashRouteGuard::Truth)
                && bucket.alternatives != [CrashRouteGuard::Truth])
        {
            return Err(ModuleError::NonCanonicalCrashRouteAlternatives {
                machine: machine.id,
                cause: bucket.cause,
            });
        }
        for guard in &bucket.alternatives {
            let CrashRouteGuard::Predicate(predicate) = guard else {
                continue;
            };
            if matches!(
                predicate.proposition(),
                Proposition::Truth | Proposition::Falsehood
            ) {
                return Err(ModuleError::NonCanonicalCrashRouteAlternatives {
                    machine: machine.id,
                    cause: bucket.cause,
                });
            }
            context
                .validate(predicate.proposition())
                .map_err(ModuleError::MalformedProposition)?;
            validate_boolean_field_terms(
                module,
                machine,
                predicate.proposition(),
                &machine.contract.requires,
            )?;
            contracts::validate_contract_scope(
                predicate.proposition(),
                contract_values,
                machine.contract.id,
                ContractClauseKind::Crash,
            )?;
        }
    }
    for block in &machine.blocks {
        let Terminator::Crash {
            cause,
            site_guard,
            frontier_lower_bound,
            ..
        } = &block.terminator
        else {
            continue;
        };
        if site_guard.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(ModuleError::NonCanonicalCrashSiteGuard(block.id));
        }
        for predicate in site_guard {
            if matches!(
                predicate.proposition(),
                Proposition::Truth | Proposition::Falsehood
            ) {
                return Err(ModuleError::NonCanonicalCrashSiteGuard(block.id));
            }
            context
                .validate(predicate.proposition())
                .map_err(ModuleError::MalformedProposition)?;
            validate_boolean_field_terms(
                module,
                machine,
                predicate.proposition(),
                &machine.contract.requires,
            )?;
        }
        let covered = machine
            .contract
            .crash_routes
            .iter()
            .filter(|bucket| bucket.cause == *cause)
            .any(|bucket| {
                bucket.alternatives.iter().any(|route| match route {
                    CrashRouteGuard::Truth => true,
                    CrashRouteGuard::Predicate(predicate) => site_guard.contains(predicate),
                })
            });
        if !covered {
            return Err(ModuleError::CrashRouteUncovered {
                block: block.id,
                cause: *cause,
            });
        }
        if frontier_lower_bound
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(ModuleError::NonCanonicalCrashFrontier(block.id));
        }
    }
    Ok(())
}

fn structural_subject_type(
    module: &TerminalModule,
    machine: &TerminalMachine,
    root: PlaceId,
    path: &[CanonicalStructuralPathSegment],
) -> Option<StructuralTypeId> {
    let mut structural_type = machine
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == root)?
        .structural_type;
    let mut selected_case_fields = None;
    for segment in path {
        let declaration = module
            .structural_types
            .iter()
            .find(|declaration| declaration.id == structural_type)?;
        if let CanonicalStructuralPathSegment::Case(case_id) = segment {
            if selected_case_fields.is_some() {
                return None;
            }
            let cases = match &declaration.shape {
                StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } => {
                    cases
                }
                _ => return None,
            };
            selected_case_fields = Some(
                &cases
                    .iter()
                    .find(|candidate| candidate.id == *case_id)?
                    .fields,
            );
            continue;
        }
        if let Some(fields) = selected_case_fields.take() {
            let CanonicalStructuralPathSegment::Field(field_id) = segment else {
                return None;
            };
            let field = fields
                .iter()
                .find(|candidate| candidate.id == *field_id)
                .filter(|field| !field.relevance.is_erased())?;
            let StructuralFieldType::Structural(next) = field.field_type else {
                return None;
            };
            structural_type = next;
            continue;
        }
        structural_type = match (segment, &declaration.shape) {
            (
                CanonicalStructuralPathSegment::Field(field_id),
                StructuralTypeShape::Record { fields } | StructuralTypeShape::Mixed { fields, .. },
            ) => {
                let field = fields
                    .iter()
                    .find(|candidate| candidate.id == *field_id)
                    .filter(|field| !field.relevance.is_erased())?;
                let StructuralFieldType::Structural(next) = field.field_type else {
                    return None;
                };
                next
            }
            (
                CanonicalStructuralPathSegment::FixedIndex(index),
                StructuralTypeShape::FixedArray { element, length },
            ) if *index < *length => *element,
            _ => return None,
        };
    }
    selected_case_fields.is_none().then_some(structural_type)
}

fn validate_boolean_field_terms(
    module: &TerminalModule,
    machine: &TerminalMachine,
    proposition: &Proposition,
    runtime_requirements: &[Proposition],
) -> Result<(), ModuleError> {
    fn validate_term(
        module: &TerminalModule,
        machine: &TerminalMachine,
        term: &ScalarTerm,
        runtime_requirements: &[Proposition],
    ) -> Result<(), ModuleError> {
        fn safe_exact_divisor(
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
            if ScalarTerm::integer(integer_type, one).is_ok_and(|one| {
                requirements.contains(&Proposition::LessOrEqual(one, divisor.clone()))
            }) {
                return true;
            }
            if integer_type.sign() != IntegerSign::Signed {
                return false;
            }
            if ScalarTerm::integer(integer_type, IntegerValue::Signed(-2)).is_ok_and(
                |negative_two| {
                    requirements.contains(&Proposition::LessOrEqual(divisor.clone(), negative_two))
                },
            ) {
                return true;
            }
            let negative_one = ScalarTerm::integer(integer_type, IntegerValue::Signed(-1))
                .expect("every signed fixed integer admits negative one");
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

        fn safe_policy_divisor(
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
            if ScalarTerm::integer(integer_type, one).is_ok_and(|one| {
                requirements.contains(&Proposition::LessOrEqual(one, divisor.clone()))
            }) {
                return true;
            }
            if integer_type.sign() != IntegerSign::Signed {
                return false;
            }
            [IntegerValue::Signed(-1), IntegerValue::Signed(-2)]
                .into_iter()
                .filter_map(|bound| ScalarTerm::integer(integer_type, bound).ok())
                .any(|bound| {
                    requirements.contains(&Proposition::LessOrEqual(divisor.clone(), bound))
                })
        }

        fn nonnegative_shift_count(value: IntegerValue) -> Option<u32> {
            match value {
                IntegerValue::Unsigned(value) => u32::try_from(value).ok(),
                IntegerValue::Signed(value) => u32::try_from(value).ok(),
            }
        }

        fn exact_shift_maximum_count(
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
                        (right_type == count_type && right > 0 && right <= width)
                            .then_some(right - 1)
                    }
                    _ => None,
                })
                .min()
        }

        fn safe_exact_shift(
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
                exact_shift_maximum_count(value_type, count_type, count, requirements)
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
                    ScalarTerm::integer(
                        value_type,
                        IntegerValue::Unsigned(maximum >> maximum_count),
                    )
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
                    let minimum = ScalarTerm::integer(
                        value_type,
                        IntegerValue::Signed(minimum >> maximum_count),
                    );
                    let maximum = ScalarTerm::integer(
                        value_type,
                        IntegerValue::Signed(maximum >> maximum_count),
                    );
                    minimum.is_ok_and(|minimum| {
                        requirements.contains(&Proposition::LessOrEqual(minimum, value.clone()))
                    }) && maximum.is_ok_and(|maximum| {
                        requirements.contains(&Proposition::LessOrEqual(value.clone(), maximum))
                    })
                }
            }
        }

        match term {
            ScalarTerm::BooleanField { root, path } => {
                if !matches!(
                    structural_leaf_type(module, machine, *root, path),
                    Some(StructuralFieldType::Scalar(ScalarType::Boolean))
                ) {
                    return Err(ModuleError::InvalidBooleanFieldTerm {
                        machine: machine.id,
                        root: *root,
                        path: path.clone(),
                    });
                }
            }
            ScalarTerm::IntegerField {
                root,
                path,
                scalar_type,
            } => {
                if !matches!(
                    structural_leaf_type(module, machine, *root, path),
                    Some(StructuralFieldType::Scalar(ScalarType::Integer(actual)))
                        if actual == scalar_type
                ) {
                    return Err(ModuleError::InvalidIntegerFieldTerm {
                        machine: machine.id,
                        root: *root,
                        path: path.clone(),
                        scalar_type: *scalar_type,
                    });
                }
            }
            ScalarTerm::BooleanNot { operand }
            | ScalarTerm::IntegerBitwiseNot { operand, .. }
            | ScalarTerm::IntegerWiden { operand, .. }
            | ScalarTerm::IntegerExactCast { operand, .. } => {
                validate_term(module, machine, operand, runtime_requirements)?;
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
            | ScalarTerm::WrappingIntegerAdd { left, right, .. }
            | ScalarTerm::SaturatingIntegerAdd { left, right, .. }
            | ScalarTerm::WrappingIntegerSubtract { left, right, .. }
            | ScalarTerm::SaturatingIntegerSubtract { left, right, .. }
            | ScalarTerm::WrappingIntegerMultiply { left, right, .. }
            | ScalarTerm::SaturatingIntegerMultiply { left, right, .. } => {
                validate_term(module, machine, left, runtime_requirements)?;
                validate_term(module, machine, right, runtime_requirements)?;
            }
            ScalarTerm::WrappingIntegerDivide {
                scalar_type,
                left,
                right,
            }
            | ScalarTerm::SaturatingIntegerDivide {
                scalar_type,
                left,
                right,
            }
            | ScalarTerm::WrappingIntegerRemainder {
                scalar_type,
                left,
                right,
            }
            | ScalarTerm::SaturatingIntegerRemainder {
                scalar_type,
                left,
                right,
            } => {
                let policy = if matches!(
                    term,
                    ScalarTerm::WrappingIntegerDivide { .. }
                        | ScalarTerm::WrappingIntegerRemainder { .. }
                ) {
                    ArithmeticDomain::Wrapping
                } else {
                    ArithmeticDomain::Saturating
                };
                let primitive = if matches!(
                    term,
                    ScalarTerm::WrappingIntegerRemainder { .. }
                        | ScalarTerm::SaturatingIntegerRemainder { .. }
                ) {
                    IntegerPolicyPrimitive::Remainder
                } else {
                    IntegerPolicyPrimitive::Divide
                };
                let conditions = integer_policy_bridge(primitive, policy).formation_conditions;
                if conditions != [IntegerFormationCondition::NonZeroDivisor]
                    || !safe_policy_divisor(*scalar_type, right, runtime_requirements)
                {
                    return Err(ModuleError::UnsafeStructuralCrashPolicyDivisor {
                        machine: machine.id,
                        scalar_type: *scalar_type,
                    });
                }
                validate_term(module, machine, left, runtime_requirements)?;
                validate_term(module, machine, right, runtime_requirements)?;
            }
            ScalarTerm::ExactIntegerDivide {
                scalar_type,
                left,
                right,
            }
            | ScalarTerm::ExactIntegerRemainder {
                scalar_type,
                left,
                right,
            } => {
                let primitive = if matches!(term, ScalarTerm::ExactIntegerRemainder { .. }) {
                    IntegerPolicyPrimitive::Remainder
                } else {
                    IntegerPolicyPrimitive::Divide
                };
                let conditions =
                    integer_policy_bridge(primitive, ArithmeticDomain::Exact).formation_conditions;
                if conditions
                    != [
                        IntegerFormationCondition::NonZeroDivisor,
                        IntegerFormationCondition::ResultRepresentable,
                    ]
                    || !safe_exact_divisor(*scalar_type, left, right, runtime_requirements)
                {
                    return Err(ModuleError::UnsafeStructuralCrashExactDivisor {
                        machine: machine.id,
                        scalar_type: *scalar_type,
                    });
                }
                validate_term(module, machine, left, runtime_requirements)?;
                validate_term(module, machine, right, runtime_requirements)?;
            }
            ScalarTerm::WrappingIntegerShiftLeft { value, count, .. }
            | ScalarTerm::WrappingIntegerShiftRight { value, count, .. } => {
                validate_term(module, machine, value, runtime_requirements)?;
                validate_term(module, machine, count, runtime_requirements)?;
            }
            ScalarTerm::ExactIntegerShiftLeft {
                value_type,
                count_type,
                value,
                count,
            }
            | ScalarTerm::ExactIntegerShiftRight {
                value_type,
                count_type,
                value,
                count,
            } => {
                let left_shift = matches!(term, ScalarTerm::ExactIntegerShiftLeft { .. });
                let primitive = if left_shift {
                    IntegerPolicyPrimitive::ShiftLeft
                } else {
                    IntegerPolicyPrimitive::ShiftRight
                };
                let conditions =
                    integer_policy_bridge(primitive, ArithmeticDomain::Exact).formation_conditions;
                let expected_conditions: &[IntegerFormationCondition] = if left_shift {
                    &[
                        IntegerFormationCondition::ShiftCountWithinWidth,
                        IntegerFormationCondition::ResultRepresentable,
                    ]
                } else {
                    &[IntegerFormationCondition::ShiftCountWithinWidth]
                };
                if conditions != expected_conditions
                    || !safe_exact_shift(
                        conditions.contains(&IntegerFormationCondition::ResultRepresentable),
                        *value_type,
                        *count_type,
                        value,
                        count,
                        runtime_requirements,
                    )
                {
                    return Err(ModuleError::UnsafeStructuralCrashExactShift {
                        machine: machine.id,
                        value_type: *value_type,
                        count_type: *count_type,
                        left_shift,
                    });
                }
                validate_term(module, machine, value, runtime_requirements)?;
                validate_term(module, machine, count, runtime_requirements)?;
            }
            ScalarTerm::Value { .. } | ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } => {}
        }
        Ok(())
    }

    match proposition {
        Proposition::Equal(left, right)
        | Proposition::LessThan(left, right)
        | Proposition::LessOrEqual(left, right) => {
            validate_term(module, machine, left, runtime_requirements)?;
            validate_term(module, machine, right, runtime_requirements)?;
        }
        Proposition::IeeeFloatComparison {
            format,
            left,
            right,
            ..
        } => {
            for field in [left, right] {
                if !matches!(
                    structural_leaf_type(module, machine, field.root(), field.path()),
                    Some(StructuralFieldType::IeeeFloat(actual)) if actual == format
                ) {
                    return Err(ModuleError::InvalidIeeeFloatFieldTerm {
                        machine: machine.id,
                        root: field.root(),
                        path: field.path().to_vec(),
                        format: *format,
                    });
                }
            }
        }
        Proposition::ByteSequenceEqual { left, right } => {
            for field in [left, right] {
                if !matches!(
                    structural_leaf_type(module, machine, field.root(), field.path()),
                    Some(StructuralFieldType::ByteSequence(_))
                ) {
                    return Err(ModuleError::InvalidByteSequenceFieldTerm {
                        machine: machine.id,
                        root: field.root(),
                        path: field.path().to_vec(),
                    });
                }
            }
        }
        Proposition::StructuralCaseMembership { subject, case } => {
            let valid = structural_subject_type(module, machine, subject.root(), subject.path())
                .and_then(|structural_type| {
                    module
                        .structural_types
                        .iter()
                        .find(|declaration| declaration.id == structural_type)
                })
                .is_some_and(|declaration| {
                    matches!(&declaration.shape,
                        StructuralTypeShape::Sum { cases }
                        | StructuralTypeShape::Mixed { cases, .. }
                        if cases.iter().any(|candidate| candidate.id == *case))
                });
            if !valid {
                return Err(ModuleError::InvalidStructuralCaseMembership {
                    machine: machine.id,
                    root: subject.root(),
                    path: subject.path().to_vec(),
                    case: *case,
                });
            }
        }
        Proposition::Conjunction(propositions) | Proposition::Disjunction(propositions) => {
            for proposition in propositions {
                validate_boolean_field_terms(module, machine, proposition, runtime_requirements)?;
            }
        }
        Proposition::Implication {
            premise,
            conclusion,
        } => {
            validate_boolean_field_terms(module, machine, premise, runtime_requirements)?;
            validate_boolean_field_terms(module, machine, conclusion, runtime_requirements)?;
        }
        Proposition::Truth
        | Proposition::Falsehood
        | Proposition::Atom(_)
        | Proposition::IntegerMathEqual(_, _)
        | Proposition::IntegerMathLessThan(_, _)
        | Proposition::IntegerMathLessOrEqual(_, _)
        | Proposition::ContentConservation(_) => {}
    }
    Ok(())
}

pub(super) fn validate_structural_case_memberships(
    module: &TerminalModule,
    machine: &TerminalMachine,
    proposition: &Proposition,
) -> Result<(), ModuleError> {
    match proposition {
        Proposition::StructuralCaseMembership { subject, case } => {
            let valid = structural_subject_type(module, machine, subject.root(), subject.path())
                .and_then(|structural_type| {
                    module
                        .structural_types
                        .iter()
                        .find(|declaration| declaration.id == structural_type)
                })
                .is_some_and(|declaration| {
                    matches!(&declaration.shape,
                        StructuralTypeShape::Sum { cases }
                        | StructuralTypeShape::Mixed { cases, .. }
                        if cases.iter().any(|candidate| candidate.id == *case))
                });
            if !valid {
                return Err(ModuleError::InvalidStructuralCaseMembership {
                    machine: machine.id,
                    root: subject.root(),
                    path: subject.path().to_vec(),
                    case: *case,
                });
            }
        }
        Proposition::Conjunction(propositions) | Proposition::Disjunction(propositions) => {
            for proposition in propositions {
                validate_structural_case_memberships(module, machine, proposition)?;
            }
        }
        Proposition::Implication {
            premise,
            conclusion,
        } => {
            validate_structural_case_memberships(module, machine, premise)?;
            validate_structural_case_memberships(module, machine, conclusion)?;
        }
        _ => {}
    }
    Ok(())
}
