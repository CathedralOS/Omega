//! Dynamic file envelope tests.

use super::{
    Candidate, Diagnostic, ELF64_HEADER_PREFIX_SIZE, ELF64_HEADER_SIZE, EM_AARCH64, EM_X86_64,
    FinalImageSection, PT_DYNAMIC, PT_INTERP, PT_LOAD, TargetProfile,
    ValidatedElfResolvedDynamicTable, decode_header, decode_program_headers, derive_contents,
    load_layout, non_authoritative_envelope_compatibility_fingerprint,
    serialize_elf_dynamic_file_envelope, validate_candidate,
};
use crate::{
    apply_elf_dynamic_address_fixups, apply_elf_section_header_placements,
    plan_elf_dynamic_link_inputs, plan_elf_dynamic_load_layout,
    plan_elf_dynamic_section_descriptors, plan_elf_dynamic_section_roster,
    plan_elf_dynamic_sections, plan_elf_dynamic_table_section_descriptor, plan_elf_dynamic_tags,
    plan_elf_indexed_section_payloads, plan_elf_procedure_linkage_relocations,
    plan_elf_procedure_linkage_section_descriptors, plan_elf_procedure_linkage_templates,
    plan_elf_relative_section_payload_layout, plan_elf_section_name_table,
    serialize_elf_dynamic_sections, serialize_elf_dynamic_table,
    serialize_elf_section_header_table,
};
use arena::Handle;
use image::{
    FinalImage, FinalImageImport, FinalImageImportPlan, FinalImageMemory, FinalImageRelocation,
    FinalImageSymbol,
};
use object_file::{RelocationKind, SymbolKind};
use target::{ForeignLocatorCandidate, normalize_elf_interpreter_plan, normalize_foreign_locator};

#[derive(Clone, Copy)]
enum EntryFixture {
    Text {
        offset: usize,
    },
    TextExplicit {
        offset: usize,
        size: usize,
        kind: SymbolKind,
    },
    Data,
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CustodySnapshot {
    image: FinalImage,
    resolved_dynamic_bytes: Vec<u8>,
    placed_section_header_bytes: Vec<u8>,
}

fn resolved(
    target: TargetProfile,
    entry_fixture: EntryFixture,
    imported_symbol: &[u8],
) -> ValidatedElfResolvedDynamicTable {
    let mut image = FinalImage::with_capacity(
        target.native_target(),
        FinalImageMemory {
            text: vec![0; 32],
            data: vec![0x5a; 13],
            bss_size: 23,
            bss_alignment: 16,
        },
        Handle::invalid(),
        2,
        1,
        1,
    );
    let entry = match entry_fixture {
        EntryFixture::Text { offset } => {
            Some((FinalImageSection::Text, offset, 4, SymbolKind::Function))
        }
        EntryFixture::TextExplicit { offset, size, kind } => {
            Some((FinalImageSection::Text, offset, size, kind))
        }
        EntryFixture::Data => Some((FinalImageSection::Data, 0, 4, SymbolKind::Function)),
        EntryFixture::Missing => None,
    };
    if let Some((section, offset, size, kind)) = entry {
        let entry = image.symbol_table.symbols.insert(FinalImageSymbol {
            name: "_start".to_owned(),
            section,
            offset,
            size,
            kind,
        });
        image.symbol_table.entry_symbol = entry;
    }
    let imported = image.symbol_table.symbols.insert(FinalImageSymbol {
        name: "__omega_dynamic_envelope_import".to_owned(),
        section: FinalImageSection::None,
        offset: 0,
        size: 0,
        kind: SymbolKind::Import,
    });
    image.symbol_table.imports.insert(FinalImageImport {
        symbol_handle: imported,
        import: FinalImageImportPlan::Normalized(
            normalize_foreign_locator(
                ForeignLocatorCandidate::ElfVersioned {
                    object: b"libdynamic-envelope.so".to_vec(),
                    symbol: imported_symbol.to_vec(),
                    version: b"DYNAMIC_ENVELOPE_1".to_vec(),
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
            symbol_handle: imported,
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
    let placed = apply_elf_section_header_placements(load).unwrap();
    apply_elf_dynamic_address_fixups(placed).unwrap()
}

fn standard_resolved(target: TargetProfile) -> ValidatedElfResolvedDynamicTable {
    resolved(
        target,
        EntryFixture::Text { offset: 8 },
        b"dynamic_envelope_call",
    )
}

fn candidate(target: TargetProfile) -> Candidate {
    let resolved_dynamic_table = standard_resolved(target);
    let contents = derive_contents(&resolved_dynamic_table).unwrap();
    let non_authoritative_envelope_compatibility_fingerprint =
        non_authoritative_envelope_compatibility_fingerprint(&resolved_dynamic_table, &contents);
    Candidate {
        resolved_dynamic_table,
        contents,
        non_authoritative_envelope_compatibility_fingerprint,
    }
}

fn custody_snapshot(resolved: &ValidatedElfResolvedDynamicTable) -> CustodySnapshot {
    CustodySnapshot {
        image: load_layout(resolved).retained_image().clone(),
        resolved_dynamic_bytes: resolved.bytes().to_vec(),
        placed_section_header_bytes: resolved.placed_section_headers().bytes().to_vec(),
    }
}

fn assert_rejected_with_custody(candidate: Candidate) -> Diagnostic {
    let upstream = candidate
        .resolved_dynamic_table
        .non_authoritative_resolved_compatibility_fingerprint();
    let exact_upstream = custody_snapshot(&candidate.resolved_dynamic_table);
    let error = validate_candidate(candidate).unwrap_err();
    assert_eq!(
        error
            .candidate
            .resolved_dynamic_table
            .non_authoritative_resolved_compatibility_fingerprint(),
        upstream,
    );
    assert_eq!(
        custody_snapshot(&error.candidate.resolved_dynamic_table),
        exact_upstream,
    );
    error.diagnostic
}

fn overwrite_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

#[test]
fn both_linux_targets_emit_exact_non_runnable_envelopes() {
    for (target, machine) in [
        (TargetProfile::LinuxX64, EM_X86_64),
        (TargetProfile::LinuxArm64, EM_AARCH64),
    ] {
        let envelope = serialize_elf_dynamic_file_envelope(standard_resolved(target)).unwrap();
        assert_eq!(envelope.header_prefix_bytes().len(), 344);
        assert_eq!(envelope.section_header_table_bytes().len(), 896);
        assert_ne!(
            envelope.non_authoritative_envelope_compatibility_fingerprint(),
            0,
        );
        let header = decode_header(envelope.header_prefix_bytes()).unwrap();
        assert_eq!(header.machine, machine);
        assert_eq!(header.entry, envelope.entry_address());
        assert_eq!(
            header.section_header_offset,
            envelope.section_header_table_file_offset(),
        );
        assert_eq!(
            envelope.section_header_table_bytes(),
            envelope
                .resolved_dynamic_table()
                .placed_section_headers()
                .bytes(),
        );
        assert_eq!(
            decode_program_headers(envelope.header_prefix_bytes())
                .unwrap()
                .iter()
                .map(|header| header.segment_type)
                .collect::<Vec<_>>(),
            [PT_INTERP, PT_LOAD, PT_LOAD, PT_LOAD, PT_DYNAMIC],
        );
    }
}

#[test]
fn exact_input_and_entry_changes_change_the_envelope() {
    let first = serialize_elf_dynamic_file_envelope(resolved(
        TargetProfile::LinuxX64,
        EntryFixture::Text { offset: 8 },
        b"first_envelope_call",
    ))
    .unwrap();
    let replay = serialize_elf_dynamic_file_envelope(resolved(
        TargetProfile::LinuxX64,
        EntryFixture::Text { offset: 8 },
        b"first_envelope_call",
    ))
    .unwrap();
    let entry_change = serialize_elf_dynamic_file_envelope(resolved(
        TargetProfile::LinuxX64,
        EntryFixture::Text { offset: 12 },
        b"first_envelope_call",
    ))
    .unwrap();
    let import_change = serialize_elf_dynamic_file_envelope(resolved(
        TargetProfile::LinuxX64,
        EntryFixture::Text { offset: 8 },
        b"second_envelope_call",
    ))
    .unwrap();
    assert_eq!(first.header_prefix_bytes(), replay.header_prefix_bytes());
    assert_eq!(
        first.section_header_table_bytes(),
        replay.section_header_table_bytes(),
    );
    assert_ne!(
        first.non_authoritative_envelope_compatibility_fingerprint(),
        entry_change.non_authoritative_envelope_compatibility_fingerprint(),
    );
    assert_ne!(
        first.non_authoritative_envelope_compatibility_fingerprint(),
        import_change.non_authoritative_envelope_compatibility_fingerprint(),
    );
}

#[test]
fn invalid_entry_rejection_returns_the_complete_upstream_owner() {
    for entry in [
        EntryFixture::Missing,
        EntryFixture::Data,
        EntryFixture::TextExplicit {
            offset: 8,
            size: 4,
            kind: SymbolKind::Object,
        },
        EntryFixture::TextExplicit {
            offset: 8,
            size: 0,
            kind: SymbolKind::Function,
        },
        EntryFixture::TextExplicit {
            offset: 31,
            size: 4,
            kind: SymbolKind::Function,
        },
    ] {
        let resolved = resolved(TargetProfile::LinuxX64, entry, b"rejected_entry_call");
        let upstream = resolved.non_authoritative_resolved_compatibility_fingerprint();
        let exact_upstream = custody_snapshot(&resolved);
        let error = serialize_elf_dynamic_file_envelope(resolved).unwrap_err();
        let (returned, diagnostic) = error.into_parts();
        assert_eq!(
            returned.non_authoritative_resolved_compatibility_fingerprint(),
            upstream,
        );
        assert_eq!(custody_snapshot(&returned), exact_upstream);
        assert!(!diagnostic.to_string().is_empty());
    }
}

#[test]
fn exact_header_and_table_substitutions_are_rejected_with_custody() {
    let mut identity = candidate(TargetProfile::LinuxX64);
    identity.contents.header_prefix_bytes[0] ^= 0xff;
    assert_rejected_with_custody(identity);

    let mut target = candidate(TargetProfile::LinuxX64);
    target.contents.header_prefix_bytes[18..20].copy_from_slice(&EM_AARCH64.to_le_bytes());
    assert_rejected_with_custody(target);

    let mut program_header = candidate(TargetProfile::LinuxX64);
    program_header.contents.header_prefix_bytes[ELF64_HEADER_SIZE + 8] ^= 1;
    assert_rejected_with_custody(program_header);

    let mut section_header = candidate(TargetProfile::LinuxX64);
    section_header.contents.section_header_table_bytes[65] ^= 1;
    assert_rejected_with_custody(section_header);

    let mut truncated = candidate(TargetProfile::LinuxX64);
    truncated.contents.header_prefix_bytes.pop();
    assert_rejected_with_custody(truncated);
}

#[test]
fn self_consistent_entry_and_file_offset_substitutions_cannot_escape_replay() {
    let mut entry = candidate(TargetProfile::LinuxX64);
    entry.contents.entry_address += 4;
    overwrite_u64(
        &mut entry.contents.header_prefix_bytes,
        24,
        entry.contents.entry_address,
    );
    entry.non_authoritative_envelope_compatibility_fingerprint =
        non_authoritative_envelope_compatibility_fingerprint(
            &entry.resolved_dynamic_table,
            &entry.contents,
        );
    assert_rejected_with_custody(entry);

    let mut section_offset = candidate(TargetProfile::LinuxX64);
    section_offset.contents.section_header_table_file_offset += 4096;
    overwrite_u64(
        &mut section_offset.contents.header_prefix_bytes,
        40,
        section_offset.contents.section_header_table_file_offset,
    );
    section_offset.non_authoritative_envelope_compatibility_fingerprint =
        non_authoritative_envelope_compatibility_fingerprint(
            &section_offset.resolved_dynamic_table,
            &section_offset.contents,
        );
    assert_rejected_with_custody(section_offset);
}

#[test]
fn section_header_fragment_overlap_and_end_overflow_are_rejected() {
    let mut overlap = candidate(TargetProfile::LinuxX64);
    overlap.contents.section_header_table_file_offset = 128;
    overwrite_u64(
        &mut overlap.contents.header_prefix_bytes,
        40,
        overlap.contents.section_header_table_file_offset,
    );
    overlap.non_authoritative_envelope_compatibility_fingerprint =
        non_authoritative_envelope_compatibility_fingerprint(
            &overlap.resolved_dynamic_table,
            &overlap.contents,
        );
    let diagnostic = assert_rejected_with_custody(overlap);
    assert!(
        diagnostic
            .to_string()
            .contains("overlaps the header prefix")
    );

    let mut load_overlap = candidate(TargetProfile::LinuxX64);
    load_overlap.contents.section_header_table_file_offset = ELF64_HEADER_PREFIX_SIZE as u64;
    overwrite_u64(
        &mut load_overlap.contents.header_prefix_bytes,
        40,
        load_overlap.contents.section_header_table_file_offset,
    );
    load_overlap.non_authoritative_envelope_compatibility_fingerprint =
        non_authoritative_envelope_compatibility_fingerprint(
            &load_overlap.resolved_dynamic_table,
            &load_overlap.contents,
        );
    let diagnostic = assert_rejected_with_custody(load_overlap);
    assert!(
        diagnostic
            .to_string()
            .contains("overlaps a loadable file extent")
    );

    let mut overflow = candidate(TargetProfile::LinuxX64);
    overflow.contents.section_header_table_file_offset = u64::MAX - 100;
    overwrite_u64(
        &mut overflow.contents.header_prefix_bytes,
        40,
        overflow.contents.section_header_table_file_offset,
    );
    overflow.non_authoritative_envelope_compatibility_fingerprint =
        non_authoritative_envelope_compatibility_fingerprint(
            &overflow.resolved_dynamic_table,
            &overflow.contents,
        );
    let diagnostic = assert_rejected_with_custody(overflow);
    assert!(diagnostic.to_string().contains("fragment end overflows"));
}

#[test]
fn compatibility_fingerprint_is_report_only_but_must_replay() {
    let mut zero = candidate(TargetProfile::LinuxX64);
    zero.non_authoritative_envelope_compatibility_fingerprint = 0;
    assert_rejected_with_custody(zero);

    let mut drifted = candidate(TargetProfile::LinuxX64);
    drifted.non_authoritative_envelope_compatibility_fingerprint ^= 1;
    assert_rejected_with_custody(drifted);
}
