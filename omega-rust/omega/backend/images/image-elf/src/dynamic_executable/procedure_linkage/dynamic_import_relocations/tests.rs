//! Dynamic import relocation tests.

use super::{
    Candidate, ElfDirectImportCallSite, FinalImageSection, R_AARCH64_ABS64, R_AARCH64_JUMP_SLOT,
    R_X86_64_64, R_X86_64_JUMP_SLOT, RelocationKind, TargetProfile,
    ValidatedElfDynamicSectionDescriptorPlan, call_site, checked_u32, derive_contents,
    non_authoritative_linkage_compatibility_fingerprint, plan_elf_procedure_linkage_relocations,
    site_end, validate_candidate, validate_contents, validate_site_spans,
};
use crate::{
    plan_elf_dynamic_link_inputs, plan_elf_dynamic_section_descriptors, plan_elf_dynamic_sections,
    serialize_elf_dynamic_sections,
};
use arena::Handle;
use image::{
    FinalImage, FinalImageImport, FinalImageImportPlan, FinalImageMemory, FinalImageRelocation,
    FinalImageSymbol,
};
use object_file::SymbolKind;
use target::{ForeignLocatorCandidate, normalize_elf_interpreter_plan, normalize_foreign_locator};

#[derive(Clone, Copy)]
struct ImportFixture {
    object: &'static [u8],
    symbol: &'static [u8],
    version: &'static [u8],
    instruction_offsets: &'static [usize],
}

const FIRST_SITES: &[usize] = &[0, 44];
const SECOND_SITES: &[usize] = &[16];
const THIRD_SITES: &[usize] = &[28];
const IMPORTS: [ImportFixture; 3] = [
    ImportFixture {
        object: b"liba\xff.so",
        symbol: b"alpha\xfe",
        version: b"V1\xfd",
        instruction_offsets: FIRST_SITES,
    },
    ImportFixture {
        object: b"liba\xff.so",
        symbol: b"beta",
        version: b"V2",
        instruction_offsets: SECOND_SITES,
    },
    ImportFixture {
        object: b"libb.so",
        symbol: b"gamma",
        version: b"V1\xfd",
        instruction_offsets: THIRD_SITES,
    },
];

fn interpreter_path(target: TargetProfile) -> &'static [u8] {
    match target {
        TargetProfile::LinuxX64 => b"/lib64/ld-linux-\xfc-x86-64.so.2",
        TargetProfile::LinuxArm64 => b"/lib/ld-linux-\xfb-aarch64.so.1",
        _ => unreachable!("procedure-linkage fixture uses a Linux target"),
    }
}

fn image(
    target: TargetProfile,
    imports: &[ImportFixture],
) -> (FinalImage, Vec<Handle<FinalImageRelocation>>) {
    let native_target = target.native_target();
    let relocation_count = imports
        .iter()
        .map(|fixture| fixture.instruction_offsets.len())
        .sum();
    let mut image = FinalImage::with_capacity(
        native_target,
        FinalImageMemory {
            text: vec![0; 64],
            ..FinalImageMemory::default()
        },
        Handle::invalid(),
        imports.len(),
        imports.len(),
        relocation_count,
    );
    let mut relocation_handles = Vec::with_capacity(relocation_count);
    for (index, fixture) in imports.iter().enumerate() {
        let symbol_handle = image.symbol_table.symbols.insert(FinalImageSymbol {
            name: format!("__omega_procedure_import_{index}"),
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
                .expect("valid procedure-linkage locator"),
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
                _ => unreachable!("fixture uses a Linux target"),
            };
            relocation_handles.push(image.relocation_table.relocations.insert(
                FinalImageRelocation {
                    section: FinalImageSection::Text,
                    offset: relocation_offset,
                    byte_width: 4,
                    symbol_handle,
                    addend: 0,
                    kind,
                },
            ));
        }
    }
    (image, relocation_handles)
}

fn descriptors_from_image(
    target: TargetProfile,
    image: FinalImage,
) -> ValidatedElfDynamicSectionDescriptorPlan {
    let interpreter = normalize_elf_interpreter_plan(interpreter_path(target).to_vec(), target)
        .expect("valid procedure-linkage interpreter");
    let inputs =
        plan_elf_dynamic_link_inputs(image, interpreter).expect("valid dynamic-link preflight");
    let sections = plan_elf_dynamic_sections(inputs).expect("valid dynamic sections");
    let payloads = serialize_elf_dynamic_sections(sections).expect("valid dynamic payloads");
    plan_elf_dynamic_section_descriptors(payloads).expect("valid dynamic descriptors")
}

fn descriptors(
    target: TargetProfile,
    imports: &[ImportFixture],
) -> ValidatedElfDynamicSectionDescriptorPlan {
    descriptors_from_image(target, image(target, imports).0)
}

fn candidate(target: TargetProfile) -> Candidate {
    let descriptors = descriptors(target, &IMPORTS);
    let contents = derive_contents(&descriptors).expect("derived procedure linkage");
    let non_authoritative_linkage_compatibility_fingerprint =
        non_authoritative_linkage_compatibility_fingerprint(&descriptors, &contents);
    Candidate {
        descriptors,
        contents,
        non_authoritative_linkage_compatibility_fingerprint,
    }
}

#[test]
fn both_linux_targets_plan_exact_slots_jump_relocations_and_call_sites() {
    for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
        let plan = plan_elf_procedure_linkage_relocations(descriptors(target, &IMPORTS))
            .expect("validated procedure-linkage plan");
        assert_eq!(plan.logical_slot_count(), 3);
        assert_eq!(plan.procedure_relocation_count(), 3);
        assert_eq!(plan.direct_call_site_count(), 4);
        assert_eq!(plan.general_dynamic_relocation_count(), 0);
        assert_ne!(
            plan.non_authoritative_linkage_compatibility_fingerprint(),
            0
        );

        let expected_source_kind = match target {
            TargetProfile::LinuxX64 => RelocationKind::X86_64Relative32,
            TargetProfile::LinuxArm64 => RelocationKind::Aarch64Branch26,
            _ => unreachable!(),
        };
        let expected_dynamic_type = match target {
            TargetProfile::LinuxX64 => R_X86_64_JUMP_SLOT,
            TargetProfile::LinuxArm64 => R_AARCH64_JUMP_SLOT,
            _ => unreachable!(),
        };
        assert_eq!(
            plan.contents
                .slots
                .iter()
                .map(|slot| slot.logical_ordinal)
                .collect::<Vec<_>>(),
            [0, 1, 2],
        );
        assert_eq!(
            plan.contents
                .slots
                .iter()
                .map(|slot| slot.dynamic_symbol_index)
                .collect::<Vec<_>>(),
            [1, 2, 3],
        );
        assert!(plan.contents.slots.iter().all(|slot| {
            !slot.call_sites.is_empty()
                && slot.call_sites.iter().all(|site| {
                    site.kind == expected_source_kind && site.byte_width == 4 && site.addend == 0
                })
        }));
        assert!(plan.contents.jump_slot_relocations.iter().enumerate().all(
            |(index, relocation)| {
                relocation.logical_got_slot_ordinal == index as u32
                    && relocation.dynamic_symbol_index == index as u32 + 1
                    && relocation.relocation_type == expected_dynamic_type
                    && relocation.addend == 0
            }
        ));
        validate_contents(plan.descriptors(), &plan.contents)
            .expect("independent procedure-linkage replay");
    }
}

#[test]
fn data_slot_import_relocations_plan_exact_general_rela_rows() {
    for (target, relocation_type) in [
        (TargetProfile::LinuxX64, R_X86_64_64),
        (TargetProfile::LinuxArm64, R_AARCH64_ABS64),
    ] {
        let mut image = FinalImage::with_capacity(
            target.native_target(),
            FinalImageMemory {
                text: vec![0; 8],
                data: vec![0; 8],
                ..FinalImageMemory::default()
            },
            Handle::invalid(),
            1,
            1,
            1,
        );
        let symbol_handle = image.symbol_table.symbols.insert(FinalImageSymbol {
            name: "__omega_data_import_0".to_owned(),
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
                        object: b"libslots.so".to_vec(),
                        symbol: b"slot_cell".to_vec(),
                        version: b"V1".to_vec(),
                    },
                    target,
                )
                .expect("valid data-slot locator"),
            ),
        });
        image
            .relocation_table
            .relocations
            .insert(FinalImageRelocation {
                section: FinalImageSection::Data,
                offset: 0,
                byte_width: 8,
                symbol_handle,
                addend: 7,
                kind: RelocationKind::Absolute64,
            });

        let plan = plan_elf_procedure_linkage_relocations(descriptors_from_image(target, image))
            .expect("validated procedure-linkage plan with a general row");
        assert_eq!(plan.logical_slot_count(), 1);
        assert_eq!(plan.procedure_relocation_count(), 1);
        assert_eq!(plan.direct_call_site_count(), 0);
        assert_eq!(plan.general_dynamic_relocation_count(), 1);
        let row = plan.contents.general_relocations[0];
        assert_eq!(row.request_index, 0);
        assert_eq!(row.dynamic_symbol_index, 1);
        assert_eq!(row.relocation_type, relocation_type);
        assert_eq!(row.source_section, FinalImageSection::Data);
        assert_eq!(row.source_offset, 0);
        assert_eq!(row.addend, 7);
        validate_contents(plan.descriptors(), &plan.contents)
            .expect("independent procedure-linkage replay");
    }
}

#[test]
fn import_permutation_preserves_identity_and_multiple_calls_share_one_slot() {
    let forward =
        plan_elf_procedure_linkage_relocations(descriptors(TargetProfile::LinuxX64, &IMPORTS))
            .expect("forward procedure linkage");
    let reversed = IMPORTS.iter().rev().copied().collect::<Vec<_>>();
    let reverse =
        plan_elf_procedure_linkage_relocations(descriptors(TargetProfile::LinuxX64, &reversed))
            .expect("reverse procedure linkage");
    let arm =
        plan_elf_procedure_linkage_relocations(descriptors(TargetProfile::LinuxArm64, &IMPORTS))
            .expect("AArch64 procedure linkage");

    assert_eq!(
        forward.non_authoritative_linkage_compatibility_fingerprint(),
        reverse.non_authoritative_linkage_compatibility_fingerprint()
    );
    assert_ne!(
        forward.non_authoritative_linkage_compatibility_fingerprint(),
        arm.non_authoritative_linkage_compatibility_fingerprint()
    );
    assert_eq!(forward.logical_slot_count(), 3);
    assert_eq!(forward.direct_call_site_count(), 4);
    assert_eq!(
        forward
            .contents
            .slots
            .iter()
            .map(|slot| slot.call_sites.len())
            .collect::<Vec<_>>(),
        [2, 1, 1],
    );
}

#[test]
fn nonprocedure_source_relocations_and_tampered_placeholders_reject_with_custody() {
    type Mutation = Box<dyn Fn(&mut FinalImage, &[Handle<FinalImageRelocation>])>;
    let mutations: Vec<Mutation> = vec![
        Box::new(|image, handles| {
            image
                .relocation_table
                .relocations
                .get_mut(handles[0])
                .section = FinalImageSection::Data;
        }),
        Box::new(|image, handles| {
            image.relocation_table.relocations.get_mut(handles[0]).kind =
                RelocationKind::Absolute64;
        }),
        Box::new(|image, handles| {
            image
                .relocation_table
                .relocations
                .get_mut(handles[0])
                .byte_width = 8;
        }),
        Box::new(|image, handles| {
            image
                .relocation_table
                .relocations
                .get_mut(handles[0])
                .addend = 1;
        }),
        Box::new(|image, handles| {
            image
                .relocation_table
                .relocations
                .get_mut(handles[0])
                .offset = usize::MAX;
        }),
        Box::new(|image, _| image.memory.text[0] = 0x90),
        Box::new(|image, handles| {
            let first = image.relocation_table.relocations.get(handles[0]).offset;
            image
                .relocation_table
                .relocations
                .get_mut(handles[2])
                .offset = first;
        }),
    ];

    for mutate in mutations {
        let (mut image, handles) = image(TargetProfile::LinuxX64, &IMPORTS);
        mutate(&mut image, &handles);
        let descriptors = descriptors_from_image(TargetProfile::LinuxX64, image);
        let expected_identity =
            descriptors.non_authoritative_descriptor_compatibility_fingerprint();
        let error = plan_elf_procedure_linkage_relocations(descriptors)
            .expect_err("invalid imported call must reject before linkage sealing");
        let (returned, _) = error.into_parts();
        assert_eq!(
            returned.non_authoritative_descriptor_compatibility_fingerprint(),
            expected_identity
        );
    }

    let (mut arm_image, arm_handles) = image(TargetProfile::LinuxArm64, &IMPORTS);
    arm_image
        .relocation_table
        .relocations
        .get_mut(arm_handles[0])
        .offset = 1;
    let descriptors = descriptors_from_image(TargetProfile::LinuxArm64, arm_image);
    let expected_identity = descriptors.non_authoritative_descriptor_compatibility_fingerprint();
    let error = plan_elf_procedure_linkage_relocations(descriptors)
        .expect_err("misaligned AArch64 BL relocation must reject");
    assert_eq!(
        error
            .into_parts()
            .0
            .non_authoritative_descriptor_compatibility_fingerprint(),
        expected_identity
    );
}

#[test]
fn independent_validation_rejects_every_slot_relocation_site_and_identity_corruption() {
    let corruptions: Vec<Box<dyn Fn(&mut Candidate)>> = vec![
        Box::new(|candidate| {
            candidate.contents.slots.pop();
        }),
        Box::new(|candidate| {
            candidate.contents.jump_slot_relocations.pop();
        }),
        Box::new(|candidate| candidate.contents.slots.swap(0, 1)),
        Box::new(|candidate| candidate.contents.slots[0].logical_ordinal += 1),
        Box::new(|candidate| candidate.contents.slots[0].request_index = usize::MAX),
        Box::new(|candidate| {
            candidate.contents.slots[0].compatibility_report_identity ^= 1;
        }),
        Box::new(|candidate| candidate.contents.slots[0].dynamic_symbol_index += 1),
        Box::new(|candidate| candidate.contents.slots[0].version_index += 1),
        Box::new(|candidate| {
            candidate.contents.slots[0].call_sites.pop();
        }),
        Box::new(|candidate| candidate.contents.slots[0].call_sites[0].instruction_offset += 1),
        Box::new(|candidate| candidate.contents.slots[0].call_sites[0].relocation_offset += 1),
        Box::new(|candidate| candidate.contents.slots[0].call_sites[0].byte_width += 1),
        Box::new(|candidate| {
            candidate.contents.slots[0].call_sites[0].kind = RelocationKind::Absolute64
        }),
        Box::new(|candidate| candidate.contents.slots[0].call_sites[0].addend += 1),
        Box::new(|candidate| {
            candidate.contents.jump_slot_relocations[0].logical_got_slot_ordinal += 1
        }),
        Box::new(|candidate| candidate.contents.jump_slot_relocations[0].dynamic_symbol_index += 1),
        Box::new(|candidate| candidate.contents.jump_slot_relocations[0].relocation_type += 1),
        Box::new(|candidate| candidate.contents.jump_slot_relocations[0].addend += 1),
        Box::new(|candidate| candidate.non_authoritative_linkage_compatibility_fingerprint ^= 1),
    ];

    for corrupt in corruptions {
        let mut candidate = candidate(TargetProfile::LinuxX64);
        let expected_identity = candidate
            .descriptors
            .non_authoritative_descriptor_compatibility_fingerprint();
        corrupt(&mut candidate);
        let error = validate_candidate(candidate)
            .expect_err("corrupt procedure-linkage candidate must reject");
        assert_eq!(
            error
                .candidate
                .descriptors
                .non_authoritative_descriptor_compatibility_fingerprint(),
            expected_identity,
            "validation failure retains exact descriptor custody",
        );
    }
}

#[test]
fn malformed_offsets_ordinals_and_spans_reject_without_panicking() {
    assert!(checked_u32(usize::MAX, "ordinal").is_err());
    let overflow_site = ElfDirectImportCallSite {
        instruction_offset: usize::MAX,
        relocation_offset: usize::MAX,
        byte_width: 4,
        kind: RelocationKind::X86_64Relative32,
        addend: 0,
    };
    assert!(site_end(&overflow_site).is_err());
    assert!(validate_site_spans(&[overflow_site, overflow_site]).is_err());

    let relocation = FinalImageRelocation {
        section: FinalImageSection::Text,
        offset: usize::MAX,
        byte_width: 4,
        symbol_handle: Handle::invalid(),
        addend: 0,
        kind: RelocationKind::X86_64Relative32,
    };
    assert!(
        call_site(
            TargetProfile::LinuxX64,
            &[0xe8, 0, 0, 0, 0],
            RelocationKind::X86_64Relative32,
            &relocation,
        )
        .is_err()
    );
}
