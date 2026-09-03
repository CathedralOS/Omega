use super::*;

#[test]
fn optimized_target_lowering_retains_exact_integer_translation_custody() {
    let cases = [
        (
            IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
            IntegerValue::Unsigned(u8::MAX.into()),
        ),
        (
            IntegerType::new(IntegerSign::Signed, 64).unwrap(),
            IntegerValue::Signed(-37),
        ),
    ];
    for target_profile in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        for (integer_type, value) in cases {
            let (semantic, proof) = integer_literal_return_artifact(integer_type, value);
            let optimized = optimize_artifact_sections(
                &semantic,
                &proof,
                &AdmissionProfile::default(),
                request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
            )
            .unwrap();
            let target = lower_optimized_to_target_operations(optimized, target_profile).unwrap();
            let receipt = target.translation_validation();
            assert_eq!(receipt.target(), target_profile);
            assert_eq!(receipt.psi(), target.optimized().plan().psi);
            assert_eq!(receipt.entry(), target.optimized().plan().entry);
            assert_eq!(receipt.function_count(), 1);
            let AbstractToTargetFunctionTranslationDisposition::Validated(
                AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerImmediate(row),
            ) = receipt.function_roster()[0].translation()
            else {
                panic!("optimized literal lowering must retain its validated family row")
            };
            assert_eq!(
                row.machine(),
                target.optimized().plan().functions[0].machine
            );
            assert_eq!(row.scalar_type(), integer_type);
            assert_eq!(row.value(), value);
            assert!(matches!(
                target.target_operations().functions[0].operation,
                TargetOperation::ReturnIntegerImmediate {
                    scalar_type,
                    value: target_value,
                    ..
                } if scalar_type == integer_type && target_value == value
            ));
        }
    }
}

#[test]
fn optimized_target_lowering_retains_exact_boolean_translation_custody() {
    for target_profile in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        for value in [false, true] {
            let (semantic, proof) = boolean_literal_return_artifact(value);
            let optimized = optimize_artifact_sections(
                &semantic,
                &proof,
                &AdmissionProfile::default(),
                request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
            )
            .unwrap();
            let target = lower_optimized_to_target_operations(optimized, target_profile).unwrap();
            let receipt = target.translation_validation();
            assert_eq!(receipt.target(), target_profile);
            assert_eq!(receipt.psi(), target.optimized().plan().psi);
            assert_eq!(receipt.entry(), target.optimized().plan().entry);
            assert_eq!(receipt.function_count(), 1);
            let AbstractToTargetFunctionTranslationDisposition::Validated(
                AbstractToTargetFunctionTranslationReceipt::StraightLineBooleanImmediate(row),
            ) = receipt.function_roster()[0].translation()
            else {
                panic!("optimized Boolean lowering must retain its validated family row")
            };
            assert_eq!(
                row.machine(),
                target.optimized().plan().functions[0].machine
            );
            assert_eq!(row.value(), value);
            assert!(matches!(
                target.target_operations().functions[0].operation,
                TargetOperation::ReturnBooleanImmediate {
                    value: target_value,
                    ..
                } if target_value == value
            ));
        }
    }
}

#[test]
fn optimized_target_lowering_retains_constant_boolean_not_immediate_custody() {
    for target_profile in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        for source_value in [false, true] {
            let (semantic, proof) = boolean_not_immediate_return_artifact(source_value);
            let optimized = optimize_artifact_sections(
                &semantic,
                &proof,
                &AdmissionProfile::default(),
                request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
            )
            .unwrap();
            let target = lower_optimized_to_target_operations(optimized, target_profile).unwrap();
            let AbstractToTargetFunctionTranslationDisposition::Validated(
                AbstractToTargetFunctionTranslationReceipt::StraightLineBooleanNotImmediate(row),
            ) = target.translation_validation().function_roster()[0].translation()
            else {
                panic!("optimized constant Boolean-not must retain its exact immediate family")
            };
            assert_eq!(row.constant_operation(), OperationId::new(68_003).unwrap());
            assert_eq!(
                row.boolean_not_operation(),
                OperationId::new(68_005).unwrap()
            );
            assert_eq!(row.constant_result(), ValueId::new(68_004).unwrap());
            assert_eq!(row.boolean_not_result(), ValueId::new(68_006).unwrap());
            assert_eq!(row.source_value(), source_value);
            assert_eq!(row.materialized_value(), !source_value);
            assert!(matches!(
                target.target_operations().functions[0].operation,
                TargetOperation::ReturnBooleanImmediate {
                    source_value: result,
                    value,
                    ..
                } if result == ValueId::new(68_006).unwrap() && value != source_value
            ));
        }
    }
}

#[test]
fn optimized_target_lowering_retains_constant_bitwise_not_immediate_custody() {
    let cases = [
        (
            IntegerType::new(IntegerSign::Unsigned, 16).unwrap(),
            IntegerValue::Unsigned(255),
        ),
        (
            IntegerType::new(IntegerSign::Signed, 8).unwrap(),
            IntegerValue::Signed(i8::MIN.into()),
        ),
        (IntegerType::address(64).unwrap(), IntegerValue::Unsigned(0)),
    ];
    for target_profile in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        for (scalar_type, source_value) in cases {
            let (semantic, proof) =
                integer_bitwise_not_immediate_return_artifact(scalar_type, source_value);
            let optimized = optimize_artifact_sections(
                &semantic,
                &proof,
                &AdmissionProfile::default(),
                request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
            )
            .unwrap();
            let target = lower_optimized_to_target_operations(optimized, target_profile).unwrap();
            let AbstractToTargetFunctionTranslationDisposition::Validated(
                AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerBitwiseNotImmediate(
                    row,
                ),
            ) = target.translation_validation().function_roster()[0].translation()
            else {
                panic!("optimized constant bitwise-not must retain its exact immediate family")
            };
            let materialized_value = scalar_type.bitwise_not(source_value).unwrap();
            assert_eq!(row.constant_operation(), OperationId::new(67_003).unwrap());
            assert_eq!(
                row.bitwise_not_operation(),
                OperationId::new(67_005).unwrap()
            );
            assert_eq!(row.constant_result(), ValueId::new(67_004).unwrap());
            assert_eq!(row.bitwise_not_result(), ValueId::new(67_006).unwrap());
            assert_eq!(row.scalar_type(), scalar_type);
            assert_eq!(row.source_value(), source_value);
            assert_eq!(row.materialized_value(), materialized_value);
            assert!(matches!(
                target.target_operations().functions[0].operation,
                TargetOperation::ReturnIntegerImmediate {
                    source_value,
                    scalar_type: target_type,
                    value,
                    ..
                } if source_value == ValueId::new(67_006).unwrap()
                    && target_type == scalar_type
                    && value == materialized_value
            ));
        }
    }
}

#[test]
fn optimized_target_lowering_retains_constant_widen_immediate_custody() {
    let source_type = IntegerType::new(IntegerSign::Unsigned, 16).unwrap();
    let target_type = IntegerType::new(IntegerSign::Signed, 64).unwrap();
    for target_profile in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let (semantic, proof) = integer_widen_immediate_return_artifact();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
        )
        .unwrap();
        let target = lower_optimized_to_target_operations(optimized, target_profile).unwrap();
        let AbstractToTargetFunctionTranslationDisposition::Validated(
            AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerWidenImmediate(row),
        ) = target.translation_validation().function_roster()[0].translation()
        else {
            panic!("optimized constant widening must retain its exact immediate family")
        };
        assert_eq!(row.constant_operation(), OperationId::new(65_003).unwrap());
        assert_eq!(row.widen_operation(), OperationId::new(65_005).unwrap());
        assert_eq!(row.constant_result(), ValueId::new(65_004).unwrap());
        assert_eq!(row.widened_result(), ValueId::new(65_006).unwrap());
        assert_eq!(row.source_type(), source_type);
        assert_eq!(row.target_type(), target_type);
        assert_eq!(row.source_value(), IntegerValue::Unsigned(65_535));
        assert_eq!(row.materialized_value(), IntegerValue::Signed(65_535));
        assert!(matches!(
            target.target_operations().functions[0].operation,
            TargetOperation::ReturnIntegerImmediate {
                source_value,
                scalar_type,
                value,
                ..
            } if source_value == ValueId::new(65_006).unwrap()
                && scalar_type == target_type
                && value == IntegerValue::Signed(65_535)
        ));
    }
}

#[test]
fn optimized_target_lowering_retains_proof_bearing_exact_cast_immediate_operand_custody() {
    let source_type = IntegerType::new(IntegerSign::Unsigned, 16).unwrap();
    let target_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    for target_profile in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let (semantic, proof) = integer_exact_cast_immediate_operand_return_artifact();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
        )
        .unwrap();
        let target = lower_optimized_to_target_operations(optimized, target_profile).unwrap();
        let AbstractToTargetFunctionTranslationDisposition::Validated(
            AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerExactCastImmediateOperand(row),
        ) = target.translation_validation().function_roster()[0].translation()
        else {
            panic!("optimized constant exact cast must retain its proof-bearing immediate-operand family")
        };
        assert_eq!(row.constant_operation(), OperationId::new(66_003).unwrap());
        assert_eq!(row.cast_operation(), OperationId::new(66_005).unwrap());
        assert_eq!(row.obligation(), ObligationId::new(66_009).unwrap());
        assert_eq!(row.constant_result(), ValueId::new(66_004).unwrap());
        assert_eq!(row.cast_result(), ValueId::new(66_006).unwrap());
        assert_eq!(row.source_type(), source_type);
        assert_eq!(row.target_type(), target_type);
        assert_eq!(row.source_value(), IntegerValue::Unsigned(255));
        assert_eq!(row.cast_value(), IntegerValue::Unsigned(255));
        assert!(matches!(
            &target.target_operations().functions[0].operation,
            TargetOperation::ReturnIntegerExpression {
                source_value,
                scalar_type,
                expression: TargetIntegerExpression::IntegerExactCast {
                    obligation,
                    source_type: target_source_type,
                    operand,
                    ..
                },
                ..
            } if *source_value == ValueId::new(66_006).unwrap()
                && *scalar_type == target_type
                && *obligation == ObligationId::new(66_009).unwrap()
                && *target_source_type == source_type
                && matches!(
                    operand.as_ref(),
                    TargetIntegerExpression::Immediate { source_value, value }
                        if *source_value == ValueId::new(66_004).unwrap()
                            && *value == IntegerValue::Unsigned(255)
                )
        ));
    }
}
