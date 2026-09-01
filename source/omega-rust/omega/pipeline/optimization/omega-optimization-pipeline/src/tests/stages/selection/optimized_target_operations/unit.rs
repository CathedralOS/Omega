use super::*;

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
