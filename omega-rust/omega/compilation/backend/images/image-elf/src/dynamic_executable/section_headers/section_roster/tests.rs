//! Section roster tests.

use super::{
    CANONICAL_KINDS, Candidate, ElfDynamicRosterSectionKind, ElfNumericSectionDescriptor,
    ValidatedElfSectionNameTablePlan, checked_u32, derive_contents, index_for_kind,
    non_authoritative_roster_compatibility_fingerprint, plan_elf_dynamic_section_roster,
    validate_candidate, validate_contents, validate_name, validate_row_against_owner,
};
use target::TargetProfile;

const IMPORTS: [crate::test_support::DynamicImportFixture; 2] = crate::test_support::PAIR_IMPORTS;

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
    crate::test_support::dynamic_section_names(inputs)
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
