use super::super::super::{LegalizationError, ScalarType, ValueShape};
use super::super::{AbstractOperation, TargetUnitOperation, ValueId};
use super::Source;
use semantic_vocabulary::IeeeFloatFormat;
use target_operations::{TargetIeeeFloatFmaOperand, TargetUnitScalarHomeRequirement};

/// Replay IEEE operations against their exact ordered, available scalar sources.
pub(super) fn validate(
    target: &TargetUnitOperation,
    abstracted: &AbstractOperation,
    sources: &mut Vec<(ValueId, Source)>,
) -> Result<(), LegalizationError> {
    let invalid = LegalizationError::custody();
    match (target, abstracted) {
        (
            TargetUnitOperation::IeeeFloatCompare {
                result_home,
                comparison,
                format,
                left,
                right,
            },
            AbstractOperation::IeeeFloatCompare {
                psi_operation,
                result,
                comparison: expected_comparison,
                format: expected_format,
                left: expected_left,
                right: expected_right,
            },
        ) => {
            if result_home.defining_operation != *psi_operation
                || result_home.source_value != *result
                || result_home.scalar_type != ScalarType::Boolean
                || result_home.shape != ValueShape::integer(1, 1)
                || comparison != expected_comparison
                || format != expected_format
                || left.source_value() != *expected_left
                || right.source_value() != *expected_right
                || left.scalar_type() != ScalarType::IeeeFloat(*format)
                || right.scalar_type() != ScalarType::IeeeFloat(*format)
                || !sources
                    .iter()
                    .filter(|(value, _)| value == expected_left)
                    .map(|(_, source)| source)
                    .eq(std::iter::once(left))
                || !sources
                    .iter()
                    .filter(|(value, _)| value == expected_right)
                    .map(|(_, source)| source)
                    .eq(std::iter::once(right))
            {
                return Err(invalid);
            }
            sources.push((*result, Source::Home(*result_home)));
        }
        (
            TargetUnitOperation::IeeeFloatConstant {
                psi_operation,
                result,
                value,
            },
            AbstractOperation::IeeeFloatConstant {
                psi_operation: expected_operation,
                result: expected_result,
                value: expected_value,
            },
        ) if psi_operation == expected_operation
            && result == expected_result
            && value == expected_value =>
        {
            sources.push((
                *result,
                Source::IeeeFloatImmediate {
                    defining_operation: *psi_operation,
                    source_value: *result,
                    value: *value,
                },
            ));
        }
        (
            TargetUnitOperation::NearestIeeeFloatFusedMultiplyAdd {
                psi_operation,
                result,
                format,
                left,
                right,
                addend,
                settlement,
            },
            AbstractOperation::NearestIeeeFloatFusedMultiplyAdd {
                psi_operation: expected_operation,
                result: expected_result,
                format: expected_format,
                left: expected_left,
                right: expected_right,
                addend: expected_addend,
            },
        ) => {
            // Each operand names one exact preceding IEEE constant: the
            // retained source must be that constant's own immediate record,
            // so a same-valued or same-typed producer cannot substitute.
            let exact_operand = |operand: &TargetIeeeFloatFmaOperand, expected: &ValueId| {
                operand.source_value == *expected
                    && sources
                        .iter()
                        .filter(|(value, _)| value == expected)
                        .map(|(_, source)| source)
                        .eq(std::iter::once(&Source::IeeeFloatImmediate {
                            defining_operation: operand.defining_operation,
                            source_value: operand.source_value,
                            value: operand.value,
                        }))
            };
            if psi_operation != expected_operation
                || result != expected_result
                || format != expected_format
                || settlement.terminal_operation != *expected_operation
                || settlement.format != *expected_format
                || !exact_operand(left, expected_left)
                || !exact_operand(right, expected_right)
                || !exact_operand(addend, expected_addend)
            {
                return Err(invalid);
            }
            sources.push((
                *result,
                Source::Home(TargetUnitScalarHomeRequirement {
                    defining_operation: *psi_operation,
                    source_value: *result,
                    scalar_type: ScalarType::IeeeFloat(*format),
                    shape: ValueShape::float(match format {
                        IeeeFloatFormat::Binary32 => 4,
                        IeeeFloatFormat::Binary64 => 8,
                    }),
                }),
            ));
        }
        _ => return Err(invalid),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        AbstractOperation, IeeeFloatFormat, ScalarType, Source, TargetIeeeFloatFmaOperand,
        TargetUnitOperation, TargetUnitScalarHomeRequirement, ValueId, ValueShape, validate,
    };
    use semantic_vocabulary::{IeeeFloatValue, OperationId};
    use target::{
        AdmittedX86ScalarFmaProvider, TargetProfile, X86_SCALAR_FMA_REQUIRED_FEATURES,
        X86ScalarFmaSlot,
    };
    use target_operations::TargetX86ScalarFmaSettlement;

    fn op(n: u64) -> OperationId {
        OperationId::new(n).unwrap()
    }

    fn val(n: u64) -> ValueId {
        ValueId::new(n).unwrap()
    }

    const LEFT_BITS: u64 = 0x3ff0_0000_0000_0000;
    const RIGHT_BITS: u64 = 0x4000_0000_0000_0000;
    const ADDEND_BITS: u64 = 0xbff8_0000_0000_0000;

    fn immediate(operation: u64, value: u64, bits: u64) -> (ValueId, Source) {
        (
            val(value),
            Source::IeeeFloatImmediate {
                defining_operation: op(operation),
                source_value: val(value),
                value: IeeeFloatValue::Binary64(bits),
            },
        )
    }

    fn operand(operation: u64, value: u64, bits: u64) -> TargetIeeeFloatFmaOperand {
        TargetIeeeFloatFmaOperand {
            defining_operation: op(operation),
            source_value: val(value),
            value: IeeeFloatValue::Binary64(bits),
        }
    }

    fn fixture() -> (
        TargetUnitOperation,
        AbstractOperation,
        Vec<(ValueId, Source)>,
    ) {
        let sources = vec![
            immediate(1, 11, LEFT_BITS),
            immediate(2, 12, RIGHT_BITS),
            immediate(3, 13, ADDEND_BITS),
        ];
        let settlement = TargetX86ScalarFmaSettlement {
            terminal_operation: op(9),
            provider_plan_report_identity: 7,
            provider_plan_digest: [0x42; 32],
            format: IeeeFloatFormat::Binary64,
            slot: X86ScalarFmaSlot::Binary64,
            provider: AdmittedX86ScalarFmaProvider::from_deployment_claim(
                TargetProfile::LinuxX64,
                &X86_SCALAR_FMA_REQUIRED_FEATURES,
            )
            .expect("canonical FMA provider fixture"),
        };
        let target = TargetUnitOperation::NearestIeeeFloatFusedMultiplyAdd {
            psi_operation: op(9),
            result: val(90),
            format: IeeeFloatFormat::Binary64,
            left: operand(1, 11, LEFT_BITS),
            right: operand(2, 12, RIGHT_BITS),
            addend: operand(3, 13, ADDEND_BITS),
            settlement,
        };
        let abstracted = AbstractOperation::NearestIeeeFloatFusedMultiplyAdd {
            psi_operation: op(9),
            result: val(90),
            format: IeeeFloatFormat::Binary64,
            left: val(11),
            right: val(12),
            addend: val(13),
        };
        (target, abstracted, sources)
    }

    #[test]
    fn fused_multiply_add_replays_exact_constant_sources() {
        let (target, abstracted, mut sources) = fixture();
        validate(&target, &abstracted, &mut sources).expect("exact operands replay");
        assert_eq!(
            sources.last(),
            Some(&(
                val(90),
                Source::Home(TargetUnitScalarHomeRequirement {
                    defining_operation: op(9),
                    source_value: val(90),
                    scalar_type: ScalarType::IeeeFloat(IeeeFloatFormat::Binary64),
                    shape: ValueShape::float(8),
                })
            ))
        );
    }

    #[test]
    fn fused_multiply_add_rejects_any_operand_drift() {
        for mutation in [
            "bits",
            "operation",
            "value",
            "order",
            "format",
            "settlement operation",
            "settlement format",
            "non-immediate source",
        ] {
            let (mut target, abstracted, mut sources) = fixture();
            let TargetUnitOperation::NearestIeeeFloatFusedMultiplyAdd {
                left,
                right,
                addend,
                format,
                settlement,
                ..
            } = &mut target
            else {
                unreachable!()
            };
            match mutation {
                "bits" => left.value = IeeeFloatValue::Binary64(LEFT_BITS ^ 1),
                "operation" => right.defining_operation = op(99),
                "value" => addend.source_value = val(99),
                "order" => addend.source_value = val(11),
                "format" => *format = IeeeFloatFormat::Binary32,
                "settlement operation" => settlement.terminal_operation = op(8),
                "settlement format" => settlement.format = IeeeFloatFormat::Binary32,
                _ => {
                    sources[2].1 = Source::Home(TargetUnitScalarHomeRequirement {
                        defining_operation: op(3),
                        source_value: val(13),
                        scalar_type: ScalarType::IeeeFloat(IeeeFloatFormat::Binary64),
                        shape: ValueShape::float(8),
                    })
                }
            }
            assert!(
                validate(&target, &abstracted, &mut sources).is_err(),
                "{mutation}"
            );
        }
    }
}
