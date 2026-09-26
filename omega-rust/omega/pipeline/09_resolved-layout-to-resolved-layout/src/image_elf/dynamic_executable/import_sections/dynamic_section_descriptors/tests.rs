//! Dynamic section descriptor tests.

use super::{
    Candidate, ElfAddressFreeSectionDescriptor, ElfDynamicSectionDescriptorContents,
    ElfDynamicSectionKind, SECTION_NAME_TABLE_NAME_OFFSET, SECTION_NAME_TABLE_SEED, SHF_ALLOC,
    SHT_DYNSYM, SHT_GNU_HASH, SHT_GNU_VERNEED, SHT_GNU_VERSYM, SHT_HASH, SHT_PROGBITS, SHT_STRTAB,
    ValidatedElfDynamicSectionPayloads, derive_contents, name_at,
    non_authoritative_descriptor_compatibility_fingerprint, plan_elf_dynamic_section_descriptors,
    read_u32, validate_candidate, validate_contents,
};
use crate::image::{
    FinalImage, FinalImageImport, FinalImageImportPlan, FinalImageMemory, FinalImageRelocation,
    FinalImageSection, FinalImageSymbol,
};
use crate::image_elf::{
    plan_elf_dynamic_link_inputs, plan_elf_dynamic_sections, serialize_elf_dynamic_sections,
};
use crate::object_file::{RelocationKind, SymbolKind};
use arena::Handle;
use target::{
    ForeignLocatorCandidate, NativeTarget, TargetProfile, normalize_elf_interpreter_plan,
    normalize_foreign_locator,
};

#[derive(Clone, Copy)]
struct ImportFixture {
    object: &'static [u8],
    symbol: &'static [u8],
    version: &'static [u8],
}

const IMPORTS: [ImportFixture; 3] = [
    ImportFixture {
        object: b"liba\xff.so",
        symbol: b"alpha\xfe",
        version: b"V1\xfd",
    },
    ImportFixture {
        object: b"liba\xff.so",
        symbol: b"beta",
        version: b"V2",
    },
    ImportFixture {
        object: b"libb.so",
        symbol: b"gamma",
        version: b"V1\xfd",
    },
];

fn interpreter_path(target: TargetProfile) -> &'static [u8] {
    match target {
        TargetProfile::LinuxX64 => b"/lib64/ld-linux-\xfc-x86-64.so.2",
        TargetProfile::LinuxArm64 => b"/lib/ld-linux-\xfb-aarch64.so.1",
        _ => unreachable!("descriptor fixture uses a Linux target"),
    }
}

fn payloads(
    target: TargetProfile,
    imports: &[ImportFixture],
) -> ValidatedElfDynamicSectionPayloads {
    let native_target = target.native_target();
    let mut image = FinalImage::with_capacity(
        native_target,
        FinalImageMemory {
            text: vec![0; 32],
            ..FinalImageMemory::default()
        },
        Handle::invalid(),
        imports.len(),
        imports.len(),
        imports.len(),
    );
    for (index, fixture) in imports.iter().enumerate() {
        let symbol_handle = image.symbol_table.symbols.insert(FinalImageSymbol {
            name: format!("__omega_descriptor_import_{index}"),
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
                .expect("valid descriptor fixture locator"),
            ),
        });
        image
            .relocation_table
            .relocations
            .insert(FinalImageRelocation {
                section: FinalImageSection::Text,
                offset: index * 8,
                byte_width: 4,
                symbol_handle,
                addend: 0,
                kind: if native_target == NativeTarget::linux_arm64() {
                    RelocationKind::Aarch64Branch26
                } else {
                    RelocationKind::X86_64Relative32
                },
            });
    }
    let interpreter = normalize_elf_interpreter_plan(interpreter_path(target).to_vec(), target)
        .expect("valid descriptor fixture interpreter");
    let inputs =
        plan_elf_dynamic_link_inputs(image, interpreter).expect("valid dynamic-link preflight");
    let sections = plan_elf_dynamic_sections(inputs).expect("valid dynamic sections");
    serialize_elf_dynamic_sections(sections).expect("valid serialized payloads")
}

fn candidate(target: TargetProfile) -> Candidate {
    let payloads = payloads(target, &IMPORTS);
    let contents = derive_contents(&payloads).expect("derived descriptors");
    let non_authoritative_descriptor_compatibility_fingerprint =
        non_authoritative_descriptor_compatibility_fingerprint(&payloads, &contents);
    Candidate {
        payloads,
        contents,
        non_authoritative_descriptor_compatibility_fingerprint,
    }
}

fn row(
    contents: &ElfDynamicSectionDescriptorContents,
    kind: ElfDynamicSectionKind,
) -> &ElfAddressFreeSectionDescriptor {
    contents
        .descriptors
        .iter()
        .find(|descriptor| descriptor.kind == kind)
        .expect("one exact descriptor kind")
}

#[test]
fn both_linux_targets_plan_exact_address_free_descriptor_metadata() {
    for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
        let plan = plan_elf_dynamic_section_descriptors(payloads(target, &IMPORTS))
            .expect("validated descriptor plan");
        let contents = &plan.contents;
        assert_eq!(contents.section_name_table_seed, SECTION_NAME_TABLE_SEED);
        assert_eq!(plan.descriptor_count(), 7);
        assert_eq!(plan.section_name_seed_byte_count(), 79);
        assert_eq!(
            contents
                .descriptors
                .iter()
                .map(|descriptor| descriptor.name_offset)
                .collect::<Vec<_>>(),
            [1, 9, 17, 25, 31, 44, 69],
        );
        assert_eq!(
            contents
                .descriptors
                .iter()
                .map(|descriptor| descriptor.payload_size)
                .collect::<Vec<_>>(),
            [
                plan.payloads().interpreter_byte_count() as u64,
                43,
                96,
                36,
                8,
                80,
                40,
            ],
        );

        assert_eq!(
            *row(contents, ElfDynamicSectionKind::Interpreter),
            ElfAddressFreeSectionDescriptor {
                kind: ElfDynamicSectionKind::Interpreter,
                name_offset: 1,
                section_type: SHT_PROGBITS,
                flags: SHF_ALLOC,
                payload_size: plan.payloads().interpreter_byte_count() as u64,
                alignment: 1,
                entry_size: 0,
                link: None,
                info: 0,
            }
        );
        assert_eq!(
            *row(contents, ElfDynamicSectionKind::DynamicSymbol),
            ElfAddressFreeSectionDescriptor {
                kind: ElfDynamicSectionKind::DynamicSymbol,
                name_offset: 17,
                section_type: SHT_DYNSYM,
                flags: SHF_ALLOC,
                payload_size: 96,
                alignment: 8,
                entry_size: 24,
                link: Some(ElfDynamicSectionKind::DynamicString),
                info: 1,
            }
        );
        assert_eq!(
            *row(contents, ElfDynamicSectionKind::SystemVHash),
            ElfAddressFreeSectionDescriptor {
                kind: ElfDynamicSectionKind::SystemVHash,
                name_offset: 25,
                section_type: SHT_HASH,
                flags: SHF_ALLOC,
                payload_size: 36,
                alignment: 4,
                entry_size: 4,
                link: Some(ElfDynamicSectionKind::DynamicSymbol),
                info: 0,
            }
        );
        assert_eq!(
            *row(contents, ElfDynamicSectionKind::GnuSymbolVersion),
            ElfAddressFreeSectionDescriptor {
                kind: ElfDynamicSectionKind::GnuSymbolVersion,
                name_offset: 31,
                section_type: SHT_GNU_VERSYM,
                flags: SHF_ALLOC,
                payload_size: 8,
                alignment: 2,
                entry_size: 2,
                link: Some(ElfDynamicSectionKind::DynamicSymbol),
                info: 0,
            }
        );
        assert_eq!(
            *row(contents, ElfDynamicSectionKind::GnuVersionRequirement),
            ElfAddressFreeSectionDescriptor {
                kind: ElfDynamicSectionKind::GnuVersionRequirement,
                name_offset: 44,
                section_type: SHT_GNU_VERNEED,
                flags: SHF_ALLOC,
                payload_size: 80,
                alignment: 4,
                entry_size: 0,
                link: Some(ElfDynamicSectionKind::DynamicString),
                info: 2,
            }
        );
        assert_eq!(
            *row(contents, ElfDynamicSectionKind::GnuHash),
            ElfAddressFreeSectionDescriptor {
                kind: ElfDynamicSectionKind::GnuHash,
                name_offset: 69,
                section_type: SHT_GNU_HASH,
                flags: SHF_ALLOC,
                payload_size: 40,
                alignment: 8,
                entry_size: 0,
                link: Some(ElfDynamicSectionKind::DynamicSymbol),
                info: 0,
            }
        );
        assert_ne!(
            plan.non_authoritative_descriptor_compatibility_fingerprint(),
            0
        );
        validate_contents(plan.payloads(), contents).expect("independent descriptor replay");
    }
}

#[test]
fn semantic_links_and_append_only_name_seed_preserve_exact_names() {
    let plan = plan_elf_dynamic_section_descriptors(payloads(TargetProfile::LinuxX64, &IMPORTS))
        .expect("validated descriptor plan");
    let mut extended = plan.contents.section_name_table_seed.clone();
    let expected = plan
        .contents
        .descriptors
        .iter()
        .map(|descriptor| {
            (
                descriptor.kind,
                name_at(&extended, descriptor.name_offset).unwrap().to_vec(),
            )
        })
        .collect::<Vec<_>>();
    extended.extend(b".dynamic\0.rela.dyn\0");

    for (kind, name) in expected {
        let descriptor = row(&plan.contents, kind);
        assert_eq!(name_at(&extended, descriptor.name_offset).unwrap(), name);
    }
    assert_eq!(
        name_at(&extended, SECTION_NAME_TABLE_NAME_OFFSET).unwrap(),
        b".shstrtab"
    );
    assert_eq!(
        row(&plan.contents, ElfDynamicSectionKind::DynamicSymbol).link,
        Some(ElfDynamicSectionKind::DynamicString)
    );
    assert_eq!(
        row(&plan.contents, ElfDynamicSectionKind::SystemVHash).link,
        Some(ElfDynamicSectionKind::DynamicSymbol)
    );
    assert_eq!(
        row(&plan.contents, ElfDynamicSectionKind::GnuHash).link,
        Some(ElfDynamicSectionKind::DynamicSymbol)
    );
}

#[test]
fn descriptor_identity_ignores_import_insertion_order_and_binds_profile() {
    let forward = plan_elf_dynamic_section_descriptors(payloads(TargetProfile::LinuxX64, &IMPORTS))
        .expect("forward descriptor plan");
    let reversed = IMPORTS.iter().rev().copied().collect::<Vec<_>>();
    let reverse =
        plan_elf_dynamic_section_descriptors(payloads(TargetProfile::LinuxX64, &reversed))
            .expect("reverse descriptor plan");
    let arm = plan_elf_dynamic_section_descriptors(payloads(TargetProfile::LinuxArm64, &IMPORTS))
        .expect("arm descriptor plan");

    assert_eq!(forward.contents, reverse.contents);
    assert_eq!(
        forward.non_authoritative_descriptor_compatibility_fingerprint(),
        reverse.non_authoritative_descriptor_compatibility_fingerprint()
    );
    assert_ne!(
        forward.non_authoritative_descriptor_compatibility_fingerprint(),
        arm.non_authoritative_descriptor_compatibility_fingerprint()
    );
}

#[test]
fn independent_validation_rejects_every_name_metadata_link_and_identity_corruption() {
    let corruptions: Vec<Box<dyn Fn(&mut Candidate)>> = vec![
        Box::new(|candidate| candidate.contents.section_name_table_seed[0] = 1),
        Box::new(|candidate| {
            candidate.contents.section_name_table_seed.pop();
        }),
        Box::new(|candidate| candidate.contents.section_name_table_seed.push(0)),
        Box::new(|candidate| {
            candidate.contents.descriptors.pop();
        }),
        Box::new(|candidate| candidate.contents.descriptors.swap(0, 1)),
        Box::new(|candidate| {
            candidate.contents.descriptors[1].kind = ElfDynamicSectionKind::Interpreter
        }),
        Box::new(|candidate| candidate.contents.descriptors[0].name_offset = u32::MAX),
        Box::new(|candidate| candidate.contents.descriptors[0].section_type = SHT_STRTAB),
        Box::new(|candidate| candidate.contents.descriptors[0].flags = 0),
        Box::new(|candidate| candidate.contents.descriptors[0].payload_size += 1),
        Box::new(|candidate| candidate.contents.descriptors[0].alignment = 3),
        Box::new(|candidate| candidate.contents.descriptors[2].entry_size = 23),
        Box::new(|candidate| {
            candidate.contents.descriptors[2].link = Some(ElfDynamicSectionKind::DynamicSymbol)
        }),
        Box::new(|candidate| candidate.contents.descriptors[2].info = 0),
        Box::new(|candidate| candidate.contents.descriptors[5].info += 1),
        Box::new(|candidate| candidate.contents.descriptors[6].entry_size = 4),
        Box::new(|candidate| candidate.non_authoritative_descriptor_compatibility_fingerprint ^= 1),
    ];

    for corrupt in corruptions {
        let mut candidate = candidate(TargetProfile::LinuxX64);
        corrupt(&mut candidate);
        let error = validate_candidate(candidate)
            .expect_err("corrupt descriptor candidate must reject before sealing");
        assert_eq!(
            error
                .candidate
                .payloads
                .plan()
                .inputs()
                .interpreter()
                .target(),
            TargetProfile::LinuxX64,
            "descriptor rejection retains exact payload custody",
        );
    }
}

#[test]
fn malformed_name_and_hash_reads_reject_without_panicking() {
    assert!(name_at(&[], 0).is_err());
    assert!(name_at(b"unterminated", 0).is_err());
    assert!(name_at(SECTION_NAME_TABLE_SEED, u32::MAX).is_err());
    assert!(read_u32(&[], 4, "hash").is_err());
    assert!(read_u32(&[0; 7], 4, "hash").is_err());
    assert!(read_u32(&[0; 8], usize::MAX, "hash").is_err());
}
