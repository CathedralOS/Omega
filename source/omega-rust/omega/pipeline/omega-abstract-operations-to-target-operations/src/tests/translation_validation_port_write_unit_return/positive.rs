use super::*;

#[test]
fn validates_port_write_unit_return_on_every_native_target() {
    for target_profile in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let source = base_plan();
        let target = lower_to_target_operations(&source, target_profile).unwrap();
        let receipt =
            validate_abstract_to_target_translation(&source, target_profile, &target).unwrap();
        let AbstractToTargetFunctionTranslationDisposition::Validated(
            AbstractToTargetFunctionTranslationReceipt::StraightLinePortWriteUnitReturn(row),
        ) = receipt.function_roster()[0].translation()
        else {
            panic!("exact port-write Unit return must publish one validated family row")
        };
        assert_eq!(row.machine(), source.entry);
        assert_eq!(row.port_operation(), port_operation());
        assert_eq!(row.service(), service());
        assert_eq!(row.port(), 0x03f8);
        assert_eq!(row.value(), 0x41);
        assert_eq!(row.return_edge(), return_edge());
    }
}

#[test]
fn changed_service_ceiling_and_return_only_shape_do_not_overlap_the_family() {
    let source = base_plan();
    let target_profile = NativeTarget::linux_x64();
    let target = lower_to_target_operations(&source, target_profile).unwrap();

    let mut changed_ceiling = source.clone();
    changed_ceiling.functions[0]
        .published_service_ceiling
        .clear();
    let receipt =
        validate_abstract_to_target_translation(&changed_ceiling, target_profile, &target).unwrap();
    assert_eq!(
        receipt.function_roster()[0].translation(),
        &AbstractToTargetFunctionTranslationDisposition::Uncovered
    );

    let mut return_only = source;
    return_only.functions[0].published_service_ceiling.clear();
    return_only.functions[0].operations.remove(0);
    let return_target = lower_to_target_operations(&return_only, target_profile).unwrap();
    let receipt =
        validate_abstract_to_target_translation(&return_only, target_profile, &return_target)
            .unwrap();
    assert!(matches!(
        receipt.function_roster()[0].translation(),
        AbstractToTargetFunctionTranslationDisposition::Validated(
            AbstractToTargetFunctionTranslationReceipt::StraightLineUnitReturn(_)
        )
    ));
}
