//! Section header byte tests.

use super::{
    Candidate, ELF64_SECTION_HEADER_SIZE, ElfDynamicRosterSectionKind,
    ElfSectionHeaderPlacementFixup, ElfSectionHeaderPlacementFixupKind, SECTION_COUNT,
    ValidatedElfDynamicSectionRoster, checked_product, checked_sum, decode_rows, encode_contents,
    non_authoritative_template_compatibility_fingerprint, read_u32, read_u64,
    serialize_elf_section_header_table, validate_candidate, validate_contents,
    validate_fixup_coverage,
};
use crate::{
    plan_elf_dynamic_link_inputs, plan_elf_dynamic_section_descriptors,
    plan_elf_dynamic_section_roster, plan_elf_dynamic_sections,
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
        _ => unreachable!("section-header fixture uses a Linux target"),
    }
}

fn roster(target: TargetProfile, imports: &[ImportFixture]) -> ValidatedElfDynamicSectionRoster {
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
            name: format!("__omega_section_header_import_{index}"),
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
                .expect("valid section-header locator"),
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
        .expect("valid section-header interpreter");
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
    let names = plan_elf_section_name_table(descriptor).expect("valid section-name table");
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

        let decoded = decode_rows(&template.contents.bytes, 14).unwrap();
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
fn every_serialized_byte_corruption_rejects_with_roster_custody() {
    for offset in 0..SECTION_COUNT * ELF64_SECTION_HEADER_SIZE {
        let mut candidate = candidate(TargetProfile::LinuxX64);
        let expected_identity = candidate
            .roster
            .non_authoritative_roster_compatibility_fingerprint();
        candidate.contents.bytes[offset] ^= 1;
        let error =
            validate_candidate(candidate).expect_err("mutated ELF section-header byte must reject");
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
    assert!(decode_rows(&[], usize::MAX).is_err());
    assert!(decode_rows(&[0; 63], 1).is_err());
    assert!(decode_rows(&[0; 65], 1).is_err());
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
