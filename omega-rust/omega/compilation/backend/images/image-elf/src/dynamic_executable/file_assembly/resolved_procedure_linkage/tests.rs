//! Resolved procedure linkage tests.

use super::{
    ElfAppliedProcedureLinkageFixup, ElfAppliedProcedureLinkageKind,
    ElfAppliedProcedureLinkageStorage, ElfAppliedProcedureLinkageTarget,
    ValidatedElfDynamicFileEnvelope, apply_elf_procedure_linkage_fixups,
    non_authoritative_resolved_linkage_compatibility_fingerprint,
};
use crate::dynamic_executable::file_assembly::resolved_procedure_linkage::candidates::{
    Candidate, derive_contents, validate_candidate, validate_nonoverlapping_applications,
};
use crate::dynamic_executable::file_assembly::resolved_procedure_linkage::storage_fields::{
    encode_field, load_layout,
};
use crate::dynamic_executable::procedure_linkage::dynamic_linkage_templates::ElfProcedureLinkageFixupKind;
use crate::{
    apply_elf_dynamic_address_fixups, apply_elf_section_header_placements,
    plan_elf_dynamic_load_layout, plan_elf_indexed_section_payloads,
    plan_elf_relative_section_payload_layout, serialize_elf_dynamic_file_envelope,
};
use arena::Handle;
use image::{
    FinalImage, FinalImageImport, FinalImageImportPlan, FinalImageMemory, FinalImageRelocation,
    FinalImageSection, FinalImageSymbol,
};
use object_file::{RelocationKind, SymbolKind};
use target::{ForeignLocatorCandidate, TargetProfile, normalize_foreign_locator};

fn standard_envelope(target: TargetProfile) -> ValidatedElfDynamicFileEnvelope {
    envelope(
        target,
        [b"alpha_call".as_slice(), b"beta_call".as_slice()],
        &[],
    )
}

fn envelope(
    target: TargetProfile,
    imported_symbols: [&[u8]; 2],
    general_slots: &[(usize, i64)],
) -> ValidatedElfDynamicFileEnvelope {
    let mut image = FinalImage::with_capacity(
        target.native_target(),
        FinalImageMemory {
            text: vec![0; 64],
            data: vec![0x5a; 13],
            bss_size: 23,
            bss_alignment: 16,
        },
        Handle::invalid(),
        3,
        2,
        3 + general_slots.len(),
    );
    let entry = image.symbol_table.symbols.insert(FinalImageSymbol {
        name: "_start".to_owned(),
        section: FinalImageSection::Text,
        offset: 24,
        size: 4,
        kind: SymbolKind::Function,
    });
    image.symbol_table.entry_symbol = entry;

    let mut imports = Vec::new();
    for (index, symbol) in imported_symbols.into_iter().enumerate() {
        let handle = image.symbol_table.symbols.insert(FinalImageSymbol {
            name: format!("__omega_resolved_import_{index}"),
            section: FinalImageSection::None,
            offset: 0,
            size: 0,
            kind: SymbolKind::Import,
        });
        image.symbol_table.imports.insert(FinalImageImport {
            symbol_handle: handle,
            import: FinalImageImportPlan::Normalized(
                normalize_foreign_locator(
                    ForeignLocatorCandidate::ElfVersioned {
                        object: b"libresolved-procedure.so".to_vec(),
                        symbol: symbol.to_vec(),
                        version: b"RESOLVED_PROCEDURE_1".to_vec(),
                    },
                    target,
                )
                .unwrap()
                .into(),
            ),
        });
        imports.push(handle);
    }
    crate::test_support::insert_text_call_relocations(
        &mut image,
        target,
        [(0, imports[0]), (8, imports[0]), (16, imports[1])],
    );
    for &(slot_offset, slot_addend) in general_slots {
        image
            .relocation_table
            .relocations
            .insert(FinalImageRelocation {
                section: FinalImageSection::Data,
                offset: slot_offset,
                byte_width: 8,
                symbol_handle: imports[0],
                addend: slot_addend,
                kind: RelocationKind::Absolute64,
            });
    }

    let linkage = crate::test_support::resolved_linkage(image, target);
    let headers = crate::test_support::section_header_table_from_linkage(linkage);
    let payloads = plan_elf_indexed_section_payloads(headers).unwrap();
    let relative = plan_elf_relative_section_payload_layout(payloads).unwrap();
    let load = plan_elf_dynamic_load_layout(relative).unwrap();
    let placed = apply_elf_section_header_placements(load).unwrap();
    let resolved = apply_elf_dynamic_address_fixups(placed).unwrap();
    serialize_elf_dynamic_file_envelope(resolved).unwrap()
}

fn candidate(target: TargetProfile) -> Candidate {
    let envelope = standard_envelope(target);
    let contents = derive_contents(&envelope).unwrap();
    let non_authoritative_resolved_linkage_compatibility_fingerprint =
        non_authoritative_resolved_linkage_compatibility_fingerprint(&envelope, &contents);
    Candidate {
        envelope,
        contents,
        non_authoritative_resolved_linkage_compatibility_fingerprint,
    }
}

fn refresh_fingerprint(candidate: &mut Candidate) {
    candidate.non_authoritative_resolved_linkage_compatibility_fingerprint =
        non_authoritative_resolved_linkage_compatibility_fingerprint(
            &candidate.envelope,
            &candidate.contents,
        );
}

fn assert_rejected_with_custody(candidate: Candidate) {
    let upstream = candidate
        .envelope
        .non_authoritative_envelope_compatibility_fingerprint();
    let error = validate_candidate(candidate).unwrap_err();
    assert_eq!(
        error
            .candidate
            .envelope
            .non_authoritative_envelope_compatibility_fingerprint(),
        upstream,
    );
    assert!(!error.diagnostic.to_string().is_empty());
}

#[test]
fn both_linux_targets_apply_every_exact_procedure_and_source_fixup() {
    for (target, expected_count) in [
        (TargetProfile::LinuxX64, 14),
        (TargetProfile::LinuxArm64, 16),
    ] {
        let envelope = standard_envelope(target);
        let original_text = load_layout(&envelope).retained_image().memory.text.clone();
        let resolved = apply_elf_procedure_linkage_fixups(envelope).unwrap();
        assert_eq!(resolved.applied_fixups().len(), expected_count);
        assert_ne!(
            resolved.non_authoritative_resolved_linkage_compatibility_fingerprint(),
            0,
        );
        assert_eq!(resolved.source_text_bytes().len(), original_text.len());
        assert_ne!(resolved.source_text_bytes(), original_text);
        assert_eq!(
            resolved
                .applied_fixups()
                .iter()
                .map(ElfAppliedProcedureLinkageFixup::ordinal)
                .collect::<Vec<_>>(),
            (0..expected_count as u32).collect::<Vec<_>>(),
        );
        for storage in [
            ElfAppliedProcedureLinkageStorage::SourceText,
            ElfAppliedProcedureLinkageStorage::ProcedureLinkage,
            ElfAppliedProcedureLinkageStorage::ProcedureGot,
            ElfAppliedProcedureLinkageStorage::ProcedureRelocation,
        ] {
            assert!(
                resolved
                    .applied_fixups()
                    .iter()
                    .any(|application| application.storage() == storage),
            );
        }
        assert_eq!(
            load_layout(resolved.envelope())
                .retained_image()
                .memory
                .text,
            original_text,
            "the retained FinalImage must remain immutable",
        );
    }
}

#[test]
fn data_slot_import_relocations_emit_and_apply_exact_general_rela_rows() {
    for (target, relocation_type) in [
        (TargetProfile::LinuxX64, 1_u64),
        (TargetProfile::LinuxArm64, 257_u64),
    ] {
        let resolved = apply_elf_procedure_linkage_fixups(envelope(
            target,
            [b"alpha_call", b"beta_call"],
            &[(0, 7)],
        ))
        .unwrap();
        let rela = resolved.general_relocation_bytes();
        assert_eq!(rela.len(), 24);
        let r_offset = u64::from_le_bytes(rela[0..8].try_into().unwrap());
        let r_info = u64::from_le_bytes(rela[8..16].try_into().unwrap());
        let r_addend = i64::from_le_bytes(rela[16..24].try_into().unwrap());
        let layout = load_layout(resolved.envelope());
        assert_eq!(r_offset, layout.image_memory().data_virtual_address());
        assert_eq!(r_info, (1_u64 << 32) | relocation_type);
        assert_eq!(r_addend, 7);
        let fixup = resolved
            .applied_fixups()
            .iter()
            .find(|fixup| fixup.storage() == ElfAppliedProcedureLinkageStorage::GeneralRelocation)
            .expect("one .rela.dyn r_offset fixup");
        assert_eq!(
            fixup.kind(),
            ElfAppliedProcedureLinkageKind::Elf64RelaOffset
        );
        assert_eq!(fixup.byte_offset(), 0);
        assert_eq!(fixup.byte_width(), 8);
        assert_eq!(
            fixup.target(),
            ElfAppliedProcedureLinkageTarget::RelocatedImageSection {
                section: FinalImageSection::Data,
                byte_offset: 0,
            }
        );
        assert_eq!(
            fixup.target_address(),
            layout.image_memory().data_virtual_address()
        );
        assert_eq!(fixup.encoded_field(), r_offset);
    }
}

#[test]
fn exact_replay_is_deterministic_and_target_or_import_bound() {
    let first =
        apply_elf_procedure_linkage_fixups(standard_envelope(TargetProfile::LinuxX64)).unwrap();
    let replay =
        apply_elf_procedure_linkage_fixups(standard_envelope(TargetProfile::LinuxX64)).unwrap();
    let target_change =
        apply_elf_procedure_linkage_fixups(standard_envelope(TargetProfile::LinuxArm64)).unwrap();
    let import_change = apply_elf_procedure_linkage_fixups(envelope(
        TargetProfile::LinuxX64,
        [b"alpha_call", b"gamma_call"],
        &[],
    ))
    .unwrap();
    assert_eq!(first.source_text_bytes(), replay.source_text_bytes());
    assert_eq!(
        first.procedure_linkage_bytes(),
        replay.procedure_linkage_bytes(),
    );
    assert_eq!(first.applied_fixups(), replay.applied_fixups());
    assert_eq!(
        first.non_authoritative_resolved_linkage_compatibility_fingerprint(),
        replay.non_authoritative_resolved_linkage_compatibility_fingerprint(),
    );
    assert_ne!(
        first.non_authoritative_resolved_linkage_compatibility_fingerprint(),
        target_change.non_authoritative_resolved_linkage_compatibility_fingerprint(),
    );
    assert_ne!(
        first.non_authoritative_resolved_linkage_compatibility_fingerprint(),
        import_change.non_authoritative_resolved_linkage_compatibility_fingerprint(),
    );
}

#[test]
fn byte_opcode_address_and_ledger_substitutions_reject_with_custody() {
    let mut mutable_field = candidate(TargetProfile::LinuxX64);
    mutable_field.contents.source_text_bytes[1] ^= 1;
    refresh_fingerprint(&mut mutable_field);
    assert_rejected_with_custody(mutable_field);

    let mut fixed_opcode = candidate(TargetProfile::LinuxX64);
    fixed_opcode.contents.source_text_bytes[0] ^= 1;
    refresh_fingerprint(&mut fixed_opcode);
    assert_rejected_with_custody(fixed_opcode);

    let mut target = candidate(TargetProfile::LinuxX64);
    target.contents.applications[0].target_address += 4;
    refresh_fingerprint(&mut target);
    assert_rejected_with_custody(target);

    let mut source = candidate(TargetProfile::LinuxX64);
    source.contents.applications[0].source_address += 4;
    refresh_fingerprint(&mut source);
    assert_rejected_with_custody(source);

    let mut kind = candidate(TargetProfile::LinuxX64);
    kind.contents.applications[0].kind = ElfAppliedProcedureLinkageKind::Absolute64;
    refresh_fingerprint(&mut kind);
    assert_rejected_with_custody(kind);
}

#[test]
fn missing_duplicate_reordered_truncated_and_report_drift_reject_with_custody() {
    let mut missing = candidate(TargetProfile::LinuxArm64);
    missing.contents.applications.pop();
    refresh_fingerprint(&mut missing);
    assert_rejected_with_custody(missing);

    let mut duplicate = candidate(TargetProfile::LinuxArm64);
    duplicate.contents.applications[1] = duplicate.contents.applications[0];
    refresh_fingerprint(&mut duplicate);
    assert_rejected_with_custody(duplicate);

    let mut reordered = candidate(TargetProfile::LinuxArm64);
    reordered.contents.applications.swap(0, 1);
    refresh_fingerprint(&mut reordered);
    assert_rejected_with_custody(reordered);

    let mut truncated = candidate(TargetProfile::LinuxArm64);
    truncated.contents.procedure_linkage_bytes.pop();
    refresh_fingerprint(&mut truncated);
    assert_rejected_with_custody(truncated);

    let mut zero = candidate(TargetProfile::LinuxArm64);
    zero.non_authoritative_resolved_linkage_compatibility_fingerprint = 0;
    assert_rejected_with_custody(zero);

    let mut drifted = candidate(TargetProfile::LinuxArm64);
    drifted.non_authoritative_resolved_linkage_compatibility_fingerprint ^= 1;
    assert_rejected_with_custody(drifted);
}

#[test]
fn range_alignment_mask_and_overlap_helpers_fail_closed() {
    assert!(
        encode_field(
            ElfProcedureLinkageFixupKind::X86PcRelative32,
            0,
            0xffff_ffff,
            0,
            i32::MAX as u64 + 5,
        )
        .is_err(),
    );
    assert!(
        encode_field(
            ElfProcedureLinkageFixupKind::Aarch64Branch26,
            0x9400_0000,
            0x03ff_ffff,
            0x1000,
            0x1002,
        )
        .is_err(),
    );
    assert!(
        encode_field(
            ElfProcedureLinkageFixupKind::Aarch64Branch26,
            0x9400_0000,
            0x03ff_ffff,
            0,
            1 << 27,
        )
        .is_err(),
    );
    assert!(
        encode_field(
            ElfProcedureLinkageFixupKind::Aarch64Page21,
            0x9000_0010,
            0x60ff_ffe0,
            0,
            1_u64 << 32,
        )
        .is_err(),
    );
    assert!(
        encode_field(
            ElfProcedureLinkageFixupKind::Aarch64Load64Low12,
            0xf940_0211,
            0x003f_fc00,
            0,
            3,
        )
        .is_err(),
    );
    assert!(
        encode_field(
            ElfProcedureLinkageFixupKind::Absolute64,
            0,
            0xffff,
            0,
            1 << 20,
        )
        .is_err(),
    );

    let first = candidate(TargetProfile::LinuxX64).contents.applications[0];
    let mut overlapping = first;
    overlapping.ordinal += 1;
    assert!(validate_nonoverlapping_applications(&[first, overlapping]).is_err());
}
