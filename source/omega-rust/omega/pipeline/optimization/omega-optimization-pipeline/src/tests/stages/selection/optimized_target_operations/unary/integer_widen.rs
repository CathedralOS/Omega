use super::*;

#[test]
fn optimized_target_lowering_retains_exact_integer_widen_parameter_custody() {
    for (source_type, target_type) in native_widenings() {
        for (target_profile, register, stack) in parameter_location_cases() {
            for (parameter_count, expected_location) in [(1, register), (9, stack)] {
                let (semantic, proof) = integer_widen_parameter_return_artifact(
                    source_type,
                    target_type,
                    parameter_count,
                );
                let optimized = optimize_artifact_sections(
                    &semantic,
                    &proof,
                    &AdmissionProfile::default(),
                    request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
                )
                .unwrap();
                let target =
                    lower_optimized_to_target_operations(optimized, target_profile).unwrap();
                let receipt = target.translation_validation();
                let AbstractToTargetFunctionTranslationDisposition::Validated(
                    AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerWidenParameter(
                        row,
                    ),
                ) = receipt.function_roster()[0].translation()
                else {
                    panic!("optimized integer widen must retain its family row")
                };
                assert_eq!(row.machine(), MachineId::new(30_001).unwrap());
                assert_eq!(row.widen_operation(), OperationId::new(30_005).unwrap());
                assert_eq!(row.return_edge(), EdgeId::new(30_006).unwrap());
                assert_eq!(row.source_value(), ValueId::new(30_003).unwrap());
                assert_eq!(row.source_type(), source_type);
                assert_eq!(row.target_type(), target_type);
                assert_eq!(
                    row.operand_value(),
                    ValueId::new(30_100 + parameter_count as u64 - 1).unwrap()
                );
                assert_eq!(row.parameter_index(), parameter_count - 1);
                assert_eq!(row.location(), expected_location);
                assert!(matches!(
                    &target.target_operations().functions[0].operation,
                    TargetOperation::ReturnIntegerExpression {
                        scalar_type,
                        expression: TargetIntegerExpression::IntegerWiden {
                            source_type: target_source_type,
                            operand,
                            ..
                        },
                        ..
                    } if *scalar_type == target_type
                        && *target_source_type == source_type
                        && matches!(
                            operand.as_ref(),
                            TargetIntegerExpression::Parameter { location, .. }
                                if *location == expected_location
                        )
                ));
            }
        }
    }
}

fn native_widenings() -> Vec<(IntegerType, IntegerType)> {
    let integer = |sign, bits| IntegerType::new(sign, bits).expect("native integer");
    let mut widenings = Vec::new();
    for sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
        for source_bits in [8, 16, 32] {
            for target_bits in [16, 32, 64] {
                if source_bits < target_bits {
                    widenings.push((integer(sign, source_bits), integer(sign, target_bits)));
                }
            }
        }
    }
    for source_bits in [8, 16, 32] {
        for target_bits in [16, 32, 64] {
            if source_bits < target_bits {
                widenings.push((
                    integer(IntegerSign::Unsigned, source_bits),
                    integer(IntegerSign::Signed, target_bits),
                ));
            }
        }
    }
    assert_eq!(widenings.len(), 18);
    widenings
}
