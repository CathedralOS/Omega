//! Resolved dynamic table tests.

use super::{
    Candidate, EXPECTED_TARGETS, ElfAppliedDynamicAddress, ElfDynamicAddressApplicationKind,
    ElfDynamicAddressApplicationTarget, ElfPlacedDynamicSectionKind,
    ValidatedElfPlacedSectionHeaderTable, apply_elf_dynamic_address_fixups, checked_product,
    checked_sum, decode_rows, derive_contents, dynamic_row, field_mut, indexed_payloads,
    non_authoritative_resolved_compatibility_fingerprint, read_i64, read_u64, validate_candidate,
};
use crate::{
    apply_elf_section_header_placements, plan_elf_dynamic_link_inputs,
    plan_elf_dynamic_load_layout, plan_elf_dynamic_section_descriptors,
    plan_elf_dynamic_section_roster, plan_elf_dynamic_sections,
    plan_elf_dynamic_table_section_descriptor, plan_elf_dynamic_tags,
    plan_elf_indexed_section_payloads, plan_elf_procedure_linkage_relocations,
    plan_elf_procedure_linkage_section_descriptors, plan_elf_procedure_linkage_templates,
    plan_elf_relative_section_payload_layout, plan_elf_section_name_table,
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

fn placed(target: TargetProfile, imported_symbol: &[u8]) -> ValidatedElfPlacedSectionHeaderTable {
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
    let symbol_handle = image.symbol_table.symbols.insert(FinalImageSymbol {
        name: "__omega_resolved_dynamic_import".to_owned(),
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
                    object: b"libresolved-dynamic.so".to_vec(),
                    symbol: imported_symbol.to_vec(),
                    version: b"RESOLVED_DYNAMIC_1".to_vec(),
                },
                target,
            )
            .unwrap(),
        ),
    });
    let (offset, kind) = match target {
        TargetProfile::LinuxX64 => {
            image.memory.text[0] = 0xe8;
            (1, RelocationKind::X86_64Relative32)
        }
        TargetProfile::LinuxArm64 => {
            image.memory.text[0..4].copy_from_slice(&[0, 0, 0, 0x94]);
            (0, RelocationKind::Aarch64Branch26)
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
    let interpreter_path = match target {
        TargetProfile::LinuxX64 => b"/lib64/ld-linux-x86-64.so.2".as_slice(),
        TargetProfile::LinuxArm64 => b"/lib/ld-linux-aarch64.so.1".as_slice(),
        _ => unreachable!(),
    };
    let interpreter = normalize_elf_interpreter_plan(interpreter_path.to_vec(), target).unwrap();
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
    let headers = serialize_elf_section_header_table(roster).unwrap();
    let payloads = plan_elf_indexed_section_payloads(headers).unwrap();
    let relative = plan_elf_relative_section_payload_layout(payloads).unwrap();
    let load = plan_elf_dynamic_load_layout(relative).unwrap();
    apply_elf_section_header_placements(load).unwrap()
}

fn standard_placed(target: TargetProfile) -> ValidatedElfPlacedSectionHeaderTable {
    placed(target, b"resolved_dynamic_call")
}

fn candidate(target: TargetProfile) -> Candidate {
    let placed_section_headers = standard_placed(target);
    let contents = derive_contents(&placed_section_headers).unwrap();
    let non_authoritative_resolved_compatibility_fingerprint =
        non_authoritative_resolved_compatibility_fingerprint(&placed_section_headers, &contents);
    Candidate {
        placed_section_headers,
        contents,
        non_authoritative_resolved_compatibility_fingerprint,
    }
}

fn write_application_value(candidate: &mut Candidate, ordinal: usize) {
    let application = candidate.contents.applications[ordinal];
    candidate.contents.bytes
        [application.byte_offset..application.byte_offset + usize::from(application.byte_width)]
        .copy_from_slice(&application.value.to_le_bytes());
}

#[test]
fn both_linux_targets_apply_exact_nine_target_virtual_addresses() {
    for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
        let resolved = apply_elf_dynamic_address_fixups(standard_placed(target)).unwrap();
        assert_eq!(resolved.applied_addresses().len(), 9);
        assert_ne!(
            resolved.non_authoritative_resolved_compatibility_fingerprint(),
            0
        );
        assert_eq!(
            resolved
                .applied_addresses()
                .iter()
                .map(ElfAppliedDynamicAddress::target)
                .collect::<Vec<_>>(),
            EXPECTED_TARGETS,
        );
        let layout = resolved.placed_section_headers().load_layout();
        for application in resolved.applied_addresses() {
            assert_eq!(application.storage_section_index(), 12);
            assert_eq!(application.byte_width(), 8);
            assert_eq!(
                application.kind(),
                ElfDynamicAddressApplicationKind::Elf64AbsoluteAddress
            );
            assert_eq!(
                application.byte_offset(),
                application.row_ordinal() as usize * 16 + 8
            );
            let target_section = &layout.sections()[application.target_section_index() as usize];
            assert_eq!(target_section.kind(), application.target_section_kind());
            assert_eq!(target_section.virtual_address(), Some(application.value()));
            assert_eq!(
                read_u64(
                    resolved.bytes(),
                    application.byte_offset(),
                    "test resolved address"
                )
                .unwrap(),
                application.value(),
            );
        }
    }
}

#[test]
fn exactly_seventy_two_mutable_bytes_change_and_every_other_byte_is_preserved() {
    let resolved =
        apply_elf_dynamic_address_fixups(standard_placed(TargetProfile::LinuxX64)).unwrap();
    let indexed = indexed_payloads(resolved.placed_section_headers());
    let source = dynamic_row(indexed).unwrap();
    let mut mutable = vec![false; source.bytes.len()];
    for application in resolved.applied_addresses() {
        let end = application.byte_offset() + usize::from(application.byte_width());
        mutable[application.byte_offset()..end].fill(true);
    }
    assert_eq!(mutable.iter().filter(|byte| **byte).count(), 72);
    for (offset, (&actual, &upstream)) in resolved.bytes().iter().zip(&source.bytes).enumerate() {
        if mutable[offset] {
            assert_eq!(upstream, 0);
        } else {
            assert_eq!(actual, upstream, "non-fixup byte {offset} changed");
        }
    }
}

#[test]
fn resolution_is_deterministic_and_binds_target_and_selected_input() {
    let first = apply_elf_dynamic_address_fixups(standard_placed(TargetProfile::LinuxX64)).unwrap();
    let second =
        apply_elf_dynamic_address_fixups(standard_placed(TargetProfile::LinuxX64)).unwrap();
    let arm = apply_elf_dynamic_address_fixups(standard_placed(TargetProfile::LinuxArm64)).unwrap();
    let other =
        apply_elf_dynamic_address_fixups(placed(TargetProfile::LinuxX64, b"resolved_dynamic_peer"))
            .unwrap();
    assert_eq!(first.bytes(), second.bytes());
    assert_eq!(first.applied_addresses(), second.applied_addresses());
    assert_eq!(
        first.non_authoritative_resolved_compatibility_fingerprint(),
        second.non_authoritative_resolved_compatibility_fingerprint()
    );
    assert_ne!(
        first.non_authoritative_resolved_compatibility_fingerprint(),
        arm.non_authoritative_resolved_compatibility_fingerprint()
    );
    assert_ne!(
        first.non_authoritative_resolved_compatibility_fingerprint(),
        other.non_authoritative_resolved_compatibility_fingerprint()
    );
}

#[test]
fn missing_duplicate_reordered_and_ledger_field_drift_reject_with_custody() {
    let corruptions: Vec<Box<dyn Fn(&mut Candidate)>> = vec![
        Box::new(|candidate| {
            candidate.contents.applications.pop();
        }),
        Box::new(|candidate| {
            candidate
                .contents
                .applications
                .push(candidate.contents.applications[0])
        }),
        Box::new(|candidate| candidate.contents.applications.swap(0, 1)),
        Box::new(|candidate| candidate.contents.applications[0].row_ordinal += 1),
        Box::new(|candidate| candidate.contents.applications[0].byte_offset += 16),
        Box::new(|candidate| candidate.contents.applications[0].byte_width = 4),
        Box::new(|candidate| candidate.contents.applications[0].kind_tag ^= 1),
        Box::new(|candidate| candidate.contents.applications[0].storage_section_index = 9),
        Box::new(|candidate| {
            candidate.contents.applications[0].target =
                ElfDynamicAddressApplicationTarget::SystemVHash
        }),
        Box::new(|candidate| candidate.contents.applications[0].target_section_index = 4),
        Box::new(|candidate| {
            candidate.contents.applications[0].target_section_kind =
                ElfPlacedDynamicSectionKind::SystemVHash
        }),
        Box::new(|candidate| candidate.contents.applications[0].value ^= 1),
        Box::new(|candidate| candidate.non_authoritative_resolved_compatibility_fingerprint = 0),
        Box::new(|candidate| candidate.non_authoritative_resolved_compatibility_fingerprint ^= 1),
    ];
    for corrupt in corruptions {
        let mut candidate = candidate(TargetProfile::LinuxArm64);
        let expected_custody = candidate
            .placed_section_headers
            .non_authoritative_placed_compatibility_fingerprint();
        corrupt(&mut candidate);
        let error =
            validate_candidate(candidate).expect_err("corrupt resolved dynamic ledger must reject");
        assert_eq!(
            error
                .candidate
                .placed_section_headers
                .non_authoritative_placed_compatibility_fingerprint(),
            expected_custody
        );
    }
}

#[test]
fn valid_sibling_file_only_and_null_target_substitution_reject() {
    let mut sibling = candidate(TargetProfile::LinuxX64);
    let hash = sibling.placed_section_headers.load_layout().sections()[4];
    sibling.contents.applications[0].target = ElfDynamicAddressApplicationTarget::SystemVHash;
    sibling.contents.applications[0].target_section_index = 4;
    sibling.contents.applications[0].target_section_kind = ElfPlacedDynamicSectionKind::SystemVHash;
    sibling.contents.applications[0].value = hash.virtual_address().unwrap();
    write_application_value(&mut sibling, 0);
    validate_candidate(sibling).expect_err("valid sibling target substitution must reject");

    let mut file_only = candidate(TargetProfile::LinuxX64);
    let shstrtab = file_only.placed_section_headers.load_layout().sections()[12];
    file_only.contents.applications[0].target_section_index = 12;
    file_only.contents.applications[0].target_section_kind =
        ElfPlacedDynamicSectionKind::SectionNameTable;
    file_only.contents.applications[0].value = shstrtab.file_offset();
    write_application_value(&mut file_only, 0);
    validate_candidate(file_only).expect_err("file-only target substitution must reject");

    let mut null = candidate(TargetProfile::LinuxX64);
    null.contents.applications[0].target_section_index = 0;
    null.contents.applications[0].target_section_kind = ElfPlacedDynamicSectionKind::Null;
    null.contents.applications[0].value = 0;
    write_application_value(&mut null, 0);
    validate_candidate(null).expect_err("null target substitution must reject");
}

#[test]
fn byte_corruption_wrong_endian_truncation_and_trailing_data_reject() {
    let mut applied = candidate(TargetProfile::LinuxX64);
    let offset = applied.contents.applications[0].byte_offset;
    applied.contents.bytes[offset] ^= 1;
    validate_candidate(applied).expect_err("applied-byte/ledger disagreement must reject");

    let mut wrong_endian = candidate(TargetProfile::LinuxX64);
    let offset = wrong_endian.contents.applications[0].byte_offset;
    wrong_endian.contents.bytes[offset..offset + 8].reverse();
    validate_candidate(wrong_endian).expect_err("big-endian address write must reject");

    let mut tag = candidate(TargetProfile::LinuxX64);
    tag.contents.bytes[0] ^= 1;
    validate_candidate(tag).expect_err("dynamic tag drift must reject");

    let mut literal = candidate(TargetProfile::LinuxX64);
    literal.contents.bytes[8] ^= 1;
    validate_candidate(literal).expect_err("non-fixup literal drift must reject");

    let mut null = candidate(TargetProfile::LinuxX64);
    let last = null.contents.bytes.len() - 1;
    null.contents.bytes[last] ^= 1;
    validate_candidate(null).expect_err("final null row drift must reject");

    let mut truncated = candidate(TargetProfile::LinuxX64);
    truncated.contents.bytes.pop();
    validate_candidate(truncated).expect_err("truncated dynamic payload must reject");

    let mut trailing = candidate(TargetProfile::LinuxX64);
    trailing.contents.bytes.push(0);
    validate_candidate(trailing).expect_err("trailing dynamic payload data must reject");
}

#[test]
fn bounds_helpers_reject_without_panicking() {
    assert!(checked_product(usize::MAX, 16, "product").is_err());
    assert!(checked_sum(usize::MAX, 8, "sum").is_err());
    assert!(read_i64(&[0; 7], 0, "sxword").is_err());
    assert!(read_u64(&[0; 7], 0, "xword").is_err());
    assert!(field_mut(&mut [0; 7], 0, 8).is_err());
    assert!(decode_rows(&[], usize::MAX).is_err());
}
