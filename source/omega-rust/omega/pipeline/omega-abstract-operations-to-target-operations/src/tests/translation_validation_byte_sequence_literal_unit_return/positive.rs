use super::*;

#[test]
fn validates_byte_sequence_literal_on_every_native_target() {
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
            AbstractToTargetFunctionTranslationReceipt::StraightLineByteSequenceLiteralUnitReturn(
                row,
            ),
        ) = receipt.function_roster()[0].translation()
        else {
            panic!("exact byte-sequence literal must publish its validated family row")
        };
        assert_eq!(row.machine(), machine());
        assert_eq!(row.establishment_operation(), establishment_operation());
        assert_eq!(row.place(), &place());
        assert_eq!(row.structural_type(), &structural_type());
        assert_eq!(row.bytes(), bytes());
        assert_eq!(row.return_edge(), return_edge());
    }
}

#[test]
fn byte_sequence_literal_classifier_does_not_overlap_adjacent_unit_families() {
    let target_profile = NativeTarget::linux_x64();
    let mut source = base_plan();
    source.functions[0].operations.remove(0);
    let target = lower_to_target_operations(&source, target_profile).unwrap();
    let receipt =
        validate_abstract_to_target_translation(&source, target_profile, &target).unwrap();
    assert!(matches!(
        receipt.function_roster()[0].translation(),
        AbstractToTargetFunctionTranslationDisposition::Validated(
            AbstractToTargetFunctionTranslationReceipt::StraightLineUnitReturn(_)
        )
    ));
}
