//! Section roster tests.

use super::{
    CANONICAL_KINDS, Candidate, ElfDynamicRosterSectionKind, ElfNumericSectionDescriptor,
    ValidatedElfSectionNameTablePlan, checked_u32, derive_contents, index_for_kind,
    non_authoritative_roster_compatibility_fingerprint, plan_elf_dynamic_section_roster,
    validate_candidate, validate_contents, validate_name, validate_row_against_owner,
};
use crate::{
    plan_elf_dynamic_section_descriptors, plan_elf_dynamic_sections,
    plan_elf_dynamic_table_section_descriptor, plan_elf_dynamic_tags,
    plan_elf_procedure_linkage_relocations, plan_elf_procedure_linkage_section_descriptors,
    plan_elf_procedure_linkage_templates, plan_elf_section_name_table,
    serialize_elf_dynamic_sections, serialize_elf_dynamic_table,
};
use target::TargetProfile;

const FIRST_SITES: &[usize] = &[0, 32];
const SECOND_SITES: &[usize] = &[16];
const IMPORTS: [ImportFixture; 2] = [
    ImportFixture {
        object: b"liba\xff.so",
        symbol: b"alpha\xfe",
        version: b"V1\xfd",
        instruction_offsets: FIRST_SITES,
    },
    ImportFixture {
        object: b"libb.so",
        symbol: b"beta",
        version: b"V2",
        instruction_offsets: SECOND_SITES,
    },
];

use crate::test_support::DynamicImportFixture as ImportFixture;
fn section_names(
    target: TargetProfile,
    imports: &[ImportFixture],
) -> ValidatedElfSectionNameTablePlan {
    let inputs = crate::test_support::dynamic_import_inputs(
        target,
        imports,
        "__omega_section_roster_import_",
        "valid section-roster locator",
        "valid section-roster interpreter",
    );
    let sections = plan_elf_dynamic_sections(inputs).expect("valid dynamic sections");
    let payloads = serialize_elf_dynamic_sections(sections).expect("valid payloads");
    let descriptors =
        plan_elf_dynamic_section_descriptors(payloads).expect("valid base descriptors");
    let linkage = plan_elf_procedure_linkage_relocations(descriptors).expect("valid linkage plan");
    let templates = plan_elf_procedure_linkage_templates(linkage).expect("valid linkage templates");
    let descriptors = plan_elf_procedure_linkage_section_descriptors(templates)
        .expect("valid linkage descriptors");
    let tags = plan_elf_dynamic_tags(descriptors).expect("valid dynamic tags");
    let payload = serialize_elf_dynamic_table(tags).expect("valid dynamic payload");
    let descriptor =
        plan_elf_dynamic_table_section_descriptor(payload).expect("valid dynamic descriptor");
    plan_elf_section_name_table(descriptor).expect("valid section-name table")
}

fn candidate(target: TargetProfile) -> Candidate {
    let section_names = section_names(target, &IMPORTS);
    let contents = derive_contents(&section_names).expect("derived section roster");
    let non_authoritative_roster_compatibility_fingerprint =
        non_authoritative_roster_compatibility_fingerprint(&section_names, &contents);
    Candidate {
        section_names,
        contents,
        non_authoritative_roster_compatibility_fingerprint,
    }
}

#[test]
fn both_targets_close_exact_fourteen_row_roster_and_shstrndx() {
    for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
        let roster = plan_elf_dynamic_section_roster(section_names(target, &IMPORTS))
            .expect("validated section roster");
        assert_eq!(roster.section_count(), 14);
        assert_eq!(roster.section_name_table_index(), 13);
        assert_eq!(roster.contents.rows.len(), 14);
        assert_eq!(
            roster
                .contents
                .rows
                .iter()
                .map(|row| row.kind)
                .collect::<Vec<_>>(),
            CANONICAL_KINDS,
        );
        for (index, row) in roster.contents.rows.iter().enumerate() {
            assert_eq!(row.index, index as u32);
            validate_name(roster.section_names.contents().bytes.as_slice(), row).unwrap();
            validate_row_against_owner(roster.section_names(), row).unwrap();
        }
        assert_ne!(
            roster.non_authoritative_roster_compatibility_fingerprint(),
            0
        );
        validate_contents(roster.section_names(), &roster.contents).unwrap();
    }
}

#[test]
fn import_permutation_preserves_roster_identity_and_target_remains_bound() {
    let forward =
        plan_elf_dynamic_section_roster(section_names(TargetProfile::LinuxX64, &IMPORTS)).unwrap();
    let reverse_imports = IMPORTS.iter().rev().copied().collect::<Vec<_>>();
    let reverse =
        plan_elf_dynamic_section_roster(section_names(TargetProfile::LinuxX64, &reverse_imports))
            .unwrap();
    let arm = plan_elf_dynamic_section_roster(section_names(TargetProfile::LinuxArm64, &IMPORTS))
        .unwrap();
    assert_eq!(forward.contents, reverse.contents);
    assert_eq!(
        forward.non_authoritative_roster_compatibility_fingerprint(),
        reverse.non_authoritative_roster_compatibility_fingerprint()
    );
    assert_ne!(
        forward.non_authoritative_roster_compatibility_fingerprint(),
        arm.non_authoritative_roster_compatibility_fingerprint()
    );
}

#[test]
fn independent_replay_rejects_missing_duplicate_reordered_and_every_field_corruption() {
    let corruptions: Vec<Box<dyn Fn(&mut Candidate)>> = vec![
        Box::new(|candidate| {
            candidate.contents.rows.pop();
        }),
        Box::new(|candidate| {
            candidate.contents.rows.push(candidate.contents.rows[0]);
        }),
        Box::new(|candidate| candidate.contents.rows.swap(1, 2)),
        Box::new(|candidate| candidate.contents.rows[1].kind = ElfDynamicRosterSectionKind::Null),
        Box::new(|candidate| candidate.contents.rows[1].index = u32::MAX),
        Box::new(|candidate| candidate.contents.rows[1].name_offset = u32::MAX),
        Box::new(|candidate| candidate.contents.rows[1].section_type ^= 1),
        Box::new(|candidate| candidate.contents.rows[1].flags ^= 1),
        Box::new(|candidate| candidate.contents.rows[1].payload_size += 1),
        Box::new(|candidate| candidate.contents.rows[1].alignment += 1),
        Box::new(|candidate| candidate.contents.rows[1].entry_size = 1),
        Box::new(|candidate| candidate.contents.rows[1].link = 2),
        Box::new(|candidate| candidate.contents.rows[1].info = 1),
        Box::new(|candidate| candidate.contents.section_name_table_index = 10),
        Box::new(|candidate| candidate.non_authoritative_roster_compatibility_fingerprint ^= 1),
    ];
    for corrupt in corruptions {
        let mut candidate = candidate(TargetProfile::LinuxArm64);
        let expected_identity = candidate
            .section_names
            .non_authoritative_table_compatibility_fingerprint();
        corrupt(&mut candidate);
        let error = validate_candidate(candidate)
            .expect_err("corrupt numeric roster candidate must reject");
        assert_eq!(
            error
                .candidate
                .section_names
                .non_authoritative_table_compatibility_fingerprint(),
            expected_identity,
        );
    }
}

#[test]
fn each_resolved_reference_and_literal_info_rejects_drift_with_custody() {
    for (kind, field) in [
        (ElfDynamicRosterSectionKind::DynamicSymbol, true),
        (ElfDynamicRosterSectionKind::SystemVHash, true),
        (ElfDynamicRosterSectionKind::GnuHash, true),
        (ElfDynamicRosterSectionKind::GnuSymbolVersion, true),
        (ElfDynamicRosterSectionKind::GnuVersionRequirement, true),
        (ElfDynamicRosterSectionKind::ProcedureRelocation, true),
        (ElfDynamicRosterSectionKind::ProcedureRelocation, false),
        (ElfDynamicRosterSectionKind::DynamicTable, true),
        (ElfDynamicRosterSectionKind::DynamicSymbol, false),
        (ElfDynamicRosterSectionKind::GnuVersionRequirement, false),
    ] {
        let mut candidate = candidate(TargetProfile::LinuxX64);
        let expected_identity = candidate
            .section_names
            .non_authoritative_table_compatibility_fingerprint();
        let row = &mut candidate.contents.rows[index_for_kind(kind) as usize];
        if field {
            row.link = row.link.saturating_add(1);
        } else {
            row.info = row.info.saturating_add(1);
        }
        let error = validate_candidate(candidate).expect_err("reference drift must reject");
        assert_eq!(
            error
                .candidate
                .section_names
                .non_authoritative_table_compatibility_fingerprint(),
            expected_identity,
        );
    }
}

#[test]
fn malformed_indexes_names_and_references_reject_without_panicking() {
    assert!(checked_u32(usize::MAX, "index").is_err());
    let names = b"\0.interp\0";
    let mut descriptor = ElfNumericSectionDescriptor {
        kind: ElfDynamicRosterSectionKind::Interpreter,
        index: 1,
        name_offset: u32::MAX,
        section_type: 1,
        flags: 2,
        payload_size: 1,
        alignment: 1,
        entry_size: 0,
        link: 0,
        info: 0,
    };
    assert!(validate_name(names, &descriptor).is_err());
    descriptor.name_offset = 1;
    assert!(validate_name(b"\0.interp", &descriptor).is_err());
    descriptor.name_offset = 2;
    assert!(validate_name(names, &descriptor).is_err());

    let mut candidate = candidate(TargetProfile::LinuxX64);
    candidate.contents.rows[3].link = u32::MAX;
    assert!(validate_candidate(candidate).is_err());
}
