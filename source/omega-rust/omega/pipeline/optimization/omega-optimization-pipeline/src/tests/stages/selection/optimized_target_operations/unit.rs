use super::*;
use omega_abstract_operations_to_target_operations::AdmittedIeeeFloatFmaSettlement;
use omega_effects::provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceSchema};
use omega_target::{TargetProfile, X86_SCALAR_FMA_REQUIRED_FEATURES, X86ScalarFmaSlot};
use psi_terminal::TerminalAffineCleanupAction;

#[test]
fn optimized_target_lowering_retains_byte_sequence_literal_custody() {
    let expected_bytes = [0x00, 0x4f, 0x6d, 0x65, 0x67, 0x61, 0xff];
    for target_profile in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let (semantic, proof) = byte_sequence_literal_unit_return_artifact();
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
            AbstractToTargetFunctionTranslationReceipt::StraightLineByteSequenceLiteralUnitReturn(
                row,
            ),
        ) = receipt.function_roster()[0].translation()
        else {
            panic!("optimized byte-sequence literal must retain its validated family row")
        };
        assert_eq!(
            row.establishment_operation(),
            OperationId::new(3_517).unwrap()
        );
        assert_eq!(row.place().id, PlaceId::new(3_516).unwrap());
        assert_eq!(
            row.structural_type().id,
            StructuralTypeId::new(3_515).unwrap()
        );
        assert_eq!(row.bytes(), expected_bytes);
        let TargetOperation::UnitBody(body) = &target.target_operations().functions[0].operation
        else {
            panic!("optimized byte-sequence literal must remain in the Unit body carrier")
        };
        assert!(matches!(
            body.operations.as_slice(),
            [
                TargetUnitOperation::EstablishByteSequenceLiteral {
                    psi_operation,
                    place,
                    structural_type,
                    bytes,
                },
                TargetUnitOperation::Return { cleanup_actions, .. },
            ] if *psi_operation == OperationId::new(3_517).unwrap()
                && place.id == PlaceId::new(3_516).unwrap()
                && structural_type.id == StructuralTypeId::new(3_515).unwrap()
                && bytes.as_slice() == expected_bytes.as_slice()
                && cleanup_actions.is_empty()
        ));
    }
}

#[test]
fn optimized_target_lowering_retains_exact_ieee_literal_custody() {
    let expected_value = psi_core::IeeeFloatValue::Binary64(0x7ff8_1234_5678_9abc);
    for target_profile in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let (semantic, proof) = ieee_float_literal_unit_return_artifact();
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
            AbstractToTargetFunctionTranslationReceipt::StraightLineIeeeFloatLiteralUnitReturn(row),
        ) = receipt.function_roster()[0].translation()
        else {
            panic!("optimized IEEE literal must retain its validated Unit family row")
        };
        assert_eq!(row.literal_operation(), OperationId::new(3_518).unwrap());
        assert_eq!(row.literal_result(), ValueId::new(3_519).unwrap());
        assert_eq!(row.value(), expected_value);
        let TargetOperation::UnitBody(body) = &target.target_operations().functions[0].operation
        else {
            panic!("optimized IEEE literal must remain in the Unit body carrier")
        };
        assert!(matches!(
            body.operations.as_slice(),
            [
                TargetUnitOperation::IeeeFloatConstant {
                    psi_operation,
                    result,
                    value,
                },
                TargetUnitOperation::Return { cleanup_actions, .. },
            ] if *psi_operation == OperationId::new(3_518).unwrap()
                && *result == ValueId::new(3_519).unwrap()
                && *value == expected_value
                && cleanup_actions.is_empty()
        ));
    }
}

#[test]
fn optimized_target_lowering_retains_finite_ieee_literal_sequence_custody() {
    let expected = [
        (
            OperationId::new(3_520).unwrap(),
            ValueId::new(3_521).unwrap(),
            psi_core::IeeeFloatValue::Binary32(0x8000_0000),
        ),
        (
            OperationId::new(3_522).unwrap(),
            ValueId::new(3_523).unwrap(),
            psi_core::IeeeFloatValue::Binary32(0x7fc1_2345),
        ),
        (
            OperationId::new(3_524).unwrap(),
            ValueId::new(3_525).unwrap(),
            psi_core::IeeeFloatValue::Binary64(0x7ff8_1234_5678_9abc),
        ),
    ];
    for target_profile in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let (semantic, proof) = ieee_float_literal_sequence_unit_return_artifact();
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
            AbstractToTargetFunctionTranslationReceipt::StraightLineIeeeFloatLiteralSequenceUnitReturn(
                row,
            ),
        ) = receipt.function_roster()[0].translation()
        else {
            panic!("optimized IEEE sequence must retain its exact validated family")
        };
        assert_eq!(row.literals().len(), expected.len());
        for (member, (operation, result, value)) in row.literals().iter().zip(expected) {
            assert_eq!(member.operation(), operation);
            assert_eq!(member.result(), result);
            assert_eq!(member.value(), value);
        }
        let TargetOperation::UnitBody(body) = &target.target_operations().functions[0].operation
        else {
            panic!("optimized IEEE sequence must remain in the Unit-body carrier")
        };
        assert_eq!(body.operations.len(), expected.len() + 1);
        for (target_literal, (operation, result, value)) in body.operations.iter().zip(expected) {
            assert!(matches!(
                target_literal,
                TargetUnitOperation::IeeeFloatConstant {
                    psi_operation,
                    result: target_result,
                    value: target_value,
                } if *psi_operation == operation
                    && *target_result == result
                    && *target_value == value
            ));
        }
        assert!(matches!(
            body.operations.last(),
            Some(TargetUnitOperation::Return { cleanup_actions, .. })
                if cleanup_actions.is_empty()
        ));
    }
}

#[test]
fn optimized_target_lowering_retains_finite_integer_literal_sequence_custody() {
    let expected = [
        (
            OperationId::new(3_540).unwrap(),
            ValueId::new(3_541).unwrap(),
            IntegerType::new(IntegerSign::Signed, 8).unwrap(),
            IntegerValue::Signed(-128),
        ),
        (
            OperationId::new(3_542).unwrap(),
            ValueId::new(3_543).unwrap(),
            IntegerType::new(IntegerSign::Unsigned, 16).unwrap(),
            IntegerValue::Unsigned(65_535),
        ),
        (
            OperationId::new(3_544).unwrap(),
            ValueId::new(3_545).unwrap(),
            IntegerType::new(IntegerSign::Signed, 64).unwrap(),
            IntegerValue::Signed(i64::MIN as i128),
        ),
    ];
    for target_profile in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let (semantic, proof) = integer_literal_sequence_unit_return_artifact();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
        )
        .unwrap();
        let target = lower_optimized_to_target_operations(optimized, target_profile).unwrap();
        let AbstractToTargetFunctionTranslationDisposition::Validated(
            AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerLiteralSequenceUnitReturn(row),
        ) = target.translation_validation().function_roster()[0].translation()
        else {
            panic!("optimized integer sequence must retain its exact validated family")
        };
        assert_eq!(row.literals().len(), expected.len());
        for (member, (operation, result, scalar_type, value)) in row.literals().iter().zip(expected)
        {
            assert_eq!(member.operation(), operation);
            assert_eq!(member.result(), result);
            assert_eq!(member.scalar_type(), scalar_type);
            assert_eq!(member.value(), value);
        }
        let TargetOperation::UnitBody(body) = &target.target_operations().functions[0].operation
        else {
            panic!("optimized integer sequence must remain in the Unit-body carrier")
        };
        assert_eq!(body.operations.len(), expected.len() + 1);
        for (target_literal, (operation, result, scalar_type, value)) in
            body.operations.iter().zip(expected)
        {
            assert!(matches!(target_literal,
                TargetUnitOperation::IntegerConstant {
                    psi_operation, result: target_result,
                    scalar_type: target_type, value: target_value,
                } if *psi_operation == operation
                    && *target_result == result
                    && *target_type == scalar_type
                    && *target_value == value));
        }
    }
}

#[test]
fn optimized_target_lowering_retains_ordered_mixed_literal_sequence_custody() {
    for target_profile in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let (semantic, proof) = integer_ieee_float_literal_sequence_unit_return_artifact();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
        )
        .unwrap();
        let target = lower_optimized_to_target_operations(optimized, target_profile).unwrap();
        let AbstractToTargetFunctionTranslationDisposition::Validated(
            AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerIeeeFloatLiteralSequenceUnitReturn(row),
        ) = target.translation_validation().function_roster()[0].translation()
        else {
            panic!("optimized mixed sequence must retain its exact validated family")
        };
        assert!(matches!(
            row.literals(),
            [
                IntegerIeeeFloatLiteralSequenceMember::Integer {
                    operation,
                    result,
                    scalar_type,
                    value,
                },
                IntegerIeeeFloatLiteralSequenceMember::IeeeFloat {
                    operation: float_operation,
                    result: float_result,
                    value: float_value,
                },
                IntegerIeeeFloatLiteralSequenceMember::Integer {
                    operation: second_operation,
                    result: second_result,
                    scalar_type: second_type,
                    value: second_value,
                },
            ] if *operation == OperationId::new(3_550).unwrap()
                && *result == ValueId::new(3_551).unwrap()
                && *scalar_type == IntegerType::new(IntegerSign::Signed, 8).unwrap()
                && *value == IntegerValue::Signed(-128)
                && *float_operation == OperationId::new(3_552).unwrap()
                && *float_result == ValueId::new(3_553).unwrap()
                && *float_value == psi_core::IeeeFloatValue::Binary32(0x7fc1_2345)
                && *second_operation == OperationId::new(3_554).unwrap()
                && *second_result == ValueId::new(3_555).unwrap()
                && *second_type == IntegerType::new(IntegerSign::Unsigned, 16).unwrap()
                && *second_value == IntegerValue::Unsigned(65_535)
        ));
        let TargetOperation::UnitBody(body) = &target.target_operations().functions[0].operation
        else {
            panic!("optimized mixed sequence must remain in the Unit-body carrier")
        };
        assert!(matches!(
            body.operations.as_slice(),
            [
                TargetUnitOperation::IntegerConstant { psi_operation: first, .. },
                TargetUnitOperation::IeeeFloatConstant { psi_operation: second, .. },
                TargetUnitOperation::IntegerConstant { psi_operation: third, .. },
                TargetUnitOperation::Return { cleanup_actions, .. },
            ] if *first == OperationId::new(3_550).unwrap()
                && *second == OperationId::new(3_552).unwrap()
                && *third == OperationId::new(3_554).unwrap()
                && cleanup_actions.is_empty()
        ));
    }
}

#[test]
fn optimized_target_lowering_retains_exact_nearest_ieee_fma_custody() {
    for (target_profile, profile) in [
        (NativeTarget::linux_x64(), TargetProfile::LinuxX64),
        (NativeTarget::windows_x64(), TargetProfile::WindowsX64),
        (NativeTarget::uefi_x64(), TargetProfile::UefiX64),
    ] {
        for (format, slot) in [
            (
                psi_core::IeeeFloatFormat::Binary32,
                X86ScalarFmaSlot::Binary32,
            ),
            (
                psi_core::IeeeFloatFormat::Binary64,
                X86ScalarFmaSlot::Binary64,
            ),
        ] {
            let (semantic, proof) =
                nearest_ieee_float_fused_multiply_add_unit_return_artifact(format);
            let optimized = optimize_artifact_sections(
                &semantic,
                &proof,
                &AdmissionProfile::default(),
                request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
            )
            .unwrap();
            let plan = ProviderPlan {
                name: format!("test::optimized_nearest_fma::{format:?}"),
                provider_type: "test::CanonicalX86FmaProvider".into(),
                provider_type_package_identity: None,
                target: profile.target_name().into(),
                schema: ServiceSchema::default(),
                rows: vec![ProviderPlanRow {
                    method: "fused_multiply_add".into(),
                    requirement_identity: slot.selected_plan_requirement_identity().into(),
                    requirement_lifetime_partition: Vec::new(),
                    binding: ProviderBinding::CompilerIntrinsic {
                        machine: slot.realization_identity().into(),
                    },
                }],
                origin_package_identity: None,
                origin_package: "test".into(),
            };
            let provider = omega_target::AdmittedX86ScalarFmaProvider::from_deployment_claim(
                profile,
                &X86_SCALAR_FMA_REQUIRED_FEATURES,
            )
            .unwrap();
            let settlement = AdmittedIeeeFloatFmaSettlement {
                terminal_operation: OperationId::new(3_536).unwrap(),
                provider_plan: &plan,
                format,
                slot,
                provider,
            };
            let target = lower_optimized_to_target_operations_with_ieee_float_fma_settlements(
                optimized,
                target_profile,
                &[settlement],
            )
            .unwrap();
            let AbstractToTargetFunctionTranslationDisposition::Validated(
                AbstractToTargetFunctionTranslationReceipt::StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturn(row),
            ) = target.translation_validation().function_roster()[0].translation()
            else {
                panic!("optimized nearest FMA must retain its exact validated family")
            };
            assert_eq!(row.fma_operation(), OperationId::new(3_536).unwrap());
            assert_eq!(row.fma_result(), ValueId::new(3_537).unwrap());
            assert_eq!(row.format(), format);
            assert_eq!(row.slot(), slot);
            assert_eq!(row.provider(), provider);
            assert_eq!(
                row.provider_plan_report_identity(),
                plan.report_fingerprint()
            );
            assert_eq!(
                row.provider_plan_digest(),
                *plan.identity_digest().as_bytes()
            );
            let TargetOperation::UnitBody(body) =
                &target.target_operations().functions[0].operation
            else {
                panic!("optimized nearest FMA must remain in the Unit-body carrier")
            };
            assert!(matches!(
                body.operations.as_slice(),
                [
                    TargetUnitOperation::IeeeFloatConstant { .. },
                    TargetUnitOperation::IeeeFloatConstant { .. },
                    TargetUnitOperation::IeeeFloatConstant { .. },
                    TargetUnitOperation::NearestIeeeFloatFusedMultiplyAdd {
                        psi_operation,
                        result,
                        format: target_format,
                        settlement: target_settlement,
                        ..
                    },
                    TargetUnitOperation::Return { cleanup_actions, .. },
                ] if *psi_operation == OperationId::new(3_536).unwrap()
                    && *result == ValueId::new(3_537).unwrap()
                    && *target_format == format
                    && target_settlement.provider_plan_digest == *plan.identity_digest().as_bytes()
                    && target_settlement.slot == slot
                    && target_settlement.provider == provider
                    && cleanup_actions.is_empty()
            ));
        }
    }
}

#[test]
fn optimized_target_lowering_retains_trivial_affine_local_cleanup_custody() {
    for target_profile in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let (semantic, proof) = trivial_affine_local_unit_return_artifact();
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
            AbstractToTargetFunctionTranslationReceipt::StraightLineTrivialAffineLocalUnitReturn(
                row,
            ),
        ) = receipt.function_roster()[0].translation()
        else {
            panic!("optimized trivial affine local must retain its validated family row")
        };
        assert_eq!(
            row.establishment_operation(),
            OperationId::new(3_514).unwrap()
        );
        assert_eq!(row.place().id, PlaceId::new(3_513).unwrap());
        assert_eq!(
            row.structural_type().id,
            StructuralTypeId::new(3_512).unwrap()
        );
        let TargetOperation::UnitBody(body) = &target.target_operations().functions[0].operation
        else {
            panic!("optimized trivial affine local must remain in the Unit body carrier")
        };
        assert!(matches!(
            body.operations.as_slice(),
            [
                TargetUnitOperation::EstablishTrivialAffineLocal {
                    psi_operation,
                    place,
                    structural_type,
                },
                TargetUnitOperation::Return { cleanup_actions, .. },
            ] if *psi_operation == OperationId::new(3_514).unwrap()
                && place.id == PlaceId::new(3_513).unwrap()
                && structural_type.id == StructuralTypeId::new(3_512).unwrap()
                && matches!(
                    cleanup_actions.as_slice(),
                    [TerminalAffineCleanupAction::DiscardRoot(root)]
                        if *root == PlaceId::new(3_513).unwrap()
                )
        ));
    }
}

#[test]
fn optimized_target_lowering_retains_parameterless_unit_call_custody() {
    for target_profile in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let (semantic, proof) = unit_call_return_artifact();
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
            AbstractToTargetFunctionTranslationReceipt::StraightLineUnitCallReturn(row),
        ) = receipt.function_roster()[0].translation()
        else {
            panic!("optimized Unit caller must retain its validated family row")
        };
        assert_eq!(row.call_operation(), OperationId::new(3_511).unwrap());
        assert_eq!(row.callee(), MachineId::new(3_507).unwrap());
        assert!(row.requirement_obligations().is_empty());
        assert!(row.crash_continuations().is_empty());
        assert!(matches!(
            receipt.function_roster()[1].translation(),
            AbstractToTargetFunctionTranslationDisposition::Validated(
                AbstractToTargetFunctionTranslationReceipt::StraightLineUnitReturn(_)
            )
        ));
        let TargetOperation::UnitBody(body) = &target.target_operations().functions[0].operation
        else {
            panic!("optimized Unit caller must remain in the Unit body carrier")
        };
        assert!(matches!(
            body.operations.as_slice(),
            [
                TargetUnitOperation::Call {
                    callee,
                    arguments,
                    claim_transfers,
                    requirement_obligations,
                    crash_continuations,
                    ..
                },
                TargetUnitOperation::Return { cleanup_actions, .. },
            ] if *callee == MachineId::new(3_507).unwrap()
                && arguments.is_empty()
                && claim_transfers.is_empty()
                && requirement_obligations.is_empty()
                && crash_continuations.is_empty()
                && cleanup_actions.is_empty()
        ));
    }
}

#[test]
fn optimized_target_lowering_retains_port_write_unit_return_custody() {
    for target_profile in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let (semantic, proof) = port_write_unit_return_artifact();
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
            AbstractToTargetFunctionTranslationReceipt::StraightLinePortWriteUnitReturn(row),
        ) = receipt.function_roster()[0].translation()
        else {
            panic!("optimized port write must retain its validated Unit family row")
        };
        assert_eq!(row.port(), 0x03f8);
        assert_eq!(row.value(), 0x41);
        let TargetOperation::UnitBody(body) = &target.target_operations().functions[0].operation
        else {
            panic!("optimized port write must remain in the Unit body carrier")
        };
        assert!(matches!(
            body.operations.as_slice(),
            [
                TargetUnitOperation::PortWrite { port: 0x03f8, value: 0x41, .. },
                TargetUnitOperation::Return { cleanup_actions, .. },
            ] if cleanup_actions.is_empty()
        ));
    }
}
