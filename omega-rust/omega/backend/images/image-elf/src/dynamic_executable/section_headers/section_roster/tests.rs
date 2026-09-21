//! Section roster tests.

use super::{
    CANONICAL_KINDS, Candidate, ElfDynamicRosterSectionKind, ElfDynamicSectionRosterContents,
    ElfNumericSectionDescriptor, ValidatedElfSectionNameTablePlan, checked_u32, derive_contents,
    index_for_kind, non_authoritative_roster_compatibility_fingerprint,
    plan_elf_dynamic_section_roster, validate_candidate, validate_contents, validate_name,
    validate_row_against_owner,
};
use crate::{
    plan_elf_dynamic_link_inputs, plan_elf_dynamic_section_descriptors, plan_elf_dynamic_sections,
    plan_elf_dynamic_table_section_descriptor, plan_elf_dynamic_tags,
    plan_elf_procedure_linkage_relocations, plan_elf_procedure_linkage_section_descriptors,
    plan_elf_procedure_linkage_templates, plan_elf_section_name_table,
    serialize_elf_dynamic_sections, serialize_elf_dynamic_table,
};
use arena::Handle;
use image::{
    FinalImage, FinalImageImport, FinalImageImportPlan, FinalImageMemory, FinalImageRelocation,
    FinalImageSection, FinalImageSymbol,
};
use object_file::{RelocationKind, SymbolKind};
use target::{
    ForeignLocatorCandidate, TargetProfile, normalize_elf_interpreter_plan,
    normalize_foreign_locator,
};

#[derive(Clone, Copy)]
struct ImportFixture {
    object: &'static [u8],
    symbol: &'static [u8],
    version: &'static [u8],
    instruction_offsets: &'static [usize],
}

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

fn interpreter_path(target: TargetProfile) -> &'static [u8] {
    match target {
        TargetProfile::LinuxX64 => b"/lib64/ld-linux-\xfc-x86-64.so.2",
        TargetProfile::LinuxArm64 => b"/lib/ld-linux-\xfb-aarch64.so.1",
        _ => unreachable!("section-roster fixture uses a Linux target"),
    }
}

fn section_names(
    target: TargetProfile,
    imports: &[ImportFixture],
) -> ValidatedElfSectionNameTablePlan {
    let relocation_count = imports
        .iter()
        .map(|fixture| fixture.instruction_offsets.len())
        .sum();
    let mut image = FinalImage::with_capacity(
        target.native_target(),
        FinalImageMemory {
            text: vec![0; 64],
            ..FinalImageMemory::default()
        },
        Handle::invalid(),
        imports.len(),
        imports.len(),
        relocation_count,
    );
    for (index, fixture) in imports.iter().enumerate() {
        let symbol_handle = image.symbol_table.symbols.insert(FinalImageSymbol {
            name: format!("__omega_section_roster_import_{index}"),
            section: FinalImageSection::None,
            offset: 0,
            size: 0,
            kind: SymbolKind::Import,
        });
        image.symbol_table.imports.insert(FinalImageImport {
            symbol_handle,
            import: FinalImageImportPlan::Normalized(
                normalize_foreign_locator(
                    ForeignLocatorCandidate::ElfVersioned {
                        object: fixture.object.to_vec(),
                        symbol: fixture.symbol.to_vec(),
                        version: fixture.version.to_vec(),
                    },
                    target,
                )
                .expect("valid section-roster locator"),
            ),
        });
        for instruction_offset in fixture.instruction_offsets {
            let (relocation_offset, kind) = match target {
                TargetProfile::LinuxX64 => {
                    image.memory.text[*instruction_offset] = 0xe8;
                    (instruction_offset + 1, RelocationKind::X86_64Relative32)
                }
                TargetProfile::LinuxArm64 => {
                    image.memory.text[*instruction_offset..*instruction_offset + 4]
                        .copy_from_slice(&[0, 0, 0, 0x94]);
                    (*instruction_offset, RelocationKind::Aarch64Branch26)
                }
                _ => unreachable!(),
            };
            image
                .relocation_table
                .relocations
                .insert(FinalImageRelocation {
                    section: FinalImageSection::Text,
                    offset: relocation_offset,
                    byte_width: 4,
                    symbol_handle,
                    addend: 0,
                    kind,
                });
        }
    }
    let interpreter = normalize_elf_interpreter_plan(interpreter_path(target).to_vec(), target)
        .expect("valid section-roster interpreter");
    let inputs = plan_elf_dynamic_link_inputs(image, interpreter).expect("valid link inputs");
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

fn row(
    contents: &ElfDynamicSectionRosterContents,
    kind: ElfDynamicRosterSectionKind,
) -> &ElfNumericSectionDescriptor {
    contents
        .rows
        .get(index_for_kind(kind) as usize)
        .expect("numeric roster row")
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
fn exact_numeric_links_and_literal_info_are_preserved() {
    let roster =
        plan_elf_dynamic_section_roster(section_names(TargetProfile::LinuxX64, &IMPORTS)).unwrap();
    assert_eq!(
        *row(&roster.contents, ElfDynamicRosterSectionKind::DynamicSymbol),
        ElfNumericSectionDescriptor {
            kind: ElfDynamicRosterSectionKind::DynamicSymbol,
            index: 3,
            name_offset: 17,
            section_type: 11,
            flags: 2,
            payload_size: 72,
            alignment: 8,
            entry_size: 24,
            link: 2,
            info: 1,
        },
    );
    let verneed = row(
        &roster.contents,
        ElfDynamicRosterSectionKind::GnuVersionRequirement,
    );
    assert_eq!((verneed.link, verneed.info), (2, 2));
    let rela = row(
        &roster.contents,
        ElfDynamicRosterSectionKind::ProcedureRelocation,
    );
    assert_eq!((rela.link, rela.info), (3, 9));
    let dynamic = row(&roster.contents, ElfDynamicRosterSectionKind::DynamicTable);
    assert_eq!(
        (dynamic.name_offset, dynamic.link, dynamic.info),
        (113, 2, 0)
    );
    let shstrtab = row(
        &roster.contents,
        ElfDynamicRosterSectionKind::SectionNameTable,
    );
    assert_eq!(
        (
            shstrtab.name_offset,
            shstrtab.section_type,
            shstrtab.payload_size,
            shstrtab.link,
            shstrtab.info,
        ),
        (59, 3, 122, 0, 0),
    );
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
