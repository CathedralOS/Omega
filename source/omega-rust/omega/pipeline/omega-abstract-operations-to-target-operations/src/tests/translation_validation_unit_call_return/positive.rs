use super::*;

#[test]
fn validates_parameterless_unit_call_and_callee_on_every_native_target() {
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
            AbstractToTargetFunctionTranslationReceipt::StraightLineUnitCallReturn(row),
        ) = receipt.function_roster()[0].translation()
        else {
            panic!("exact parameterless Unit caller must publish its validated family row")
        };
        assert_eq!(row.machine(), caller());
        assert_eq!(row.call_operation(), call_operation());
        assert_eq!(row.callee(), callee());
        assert_eq!(row.requirement_obligations(), &[requirement()]);
        assert_eq!(row.crash_continuations(), &[crash_continuation()]);
        assert_eq!(row.return_edge(), caller_return_edge());
        assert!(matches!(
            receipt.function_roster()[1].translation(),
            AbstractToTargetFunctionTranslationDisposition::Validated(
                AbstractToTargetFunctionTranslationReceipt::StraightLineUnitReturn(_)
            )
        ));
    }
}

#[test]
fn unit_call_classifier_does_not_overlap_adjacent_unit_families() {
    let target_profile = NativeTarget::linux_x64();

    let mut return_only = base_plan();
    return_only.functions[0].operations.remove(0);
    let target = lower_to_target_operations(&return_only, target_profile).unwrap();
    let receipt =
        validate_abstract_to_target_translation(&return_only, target_profile, &target).unwrap();
    assert!(matches!(
        receipt.function_roster()[0].translation(),
        AbstractToTargetFunctionTranslationDisposition::Validated(
            AbstractToTargetFunctionTranslationReceipt::StraightLineUnitReturn(_)
        )
    ));

    let source = base_plan();
    let target = lower_to_target_operations(&source, target_profile).unwrap();
    let mut structural_call = source;
    let AbstractOperation::CallUnit {
        structural_arguments,
        ..
    } = &mut structural_call.functions[0].operations[0]
    else {
        unreachable!()
    };
    structural_arguments.push(StructuralArgument {
        place: PlaceId::new(55_060).unwrap(),
        access: StructuralAccess::Owned,
        path: Vec::new(),
    });
    let receipt =
        validate_abstract_to_target_translation(&structural_call, target_profile, &target).unwrap();
    assert_eq!(
        receipt.function_roster()[0].translation(),
        &AbstractToTargetFunctionTranslationDisposition::Uncovered
    );
}
