//! Dynamic load layout tests.

use super::{
    Candidate, DYNAMIC_MAX_PAGE_SIZE, ElfLoadProgramHeader, ElfLoadProgramHeaderKind,
    ElfPlacedDynamicSectionKind, ElfSectionPlacementResolutionKind, IMAGE_BASE, PF_R, PF_W, PF_X,
    ValidatedElfRelativeSectionPayloadLayout, derive_contents,
    non_authoritative_layout_compatibility_fingerprint, plan_elf_dynamic_load_layout,
    retained_target, validate_candidate,
};
use crate::dynamic_executable::load_placement::load_layout::abi_validation::{
    aarch64_page_delta_covers_extent, validate_abi, validate_deferred_constraint_envelope,
};
use crate::dynamic_executable::load_placement::load_layout::compatibility_fingerprint::{
    checked_align, checked_product, checked_sum,
};
use crate::dynamic_executable::procedure_linkage::dynamic_linkage_templates::ElfProcedureLinkagePlacementConstraintKind;
use crate::dynamic_executable::section_headers::relative_section_layout::ElfRelativeSectionPayloadRegion;
use crate::{
    plan_elf_dynamic_link_inputs, plan_elf_dynamic_section_descriptors,
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

fn relative(target: TargetProfile) -> ValidatedElfRelativeSectionPayloadLayout {
    relative_with_bss_alignment(target, 16)
}

fn relative_with_bss_alignment(
    target: TargetProfile,
    bss_alignment: usize,
) -> ValidatedElfRelativeSectionPayloadLayout {
    let mut image = FinalImage::with_capacity(
        target.native_target(),
        FinalImageMemory {
            text: vec![0; 32],
            data: vec![0x5a; 13],
            bss_size: 23,
            bss_alignment,
        },
        Handle::invalid(),
        1,
        1,
        1,
    );
    let symbol_handle = image.symbol_table.symbols.insert(FinalImageSymbol {
        name: "__omega_load_layout_import".to_owned(),
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
                    object: b"libload-layout.so".to_vec(),
                    symbol: b"load_layout_call".to_vec(),
                    version: b"LOAD_LAYOUT_1".to_vec(),
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
    let path = match target {
        TargetProfile::LinuxX64 => b"/lib64/ld-linux-x86-64.so.2".as_slice(),
        TargetProfile::LinuxArm64 => b"/lib/ld-linux-aarch64.so.1".as_slice(),
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
    let headers = serialize_elf_section_header_table(roster).unwrap();
    let payloads = plan_elf_indexed_section_payloads(headers).unwrap();
    plan_elf_relative_section_payload_layout(payloads).unwrap()
}

fn candidate(target: TargetProfile) -> Candidate {
    let relative = relative(target);
    let target = retained_target(&relative);
    let (program_headers, image_memory, sections, section_header_table_file_offset, resolutions) =
        derive_contents(&relative, target).unwrap();
    let non_authoritative_layout_compatibility_fingerprint =
        non_authoritative_layout_compatibility_fingerprint(
            &relative,
            target,
            IMAGE_BASE,
            DYNAMIC_MAX_PAGE_SIZE,
            &program_headers,
            &image_memory,
            &sections,
            section_header_table_file_offset,
            &resolutions,
        );
    Candidate {
        relative,
        target,
        image_base: IMAGE_BASE,
        max_page_alignment: DYNAMIC_MAX_PAGE_SIZE,
        program_headers,
        image_memory,
        sections,
        section_header_table_file_offset,
        section_header_resolutions: resolutions,
        non_authoritative_layout_compatibility_fingerprint,
    }
}

#[test]
fn both_targets_close_ordered_congruent_wx_loads_and_owned_memory() {
    for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
        let layout = plan_elf_dynamic_load_layout(relative(target)).unwrap();
        assert_eq!(layout.target(), target);
        assert_eq!(layout.image_base(), IMAGE_BASE);
        assert_eq!(layout.max_page_alignment(), 0x1_0000);
        assert_ne!(
            layout.non_authoritative_layout_compatibility_fingerprint(),
            0
        );
        assert_eq!(
            layout
                .program_headers()
                .iter()
                .map(ElfLoadProgramHeader::kind)
                .collect::<Vec<_>>(),
            vec![
                ElfLoadProgramHeaderKind::Interpreter,
                ElfLoadProgramHeaderKind::LoadReadOnly,
                ElfLoadProgramHeaderKind::LoadReadExecute,
                ElfLoadProgramHeaderKind::LoadReadWrite,
                ElfLoadProgramHeaderKind::Dynamic,
            ]
        );
        for header in &layout.program_headers()[1..=3] {
            assert_eq!(header.alignment(), layout.max_page_alignment());
            assert_eq!(
                header.file_offset() % header.alignment(),
                header.virtual_address() % header.alignment()
            );
            assert_eq!(header.physical_address(), header.virtual_address());
            assert_ne!(header.flags() & PF_R, 0);
            assert_ne!(header.flags() & (PF_W | PF_X), PF_W | PF_X);
        }
        assert_eq!(layout.program_headers()[1].flags(), PF_R);
        assert_eq!(layout.program_headers()[2].flags(), PF_R | PF_X);
        assert_eq!(layout.program_headers()[3].flags(), PF_R | PF_W);
        assert_eq!(layout.image_memory().text_size(), 32);
        assert_eq!(layout.image_memory().data_size(), 13);
        assert_eq!(layout.image_memory().bss_size(), 23);
        assert_eq!(layout.image_memory().bss_alignment(), 16);
        assert_eq!(
            layout.final_image_layout().text_address,
            layout.image_memory().text_virtual_address()
        );
        validate_abi(&candidate(target)).unwrap();
    }
}

#[test]
fn special_headers_alias_sections_and_file_only_metadata_stays_outside_loads() {
    let layout = plan_elf_dynamic_load_layout(relative(TargetProfile::LinuxX64)).unwrap();
    let interpreter = &layout.sections()[ElfPlacedDynamicSectionKind::Interpreter as usize];
    let dynamic = layout.dynamic_section();
    let shstrtab = &layout.sections()[ElfPlacedDynamicSectionKind::SectionNameTable as usize];
    for (header, placed) in [
        (&layout.program_headers()[0], interpreter),
        (&layout.program_headers()[4], dynamic),
    ] {
        assert_eq!(header.file_offset(), placed.file_offset());
        assert_eq!(header.virtual_address(), placed.virtual_address().unwrap());
        assert_eq!(header.file_size(), placed.byte_size());
        assert_eq!(header.memory_size(), placed.byte_size());
    }
    assert_eq!(
        shstrtab.region(),
        Some(ElfRelativeSectionPayloadRegion::FileOnly)
    );
    assert_eq!(shstrtab.virtual_address(), None);
    let rw = layout.program_headers()[3];
    assert!(shstrtab.file_offset() >= rw.file_offset() + rw.file_size());
    assert!(
        layout.section_header_table_file_offset() >= shstrtab.file_offset() + shstrtab.byte_size()
    );
    assert_eq!(layout.section_header_table_byte_size(), 14 * 64);
}

#[test]
fn exact_twenty_five_fixups_are_resolved_without_mutating_template_bytes() {
    let layout = plan_elf_dynamic_load_layout(relative(TargetProfile::LinuxArm64)).unwrap();
    let template = layout.relative().payloads().section_headers().contents();
    assert_eq!(layout.section_header_resolutions().len(), 25);
    assert_eq!(template.placement_fixups.len(), 25);
    for (fixup, resolution) in template
        .placement_fixups
        .iter()
        .zip(layout.section_header_resolutions())
    {
        assert_eq!(resolution.row_index(), fixup.row_index);
        assert_eq!(resolution.byte_offset(), fixup.byte_offset);
        assert_eq!(resolution.byte_width(), 8);
        assert_eq!(
            &template.bytes[fixup.byte_offset..fixup.byte_offset + 8],
            &[0; 8]
        );
        let placed = &layout.sections()[fixup.row_index as usize];
        let expected = match resolution.kind() {
            ElfSectionPlacementResolutionKind::VirtualAddress => placed.virtual_address().unwrap(),
            ElfSectionPlacementResolutionKind::FileOffset => placed.file_offset(),
        };
        assert_eq!(resolution.value(), expected);
        assert_eq!(resolution.section_kind(), placed.kind());
    }
}

#[test]
fn placement_is_deterministic_and_target_bound() {
    let first = plan_elf_dynamic_load_layout(relative(TargetProfile::LinuxX64)).unwrap();
    let second = plan_elf_dynamic_load_layout(relative(TargetProfile::LinuxX64)).unwrap();
    let arm = plan_elf_dynamic_load_layout(relative(TargetProfile::LinuxArm64)).unwrap();
    assert_eq!(first.program_headers(), second.program_headers());
    assert_eq!(first.sections(), second.sections());
    assert_eq!(
        first.non_authoritative_layout_compatibility_fingerprint(),
        second.non_authoritative_layout_compatibility_fingerprint()
    );
    assert_ne!(
        first.non_authoritative_layout_compatibility_fingerprint(),
        arm.non_authoritative_layout_compatibility_fingerprint()
    );
}

#[test]
fn geometry_resolution_policy_and_identity_drift_reject_with_custody() {
    let corruptions: Vec<Box<dyn Fn(&mut Candidate)>> = vec![
        Box::new(|candidate| candidate.program_headers.swap(0, 1)),
        Box::new(|candidate| candidate.program_headers[2].physical_address ^= 1),
        Box::new(|candidate| candidate.image_memory.data_file_offset ^= 1),
        Box::new(|candidate| candidate.sections[1].file_offset ^= 1),
        Box::new(|candidate| candidate.section_header_table_file_offset ^= 1),
        Box::new(|candidate| {
            candidate
                .section_header_resolutions
                .pop()
                .map(|_| ())
                .unwrap()
        }),
        Box::new(|candidate| candidate.section_header_resolutions[0].value ^= 1),
        Box::new(|candidate| candidate.max_page_alignment >>= 1),
        Box::new(|candidate| candidate.non_authoritative_layout_compatibility_fingerprint ^= 1),
    ];
    for corrupt in corruptions {
        let mut candidate = candidate(TargetProfile::LinuxX64);
        let relative_identity = candidate
            .relative
            .non_authoritative_layout_compatibility_fingerprint();
        corrupt(&mut candidate);
        let error = validate_candidate(candidate)
            .expect_err("absolute ELF load-layout corruption must reject");
        assert_eq!(
            error
                .candidate
                .relative
                .non_authoritative_layout_compatibility_fingerprint(),
            relative_identity
        );
    }
}

#[test]
fn independent_abi_replay_rejects_each_source_and_auxiliary_geometry_family() {
    let corruptions: Vec<Box<dyn Fn(&mut Candidate)>> = vec![
        Box::new(|candidate| candidate.image_memory.text_file_offset ^= 1),
        Box::new(|candidate| candidate.image_memory.text_virtual_address ^= 1),
        Box::new(|candidate| candidate.image_memory.data_virtual_address ^= 1),
        Box::new(|candidate| {
            candidate.image_memory.bss_virtual_address = candidate.image_memory.data_virtual_address
        }),
        Box::new(|candidate| candidate.image_memory.bss_alignment = 0),
        Box::new(|candidate| candidate.program_headers[2].memory_size = 0),
        Box::new(|candidate| {
            candidate.program_headers[3].virtual_address =
                candidate.program_headers[2].virtual_address
        }),
        Box::new(|candidate| candidate.sections[1].alignment = 3),
        Box::new(|candidate| candidate.program_headers[0].flags = PF_R | PF_W),
        Box::new(|candidate| candidate.program_headers[0].alignment = 2),
        Box::new(|candidate| candidate.program_headers[4].alignment = 1),
    ];
    for corrupt in corruptions {
        let mut candidate = candidate(TargetProfile::LinuxX64);
        corrupt(&mut candidate);
        validate_abi(&candidate).expect_err("independent ELF ABI replay must reject drift");
    }

    let mut source_section_overlap = candidate(TargetProfile::LinuxX64);
    let rw = source_section_overlap.program_headers[3];
    source_section_overlap.image_memory.data_file_offset = rw.file_offset;
    source_section_overlap.image_memory.data_virtual_address = rw.virtual_address;
    source_section_overlap.image_memory.bss_virtual_address = checked_align(
        rw.virtual_address + source_section_overlap.image_memory.data_size,
        source_section_overlap.image_memory.bss_alignment,
        "test BSS",
    )
    .unwrap();
    validate_abi(&source_section_overlap)
        .expect_err("source data overlapping dynamic storage must reject");
}

#[test]
fn aarch64_relocation_envelopes_use_exact_four_kibibyte_page_boundaries() {
    let source = 0x2_0000_0000_u64;
    let maximum_positive_page = source + (((1_u64 << 20) - 1) * 0x1000);
    assert!(aarch64_page_delta_covers_extent(source, maximum_positive_page, 0x1000).unwrap());
    assert!(
        !aarch64_page_delta_covers_extent(source, maximum_positive_page, 0x1001).unwrap(),
        "the last byte crossing into the first out-of-range page must reject"
    );
    let maximum_negative_page = source - ((1_u64 << 20) * 0x1000);
    assert!(aarch64_page_delta_covers_extent(source, maximum_negative_page, 1).unwrap());
    assert!(!aarch64_page_delta_covers_extent(source, maximum_negative_page - 0x1000, 1).unwrap());
    assert!(!aarch64_page_delta_covers_extent(source, source, 0).unwrap());

    let arm = candidate(TargetProfile::LinuxArm64);
    let constraint_kinds = arm
        .relative
        .payloads()
        .contents()
        .procedure_constraints
        .iter()
        .map(|constraint| constraint.kind)
        .collect::<Vec<_>>();
    assert!(
        constraint_kinds.contains(&ElfProcedureLinkagePlacementConstraintKind::Aarch64PageDelta21)
    );
    assert!(
        constraint_kinds
            .contains(&ElfProcedureLinkagePlacementConstraintKind::Aarch64Load64Low12Aligned)
    );
    assert!(
        constraint_kinds.contains(&ElfProcedureLinkagePlacementConstraintKind::Aarch64Branch26)
    );

    let mut branch_drift = candidate(TargetProfile::LinuxArm64);
    branch_drift.sections[ElfPlacedDynamicSectionKind::ProcedureLinkage as usize].virtual_address =
        Some(1_u64 << 40);
    validate_deferred_constraint_envelope(&branch_drift)
        .expect_err("out-of-range AArch64 branch target must reject");

    let mut low_twelve_drift = candidate(TargetProfile::LinuxArm64);
    low_twelve_drift.sections[ElfPlacedDynamicSectionKind::ProcedureGot as usize].virtual_address =
        low_twelve_drift.sections[ElfPlacedDynamicSectionKind::ProcedureGot as usize]
            .virtual_address
            .map(|address| address + 1);
    validate_deferred_constraint_envelope(&low_twelve_drift)
        .expect_err("misaligned AArch64 low-12 target must reject");
}

#[test]
fn invalid_bss_alignment_rejects_with_exact_relative_layout_custody() {
    for alignment in [0, 3] {
        let relative = relative_with_bss_alignment(TargetProfile::LinuxX64, alignment);
        let identity = relative.non_authoritative_layout_compatibility_fingerprint();
        let error = plan_elf_dynamic_load_layout(relative)
            .expect_err("invalid retained BSS alignment must reject");
        let (relative, _) = error.into_parts();
        assert_eq!(
            relative.non_authoritative_layout_compatibility_fingerprint(),
            identity
        );
    }
}

#[test]
fn checked_arithmetic_rejects_overflow_and_invalid_alignment() {
    assert!(checked_sum(u64::MAX, 1, "sum").is_err());
    assert!(checked_product(u64::MAX, 2, "product").is_err());
    assert!(checked_align(u64::MAX, 8, "alignment").is_err());
    assert!(checked_align(7, 3, "alignment").is_err());
}
