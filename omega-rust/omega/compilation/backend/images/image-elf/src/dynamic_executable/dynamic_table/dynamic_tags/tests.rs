//! Dynamic tag tests.

use super::{
    Candidate, ElfDynamicAddressObligation, ElfDynamicAddressTarget, ElfDynamicSemanticRow,
    ElfDynamicTag, ElfDynamicValue, TargetProfile,
    ValidatedElfProcedureLinkageSectionDescriptorPlan, checked_product, checked_sum, checked_u32,
    derive_contents, dynamic_string, non_authoritative_tag_compatibility_fingerprint,
    plan_elf_dynamic_tags, structural_contents, validate_candidate, validate_contents,
};

const IMPORTS: [crate::test_support::DynamicImportFixture; 2] = crate::test_support::PAIR_IMPORTS;

use crate::test_support::DynamicImportFixture as ImportFixture;
fn descriptors(
    target: TargetProfile,
    imports: &[ImportFixture],
) -> ValidatedElfProcedureLinkageSectionDescriptorPlan {
    let inputs = crate::test_support::dynamic_import_inputs(
        target,
        imports,
        "__omega_dynamic_tag_import_",
        "valid dynamic-tag locator",
        "valid dynamic-tag interpreter",
    );
    crate::test_support::linkage_descriptors(inputs)
}

fn candidate(target: TargetProfile) -> Candidate {
    let descriptors = descriptors(target, &IMPORTS);
    let contents = derive_contents(&descriptors).expect("derived dynamic tags");
    let non_authoritative_tag_compatibility_fingerprint =
        non_authoritative_tag_compatibility_fingerprint(&descriptors, &contents);
    Candidate {
        descriptors,
        contents,
        non_authoritative_tag_compatibility_fingerprint,
    }
}

#[test]
fn both_targets_plan_exact_needed_prefix_fixed_tags_and_address_obligations() {
    for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
        let plan = plan_elf_dynamic_tags(descriptors(target, &IMPORTS))
            .expect("validated semantic dynamic tags");
        assert_eq!(plan.descriptors().descriptor_count(), 11);
        assert_eq!(plan.needed_row_count(), 2);
        assert_eq!(plan.row_count(), 19);
        assert_eq!(plan.address_obligation_count(), 9);
        assert_ne!(plan.non_authoritative_tag_compatibility_fingerprint(), 0);

        let structural = structural_contents(&plan.descriptors);
        assert_eq!(
            &plan.contents.rows[..2],
            &[
                ElfDynamicSemanticRow {
                    tag: ElfDynamicTag::Needed,
                    value: ElfDynamicValue::NeededStringOffset(structural.needed[0]),
                },
                ElfDynamicSemanticRow {
                    tag: ElfDynamicTag::Needed,
                    value: ElfDynamicValue::NeededStringOffset(structural.needed[1]),
                },
            ],
        );
        assert_eq!(
            plan.contents
                .rows
                .iter()
                .map(|row| row.tag)
                .collect::<Vec<_>>(),
            [
                ElfDynamicTag::Needed,
                ElfDynamicTag::Needed,
                ElfDynamicTag::ProcedureRelocationSize,
                ElfDynamicTag::ProcedureGot,
                ElfDynamicTag::SystemVHash,
                ElfDynamicTag::GnuHash,
                ElfDynamicTag::DynamicString,
                ElfDynamicTag::DynamicSymbol,
                ElfDynamicTag::DynamicStringSize,
                ElfDynamicTag::DynamicSymbolEntrySize,
                ElfDynamicTag::ProcedureRelocationKind,
                ElfDynamicTag::ProcedureRelocation,
                ElfDynamicTag::Rela,
                ElfDynamicTag::GeneralRelocationSize,
                ElfDynamicTag::GeneralRelocationEntrySize,
                ElfDynamicTag::GnuSymbolVersion,
                ElfDynamicTag::GnuVersionRequirement,
                ElfDynamicTag::GnuVersionRequirementCount,
                ElfDynamicTag::Null,
            ],
        );
        assert_eq!(
            plan.contents.address_obligations,
            [
                ElfDynamicAddressObligation {
                    row_ordinal: 3,
                    byte_width: 8,
                    target: ElfDynamicAddressTarget::ProcedureGot,
                },
                ElfDynamicAddressObligation {
                    row_ordinal: 4,
                    byte_width: 8,
                    target: ElfDynamicAddressTarget::SystemVHash,
                },
                ElfDynamicAddressObligation {
                    row_ordinal: 5,
                    byte_width: 8,
                    target: ElfDynamicAddressTarget::GnuHash,
                },
                ElfDynamicAddressObligation {
                    row_ordinal: 6,
                    byte_width: 8,
                    target: ElfDynamicAddressTarget::DynamicString,
                },
                ElfDynamicAddressObligation {
                    row_ordinal: 7,
                    byte_width: 8,
                    target: ElfDynamicAddressTarget::DynamicSymbol,
                },
                ElfDynamicAddressObligation {
                    row_ordinal: 11,
                    byte_width: 8,
                    target: ElfDynamicAddressTarget::ProcedureRelocation,
                },
                ElfDynamicAddressObligation {
                    row_ordinal: 12,
                    byte_width: 8,
                    target: ElfDynamicAddressTarget::GeneralRelocation,
                },
                ElfDynamicAddressObligation {
                    row_ordinal: 15,
                    byte_width: 8,
                    target: ElfDynamicAddressTarget::GnuSymbolVersion,
                },
                ElfDynamicAddressObligation {
                    row_ordinal: 16,
                    byte_width: 8,
                    target: ElfDynamicAddressTarget::GnuVersionRequirement,
                },
            ],
        );
        assert_eq!(
            plan.contents.rows[2].value,
            ElfDynamicValue::ProcedureRelocationByteCount(48),
        );
        assert_eq!(
            plan.contents.rows[9].value,
            ElfDynamicValue::DynamicSymbolEntryByteCount(24),
        );
        assert_eq!(
            plan.contents.rows[10].value,
            ElfDynamicValue::RelocationTag(ElfDynamicTag::Rela),
        );
        assert_eq!(
            plan.contents.rows[12].value,
            ElfDynamicValue::AddressPlaceholder,
        );
        assert_eq!(
            plan.contents.rows[13].value,
            ElfDynamicValue::GeneralRelocationByteCount(0),
        );
        assert_eq!(
            plan.contents.rows[14].value,
            ElfDynamicValue::GeneralRelocationEntryByteCount(24),
        );
        assert_eq!(
            plan.contents.rows[17].value,
            ElfDynamicValue::VersionRequirementRecordCount(2),
        );
        assert_eq!(plan.contents.rows[18].value, ElfDynamicValue::Null);
        validate_contents(plan.descriptors(), &plan.contents)
            .expect("independent dynamic-tag replay");
    }
}

#[test]
fn import_permutation_preserves_rows_and_identity_while_target_stays_bound() {
    let forward = plan_elf_dynamic_tags(descriptors(TargetProfile::LinuxX64, &IMPORTS)).unwrap();
    let reverse_imports = IMPORTS.iter().rev().copied().collect::<Vec<_>>();
    let reverse =
        plan_elf_dynamic_tags(descriptors(TargetProfile::LinuxX64, &reverse_imports)).unwrap();
    let arm = plan_elf_dynamic_tags(descriptors(TargetProfile::LinuxArm64, &IMPORTS)).unwrap();
    assert_eq!(forward.contents, reverse.contents);
    assert_eq!(
        forward.non_authoritative_tag_compatibility_fingerprint(),
        reverse.non_authoritative_tag_compatibility_fingerprint()
    );
    assert_ne!(
        forward.non_authoritative_tag_compatibility_fingerprint(),
        arm.non_authoritative_tag_compatibility_fingerprint()
    );
}

#[test]
fn exact_tag_set_binds_general_relocation_and_omits_optional_policies() {
    let plan = plan_elf_dynamic_tags(descriptors(TargetProfile::LinuxX64, &IMPORTS)).unwrap();
    assert_eq!(
        plan.contents
            .rows
            .iter()
            .filter(|row| row.tag == ElfDynamicTag::Rela)
            .count(),
        1,
    );
    assert_eq!(
        plan.descriptors
            .templates()
            .linkage()
            .general_dynamic_relocation_count(),
        0,
    );
    assert_eq!(
        plan.contents
            .rows
            .iter()
            .filter(|row| row.tag == ElfDynamicTag::GeneralRelocationEntrySize)
            .count(),
        1,
    );
    assert_eq!(
        plan.contents
            .rows
            .iter()
            .filter(|row| row.tag == ElfDynamicTag::Null)
            .count(),
        1,
    );
    assert_eq!(plan.contents.rows.last().unwrap().tag, ElfDynamicTag::Null);
}

#[test]
fn independent_replay_rejects_every_value_family_order_and_identity_corruption() {
    let corruptions: Vec<Box<dyn Fn(&mut Candidate)>> = vec![
        Box::new(|candidate| candidate.contents.rows.swap(0, 1)),
        Box::new(|candidate| candidate.contents.rows.swap(2, 3)),
        Box::new(|candidate| {
            candidate.contents.rows.pop();
        }),
        Box::new(|candidate| candidate.contents.rows.push(candidate.contents.rows[18])),
        Box::new(|candidate| candidate.contents.rows[0].tag = ElfDynamicTag::Null),
        Box::new(|candidate| {
            candidate.contents.rows[0].value = ElfDynamicValue::NeededStringOffset(0)
        }),
        Box::new(|candidate| {
            candidate.contents.rows[2].value = ElfDynamicValue::ProcedureRelocationByteCount(47)
        }),
        Box::new(|candidate| candidate.contents.rows[3].value = ElfDynamicValue::Null),
        Box::new(|candidate| {
            candidate.contents.rows[8].value = ElfDynamicValue::DynamicStringByteCount(0)
        }),
        Box::new(|candidate| {
            candidate.contents.rows[9].value = ElfDynamicValue::DynamicSymbolEntryByteCount(16)
        }),
        Box::new(|candidate| {
            candidate.contents.rows[10].value = ElfDynamicValue::RelocationTag(ElfDynamicTag::Null)
        }),
        Box::new(|candidate| {
            candidate.contents.rows[14].value = ElfDynamicValue::GeneralRelocationEntryByteCount(16)
        }),
        Box::new(|candidate| {
            candidate.contents.rows[17].value = ElfDynamicValue::VersionRequirementRecordCount(1)
        }),
        Box::new(|candidate| {
            candidate.contents.rows[18].value = ElfDynamicValue::AddressPlaceholder
        }),
        Box::new(|candidate| candidate.non_authoritative_tag_compatibility_fingerprint ^= 1),
    ];
    for corrupt in corruptions {
        let mut candidate = candidate(TargetProfile::LinuxX64);
        let expected_identity = candidate
            .descriptors
            .non_authoritative_descriptor_compatibility_fingerprint();
        corrupt(&mut candidate);
        let error =
            validate_candidate(candidate).expect_err("corrupt semantic dynamic tags must reject");
        assert_eq!(
            error
                .candidate
                .descriptors
                .non_authoritative_descriptor_compatibility_fingerprint(),
            expected_identity,
            "dynamic-tag rejection retains exact descriptor custody",
        );
    }
}

#[test]
fn independent_replay_rejects_missing_duplicate_or_misdirected_address_obligations() {
    let corruptions: Vec<Box<dyn Fn(&mut Candidate)>> = vec![
        Box::new(|candidate| {
            candidate.contents.address_obligations.pop();
        }),
        Box::new(|candidate| {
            candidate
                .contents
                .address_obligations
                .push(candidate.contents.address_obligations[0])
        }),
        Box::new(|candidate| candidate.contents.address_obligations.swap(0, 1)),
        Box::new(|candidate| candidate.contents.address_obligations[0].row_ordinal = u32::MAX),
        Box::new(|candidate| candidate.contents.address_obligations[0].byte_width = 4),
        Box::new(|candidate| {
            candidate.contents.address_obligations[0].target =
                ElfDynamicAddressTarget::DynamicString
        }),
    ];
    for corrupt in corruptions {
        let mut candidate = candidate(TargetProfile::LinuxArm64);
        let expected_identity = candidate
            .descriptors
            .non_authoritative_descriptor_compatibility_fingerprint();
        corrupt(&mut candidate);
        let error = validate_candidate(candidate)
            .expect_err("corrupt dynamic address obligations must reject");
        assert_eq!(
            error
                .candidate
                .descriptors
                .non_authoritative_descriptor_compatibility_fingerprint(),
            expected_identity,
        );
    }
}

#[test]
fn malformed_offsets_counts_and_arithmetic_reject_without_panicking() {
    assert!(checked_sum(usize::MAX, 1, "sum").is_err());
    assert!(checked_product(usize::MAX, 2, "product").is_err());
    assert!(checked_u32(usize::MAX, "word").is_err());
    assert!(dynamic_string(&[], 0).is_err());
    assert!(dynamic_string(b"unterminated", 0).is_err());
    assert!(dynamic_string(b"\0", u32::MAX).is_err());

    let mut candidate = candidate(TargetProfile::LinuxX64);
    candidate.contents.address_obligations[0].row_ordinal = u32::MAX;
    assert!(validate_candidate(candidate).is_err());
}

#[test]
fn unversioned_component_remains_needed_without_inventing_a_gnu_version() {
    let profile = TargetProfile::LinuxX64;
    let inputs = crate::test_support::dynamic_import_inputs(
        profile,
        &IMPORTS,
        "__omega_mixed_import_",
        "valid locator",
        "valid interpreter",
    );
    let (mut image, interpreter, _) = inputs.into_parts();
    let locator =
        target::NormalizedComponentLocator::for_image(profile, b"component bytes", [7; 32])
            .expect("exact component locator");
    image.memory.text[48] = 0xe8;
    crate::test_support::import_with_text_relocations(
        &mut image,
        "__omega_component",
        image::FinalImageImportPlan::Normalized(locator.into()),
        &[49],
    );
    let inputs = crate::plan_elf_dynamic_link_inputs(image, interpreter).unwrap();
    let descriptors = crate::test_support::linkage_descriptors(inputs);
    let plan = plan_elf_dynamic_tags(descriptors).expect("mixed versioned and component imports");
    assert_eq!(plan.needed_row_count(), 3);
    assert_eq!(structural_contents(plan.descriptors()).verneed.len(), 2);
    let sections = plan.descriptors().templates().linkage().descriptors();
    let requirements = sections.contents().descriptors.iter().find(|row|
        row.kind == crate::dynamic_executable::import_sections::dynamic_section_descriptors::ElfDynamicSectionKind::GnuVersionRequirement).unwrap();
    assert_eq!(
        requirements.info, 2,
        "section sh_info counts Verneed records, not DT_NEEDED objects"
    );

    let count_row = plan
        .contents
        .rows
        .iter()
        .position(|row| row.tag == ElfDynamicTag::GnuVersionRequirementCount)
        .unwrap();
    assert_eq!(
        plan.contents.rows[count_row].value,
        ElfDynamicValue::VersionRequirementRecordCount(2)
    );
    for incorrect_count in [1, 3] {
        let mut malformed = plan.contents.clone();
        malformed.rows[count_row].value =
            ElfDynamicValue::VersionRequirementRecordCount(incorrect_count);
        assert!(
            validate_contents(plan.descriptors(), &malformed).is_err(),
            "missing/extra version-need counts must still reject"
        );
    }
}

#[test]
fn component_only_imports_omit_absent_version_records_and_their_address_fixup() {
    let profile = TargetProfile::LinuxX64;
    let mut image = image::FinalImage::with_capacity(
        profile.native_target(),
        image::FinalImageMemory {
            text: vec![0; 64],
            ..image::FinalImageMemory::default()
        },
        arena::Handle::invalid(),
        2,
        2,
        2,
    );
    for index in 0..2 {
        let locator = target::NormalizedComponentLocator::for_image(
            profile,
            &[index as u8],
            [index as u8 + 1; 32],
        )
        .unwrap();
        let offset = index * 16;
        image.memory.text[offset] = 0xe8;
        crate::test_support::import_with_text_relocations(
            &mut image,
            &format!("__omega_component_{index}"),
            image::FinalImageImportPlan::Normalized(locator.into()),
            &[offset + 1],
        );
    }
    let inputs =
        crate::plan_elf_dynamic_link_inputs(image, crate::test_support::interpreter(profile))
            .unwrap();
    let descriptors = crate::test_support::linkage_descriptors(inputs);
    let plan = plan_elf_dynamic_tags(descriptors).unwrap();
    assert_eq!(plan.needed_row_count(), 2);
    assert_eq!(plan.row_count(), 17);
    assert_eq!(plan.address_obligation_count(), 8);
    assert!(!plan.has_version_requirements());
    assert!(!plan.contents.rows.iter().any(|row| matches!(
        row.tag,
        ElfDynamicTag::GnuVersionRequirement | ElfDynamicTag::GnuVersionRequirementCount
    )));
    let mut malformed = plan.contents.clone();
    let terminal = malformed.rows.pop().unwrap();
    malformed.rows.extend([
        super::address_row(ElfDynamicTag::GnuVersionRequirement),
        ElfDynamicSemanticRow {
            tag: ElfDynamicTag::GnuVersionRequirementCount,
            value: ElfDynamicValue::VersionRequirementRecordCount(0),
        },
        terminal,
    ]);
    assert!(
        validate_contents(plan.descriptors(), &malformed).is_err(),
        "a zero count cannot justify a present pointer to an absent Verneed record"
    );
}
