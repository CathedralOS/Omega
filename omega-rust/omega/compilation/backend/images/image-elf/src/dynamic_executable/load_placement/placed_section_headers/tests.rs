//! Placed section header tests.

use super::{
    Candidate, DecodedElf64SectionHeader, ELF64_SECTION_HEADER_SIZE,
    ElfAppliedSectionHeaderPlacement, ElfSectionPlacementResolutionKind, SECTION_COUNT,
    SECTION_NAME_TABLE_INDEX, ValidatedElfDynamicLoadLayout, apply_elf_section_header_placements,
    checked_product, checked_sum, decode_section_header_rows, derive_contents, field_mut,
    non_authoritative_placed_compatibility_fingerprint, validate_application_coverage,
    validate_candidate, validate_contents,
};
use crate::ElfPlacedDynamicSectionKind;
use crate::dynamic_executable::checked::{read_u32, read_u64};
use arena::Handle;
use image::{FinalImage, FinalImageMemory};
use target::TargetProfile;

fn load_layout(target: TargetProfile) -> ValidatedElfDynamicLoadLayout {
    let mut image = FinalImage::with_capacity(
        target.native_target(),
        FinalImageMemory {
            text: vec![0; 32],
            data: vec![0x5a; 13],
            bss_size: 23,
            bss_alignment: 16,
        },
        Handle::invalid(),
        1,
        1,
        1,
    );
    let symbol_handle = crate::test_support::versioned_import(
        &mut image,
        "__omega_placed_section_header_import",
        b"libplaced-section-header.so",
        b"placed_section_header_call",
        b"PLACED_SECTION_HEADER_1",
        target,
        "versioned import locator",
    );
    let load = crate::test_support::import_call_load_layout(image, symbol_handle, target);
    load
}

fn candidate(target: TargetProfile) -> Candidate {
    let load_layout = load_layout(target);
    let contents = derive_contents(&load_layout).unwrap();
    let non_authoritative_placed_compatibility_fingerprint =
        non_authoritative_placed_compatibility_fingerprint(&load_layout, &contents);
    Candidate {
        load_layout,
        contents,
        non_authoritative_placed_compatibility_fingerprint,
    }
}

#[test]
fn both_linux_targets_apply_exact_twenty_five_little_endian_placements() {
    for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
        let placed = apply_elf_section_header_placements(load_layout(target)).unwrap();
        assert_eq!(placed.load_layout().target(), target);
        assert_eq!(placed.bytes().len(), 896);
        assert_eq!(placed.applied_placements().len(), 25);
        assert_ne!(
            placed.non_authoritative_placed_compatibility_fingerprint(),
            0
        );

        let template = placed
            .load_layout()
            .relative()
            .payloads()
            .section_headers()
            .contents();
        assert_eq!(template.bytes.len(), placed.bytes().len());
        for (application, resolution) in placed
            .applied_placements()
            .iter()
            .zip(placed.load_layout().section_header_resolutions())
        {
            assert_eq!(application.row_index(), resolution.row_index());
            assert_eq!(application.section_kind(), resolution.section_kind());
            assert_eq!(application.byte_offset(), resolution.byte_offset());
            assert_eq!(application.byte_width(), 8);
            assert_eq!(application.kind(), resolution.kind());
            assert_eq!(application.value(), resolution.value());
            assert_eq!(
                &placed.bytes()[application.byte_offset()..application.byte_offset() + 8],
                &application.value().to_le_bytes(),
            );
            assert_eq!(
                &template.bytes[application.byte_offset()..application.byte_offset() + 8],
                &[0; 8],
                "the retained upstream template must remain unchanged",
            );
        }
    }
}

#[test]
fn unchanged_template_bytes_are_copied_exactly() {
    let placed = apply_elf_section_header_placements(load_layout(TargetProfile::LinuxX64)).unwrap();
    let template = placed
        .load_layout()
        .relative()
        .payloads()
        .section_headers()
        .contents();
    let mut mutable = vec![false; placed.bytes().len()];
    for application in placed.applied_placements() {
        mutable[application.byte_offset()..application.byte_offset() + 8].fill(true);
    }
    for (offset, (&actual, &source)) in placed.bytes().iter().zip(&template.bytes).enumerate() {
        if !mutable[offset] {
            assert_eq!(actual, source, "non-fixup byte {offset} drifted");
        }
    }
    validate_contents(placed.load_layout(), &placed.contents).unwrap();
}

#[test]
fn ledger_has_exact_field_split_and_file_only_section_name_table() {
    let placed = apply_elf_section_header_placements(load_layout(TargetProfile::LinuxX64)).unwrap();
    assert_eq!(
        placed
            .applied_placements()
            .iter()
            .filter(|application| {
                application.kind() == ElfSectionPlacementResolutionKind::FileOffset
            })
            .count(),
        13,
    );
    assert_eq!(
        placed
            .applied_placements()
            .iter()
            .filter(|application| {
                application.kind() == ElfSectionPlacementResolutionKind::VirtualAddress
            })
            .count(),
        12,
    );
    assert!(
        placed
            .applied_placements()
            .iter()
            .all(|application| application.row_index() != 0),
    );
    assert!(placed.applied_placements().iter().any(|application| {
        application.row_index() == SECTION_NAME_TABLE_INDEX as u32
            && application.kind() == ElfSectionPlacementResolutionKind::FileOffset
    }));
    assert!(!placed.applied_placements().iter().any(|application| {
        application.row_index() == SECTION_NAME_TABLE_INDEX as u32
            && application.kind() == ElfSectionPlacementResolutionKind::VirtualAddress
    }));
    let decoded = decode_section_header_rows(placed.bytes(), SECTION_COUNT).unwrap();
    assert_eq!(
        decoded[0],
        DecodedElf64SectionHeader {
            name_offset: 0,
            section_type: 0,
            flags: 0,
            address: 0,
            file_offset: 0,
            payload_size: 0,
            link: 0,
            info: 0,
            alignment: 0,
            entry_size: 0,
        }
    );
    assert_eq!(decoded[SECTION_NAME_TABLE_INDEX].address, 0);
    assert_ne!(decoded[SECTION_NAME_TABLE_INDEX].file_offset, 0);
}

#[test]
fn placement_is_deterministic_for_same_input_and_target_bound() {
    let first = apply_elf_section_header_placements(load_layout(TargetProfile::LinuxX64)).unwrap();
    let second = apply_elf_section_header_placements(load_layout(TargetProfile::LinuxX64)).unwrap();
    let arm = apply_elf_section_header_placements(load_layout(TargetProfile::LinuxArm64)).unwrap();
    assert_eq!(first.bytes(), second.bytes());
    assert_eq!(first.applied_placements(), second.applied_placements());
    assert_eq!(
        first.non_authoritative_placed_compatibility_fingerprint(),
        second.non_authoritative_placed_compatibility_fingerprint()
    );
    assert_ne!(
        first.non_authoritative_placed_compatibility_fingerprint(),
        arm.non_authoritative_placed_compatibility_fingerprint()
    );
}

#[test]
fn missing_reordered_width_coordinate_value_and_identity_drift_reject_with_custody() {
    let corruptions: Vec<Box<dyn Fn(&mut Candidate)>> = vec![
        Box::new(|candidate| {
            candidate.contents.applications.pop();
        }),
        Box::new(|candidate| candidate.contents.applications.swap(0, 1)),
        Box::new(|candidate| candidate.contents.applications[0].byte_width = 4),
        Box::new(|candidate| candidate.contents.applications[0].byte_offset += 1),
        Box::new(|candidate| candidate.contents.applications[0].value ^= 1),
        Box::new(|candidate| {
            candidate.contents.applications[0].kind = ElfSectionPlacementResolutionKind::FileOffset
        }),
        Box::new(|candidate| candidate.contents.applications[0].row_index = 2),
        Box::new(|candidate| candidate.non_authoritative_placed_compatibility_fingerprint ^= 1),
    ];
    for corrupt in corruptions {
        let mut candidate = candidate(TargetProfile::LinuxArm64);
        let expected_layout_identity = candidate
            .load_layout
            .non_authoritative_layout_compatibility_fingerprint();
        corrupt(&mut candidate);
        let error =
            validate_candidate(candidate).expect_err("drifted placement application must reject");
        assert_eq!(
            error
                .candidate
                .load_layout
                .non_authoritative_layout_compatibility_fingerprint(),
            expected_layout_identity
        );
    }
}

#[test]
fn resolved_field_corruption_and_non_fixup_roster_drift_reject() {
    let mut resolved = candidate(TargetProfile::LinuxX64);
    let first_offset = resolved.contents.applications[0].byte_offset;
    resolved.contents.bytes[first_offset] ^= 1;
    validate_candidate(resolved).expect_err("resolved field byte corruption must reject");

    let mut non_fixup = candidate(TargetProfile::LinuxX64);
    let first_payload_size = ELF64_SECTION_HEADER_SIZE + 32;
    assert!(non_fixup.contents.applications.iter().all(|application| {
        !(application.byte_offset..application.byte_offset + 8).contains(&first_payload_size)
    }));
    non_fixup.contents.bytes[first_payload_size] ^= 1;
    validate_candidate(non_fixup).expect_err("non-fixup roster byte drift must reject");

    let mut truncated = candidate(TargetProfile::LinuxX64);
    truncated.contents.bytes.pop();
    validate_candidate(truncated).expect_err("truncated table must reject");

    let mut trailing = candidate(TargetProfile::LinuxX64);
    trailing.contents.bytes.push(0);
    validate_candidate(trailing).expect_err("trailing table byte must reject");
}

#[test]
fn bounds_helpers_reject_without_panicking() {
    assert!(checked_product(usize::MAX, 64, "product").is_err());
    assert!(checked_sum(usize::MAX, 8, "sum").is_err());
    assert!(read_u32(&[0; 3], 0, "word").is_err());
    assert!(read_u64(&[0; 7], 0, "xword").is_err());
    assert!(field_mut(&mut [0; 7], 0, 8, "test placement field").is_err());

    let application = ElfAppliedSectionHeaderPlacement {
        row_index: u32::MAX,
        section_kind: ElfPlacedDynamicSectionKind::Interpreter,
        byte_offset: usize::MAX,
        byte_width: 8,
        kind: ElfSectionPlacementResolutionKind::VirtualAddress,
        value: 1,
    };
    assert!(validate_application_coverage(768, &[application]).is_err());
}
