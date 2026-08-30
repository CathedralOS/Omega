use crate::tests::*;
use omega_abstract_operations_to_target_operations::{
    AbstractToTargetFunctionTranslationDisposition, AbstractToTargetFunctionTranslationReceipt,
};
use omega_target_operations::{
    ScalarParameterLocation, TargetBooleanExpression, TargetIntegerExpression,
};

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
fn optimized_target_lowering_retains_exact_scalar_crash_custody() {
    let integer = ScalarType::Integer(
        IntegerType::new(IntegerSign::Unsigned, 32).expect("native integer type"),
    );
    for target_profile in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        for cause in [CrashCause::Trap, CrashCause::Abort] {
            for result_type in [ScalarType::Boolean, integer] {
                let (semantic, proof) = scalar_crash_artifact(result_type, cause);
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
                assert_eq!(receipt.target(), target_profile);
                assert_eq!(receipt.function_count(), 1);
                let AbstractToTargetFunctionTranslationDisposition::Validated(
                    AbstractToTargetFunctionTranslationReceipt::StraightLineScalarCrash(row),
                ) = receipt.function_roster()[0].translation()
                else {
                    panic!("optimized scalar Crash must retain its validated family row")
                };
                assert_eq!(row.result_type(), result_type);
                assert_eq!(row.cause(), cause);
                assert!(row.site_guard().is_empty());
                assert!(row.frontier_lower_bound().is_empty());
                assert!(matches!(
                    &target.target_operations().functions[0].operation,
                    TargetOperation::Crash {
                        cause: target_cause,
                        site_guard,
                        frontier_lower_bound,
                        ..
                    } if *target_cause == cause
                        && site_guard.is_empty()
                        && frontier_lower_bound.is_empty()
                ));
            }
        }
    }
}

#[test]
fn optimized_target_lowering_retains_exact_integer_parameter_custody() {
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("native integer type");
    for (target_profile, register, stack) in parameter_location_cases() {
        for (parameter_count, expected_location) in [(1, register), (9, stack)] {
            let (semantic, proof) =
                integer_parameter_return_artifact(integer_type, parameter_count);
            let optimized = optimize_artifact_sections(
                &semantic,
                &proof,
                &AdmissionProfile::default(),
                request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
            )
            .unwrap();
            let target = lower_optimized_to_target_operations(optimized, target_profile).unwrap();
            let receipt = target.translation_validation();
            let AbstractToTargetFunctionTranslationDisposition::Validated(
                AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerParameter(row),
            ) = receipt.function_roster()[0].translation()
            else {
                panic!("optimized parameter lowering must retain its validated family row")
            };
            assert_eq!(row.scalar_type(), integer_type);
            assert_eq!(row.parameter_index(), parameter_count - 1);
            assert_eq!(row.location(), expected_location);
            assert!(matches!(
                target.target_operations().functions[0].operation,
                TargetOperation::ReturnIntegerParameter { location, .. }
                    if location == expected_location
            ));
        }
    }
}

#[test]
fn optimized_target_lowering_retains_exact_boolean_parameter_custody() {
    for (target_profile, register, stack) in parameter_location_cases() {
        for (parameter_count, expected_location) in [(1, register), (9, stack)] {
            let (semantic, proof) = boolean_parameter_return_artifact(parameter_count);
            let optimized = optimize_artifact_sections(
                &semantic,
                &proof,
                &AdmissionProfile::default(),
                request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
            )
            .unwrap();
            let target = lower_optimized_to_target_operations(optimized, target_profile).unwrap();
            let receipt = target.translation_validation();
            let AbstractToTargetFunctionTranslationDisposition::Validated(
                AbstractToTargetFunctionTranslationReceipt::StraightLineBooleanParameter(row),
            ) = receipt.function_roster()[0].translation()
            else {
                panic!("optimized Boolean parameter lowering must retain its family row")
            };
            assert_eq!(row.parameter_index(), parameter_count - 1);
            assert_eq!(row.location(), expected_location);
            assert!(matches!(
                target.target_operations().functions[0].operation,
                TargetOperation::ReturnBooleanParameter { location, .. }
                    if location == expected_location
            ));
        }
    }
}

#[test]
fn optimized_target_lowering_retains_exact_boolean_not_parameter_custody() {
    for (target_profile, register, stack) in parameter_location_cases() {
        for (parameter_count, expected_location) in [(1, register), (9, stack)] {
            let (semantic, proof) = boolean_not_parameter_return_artifact(parameter_count);
            let optimized = optimize_artifact_sections(
                &semantic,
                &proof,
                &AdmissionProfile::default(),
                request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
            )
            .unwrap();
            let target = lower_optimized_to_target_operations(optimized, target_profile).unwrap();
            let receipt = target.translation_validation();
            let AbstractToTargetFunctionTranslationDisposition::Validated(
                AbstractToTargetFunctionTranslationReceipt::StraightLineBooleanNotParameter(row),
            ) = receipt.function_roster()[0].translation()
            else {
                panic!("optimized Boolean-not parameter must retain its family row")
            };
            assert_eq!(row.not_operation(), OperationId::new(30_005).unwrap());
            assert_eq!(
                row.operand_value(),
                ValueId::new(30_100 + parameter_count as u64 - 1).unwrap()
            );
            assert_eq!(row.parameter_index(), parameter_count - 1);
            assert_eq!(row.location(), expected_location);
            assert!(matches!(
                target.target_operations().functions[0].operation,
                TargetOperation::ReturnBooleanNotParameter { location, .. }
                    if location == expected_location
            ));
        }
    }
}

#[test]
fn optimized_target_lowering_retains_exact_boolean_equality_parameter_custody() {
    for (target_profile, registers, stack) in boolean_equal_location_cases() {
        for (parameter_count, expected) in [(2, registers), (10, stack)] {
            let (semantic, proof) = boolean_equal_parameters_return_artifact(parameter_count);
            let optimized = optimize_artifact_sections(
                &semantic,
                &proof,
                &AdmissionProfile::default(),
                request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
            )
            .unwrap();
            let target = lower_optimized_to_target_operations(optimized, target_profile).unwrap();
            let receipt = target.translation_validation();
            let AbstractToTargetFunctionTranslationDisposition::Validated(
                AbstractToTargetFunctionTranslationReceipt::StraightLineBooleanEqualParameters(row),
            ) = receipt.function_roster()[0].translation()
            else {
                panic!("optimized Boolean equality must retain its family row")
            };
            assert_eq!(row.equal_operation(), OperationId::new(30_005).unwrap());
            assert_eq!(row.left_parameter_index(), parameter_count - 2);
            assert_eq!(row.right_parameter_index(), parameter_count - 1);
            assert_eq!(row.left_location(), expected[0]);
            assert_eq!(row.right_location(), expected[1]);
            assert!(matches!(
                &target.target_operations().functions[0].operation,
                TargetOperation::ReturnBooleanExpression {
                    expression: TargetBooleanExpression::Equal { left, right, .. },
                    ..
                } if matches!(
                    left.as_ref(),
                    TargetBooleanExpression::Parameter { location, .. }
                        if *location == expected[0]
                ) && matches!(
                    right.as_ref(),
                    TargetBooleanExpression::Parameter { location, .. }
                        if *location == expected[1]
                )
            ));
        }
    }
}

#[test]
fn optimized_target_lowering_retains_exact_integer_equality_parameter_custody() {
    let scalar_type = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    for (target_profile, registers, stack) in boolean_equal_location_cases() {
        for (parameter_count, expected) in [(2, registers), (10, stack)] {
            let (semantic, proof) =
                integer_equal_parameters_return_artifact(scalar_type, parameter_count);
            let optimized = optimize_artifact_sections(
                &semantic,
                &proof,
                &AdmissionProfile::default(),
                request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
            )
            .unwrap();
            let target = lower_optimized_to_target_operations(optimized, target_profile).unwrap();
            let receipt = target.translation_validation();
            let AbstractToTargetFunctionTranslationDisposition::Validated(
                AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerEqualParameters(row),
            ) = receipt.function_roster()[0].translation()
            else {
                panic!("optimized integer equality must retain its family row")
            };
            assert_eq!(row.equal_operation(), OperationId::new(30_005).unwrap());
            assert_eq!(row.scalar_type(), scalar_type);
            assert_eq!(row.left_parameter_index(), parameter_count - 2);
            assert_eq!(row.right_parameter_index(), parameter_count - 1);
            assert_eq!(row.left_location(), expected[0]);
            assert_eq!(row.right_location(), expected[1]);
            assert!(matches!(
                &target.target_operations().functions[0].operation,
                TargetOperation::ReturnBooleanExpression {
                    expression: TargetBooleanExpression::IntegerEqual {
                        scalar_type: target_type,
                        left,
                        right,
                        ..
                    },
                    ..
                } if *target_type == scalar_type && matches!(
                    left.as_ref(),
                    TargetIntegerExpression::Parameter { location, .. }
                        if *location == expected[0]
                ) && matches!(
                    right.as_ref(),
                    TargetIntegerExpression::Parameter { location, .. }
                        if *location == expected[1]
                )
            ));
        }
    }
}

#[test]
fn optimized_target_lowering_retains_exact_integer_less_than_parameter_custody() {
    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 16).unwrap();
    for (target_profile, registers, stack) in boolean_equal_location_cases() {
        for (parameter_count, expected) in [(2, registers), (10, stack)] {
            let (semantic, proof) =
                integer_less_than_parameters_return_artifact(scalar_type, parameter_count);
            let optimized = optimize_artifact_sections(
                &semantic,
                &proof,
                &AdmissionProfile::default(),
                request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
            )
            .unwrap();
            let target = lower_optimized_to_target_operations(optimized, target_profile).unwrap();
            let receipt = target.translation_validation();
            let AbstractToTargetFunctionTranslationDisposition::Validated(
                AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerLessThanParameters(
                    row,
                ),
            ) = receipt.function_roster()[0].translation()
            else {
                panic!("optimized integer less-than must retain its family row")
            };
            assert_eq!(row.less_than_operation(), OperationId::new(30_005).unwrap());
            assert_eq!(row.scalar_type(), scalar_type);
            assert_eq!(row.left_parameter_index(), parameter_count - 2);
            assert_eq!(row.right_parameter_index(), parameter_count - 1);
            assert_eq!(row.left_location(), expected[0]);
            assert_eq!(row.right_location(), expected[1]);
            assert!(matches!(
                &target.target_operations().functions[0].operation,
                TargetOperation::ReturnBooleanExpression {
                    expression: TargetBooleanExpression::IntegerLessThan {
                        scalar_type: target_type,
                        left,
                        right,
                        ..
                    },
                    ..
                } if *target_type == scalar_type && matches!(
                    left.as_ref(),
                    TargetIntegerExpression::Parameter { location, .. }
                        if *location == expected[0]
                ) && matches!(
                    right.as_ref(),
                    TargetIntegerExpression::Parameter { location, .. }
                        if *location == expected[1]
                )
            ));
        }
    }
}

#[test]
fn optimized_target_lowering_retains_exact_integer_less_or_equal_parameter_custody() {
    let scalar_type = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    for (target_profile, registers, stack) in boolean_equal_location_cases() {
        for (parameter_count, expected) in [(2, registers), (10, stack)] {
            let (semantic, proof) =
                integer_less_or_equal_parameters_return_artifact(scalar_type, parameter_count);
            let optimized = optimize_artifact_sections(
                &semantic,
                &proof,
                &AdmissionProfile::default(),
                request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
            )
            .unwrap();
            let target = lower_optimized_to_target_operations(optimized, target_profile).unwrap();
            let receipt = target.translation_validation();
            let AbstractToTargetFunctionTranslationDisposition::Validated(
                AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerLessOrEqualParameters(
                    row,
                ),
            ) = receipt.function_roster()[0].translation()
            else {
                panic!("optimized integer less-or-equal must retain its family row")
            };
            assert_eq!(
                row.less_or_equal_operation(),
                OperationId::new(30_005).unwrap()
            );
            assert_eq!(row.scalar_type(), scalar_type);
            assert_eq!(row.left_parameter_index(), parameter_count - 2);
            assert_eq!(row.right_parameter_index(), parameter_count - 1);
            assert_eq!(row.left_location(), expected[0]);
            assert_eq!(row.right_location(), expected[1]);
            assert!(matches!(
                &target.target_operations().functions[0].operation,
                TargetOperation::ReturnBooleanExpression {
                    expression: TargetBooleanExpression::IntegerLessOrEqual {
                        scalar_type: target_type,
                        left,
                        right,
                        ..
                    },
                    ..
                } if *target_type == scalar_type && matches!(
                    left.as_ref(),
                    TargetIntegerExpression::Parameter { location, .. }
                        if *location == expected[0]
                ) && matches!(
                    right.as_ref(),
                    TargetIntegerExpression::Parameter { location, .. }
                        if *location == expected[1]
                )
            ));
        }
    }
}

#[test]
fn optimized_target_lowering_retains_exact_integer_bitwise_not_parameter_custody() {
    let scalar_type = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    for (target_profile, register, stack) in parameter_location_cases() {
        for (parameter_count, expected_location) in [(1, register), (9, stack)] {
            let (semantic, proof) =
                integer_bitwise_not_parameter_return_artifact(scalar_type, parameter_count);
            let optimized = optimize_artifact_sections(
                &semantic,
                &proof,
                &AdmissionProfile::default(),
                request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
            )
            .unwrap();
            let target = lower_optimized_to_target_operations(optimized, target_profile).unwrap();
            let receipt = target.translation_validation();
            let AbstractToTargetFunctionTranslationDisposition::Validated(
                AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerBitwiseNotParameter(
                    row,
                ),
            ) = receipt.function_roster()[0].translation()
            else {
                panic!("optimized integer bitwise-not must retain its family row")
            };
            assert_eq!(
                row.bitwise_not_operation(),
                OperationId::new(30_005).unwrap()
            );
            assert_eq!(row.scalar_type(), scalar_type);
            assert_eq!(row.parameter_index(), parameter_count - 1);
            assert_eq!(row.location(), expected_location);
            assert!(matches!(
                &target.target_operations().functions[0].operation,
                TargetOperation::ReturnIntegerExpression {
                    scalar_type: target_type,
                    expression: TargetIntegerExpression::BitwiseNot { operand, .. },
                    ..
                } if *target_type == scalar_type && matches!(
                    operand.as_ref(),
                    TargetIntegerExpression::Parameter { location, .. }
                        if *location == expected_location
                )
            ));
        }
    }
}

#[test]
fn optimized_target_lowering_retains_exact_integer_widen_parameter_custody() {
    let source_type = IntegerType::new(IntegerSign::Unsigned, 16).unwrap();
    let target_type = IntegerType::new(IntegerSign::Signed, 64).unwrap();
    for (target_profile, register, stack) in parameter_location_cases() {
        for (parameter_count, expected_location) in [(1, register), (9, stack)] {
            let (semantic, proof) =
                integer_widen_parameter_return_artifact(source_type, target_type, parameter_count);
            let optimized = optimize_artifact_sections(
                &semantic,
                &proof,
                &AdmissionProfile::default(),
                request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
            )
            .unwrap();
            let target = lower_optimized_to_target_operations(optimized, target_profile).unwrap();
            let receipt = target.translation_validation();
            let AbstractToTargetFunctionTranslationDisposition::Validated(
                AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerWidenParameter(row),
            ) = receipt.function_roster()[0].translation()
            else {
                panic!("optimized integer widen must retain its family row")
            };
            assert_eq!(row.widen_operation(), OperationId::new(30_005).unwrap());
            assert_eq!(row.source_type(), source_type);
            assert_eq!(row.target_type(), target_type);
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

fn parameter_location_cases() -> [(
    NativeTarget,
    ScalarParameterLocation,
    ScalarParameterLocation,
); 5] {
    [
        (
            NativeTarget::linux_x64(),
            ScalarParameterLocation::Register(MachineRegister::X86Rdi),
            ScalarParameterLocation::IncomingStack { byte_offset: 16 },
        ),
        (
            NativeTarget::windows_x64(),
            ScalarParameterLocation::Register(MachineRegister::X86Rcx),
            ScalarParameterLocation::IncomingStack { byte_offset: 64 },
        ),
        (
            NativeTarget::uefi_x64(),
            ScalarParameterLocation::Register(MachineRegister::X86Rcx),
            ScalarParameterLocation::IncomingStack { byte_offset: 64 },
        ),
        (
            NativeTarget::linux_arm64(),
            ScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
            ScalarParameterLocation::IncomingStack { byte_offset: 0 },
        ),
        (
            NativeTarget::macos_arm64(),
            ScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
            ScalarParameterLocation::IncomingStack { byte_offset: 0 },
        ),
    ]
}

fn boolean_equal_location_cases() -> [(
    NativeTarget,
    [ScalarParameterLocation; 2],
    [ScalarParameterLocation; 2],
); 5] {
    [
        (
            NativeTarget::linux_x64(),
            [
                ScalarParameterLocation::Register(MachineRegister::X86Rdi),
                ScalarParameterLocation::Register(MachineRegister::X86Rsi),
            ],
            [
                ScalarParameterLocation::IncomingStack { byte_offset: 16 },
                ScalarParameterLocation::IncomingStack { byte_offset: 24 },
            ],
        ),
        (
            NativeTarget::windows_x64(),
            [
                ScalarParameterLocation::Register(MachineRegister::X86Rcx),
                ScalarParameterLocation::Register(MachineRegister::X86Rdx),
            ],
            [
                ScalarParameterLocation::IncomingStack { byte_offset: 64 },
                ScalarParameterLocation::IncomingStack { byte_offset: 72 },
            ],
        ),
        (
            NativeTarget::uefi_x64(),
            [
                ScalarParameterLocation::Register(MachineRegister::X86Rcx),
                ScalarParameterLocation::Register(MachineRegister::X86Rdx),
            ],
            [
                ScalarParameterLocation::IncomingStack { byte_offset: 64 },
                ScalarParameterLocation::IncomingStack { byte_offset: 72 },
            ],
        ),
        (
            NativeTarget::linux_arm64(),
            [
                ScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
                ScalarParameterLocation::Register(MachineRegister::Aarch64X(1)),
            ],
            [
                ScalarParameterLocation::IncomingStack { byte_offset: 0 },
                ScalarParameterLocation::IncomingStack { byte_offset: 8 },
            ],
        ),
        (
            NativeTarget::macos_arm64(),
            [
                ScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
                ScalarParameterLocation::Register(MachineRegister::Aarch64X(1)),
            ],
            [
                ScalarParameterLocation::IncomingStack { byte_offset: 0 },
                ScalarParameterLocation::IncomingStack { byte_offset: 8 },
            ],
        ),
    ]
}
