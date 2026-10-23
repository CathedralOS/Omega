//! Section header byte tests.

use super::{
    Candidate, ELF64_SECTION_HEADER_SIZE, ElfDynamicRosterSectionKind,
    ElfSectionHeaderPlacementFixup, ElfSectionHeaderPlacementFixupKind, SECTION_COUNT,
    ValidatedElfDynamicSectionRoster, checked_product, checked_sum, encode_contents,
    non_authoritative_template_compatibility_fingerprint, serialize_elf_section_header_table,
    validate_candidate, validate_contents, validate_fixup_coverage,
};
use crate::dynamic_executable::checked::{decode_section_header_rows, read_u32, read_u64};
use crate::plan_elf_dynamic_section_roster;
use target::TargetProfile;

const IMPORTS: [crate::test_support::DynamicImportFixture; 2] = crate::test_support::PAIR_IMPORTS;

use crate::test_support::DynamicImportFixture as ImportFixture;
fn roster(target: TargetProfile, imports: &[ImportFixture]) -> ValidatedElfDynamicSectionRoster {
    let inputs = crate::test_support::dynamic_import_inputs(
        target,
        imports,
        "__omega_section_header_import_",
        "valid section-header locator",
        "valid section-header interpreter",
    );
    let names = crate::test_support::dynamic_section_names(inputs);
    plan_elf_dynamic_section_roster(names).expect("valid section roster")
}

fn candidate(target: TargetProfile) -> Candidate {
    let roster = roster(target, &IMPORTS);
    let contents = encode_contents(&roster).expect("encoded section-header table");
    let non_authoritative_template_compatibility_fingerprint =
        non_authoritative_template_compatibility_fingerprint(&roster, &contents);
    Candidate {
        roster,
        contents,
        non_authoritative_template_compatibility_fingerprint,
    }
}

fn row(bytes: &[u8], index: usize) -> &[u8] {
    let offset = index * ELF64_SECTION_HEADER_SIZE;
    &bytes[offset..offset + ELF64_SECTION_HEADER_SIZE]
}

#[test]
fn both_targets_serialize_exact_rows_and_twenty_five_zero_fixups() {
    for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
        let template = serialize_elf_section_header_table(roster(target, &IMPORTS))
            .expect("validated section-header template");
        assert_eq!(template.row_count(), 14);
        assert_eq!(template.byte_count(), 896);
        assert_eq!(template.placement_fixup_count(), 25);
        assert_eq!(row(&template.contents.bytes, 0), &[0; 64]);

        let decoded = decode_section_header_rows(&template.contents.bytes, 14).unwrap();
        assert_eq!(decoded[3].name_offset, 17);
        assert_eq!(decoded[3].section_type, 11);
        assert_eq!(decoded[3].flags, 2);
        assert_eq!(decoded[3].payload_size, 72);
        assert_eq!((decoded[3].link, decoded[3].info), (2, 1));
        assert_eq!((decoded[3].alignment, decoded[3].entry_size), (8, 24));
        assert_eq!((decoded[7].section_type, decoded[7].link), (0x6fff_fff6, 3));
        assert_eq!((decoded[10].link, decoded[10].info), (3, 9));
        assert_eq!((decoded[11].link, decoded[11].info), (3, 0));
        assert_eq!(decoded[12].link, 2);
        assert_eq!(decoded[13].name_offset, 59);
        assert_eq!(decoded[13].payload_size, 122);
        assert_eq!((decoded[13].address, decoded[13].file_offset), (0, 0));
        assert_ne!(
            template.non_authoritative_template_compatibility_fingerprint(),
            0
        );
        validate_contents(template.roster(), &template.contents).unwrap();
    }
}

#[test]
fn exact_fixup_coordinates_cover_only_owned_placement_fields() {
    let template =
        serialize_elf_section_header_table(roster(TargetProfile::LinuxX64, &IMPORTS)).unwrap();
    assert_eq!(
        template.contents.placement_fixups[0],
        ElfSectionHeaderPlacementFixup {
            row_index: 1,
            section_kind: ElfDynamicRosterSectionKind::Interpreter,
            byte_offset: 80,
            byte_width: 8,
            kind: ElfSectionHeaderPlacementFixupKind::VirtualAddress,
        },
    );
    assert_eq!(template.contents.placement_fixups[1].byte_offset, 88);
    assert_eq!(
        template.contents.placement_fixups[20],
        ElfSectionHeaderPlacementFixup {
            row_index: 11,
            section_kind: ElfDynamicRosterSectionKind::GeneralRelocation,
            byte_offset: 720,
            byte_width: 8,
            kind: ElfSectionHeaderPlacementFixupKind::VirtualAddress,
        },
    );
    assert_eq!(template.contents.placement_fixups[21].byte_offset, 728);
    assert_eq!(
        template.contents.placement_fixups[22],
        ElfSectionHeaderPlacementFixup {
            row_index: 12,
            section_kind: ElfDynamicRosterSectionKind::DynamicTable,
            byte_offset: 784,
            byte_width: 8,
            kind: ElfSectionHeaderPlacementFixupKind::VirtualAddress,
        },
    );
    assert_eq!(template.contents.placement_fixups[23].byte_offset, 792);
    assert_eq!(
        template.contents.placement_fixups[24],
        ElfSectionHeaderPlacementFixup {
            row_index: 13,
            section_kind: ElfDynamicRosterSectionKind::SectionNameTable,
            byte_offset: 856,
            byte_width: 8,
            kind: ElfSectionHeaderPlacementFixupKind::FileOffset,
        },
    );
    assert_eq!(
        template
            .contents
            .placement_fixups
            .iter()
            .filter(|fixup| { fixup.kind == ElfSectionHeaderPlacementFixupKind::VirtualAddress })
            .count(),
        12,
    );
    assert!(template.contents.placement_fixups.iter().all(|fixup| {
        read_u64(
            &template.contents.bytes,
            fixup.byte_offset,
            "test placeholder",
        ) == Ok(0)
    }));
}

#[test]
fn import_permutation_preserves_templates_and_target_remains_identity_bound() {
    let forward =
        serialize_elf_section_header_table(roster(TargetProfile::LinuxX64, &IMPORTS)).unwrap();
    let reverse_imports = IMPORTS.iter().rev().copied().collect::<Vec<_>>();
    let reverse =
        serialize_elf_section_header_table(roster(TargetProfile::LinuxX64, &reverse_imports))
            .unwrap();
    let arm =
        serialize_elf_section_header_table(roster(TargetProfile::LinuxArm64, &IMPORTS)).unwrap();
    assert_eq!(forward.contents, reverse.contents);
    assert_eq!(
        forward.non_authoritative_template_compatibility_fingerprint(),
        reverse.non_authoritative_template_compatibility_fingerprint()
    );
    assert_ne!(
        forward.non_authoritative_template_compatibility_fingerprint(),
        arm.non_authoritative_template_compatibility_fingerprint()
    );
}

#[test]
fn header_field_boundaries_reject_with_roster_custody() {
    // Exercise both ends of each decoded field in null, allocated, and final rows.
    // The remaining rows use the same decoder and roster comparison.
    for row_index in [0, 1, SECTION_COUNT - 1] {
        for (field_offset, field_width) in [
            (0, 4),
            (4, 4),
            (8, 8),
            (16, 8),
            (24, 8),
            (32, 8),
            (40, 4),
            (44, 4),
            (48, 8),
            (56, 8),
        ] {
            for byte_offset in [field_offset, field_offset + field_width - 1] {
                let mut candidate = candidate(TargetProfile::LinuxX64);
                let expected_identity = candidate
                    .roster
                    .non_authoritative_roster_compatibility_fingerprint();
                candidate.contents.bytes[row_index * ELF64_SECTION_HEADER_SIZE + byte_offset] ^= 1;
                let error = validate_candidate(candidate)
                    .expect_err("mutated ELF section-header field must reject");
                assert!(
                    error
                        .diagnostic
                        .to_string()
                        .contains("decoded ELF64 section header drifted")
                );
                assert_eq!(
                    error
                        .candidate
                        .roster
                        .non_authoritative_roster_compatibility_fingerprint(),
                    expected_identity,
                );
            }
        }
    }
}

#[test]
fn independent_replay_rejects_every_fixup_field_length_order_and_identity_corruption() {
    let corruptions: Vec<Box<dyn Fn(&mut Candidate)>> = vec![
        Box::new(|candidate| {
            candidate.contents.bytes.pop();
        }),
        Box::new(|candidate| candidate.contents.bytes.push(0)),
        Box::new(|candidate| {
            candidate.contents.placement_fixups.pop();
        }),
        Box::new(|candidate| {
            candidate
                .contents
                .placement_fixups
                .push(candidate.contents.placement_fixups[0])
        }),
        Box::new(|candidate| candidate.contents.placement_fixups.swap(0, 1)),
        Box::new(|candidate| candidate.contents.placement_fixups[0].row_index = u32::MAX),
        Box::new(|candidate| {
            candidate.contents.placement_fixups[0].section_kind =
                ElfDynamicRosterSectionKind::DynamicString
        }),
        Box::new(|candidate| candidate.contents.placement_fixups[0].byte_offset += 1),
        Box::new(|candidate| candidate.contents.placement_fixups[0].byte_width = 4),
        Box::new(|candidate| {
            candidate.contents.placement_fixups[0].kind =
                ElfSectionHeaderPlacementFixupKind::FileOffset
        }),
        Box::new(|candidate| candidate.non_authoritative_template_compatibility_fingerprint ^= 1),
    ];
    for corrupt in corruptions {
        let mut candidate = candidate(TargetProfile::LinuxArm64);
        let expected_identity = candidate
            .roster
            .non_authoritative_roster_compatibility_fingerprint();
        corrupt(&mut candidate);
        let error = validate_candidate(candidate)
            .expect_err("corrupt section-header candidate must reject");
        assert_eq!(
            error
                .candidate
                .roster
                .non_authoritative_roster_compatibility_fingerprint(),
            expected_identity
        );
    }
}

#[test]
fn decoder_fixup_bounds_and_arithmetic_reject_without_panicking() {
    assert!(checked_product(usize::MAX, 64, "product").is_err());
    assert!(checked_sum(usize::MAX, 8, "sum").is_err());
    assert!(decode_section_header_rows(&[], usize::MAX).is_err());
    assert!(decode_section_header_rows(&[0; 63], 1).is_err());
    assert!(decode_section_header_rows(&[0; 65], 1).is_err());
    assert!(read_u32(&[0; 3], 0, "word").is_err());
    assert!(read_u64(&[0; 7], 0, "xword").is_err());

    let overflow = ElfSectionHeaderPlacementFixup {
        row_index: u32::MAX,
        section_kind: ElfDynamicRosterSectionKind::Interpreter,
        byte_offset: usize::MAX,
        byte_width: 8,
        kind: ElfSectionHeaderPlacementFixupKind::VirtualAddress,
    };
    assert!(validate_fixup_coverage(768, &[overflow, overflow]).is_err());
}
