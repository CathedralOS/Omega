use super::super::semantics::declarations::nominal_identity;
use crate::evidence::{
    PackageReviewArithmeticDomain, PackageReviewBooleanExpression, PackageReviewCrash,
    PackageReviewCrashCall, PackageReviewCrashCause, PackageReviewCrashInterface,
    PackageReviewCrashPredicate, PackageReviewCrashRoute, PackageReviewCrashRouteGuard,
    PackageReviewCrashSite, PackageReviewIeeeFloatComparisonKind, PackageReviewIntegerBinaryKind,
    PackageReviewIntegerComparisonKind, PackageReviewIntegerLiteral,
    PackageReviewIntegerLiteralLanding, PackageReviewIntegerRange, PackageReviewPermissionClaim,
    PackageReviewPermissionSource, PackageReviewPrimitiveType, PackageReviewScalarExpression,
    PackageReviewStructuralParameterField, PackageReviewStructuralPredicatePathSegment,
};
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;

pub(crate) fn project_crash(
    compilation: &CheckedCompilation,
    plan: &psi_checked_trees::CrashPlan,
) -> Result<PackageReviewCrash, Vec<Diagnostic>> {
    let interface = match plan.interface() {
        psi_checked_trees::CrashInterface::InternalInferred => {
            PackageReviewCrashInterface::InternalInferred
        }
        psi_checked_trees::CrashInterface::PublishedCeiling => {
            PackageReviewCrashInterface::PublishedCeiling
        }
    };
    let published = project_crash_routes(plan.published());
    let mut checked_sites = plan
        .checked_sites()
        .iter()
        .map(|site| {
            let location = site.location();
            let mut frontier_lower_bound = site
                .frontier_lower_bound()
                .iter()
                .map(|claim| project_permission_claim(compilation, *claim))
                .collect::<Result<Vec<_>, _>>()?;
            frontier_lower_bound.sort();
            frontier_lower_bound.dedup();
            Ok(PackageReviewCrashSite {
                state: nominal_identity(compilation, location.state())?,
                statement_ordinal: location.statement_ordinal(),
                cause: project_crash_cause(site.cause()),
                path_guard_conjuncts: project_crash_predicates(site.path_guard_conjuncts()),
                path_guard_consequences: project_crash_predicates(site.path_guard_consequences()),
                guard_covering_buckets: site
                    .guard_covering_buckets()
                    .iter()
                    .map(|bucket| bucket.get())
                    .collect(),
                frontier_lower_bound,
            })
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    checked_sites.sort();
    checked_sites.dedup();

    let mut checked_calls = plan
        .checked_calls()
        .iter()
        .map(|call| {
            let location = call.location();
            Ok(PackageReviewCrashCall {
                state: nominal_identity(compilation, location.state())?,
                statement_ordinal: location.statement_ordinal(),
                call_ordinal: location.call_ordinal(),
                target_machine: nominal_identity(compilation, call.target_machine())?,
                target_state: nominal_identity(compilation, call.target_state())?,
                path_guard_conjuncts: project_crash_predicates(call.path_guard_conjuncts()),
                path_guard_consequences: project_crash_predicates(call.path_guard_consequences()),
                surviving_buckets: project_crash_routes(call.surviving_buckets()),
            })
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    checked_calls.sort();
    checked_calls.dedup();

    Ok(PackageReviewCrash {
        interface,
        published,
        structural_runtime_requirements: plan.structural_runtime_requirements().map(
            |requirements| {
                requirements
                    .iter()
                    .map(project_boolean_expression)
                    .collect()
            },
        ),
        checked_sites,
        checked_calls,
    })
}

pub(crate) fn project_crash_routes(
    routes: &[psi_checked_trees::CrashRouteBucket],
) -> Vec<PackageReviewCrashRoute> {
    let mut projected = routes
        .iter()
        .map(|route| PackageReviewCrashRoute {
            cause: project_crash_cause(route.cause()),
            alternative_guards: route
                .alternative_guards()
                .iter()
                .map(|guard| match guard {
                    psi_checked_trees::CrashRouteGuard::Truth => {
                        PackageReviewCrashRouteGuard::Truth
                    }
                    psi_checked_trees::CrashRouteGuard::Predicate(predicate) => {
                        PackageReviewCrashRouteGuard::Predicate(project_crash_predicate(predicate))
                    }
                })
                .collect(),
        })
        .collect::<Vec<_>>();
    projected.sort();
    projected.dedup();
    projected
}

pub(crate) const fn project_crash_cause(
    cause: psi_checked_trees::CrashCause,
) -> PackageReviewCrashCause {
    match cause {
        psi_checked_trees::CrashCause::Trap => PackageReviewCrashCause::Trap,
        psi_checked_trees::CrashCause::Abort => PackageReviewCrashCause::Abort,
    }
}

fn project_boolean_expression(
    expression: &psi_checked_trees::CheckedBooleanExpression,
) -> PackageReviewBooleanExpression {
    use psi_checked_trees::CheckedBooleanExpression;

    match expression {
        CheckedBooleanExpression::Constant(value) => {
            PackageReviewBooleanExpression::Constant(*value)
        }
        CheckedBooleanExpression::Parameter { position } => {
            PackageReviewBooleanExpression::Parameter {
                position: *position,
            }
        }
        CheckedBooleanExpression::Local { position } => PackageReviewBooleanExpression::Local {
            position: *position,
        },
        CheckedBooleanExpression::StructuralParameterField {
            parameter_position,
            path,
        } => PackageReviewBooleanExpression::StructuralParameterField {
            parameter_position: *parameter_position,
            path: project_structural_path(path),
        },
        CheckedBooleanExpression::Not(operand) => {
            PackageReviewBooleanExpression::Not(Box::new(project_boolean_expression(operand)))
        }
        CheckedBooleanExpression::Equal { left, right } => PackageReviewBooleanExpression::Equal {
            left: Box::new(project_boolean_expression(left)),
            right: Box::new(project_boolean_expression(right)),
        },
        CheckedBooleanExpression::IntegerComparison { kind, left, right } => {
            PackageReviewBooleanExpression::IntegerComparison {
                kind: project_integer_comparison_kind(*kind),
                left: Box::new(project_scalar_expression(left)),
                right: Box::new(project_scalar_expression(right)),
            }
        }
        CheckedBooleanExpression::IeeeFloatComparison {
            kind,
            primitive_type,
            left,
            right,
        } => PackageReviewBooleanExpression::IeeeFloatComparison {
            kind: match kind {
                psi_checked_trees::CheckedIeeeFloatComparisonKind::Equal => {
                    PackageReviewIeeeFloatComparisonKind::Equal
                }
                psi_checked_trees::CheckedIeeeFloatComparisonKind::NotEqual => {
                    PackageReviewIeeeFloatComparisonKind::NotEqual
                }
            },
            primitive_type: project_primitive_type(*primitive_type),
            left: project_structural_field(left),
            right: project_structural_field(right),
        },
        CheckedBooleanExpression::ByteSequenceEqual { left, right } => {
            PackageReviewBooleanExpression::ByteSequenceEqual {
                left: project_structural_field(left),
                right: project_structural_field(right),
            }
        }
        CheckedBooleanExpression::PayloadlessSumEqual { left, right, cases } => {
            PackageReviewBooleanExpression::PayloadlessSumEqual {
                left: project_structural_field(left),
                right: project_structural_field(right),
                cases: cases.clone(),
            }
        }
        CheckedBooleanExpression::StructuralCaseMembership { subject, case } => {
            PackageReviewBooleanExpression::StructuralCaseMembership {
                subject: project_structural_field(subject),
                case: case.clone(),
            }
        }
        CheckedBooleanExpression::And { left, right } => PackageReviewBooleanExpression::And {
            left: Box::new(project_boolean_expression(left)),
            right: Box::new(project_boolean_expression(right)),
        },
        CheckedBooleanExpression::Or { left, right } => PackageReviewBooleanExpression::Or {
            left: Box::new(project_boolean_expression(left)),
            right: Box::new(project_boolean_expression(right)),
        },
    }
}

fn project_scalar_expression(
    expression: &psi_checked_trees::CheckedScalarExpression,
) -> PackageReviewScalarExpression {
    use psi_checked_trees::CheckedScalarExpression;

    match expression {
        CheckedScalarExpression::Parameter {
            position,
            primitive_type,
        } => PackageReviewScalarExpression::Parameter {
            position: *position,
            primitive_type: project_primitive_type(*primitive_type),
        },
        CheckedScalarExpression::Local {
            position,
            primitive_type,
        } => PackageReviewScalarExpression::Local {
            position: *position,
            primitive_type: project_primitive_type(*primitive_type),
        },
        CheckedScalarExpression::StructuralParameterField {
            parameter_position,
            path,
            primitive_type,
        } => PackageReviewScalarExpression::StructuralParameterField {
            parameter_position: *parameter_position,
            path: project_structural_path(path),
            primitive_type: project_primitive_type(*primitive_type),
        },
        CheckedScalarExpression::IntegerLiteral { literal } => {
            PackageReviewScalarExpression::IntegerLiteral(PackageReviewIntegerLiteral {
                canonical_text: literal.text().to_owned(),
                landing: literal
                    .landing()
                    .map(|landing| PackageReviewIntegerLiteralLanding {
                        landed_type: project_landed_integer_type(landing.landed_type),
                        arithmetic_domain: project_arithmetic_domain(landing.domain),
                    }),
            })
        }
        CheckedScalarExpression::IntegerBinary {
            kind,
            primitive_type,
            left,
            right,
        } => PackageReviewScalarExpression::IntegerBinary {
            kind: project_integer_binary_kind(*kind),
            primitive_type: project_primitive_type(*primitive_type),
            left: Box::new(project_scalar_expression(left)),
            right: Box::new(project_scalar_expression(right)),
        },
        CheckedScalarExpression::IntegerBitwiseNot {
            primitive_type,
            operand,
        } => PackageReviewScalarExpression::IntegerBitwiseNot {
            primitive_type: project_primitive_type(*primitive_type),
            operand: Box::new(project_scalar_expression(operand)),
        },
        CheckedScalarExpression::IntegerWiden {
            primitive_type,
            operand,
        } => PackageReviewScalarExpression::IntegerWiden {
            primitive_type: project_primitive_type(*primitive_type),
            operand: Box::new(project_scalar_expression(operand)),
        },
        CheckedScalarExpression::IntegerExactCast {
            primitive_type,
            operand,
            range,
        } => PackageReviewScalarExpression::IntegerExactCast {
            primitive_type: project_primitive_type(*primitive_type),
            operand: Box::new(project_scalar_expression(operand)),
            range: PackageReviewIntegerRange {
                minimum: range.minimum.to_string(),
                maximum: range.maximum.to_string(),
            },
        },
        CheckedScalarExpression::Boolean(expression) => {
            PackageReviewScalarExpression::Boolean(Box::new(project_boolean_expression(expression)))
        }
    }
}

fn project_structural_field(
    field: &psi_checked_trees::CheckedStructuralParameterField,
) -> PackageReviewStructuralParameterField {
    PackageReviewStructuralParameterField {
        parameter_position: field.parameter_position,
        path: project_structural_path(&field.path),
    }
}

fn project_structural_path(
    path: &[psi_checked_trees::CheckedStructuralPredicatePathSegment],
) -> Vec<PackageReviewStructuralPredicatePathSegment> {
    path.iter()
        .map(|segment| match segment {
            psi_checked_trees::CheckedStructuralPredicatePathSegment::Field(field) => {
                PackageReviewStructuralPredicatePathSegment::Field(field.clone())
            }
            psi_checked_trees::CheckedStructuralPredicatePathSegment::Case(case) => {
                PackageReviewStructuralPredicatePathSegment::Case(case.clone())
            }
        })
        .collect()
}

const fn project_primitive_type(
    primitive_type: psi_typed_trees::types::PrimitiveType,
) -> PackageReviewPrimitiveType {
    use psi_typed_trees::types::PrimitiveType;
    match primitive_type {
        PrimitiveType::Bool => PackageReviewPrimitiveType::Bool,
        PrimitiveType::F32 => PackageReviewPrimitiveType::F32,
        PrimitiveType::F64 => PackageReviewPrimitiveType::F64,
        PrimitiveType::I8 => PackageReviewPrimitiveType::I8,
        PrimitiveType::I16 => PackageReviewPrimitiveType::I16,
        PrimitiveType::I32 => PackageReviewPrimitiveType::I32,
        PrimitiveType::I64 => PackageReviewPrimitiveType::I64,
        PrimitiveType::U8 => PackageReviewPrimitiveType::U8,
        PrimitiveType::U16 => PackageReviewPrimitiveType::U16,
        PrimitiveType::U32 => PackageReviewPrimitiveType::U32,
        PrimitiveType::U64 => PackageReviewPrimitiveType::U64,
        PrimitiveType::Addr => PackageReviewPrimitiveType::Addr,
    }
}

const fn project_landed_integer_type(
    landed_type: psi_numerics::literals::LandedIntegerType,
) -> PackageReviewPrimitiveType {
    use psi_numerics::literals::LandedIntegerType;
    match landed_type {
        LandedIntegerType::I8 => PackageReviewPrimitiveType::I8,
        LandedIntegerType::I16 => PackageReviewPrimitiveType::I16,
        LandedIntegerType::I32 => PackageReviewPrimitiveType::I32,
        LandedIntegerType::I64 => PackageReviewPrimitiveType::I64,
        LandedIntegerType::U8 => PackageReviewPrimitiveType::U8,
        LandedIntegerType::U16 => PackageReviewPrimitiveType::U16,
        LandedIntegerType::U32 => PackageReviewPrimitiveType::U32,
        LandedIntegerType::U64 => PackageReviewPrimitiveType::U64,
        LandedIntegerType::Addr => PackageReviewPrimitiveType::Addr,
    }
}

const fn project_arithmetic_domain(
    domain: psi_numerics::arithmetic::ArithmeticDomain,
) -> PackageReviewArithmeticDomain {
    use psi_numerics::arithmetic::ArithmeticDomain;
    match domain {
        ArithmeticDomain::Exact => PackageReviewArithmeticDomain::Exact,
        ArithmeticDomain::Wrapping => PackageReviewArithmeticDomain::Wrapping,
        ArithmeticDomain::Saturating => PackageReviewArithmeticDomain::Saturating,
        ArithmeticDomain::Trapping => PackageReviewArithmeticDomain::Trapping,
    }
}

const fn project_integer_comparison_kind(
    kind: psi_checked_trees::CheckedIntegerComparisonKind,
) -> PackageReviewIntegerComparisonKind {
    match kind {
        psi_checked_trees::CheckedIntegerComparisonKind::Equal => {
            PackageReviewIntegerComparisonKind::Equal
        }
        psi_checked_trees::CheckedIntegerComparisonKind::LessThan => {
            PackageReviewIntegerComparisonKind::LessThan
        }
        psi_checked_trees::CheckedIntegerComparisonKind::LessOrEqual => {
            PackageReviewIntegerComparisonKind::LessOrEqual
        }
    }
}

const fn project_integer_binary_kind(
    kind: psi_checked_trees::CheckedIntegerBinaryKind,
) -> PackageReviewIntegerBinaryKind {
    use psi_checked_trees::CheckedIntegerBinaryKind;
    match kind {
        CheckedIntegerBinaryKind::ExactAdd => PackageReviewIntegerBinaryKind::ExactAdd,
        CheckedIntegerBinaryKind::ExactSubtract => PackageReviewIntegerBinaryKind::ExactSubtract,
        CheckedIntegerBinaryKind::ExactMultiply => PackageReviewIntegerBinaryKind::ExactMultiply,
        CheckedIntegerBinaryKind::ExactDivide => PackageReviewIntegerBinaryKind::ExactDivide,
        CheckedIntegerBinaryKind::ExactRemainder => PackageReviewIntegerBinaryKind::ExactRemainder,
        CheckedIntegerBinaryKind::WrappingDivide => PackageReviewIntegerBinaryKind::WrappingDivide,
        CheckedIntegerBinaryKind::WrappingRemainder => {
            PackageReviewIntegerBinaryKind::WrappingRemainder
        }
        CheckedIntegerBinaryKind::SaturatingDivide => {
            PackageReviewIntegerBinaryKind::SaturatingDivide
        }
        CheckedIntegerBinaryKind::SaturatingRemainder => {
            PackageReviewIntegerBinaryKind::SaturatingRemainder
        }
        CheckedIntegerBinaryKind::WrappingAdd => PackageReviewIntegerBinaryKind::WrappingAdd,
        CheckedIntegerBinaryKind::SaturatingAdd => PackageReviewIntegerBinaryKind::SaturatingAdd,
        CheckedIntegerBinaryKind::WrappingSubtract => {
            PackageReviewIntegerBinaryKind::WrappingSubtract
        }
        CheckedIntegerBinaryKind::SaturatingSubtract => {
            PackageReviewIntegerBinaryKind::SaturatingSubtract
        }
        CheckedIntegerBinaryKind::WrappingMultiply => {
            PackageReviewIntegerBinaryKind::WrappingMultiply
        }
        CheckedIntegerBinaryKind::SaturatingMultiply => {
            PackageReviewIntegerBinaryKind::SaturatingMultiply
        }
        CheckedIntegerBinaryKind::BitwiseAnd => PackageReviewIntegerBinaryKind::BitwiseAnd,
        CheckedIntegerBinaryKind::BitwiseOr => PackageReviewIntegerBinaryKind::BitwiseOr,
        CheckedIntegerBinaryKind::BitwiseXor => PackageReviewIntegerBinaryKind::BitwiseXor,
        CheckedIntegerBinaryKind::WrappingShiftLeft => {
            PackageReviewIntegerBinaryKind::WrappingShiftLeft
        }
        CheckedIntegerBinaryKind::WrappingShiftRight => {
            PackageReviewIntegerBinaryKind::WrappingShiftRight
        }
        CheckedIntegerBinaryKind::ExactShiftLeft => PackageReviewIntegerBinaryKind::ExactShiftLeft,
        CheckedIntegerBinaryKind::ExactShiftRight => {
            PackageReviewIntegerBinaryKind::ExactShiftRight
        }
    }
}

fn project_crash_predicates(
    predicates: &[psi_checked_trees::CrashPredicateIdentity],
) -> Vec<PackageReviewCrashPredicate> {
    let mut projected = predicates
        .iter()
        .map(project_crash_predicate)
        .collect::<Vec<_>>();
    projected.sort();
    projected.dedup();
    projected
}

fn project_crash_predicate(
    predicate: &psi_checked_trees::CrashPredicateIdentity,
) -> PackageReviewCrashPredicate {
    PackageReviewCrashPredicate {
        canonical_bytes: predicate.canonical_bytes().to_vec(),
    }
}

fn project_permission_claim(
    compilation: &CheckedCompilation,
    claim: psi_language_semantics::PermissionClaimIdentity,
) -> Result<PackageReviewPermissionClaim, Vec<Diagnostic>> {
    let psi_language_semantics::PermissionClaimIdentity::Established {
        machine_symbol,
        state_symbol,
        source,
        ordinal,
    } = claim
    else {
        return Err(vec![Diagnostic::error(
            "package review crash frontier contains an unidentified permission claim",
        )]);
    };
    let source = match source {
        psi_language_semantics::PermissionEventSource::StateEntry => {
            PackageReviewPermissionSource::StateEntry
        }
        psi_language_semantics::PermissionEventSource::Statement { statement_index } => {
            PackageReviewPermissionSource::Statement {
                statement_ordinal: portable_ordinal(statement_index)?,
            }
        }
        psi_language_semantics::PermissionEventSource::Call {
            statement_index,
            call_ordinal,
            target_symbol,
        } => PackageReviewPermissionSource::Call {
            statement_ordinal: portable_ordinal(statement_index)?,
            call_ordinal: portable_ordinal(call_ordinal)?,
            target: nominal_identity(compilation, target_symbol)?,
        },
        psi_language_semantics::PermissionEventSource::StateExit => {
            PackageReviewPermissionSource::StateExit
        }
    };
    Ok(PackageReviewPermissionClaim {
        machine: nominal_identity(compilation, machine_symbol)?,
        state: nominal_identity(compilation, state_symbol)?,
        source,
        ordinal,
    })
}

fn portable_ordinal(ordinal: usize) -> Result<u64, Vec<Diagnostic>> {
    u64::try_from(ordinal).map_err(|_| {
        vec![Diagnostic::error(
            "package review semantic ordinal exceeds the portable identity range",
        )]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structural_runtime_requirement_crosses_as_closed_review_evidence() {
        let source = psi_checked_trees::CheckedBooleanExpression::IntegerComparison {
            kind: psi_checked_trees::CheckedIntegerComparisonKind::LessOrEqual,
            left: Box::new(psi_checked_trees::CheckedScalarExpression::IntegerLiteral {
                literal: psi_numerics::literals::IntegerLiteral::from_value(7).with_landing(
                    psi_numerics::literals::IntegerLanding {
                        landed_type: psi_numerics::literals::LandedIntegerType::U32,
                        domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
                    },
                ),
            }),
            right: Box::new(
                psi_checked_trees::CheckedScalarExpression::IntegerExactCast {
                    primitive_type: psi_typed_trees::types::PrimitiveType::U32,
                    operand: Box::new(psi_checked_trees::CheckedScalarExpression::Parameter {
                        position: 3,
                        primitive_type: psi_typed_trees::types::PrimitiveType::U16,
                    }),
                    range: psi_checked_trees::CheckedIntegerRange {
                        minimum: psi_numerics::bignum::BigInt::from_u64(0),
                        maximum: psi_numerics::bignum::BigInt::from_u64(u32::MAX as u64),
                    },
                },
            ),
        };

        let PackageReviewBooleanExpression::IntegerComparison { kind, left, right } =
            project_boolean_expression(&source)
        else {
            panic!("integer comparison must retain its closed review shape")
        };
        assert_eq!(kind, PackageReviewIntegerComparisonKind::LessOrEqual);
        assert!(matches!(
            left.as_ref(),
            PackageReviewScalarExpression::IntegerLiteral(literal)
                if literal.canonical_text() == "7"
                    && literal.landing().is_some_and(|landing|
                        landing.landed_type() == PackageReviewPrimitiveType::U32
                            && landing.arithmetic_domain() == PackageReviewArithmeticDomain::Exact)
        ));
        assert!(matches!(
            right.as_ref(),
            PackageReviewScalarExpression::IntegerExactCast {
                primitive_type: PackageReviewPrimitiveType::U32,
                operand,
                range,
            } if range.minimum() == "0"
                && range.maximum() == u32::MAX.to_string()
                && matches!(
                    operand.as_ref(),
                    PackageReviewScalarExpression::Parameter {
                        position: 3,
                        primitive_type: PackageReviewPrimitiveType::U16,
                    }
                )
        ));
    }

    #[test]
    fn crash_cause_crosses_the_review_boundary_as_closed_evidence() {
        assert_eq!(
            project_crash_cause(psi_checked_trees::CrashCause::Trap),
            PackageReviewCrashCause::Trap,
        );
        assert_eq!(
            project_crash_cause(psi_checked_trees::CrashCause::Abort),
            PackageReviewCrashCause::Abort,
        );
    }
}
