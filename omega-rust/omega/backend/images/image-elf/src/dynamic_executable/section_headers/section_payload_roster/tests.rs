//! Section payload roster tests.

use super::{
    Candidate, ElfDynamicRosterSectionKind, ElfIndexedProcedureFixupStorage,
    ValidatedElfSectionHeaderTableTemplate, checked_sum, checked_u32, derive_contents,
    non_authoritative_payload_roster_compatibility_fingerprint, plan_elf_indexed_section_payloads,
    procedure_target_section, procedure_templates, read_field, upstream_payload,
    validate_candidate, validate_contents,
};
use crate::{
    plan_elf_dynamic_link_inputs, plan_elf_dynamic_section_descriptors,
    plan_elf_dynamic_section_roster, plan_elf_dynamic_sections,
    plan_elf_dynamic_table_section_descriptor, plan_elf_dynamic_tags,
    plan_elf_procedure_linkage_relocations, plan_elf_procedure_linkage_section_descriptors,
    plan_elf_procedure_linkage_templates, plan_elf_section_name_table,
    serialize_elf_dynamic_sections, serialize_elf_dynamic_table,
    serialize_elf_section_header_table,
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
    sites: &'static [usize],
}

const IMPORTS: [ImportFixture; 2] = [
    ImportFixture {
        object: b"liba\xff.so",
        symbol: b"alpha\xfe",
        version: b"V1\xfd",
        sites: &[0, 32],
    },
    ImportFixture {
        object: b"libb.so",
        symbol: b"beta",
        version: b"V2",
        sites: &[16],
    },
];

fn headers(
    target: TargetProfile,
    imports: &[ImportFixture],
) -> ValidatedElfSectionHeaderTableTemplate {
    let mut image = FinalImage::with_capacity(
        target.native_target(),
        FinalImageMemory {
            text: vec![0; 64],
            ..FinalImageMemory::default()
        },
        Handle::invalid(),
        imports.len(),
        imports.len(),
        imports.iter().map(|row| row.sites.len()).sum(),
    );
    for (index, fixture) in imports.iter().enumerate() {
        let symbol_handle = image.symbol_table.symbols.insert(FinalImageSymbol {
            name: format!("__omega_payload_roster_import_{index}"),
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
                .unwrap(),
            ),
        });
        for site in fixture.sites {
            let (offset, kind) = match target {
                TargetProfile::LinuxX64 => {
                    image.memory.text[*site] = 0xe8;
                    (site + 1, RelocationKind::X86_64Relative32)
                }
                TargetProfile::LinuxArm64 => {
                    image.memory.text[*site..*site + 4].copy_from_slice(&[0, 0, 0, 0x94]);
                    (*site, RelocationKind::Aarch64Branch26)
                }
                _ => unreachable!(),
            };
            image
                .relocation_table
                .relocations
                .insert(FinalImageRelocation {
                    section: FinalImageSection::Text,
                    offset,
                    byte_width: 4,
                    symbol_handle,
                    addend: 0,
                    kind,
                });
        }
    }
    let path = match target {
        TargetProfile::LinuxX64 => b"/lib64/ld-linux-\xfc-x86-64.so.2".as_slice(),
        TargetProfile::LinuxArm64 => b"/lib/ld-linux-\xfb-aarch64.so.1".as_slice(),
        _ => unreachable!(),
    };
    let interpreter = normalize_elf_interpreter_plan(path.to_vec(), target).unwrap();
    let inputs = plan_elf_dynamic_link_inputs(image, interpreter).unwrap();
    let sections = plan_elf_dynamic_sections(inputs).unwrap();
    let payloads = serialize_elf_dynamic_sections(sections).unwrap();
    let descriptors = plan_elf_dynamic_section_descriptors(payloads).unwrap();
    let linkage = plan_elf_procedure_linkage_relocations(descriptors).unwrap();
    let templates = plan_elf_procedure_linkage_templates(linkage).unwrap();
    let descriptors = plan_elf_procedure_linkage_section_descriptors(templates).unwrap();
    let tags = plan_elf_dynamic_tags(descriptors).unwrap();
    let dynamic = serialize_elf_dynamic_table(tags).unwrap();
    let descriptor = plan_elf_dynamic_table_section_descriptor(dynamic).unwrap();
    let names = plan_elf_section_name_table(descriptor).unwrap();
    let roster = plan_elf_dynamic_section_roster(names).unwrap();
    serialize_elf_section_header_table(roster).unwrap()
}

fn candidate(target: TargetProfile) -> Candidate {
    let section_headers = headers(target, &IMPORTS);
    let contents = derive_contents(&section_headers).unwrap();
    let non_authoritative_payload_roster_compatibility_fingerprint =
        non_authoritative_payload_roster_compatibility_fingerprint(&section_headers, &contents);
    Candidate {
        section_headers,
        contents,
        non_authoritative_payload_roster_compatibility_fingerprint,
    }
}

#[test]
fn both_targets_join_exact_fourteen_payloads_and_indexed_fixups() {
    for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
        let plan = plan_elf_indexed_section_payloads(headers(target, &IMPORTS)).unwrap();
        assert_eq!(plan.row_count(), 14);
        assert_eq!(plan.dynamic_fixup_count(), 9);
        assert!(plan.contents.rows[0].bytes.is_empty());
        assert_eq!(plan.contents.rows[7].bytes.len(), 36);
        assert_eq!(plan.contents.rows[11].bytes.len(), 0);
        assert_eq!(plan.contents.rows[12].bytes.len(), 288);
        assert_eq!(plan.contents.rows[13].bytes.len(), 122);
        for (index, row) in plan.contents.rows.iter().enumerate() {
            assert_eq!(row.index, index as u32);
            assert_eq!(
                row.bytes,
                upstream_payload(plan.section_headers(), row.kind).unwrap()
            );
            assert_eq!(
                row.bytes.len() as u64,
                plan.section_headers.roster().contents().rows[index].payload_size
            );
        }
        validate_contents(plan.section_headers(), &plan.contents).unwrap();
        assert_ne!(
            plan.non_authoritative_payload_roster_compatibility_fingerprint(),
            0
        );
    }
}

#[test]
fn indexed_storage_targets_constraints_and_raw_bytes_are_exact() {
    let plan =
        plan_elf_indexed_section_payloads(headers(TargetProfile::LinuxX64, &IMPORTS)).unwrap();
    assert!(
        plan.contents.rows[2]
            .bytes
            .windows(8)
            .any(|window| window == b"liba\xff.so")
    );
    assert!(
        plan.contents
            .procedure_fixups
            .iter()
            .any(|fixup| fixup.storage == ElfIndexedProcedureFixupStorage::SourceText)
    );
    for fixup in &plan.contents.procedure_fixups {
        assert_eq!(
            fixup.target_section_index,
            procedure_target_section(fixup.target)
        );
        if let ElfIndexedProcedureFixupStorage::Section { index, kind } = fixup.storage {
            assert_eq!(plan.contents.rows[index as usize].kind, kind);
        }
    }
    assert_eq!(
        plan.contents.procedure_constraints,
        procedure_templates(plan.section_headers())
            .contents()
            .constraints
    );
    assert_eq!(
        plan.contents
            .dynamic_fixups
            .iter()
            .map(|fixup| fixup.target_section_index)
            .collect::<Vec<_>>(),
        [9, 4, 7, 2, 3, 10, 11, 5, 6]
    );
}

#[test]
fn permutation_preserves_identity_and_target_remains_bound() {
    let forward =
        plan_elf_indexed_section_payloads(headers(TargetProfile::LinuxX64, &IMPORTS)).unwrap();
    let reverse_imports = IMPORTS.iter().rev().copied().collect::<Vec<_>>();
    let reverse =
        plan_elf_indexed_section_payloads(headers(TargetProfile::LinuxX64, &reverse_imports))
            .unwrap();
    let arm =
        plan_elf_indexed_section_payloads(headers(TargetProfile::LinuxArm64, &IMPORTS)).unwrap();
    assert_eq!(forward.contents, reverse.contents);
    assert_eq!(
        forward.non_authoritative_payload_roster_compatibility_fingerprint(),
        reverse.non_authoritative_payload_roster_compatibility_fingerprint()
    );
    assert_ne!(
        forward.non_authoritative_payload_roster_compatibility_fingerprint(),
        arm.non_authoritative_payload_roster_compatibility_fingerprint()
    );
}

#[test]
fn every_payload_byte_corruption_rejects_with_header_custody() {
    let lengths = candidate(TargetProfile::LinuxX64)
        .contents
        .rows
        .iter()
        .map(|row| row.bytes.len())
        .collect::<Vec<_>>();
    for (row, length) in lengths.into_iter().enumerate() {
        for offset in 0..length {
            let mut candidate = candidate(TargetProfile::LinuxX64);
            let identity = candidate
                .section_headers
                .non_authoritative_template_compatibility_fingerprint();
            candidate.contents.rows[row].bytes[offset] ^= 1;
            let error = validate_candidate(candidate).expect_err("payload corruption must reject");
            assert_eq!(
                error
                    .candidate
                    .section_headers
                    .non_authoritative_template_compatibility_fingerprint(),
                identity
            );
        }
    }
}

#[test]
fn row_fixup_constraint_and_identity_corruption_reject_recoverably() {
    let corruptions: Vec<Box<dyn Fn(&mut Candidate)>> = vec![
        Box::new(|c| {
            c.contents.rows.pop();
        }),
        Box::new(|c| c.contents.rows.push(c.contents.rows[0].clone())),
        Box::new(|c| c.contents.rows.swap(1, 2)),
        Box::new(|c| c.contents.rows[1].index = u32::MAX),
        Box::new(|c| c.contents.rows[1].kind = ElfDynamicRosterSectionKind::DynamicString),
        Box::new(|c| {
            c.contents.procedure_fixups.pop();
        }),
        Box::new(|c| c.contents.procedure_fixups[0].byte_offset = usize::MAX),
        Box::new(|c| c.contents.procedure_fixups[0].byte_width = 3),
        Box::new(|c| c.contents.procedure_fixups[0].mutable_mask ^= 1),
        Box::new(|c| c.contents.procedure_fixups[0].target_section_index ^= 1),
        Box::new(|c| {
            c.contents.procedure_constraints.pop();
        }),
        Box::new(|c| {
            c.contents.dynamic_fixups.pop();
        }),
        Box::new(|c| c.contents.dynamic_fixups[0].storage_section_index = 9),
        Box::new(|c| c.contents.dynamic_fixups[0].target_section_index ^= 1),
        Box::new(|c| c.non_authoritative_payload_roster_compatibility_fingerprint ^= 1),
    ];
    for corrupt in corruptions {
        let mut candidate = candidate(TargetProfile::LinuxArm64);
        let identity = candidate
            .section_headers
            .non_authoritative_template_compatibility_fingerprint();
        corrupt(&mut candidate);
        let error =
            validate_candidate(candidate).expect_err("corrupt indexed payloads must reject");
        assert_eq!(
            error
                .candidate
                .section_headers
                .non_authoritative_template_compatibility_fingerprint(),
            identity
        );
    }
}

#[test]
fn malformed_bounds_widths_and_arithmetic_reject_without_panicking() {
    assert!(checked_sum(usize::MAX, 8, "sum").is_err());
    assert!(checked_u32(usize::MAX, "word").is_err());
    assert!(read_field(&[], usize::MAX, 8).is_err());
    assert!(read_field(&[0; 3], 0, 4).is_err());
    assert!(read_field(&[0; 8], 0, 3).is_err());
}
