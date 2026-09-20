//! Assembled dynamic file tests.

use super::{
    Candidate, Diagnostic, ElfDynamicFileFragmentKind, ElfDynamicFileFragmentPlacement,
    ValidatedElfResolvedProcedureLinkage, admit_elf_dynamic_executable, assemble_elf_dynamic_file,
    derive_contents, derive_executable_output, load_layout,
    non_authoritative_assembled_file_compatibility_fingerprint, section_bytes, validate_candidate,
    validate_executable_output,
};
use crate::{
    apply_elf_dynamic_address_fixups, apply_elf_procedure_linkage_fixups,
    apply_elf_section_header_placements, plan_elf_dynamic_link_inputs,
    plan_elf_dynamic_load_layout, plan_elf_dynamic_section_descriptors,
    plan_elf_dynamic_section_roster, plan_elf_dynamic_sections,
    plan_elf_dynamic_table_section_descriptor, plan_elf_dynamic_tags,
    plan_elf_indexed_section_payloads, plan_elf_procedure_linkage_relocations,
    plan_elf_procedure_linkage_section_descriptors, plan_elf_procedure_linkage_templates,
    plan_elf_relative_section_payload_layout, plan_elf_section_name_table,
    serialize_elf_dynamic_file_envelope, serialize_elf_dynamic_sections,
    serialize_elf_dynamic_table, serialize_elf_section_header_table,
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolvedCustodySnapshot {
    image: FinalImage,
    header_prefix: Vec<u8>,
    section_headers: Vec<u8>,
    resolved_dynamic: Vec<u8>,
    source_text: Vec<u8>,
    procedure_linkage: Vec<u8>,
    procedure_got: Vec<u8>,
    procedure_relocation: Vec<u8>,
    applications: Vec<crate::ElfAppliedProcedureLinkageFixup>,
}

fn standard_resolved_linkage(target: TargetProfile) -> ValidatedElfResolvedProcedureLinkage {
    resolved_linkage(target, [b"alpha_call", b"beta_call"])
}

fn resolved_linkage(
    target: TargetProfile,
    imported_symbols: [&[u8]; 2],
) -> ValidatedElfResolvedProcedureLinkage {
    let mut image = FinalImage::with_capacity(
        target.native_target(),
        FinalImageMemory {
            text: vec![0; 64],
            data: (0_u8..13).map(|byte| byte.wrapping_add(0x50)).collect(),
            bss_size: 23,
            bss_alignment: 16,
        },
        Handle::invalid(),
        3,
        2,
        3,
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
            name: format!("__omega_assembled_import_{index}"),
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
                        object: b"libassembled-file.so".to_vec(),
                        symbol: symbol.to_vec(),
                        version: b"ASSEMBLED_FILE_1".to_vec(),
                    },
                    target,
                )
                .unwrap(),
            ),
        });
        imports.push(handle);
    }
    for (instruction_offset, symbol_handle) in [(0, imports[0]), (8, imports[0]), (16, imports[1])]
    {
        let (relocation_offset, kind) = match target {
            TargetProfile::LinuxX64 => {
                image.memory.text[instruction_offset] = 0xe8;
                (instruction_offset + 1, RelocationKind::X86_64Relative32)
            }
            TargetProfile::LinuxArm64 => {
                image.memory.text[instruction_offset..instruction_offset + 4]
                    .copy_from_slice(&[0, 0, 0, 0x94]);
                (instruction_offset, RelocationKind::Aarch64Branch26)
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
    let resolved = apply_elf_dynamic_address_fixups(placed).unwrap();
    let envelope = serialize_elf_dynamic_file_envelope(resolved).unwrap();
    apply_elf_procedure_linkage_fixups(envelope).unwrap()
}

fn candidate(target: TargetProfile) -> Candidate {
    let resolved_linkage = standard_resolved_linkage(target);
    let contents = derive_contents(&resolved_linkage).unwrap();
    let non_authoritative_assembled_file_compatibility_fingerprint =
        non_authoritative_assembled_file_compatibility_fingerprint(&resolved_linkage, &contents);
    Candidate {
        resolved_linkage,
        contents,
        non_authoritative_assembled_file_compatibility_fingerprint,
    }
}

fn refresh_fingerprint(candidate: &mut Candidate) {
    candidate.non_authoritative_assembled_file_compatibility_fingerprint =
        non_authoritative_assembled_file_compatibility_fingerprint(
            &candidate.resolved_linkage,
            &candidate.contents,
        );
}

fn custody_snapshot(
    resolved_linkage: &ValidatedElfResolvedProcedureLinkage,
) -> ResolvedCustodySnapshot {
    let envelope = resolved_linkage.envelope();
    ResolvedCustodySnapshot {
        image: load_layout(resolved_linkage).retained_image().clone(),
        header_prefix: envelope.header_prefix_bytes().to_vec(),
        section_headers: envelope.section_header_table_bytes().to_vec(),
        resolved_dynamic: envelope.resolved_dynamic_table().bytes().to_vec(),
        source_text: resolved_linkage.source_text_bytes().to_vec(),
        procedure_linkage: resolved_linkage.procedure_linkage_bytes().to_vec(),
        procedure_got: resolved_linkage.procedure_got_bytes().to_vec(),
        procedure_relocation: resolved_linkage.procedure_relocation_bytes().to_vec(),
        applications: resolved_linkage.applied_fixups().to_vec(),
    }
}

fn assert_rejected_with_exact_custody(candidate: Candidate) -> Diagnostic {
    let compact = candidate
        .resolved_linkage
        .non_authoritative_resolved_linkage_compatibility_fingerprint();
    let exact = custody_snapshot(&candidate.resolved_linkage);
    let error = validate_candidate(candidate).unwrap_err();
    assert_eq!(
        error
            .candidate
            .resolved_linkage
            .non_authoritative_resolved_linkage_compatibility_fingerprint(),
        compact,
    );
    assert_eq!(custody_snapshot(&error.candidate.resolved_linkage), exact);
    error.diagnostic
}

fn byte_range(placement: &ElfDynamicFileFragmentPlacement) -> std::ops::Range<usize> {
    let start = usize::try_from(placement.file_offset()).unwrap();
    let count = usize::try_from(placement.byte_count()).unwrap();
    start..start + count
}

#[test]
fn both_linux_targets_assemble_every_exact_fragment_and_zero_gap_without_bss_bytes() {
    for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
        let resolved = standard_resolved_linkage(target);
        let image_before = load_layout(&resolved).retained_image().clone();
        let assembled = assemble_elf_dynamic_file(resolved).unwrap();
        let resolved = assembled.resolved_linkage();
        let load = load_layout(resolved);
        let envelope = resolved.envelope();

        assert_eq!(assembled.fragment_placements().len(), 17);
        assert_eq!(
            assembled.bytes().len(),
            usize::try_from(envelope.section_header_table_file_offset()).unwrap()
                + envelope.section_header_table_bytes().len(),
        );
        assert_ne!(
            assembled.non_authoritative_assembled_file_compatibility_fingerprint(),
            0,
        );
        assert_eq!(load.retained_image(), &image_before);

        let indexed = load.relative().payloads().contents();
        let mut occupied = vec![false; assembled.bytes().len()];
        for (ordinal, placement) in assembled.fragment_placements().iter().enumerate() {
            assert_eq!(placement.ordinal(), ordinal as u32);
            let (expected_offset, expected) = match placement.kind() {
                ElfDynamicFileFragmentKind::HeaderPrefix => (0, envelope.header_prefix_bytes()),
                ElfDynamicFileFragmentKind::SourceText => (
                    load.image_memory().text_file_offset(),
                    resolved.source_text_bytes(),
                ),
                ElfDynamicFileFragmentKind::SourceData => (
                    load.image_memory().data_file_offset(),
                    load.retained_image().memory.data.as_slice(),
                ),
                ElfDynamicFileFragmentKind::Section { index, kind } => {
                    let placed = &load.sections()[index as usize];
                    assert_eq!(placed.index(), index);
                    assert_eq!(placed.kind(), kind);
                    (
                        placed.file_offset(),
                        section_bytes(
                            resolved,
                            index as usize,
                            &indexed.rows[index as usize].bytes,
                        ),
                    )
                }
                ElfDynamicFileFragmentKind::SectionHeaderTable => (
                    envelope.section_header_table_file_offset(),
                    envelope.section_header_table_bytes(),
                ),
            };
            assert_eq!(placement.file_offset(), expected_offset);
            let range = byte_range(placement);
            assert_eq!(range.len(), expected.len());
            assert_eq!(&assembled.bytes()[range.clone()], expected);
            assert!(occupied[range.clone()].iter().all(|occupied| !occupied));
            occupied[range].fill(true);
        }
        assert!(
            assembled
                .bytes()
                .iter()
                .zip(&occupied)
                .all(|(byte, occupied)| *occupied || *byte == 0),
        );

        let memory = load.image_memory();
        let read_write = load
            .program_headers()
            .iter()
            .find(|header| header.kind() == crate::ElfLoadProgramHeaderKind::LoadReadWrite)
            .unwrap();
        assert!(read_write.memory_size() > read_write.file_size());
        assert!(
            memory.bss_virtual_address() >= read_write.virtual_address() + read_write.file_size()
        );
        assert!(
            memory.bss_virtual_address() + memory.bss_size()
                <= read_write.virtual_address() + read_write.memory_size()
        );
        assert!(
            assembled
                .fragment_placements()
                .iter()
                .all(|placement| !matches!(
                    placement.kind(),
                    ElfDynamicFileFragmentKind::Section { index: 0, .. }
                ))
        );
    }
}

#[test]
fn assembly_is_deterministic_and_bound_to_target_and_exact_imports() {
    let first =
        assemble_elf_dynamic_file(standard_resolved_linkage(TargetProfile::LinuxX64)).unwrap();
    let replay =
        assemble_elf_dynamic_file(standard_resolved_linkage(TargetProfile::LinuxX64)).unwrap();
    let target_change =
        assemble_elf_dynamic_file(standard_resolved_linkage(TargetProfile::LinuxArm64)).unwrap();
    let import_change = assemble_elf_dynamic_file(resolved_linkage(
        TargetProfile::LinuxX64,
        [b"alpha_call", b"gamma_call"],
    ))
    .unwrap();
    assert_eq!(first.bytes(), replay.bytes());
    assert_eq!(first.fragment_placements(), replay.fragment_placements());
    assert_eq!(
        first.non_authoritative_assembled_file_compatibility_fingerprint(),
        replay.non_authoritative_assembled_file_compatibility_fingerprint(),
    );
    assert_ne!(first.bytes(), target_change.bytes());
    assert_ne!(first.bytes(), import_change.bytes());
    assert_ne!(
        first.non_authoritative_assembled_file_compatibility_fingerprint(),
        target_change.non_authoritative_assembled_file_compatibility_fingerprint(),
    );
    assert_ne!(
        first.non_authoritative_assembled_file_compatibility_fingerprint(),
        import_change.non_authoritative_assembled_file_compatibility_fingerprint(),
    );
}

#[test]
fn every_fragment_and_one_alignment_gap_mutation_reject_with_exact_custody() {
    let placements = candidate(TargetProfile::LinuxX64)
        .contents
        .fragment_placements
        .clone();
    for placement in placements {
        let mut changed = candidate(TargetProfile::LinuxX64);
        let range = byte_range(&placement);
        if range.is_empty() {
            continue;
        }
        changed.contents.bytes[range.start] ^= 1;
        refresh_fingerprint(&mut changed);
        let diagnostic = assert_rejected_with_exact_custody(changed);
        assert!(
            diagnostic
                .to_string()
                .contains("fragment bytes do not replay")
        );
    }

    let mut padding = candidate(TargetProfile::LinuxX64);
    let mut occupied = vec![false; padding.contents.bytes.len()];
    for placement in &padding.contents.fragment_placements {
        occupied[byte_range(placement)].fill(true);
    }
    let gap = occupied
        .iter()
        .position(|occupied| !occupied)
        .expect("fixture must contain alignment padding");
    padding.contents.bytes[gap] = 1;
    refresh_fingerprint(&mut padding);
    let diagnostic = assert_rejected_with_exact_custody(padding);
    assert!(
        diagnostic
            .to_string()
            .contains("padding is not zero-filled")
    );
}

#[test]
fn truncation_append_and_ledger_substitutions_reject_with_exact_custody() {
    let mut truncated = candidate(TargetProfile::LinuxArm64);
    truncated.contents.bytes.pop();
    refresh_fingerprint(&mut truncated);
    assert!(
        assert_rejected_with_exact_custody(truncated)
            .to_string()
            .contains("file length drifted"),
    );

    let mut appended = candidate(TargetProfile::LinuxArm64);
    appended.contents.bytes.push(0);
    refresh_fingerprint(&mut appended);
    assert!(
        assert_rejected_with_exact_custody(appended)
            .to_string()
            .contains("file length drifted"),
    );

    let mut missing = candidate(TargetProfile::LinuxArm64);
    missing.contents.fragment_placements.pop();
    refresh_fingerprint(&mut missing);
    assert!(
        assert_rejected_with_exact_custody(missing)
            .to_string()
            .contains("ledger coverage drifted"),
    );

    let mut reordered = candidate(TargetProfile::LinuxArm64);
    reordered.contents.fragment_placements.swap(1, 2);
    refresh_fingerprint(&mut reordered);
    assert!(
        assert_rejected_with_exact_custody(reordered)
            .to_string()
            .contains("ledger drifted"),
    );

    let mut kind = candidate(TargetProfile::LinuxArm64);
    kind.contents.fragment_placements[0].kind = ElfDynamicFileFragmentKind::SourceData;
    refresh_fingerprint(&mut kind);
    assert_rejected_with_exact_custody(kind);

    let mut offset = candidate(TargetProfile::LinuxArm64);
    offset.contents.fragment_placements[1].file_offset += 1;
    refresh_fingerprint(&mut offset);
    assert_rejected_with_exact_custody(offset);

    let mut count = candidate(TargetProfile::LinuxArm64);
    count.contents.fragment_placements[1].byte_count -= 1;
    refresh_fingerprint(&mut count);
    assert_rejected_with_exact_custody(count);

    let mut ordinal = candidate(TargetProfile::LinuxArm64);
    ordinal.contents.fragment_placements[1].ordinal += 1;
    refresh_fingerprint(&mut ordinal);
    assert_rejected_with_exact_custody(ordinal);
}

#[test]
fn report_fingerprint_zero_or_drift_rejects_with_exact_custody() {
    let mut zero = candidate(TargetProfile::LinuxX64);
    zero.non_authoritative_assembled_file_compatibility_fingerprint = 0;
    assert!(
        assert_rejected_with_exact_custody(zero)
            .to_string()
            .contains("fingerprint does not replay"),
    );

    let mut drift = candidate(TargetProfile::LinuxX64);
    drift.non_authoritative_assembled_file_compatibility_fingerprint ^= 1;
    assert!(
        assert_rejected_with_exact_custody(drift)
            .to_string()
            .contains("fingerprint does not replay"),
    );
}

#[test]
fn both_linux_targets_consume_the_retained_image_into_exact_admitted_bytes() {
    for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
        let resolved = standard_resolved_linkage(target);
        let original = load_layout(&resolved).retained_image().clone();
        let resolved_text = resolved.source_text_bytes().to_vec();
        assert_ne!(original.memory.text, resolved_text);
        let assembled = assemble_elf_dynamic_file(resolved).unwrap();
        let assembled_bytes = assembled.bytes().to_vec();
        let assembled_fingerprint =
            assembled.non_authoritative_assembled_file_compatibility_fingerprint();

        let admitted = admit_elf_dynamic_executable(assembled).unwrap();
        assert_eq!(admitted.image().memory.text, resolved_text);
        assert_eq!(admitted.image().memory.data, original.memory.data);
        assert_eq!(admitted.image().memory.bss_size, original.memory.bss_size);
        assert_eq!(admitted.output().bytes, assembled_bytes);
        assert_eq!(admitted.output().final_text_bytes, resolved_text);
        assert_eq!(admitted.output().imports, 2);
        assert_eq!(admitted.output().relocations, 3);
        assert_eq!(
            admitted.non_authoritative_assembled_file_compatibility_fingerprint(),
            assembled_fingerprint,
        );
        assert!(admitted.output().bytes.starts_with(b"\x7fELF"));
        assert!(admitted.output().format.contains("dynamic-executable"));
    }
}

#[test]
fn final_byte_admission_rejects_independent_output_drift() {
    let assembled =
        assemble_elf_dynamic_file(standard_resolved_linkage(TargetProfile::LinuxX64)).unwrap();
    let mut image = load_layout(&assembled.resolved_linkage)
        .retained_image()
        .clone();
    image.memory.text = assembled.resolved_linkage.source_text_bytes().to_vec();
    let output = derive_executable_output(&assembled, &image).unwrap();
    validate_executable_output(&assembled, &image, &output).unwrap();

    let mut bytes = output.clone();
    bytes.bytes[0] ^= 1;
    assert!(validate_executable_output(&assembled, &image, &bytes).is_err());

    let mut text = output.clone();
    text.final_text_bytes[0] ^= 1;
    assert!(validate_executable_output(&assembled, &image, &text).is_err());

    let mut format = output.clone();
    format.format.push_str("-drift");
    assert!(validate_executable_output(&assembled, &image, &format).is_err());

    let mut statistics = output;
    statistics.imports += 1;
    assert!(validate_executable_output(&assembled, &image, &statistics).is_err());
}
