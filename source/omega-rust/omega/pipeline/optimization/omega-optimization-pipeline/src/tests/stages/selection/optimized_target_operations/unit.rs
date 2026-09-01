use super::*;
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
