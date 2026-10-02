//! Validate an IEEE result adapter independently of its arithmetic producer.
//!
//! The adapter consumes already evaluated values. Its result format determines
//! every operand format; no implicit conversion or reinterpretation is allowed.
//! Source replay separately binds these identities and the saturation rule to
//! the checked call. Terminal formation cannot infer that source relationship.
//!
//! A Trapping adapter owns an operation-level Trap site. Its failure predicate
//! is the complement of the open interval (-infinity, +infinity), using IEEE
//! comparisons so that unordered NaNs fail too. These are verifier propositions,
//! not invented authored comparison operations. The normal continuation alone
//! learns that the preserved payload is finite.

use std::collections::{BTreeMap, BTreeSet};

use semantic_vocabulary::{
    IeeeFloatComparisonKind, IeeeFloatFormat, IeeeFloatValue, Proposition, ScalarTerm, ScalarType,
    ValueId,
};
use terminal_psi::{CrashCause, FloatResultPolicy, Operation, OperationKind, TerminalMachine};

use super::ModuleError;
use super::operations::require_defined;

pub(crate) fn format(operation: &Operation) -> Result<IeeeFloatFormat, ModuleError> {
    match operation
        .result
        .scalar_ref()
        .map(|result| result.scalar_type)
    {
        Some(ScalarType::IeeeFloat(format)) => Ok(format),
        _ => Err(ModuleError::FloatResultPolicyRequiresFloatResult(
            operation.id,
        )),
    }
}

pub(super) fn validate_operands(
    operation: &Operation,
    policy: FloatResultPolicy,
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    let expected = ScalarType::IeeeFloat(format(operation)?);
    for operand in policy.operands() {
        require_defined(operand, value_types, defined)?;
        if value_types.get(&operand) != Some(&expected) {
            return Err(ModuleError::FloatResultPolicyOperandTypeMismatch {
                operation: operation.id,
                operand,
                expected,
            });
        }
    }
    Ok(())
}

/// Structural formation requires a same-cause route. Guarded routes still owe
/// their coverage proof under the operation's independently reconstructed failure
/// predicate; a declared guard is never assumed at the site.
pub(super) fn validate_crash_sites(machine: &TerminalMachine) -> Result<(), ModuleError> {
    let has_route = machine
        .contract
        .crash_routes
        .iter()
        .any(|route| route.cause == CrashCause::Trap && !route.alternatives.is_empty());
    for block in &machine.blocks {
        for operation in &block.operations {
            if matches!(operation.kind, OperationKind::FloatResultPolicy { policy } if policy.can_trap())
                && !has_route
            {
                return Err(ModuleError::FloatResultPolicyCrashUncovered {
                    machine: machine.id,
                    block: block.id,
                    operation: operation.id,
                });
            }
        }
    }
    Ok(())
}

pub(crate) fn failure_condition(
    operation: &Operation,
    raw_result: ValueId,
) -> Result<Proposition, ModuleError> {
    let format = format(operation)?;
    Ok(Proposition::Disjunction(finite_bounds(
        format,
        raw_result,
        IeeeFloatComparisonKind::NotLessThan,
    )))
}

pub(crate) fn normal_return_facts(
    operation: &Operation,
    raw_result: ValueId,
) -> Result<Vec<Proposition>, ModuleError> {
    let format = format(operation)?;
    let result =
        operation
            .result
            .scalar_ref()
            .ok_or(ModuleError::FloatResultPolicyRequiresFloatResult(
                operation.id,
            ))?;
    let mut facts = finite_bounds(format, result.id, IeeeFloatComparisonKind::LessThan);
    facts.push(Proposition::Equal(
        ScalarTerm::value(result.id, result.scalar_type),
        ScalarTerm::value(raw_result, result.scalar_type),
    ));
    Ok(facts)
}

/// Saturation has a conditional range, independently of who produced the raw
/// payload. A raw NaN stays NaN. Finite division by zero preserves infinity.
/// The arithmetic producer's normal contract must establish those premises.
pub(crate) fn saturating_return_facts(
    operation: &Operation,
    policy: FloatResultPolicy,
) -> Result<Vec<Proposition>, ModuleError> {
    let FloatResultPolicy::Saturating {
        raw_result,
        left,
        right,
        rule,
    } = policy
    else {
        return Ok(Vec::new());
    };
    let format = format(operation)?;
    let scalar_type = ScalarType::IeeeFloat(format);
    let result =
        operation
            .result
            .scalar_ref()
            .ok_or(ModuleError::FloatResultPolicyRequiresFloatResult(
                operation.id,
            ))?;
    let comparison = |kind, left, right| Proposition::ScalarIeeeFloatComparison {
        kind,
        format,
        left,
        right,
    };
    let (minimum, maximum, zero) = match format {
        IeeeFloatFormat::Binary32 => (
            IeeeFloatValue::Binary32(0xff7f_ffff),
            IeeeFloatValue::Binary32(0x7f7f_ffff),
            IeeeFloatValue::Binary32(0),
        ),
        IeeeFloatFormat::Binary64 => (
            IeeeFloatValue::Binary64(0xffef_ffff_ffff_ffff),
            IeeeFloatValue::Binary64(0x7fef_ffff_ffff_ffff),
            IeeeFloatValue::Binary64(0),
        ),
    };
    let bounds = |value| {
        vec![
            comparison(
                IeeeFloatComparisonKind::LessOrEqual,
                ScalarTerm::IeeeFloatLiteral(minimum),
                ScalarTerm::value(value, scalar_type),
            ),
            comparison(
                IeeeFloatComparisonKind::LessOrEqual,
                ScalarTerm::value(value, scalar_type),
                ScalarTerm::IeeeFloatLiteral(maximum),
            ),
        ]
    };
    let mut requirements = bounds(left);
    requirements.extend(bounds(right));
    requirements.push(comparison(
        IeeeFloatComparisonKind::Equal,
        ScalarTerm::value(raw_result, scalar_type),
        ScalarTerm::value(raw_result, scalar_type),
    ));
    if rule == terminal_psi::FloatSaturationRule::FiniteDivisionOverflow {
        requirements.push(comparison(
            IeeeFloatComparisonKind::NotEqual,
            ScalarTerm::value(right, scalar_type),
            ScalarTerm::IeeeFloatLiteral(zero),
        ));
    }
    Ok(vec![Proposition::Implication {
        premise: Box::new(Proposition::Conjunction(requirements)),
        conclusion: Box::new(Proposition::Conjunction(bounds(result.id))),
    }])
}

fn finite_bounds(
    format: IeeeFloatFormat,
    value: ValueId,
    kind: IeeeFloatComparisonKind,
) -> Vec<Proposition> {
    let (negative_infinity, positive_infinity) = match format {
        IeeeFloatFormat::Binary32 => (
            IeeeFloatValue::Binary32(0xff80_0000),
            IeeeFloatValue::Binary32(0x7f80_0000),
        ),
        IeeeFloatFormat::Binary64 => (
            IeeeFloatValue::Binary64(0xfff0_0000_0000_0000),
            IeeeFloatValue::Binary64(0x7ff0_0000_0000_0000),
        ),
    };
    let payload = ScalarTerm::value(value, ScalarType::IeeeFloat(format));
    vec![
        Proposition::ScalarIeeeFloatComparison {
            kind,
            format,
            left: ScalarTerm::IeeeFloatLiteral(negative_infinity),
            right: payload.clone(),
        },
        Proposition::ScalarIeeeFloatComparison {
            kind,
            format,
            left: payload,
            right: ScalarTerm::IeeeFloatLiteral(positive_infinity),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use proof_admission::check_value_equality_denotation;
    use semantic_vocabulary::{OperationId, PropositionContext};
    use terminal_psi::{OperationResult, ValueDeclaration};

    #[test]
    fn float_result_policy_failure_is_exactly_nonfinite() {
        // Pin IEEE edge classes, including both NaN encodings and their sign;
        // evaluate the actual reconstructed proposition through proof denotation.
        let cases = [
            (0_u32, 0_u64, false),
            (0x8000_0000, 0x8000_0000_0000_0000, false),
            (1, 1, false),
            (0x8000_0001, 0x8000_0000_0000_0001, false),
            (0x7f7f_ffff, 0x7fef_ffff_ffff_ffff, false),
            (0xff7f_ffff, 0xffef_ffff_ffff_ffff, false),
            (0x7f80_0000, 0x7ff0_0000_0000_0000, true),
            (0xff80_0000, 0xfff0_0000_0000_0000, true),
            (0x7fc0_0001, 0x7ff8_0000_0000_0001, true),
            (0xffc0_0001, 0xfff8_0000_0000_0001, true),
            (0x7f80_0001, 0x7ff0_0000_0000_0001, true),
            (0xff80_0001, 0xfff0_0000_0000_0001, true),
        ];
        let raw_result = ValueId::new(1).unwrap();
        for (binary32, binary64, nonfinite) in cases {
            for bits in [
                IeeeFloatValue::Binary32(binary32),
                IeeeFloatValue::Binary64(binary64),
            ] {
                let scalar_type = ScalarType::IeeeFloat(bits.format());
                let operation = Operation {
                    static_reach_binding: None,
                    suspension_crossing: None,
                    id: OperationId::new(1).unwrap(),
                    result: OperationResult::Scalar(ValueDeclaration {
                        id: ValueId::new(2).unwrap(),
                        scalar_type,
                        qualifications: Default::default(),
                    }),
                    kind: OperationKind::FloatResultPolicy {
                        policy: FloatResultPolicy::Trapping { raw_result },
                    },
                };
                let context =
                    PropositionContext::from_value_types([(raw_result, scalar_type)]).unwrap();
                let payload = Proposition::Equal(
                    ScalarTerm::value(raw_result, scalar_type),
                    ScalarTerm::IeeeFloatLiteral(bits),
                );
                assert_eq!(
                    check_value_equality_denotation(
                        &context,
                        &failure_condition(&operation, raw_result).unwrap(),
                        [&payload]
                    )
                    .unwrap(),
                    if nonfinite {
                        Proposition::Truth
                    } else {
                        Proposition::Falsehood
                    },
                    "{bits:?}",
                );
            }
        }
    }
    #[test]
    fn saturation_guarantee_requires_ordered_raw_payload_and_nonzero_division() {
        use terminal_psi::FloatSaturationRule;
        let scalar_type = ScalarType::IeeeFloat(IeeeFloatFormat::Binary32);
        let identifiers = [1, 2, 3, 4].map(|id| ValueId::new(id).unwrap());
        let [raw_result, left, right, result] = identifiers;
        let context =
            PropositionContext::from_value_types(identifiers.map(|id| (id, scalar_type))).unwrap();
        let one = 1.0_f32.to_bits();
        let two = 2.0_f32.to_bits();
        let maximum = f32::MAX.to_bits();
        let infinity = f32::INFINITY.to_bits();
        let nan = f32::NAN.to_bits();
        for (rule, payloads, expected) in [
            (
                FloatSaturationRule::FiniteOperandOverflow,
                [infinity, maximum, two, maximum],
                true,
            ),
            (
                FloatSaturationRule::FiniteOperandOverflow,
                [infinity, maximum, two, infinity],
                false,
            ),
            (
                FloatSaturationRule::FiniteOperandOverflow,
                [nan, one, two, nan],
                true,
            ),
            (
                FloatSaturationRule::FiniteOperandOverflow,
                [infinity, infinity, two, infinity],
                true,
            ),
            (
                FloatSaturationRule::FiniteDivisionOverflow,
                [infinity, one, 0, infinity],
                true,
            ),
            (
                FloatSaturationRule::FiniteDivisionOverflow,
                [infinity, maximum, two, maximum],
                true,
            ),
            (
                FloatSaturationRule::FiniteDivisionOverflow,
                [infinity, maximum, two, infinity],
                false,
            ),
        ] {
            let policy = FloatResultPolicy::Saturating {
                raw_result,
                left,
                right,
                rule,
            };
            let operation = Operation {
                id: OperationId::new(1).unwrap(),
                static_reach_binding: None,
                suspension_crossing: None,
                result: OperationResult::Scalar(ValueDeclaration {
                    id: result,
                    scalar_type,
                    qualifications: Default::default(),
                }),
                kind: OperationKind::FloatResultPolicy { policy },
            };
            let facts = identifiers
                .into_iter()
                .zip(payloads)
                .map(|(id, bits)| {
                    Proposition::Equal(
                        ScalarTerm::value(id, scalar_type),
                        ScalarTerm::IeeeFloatLiteral(IeeeFloatValue::Binary32(bits)),
                    )
                })
                .collect::<Vec<_>>();
            let guarantees = saturating_return_facts(&operation, policy).unwrap();
            let guarantee = &guarantees[0];
            assert_eq!(
                check_value_equality_denotation(&context, guarantee, &facts).unwrap(),
                if expected {
                    Proposition::Truth
                } else {
                    Proposition::Falsehood
                },
                "{rule:?}: {payloads:?}"
            );
        }
    }
}
