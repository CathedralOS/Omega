use super::*;

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
