//! Dynamic linkage descriptor tests.

use super::{
    Candidate, ElfProcedureLinkageSectionDescriptor, ElfProcedureLinkageSectionDescriptorContents,
    ElfProcedureLinkageSectionInfo, ElfProcedureLinkageSectionKind, ElfProcedureLinkageSectionLink,
    PROCEDURE_LINKAGE_NAME_SUFFIX, SHF_AARCH64_PURECODE, SHF_ALLOC, SHF_EXECINSTR, SHF_INFO_LINK,
    SHF_WRITE, SHT_PROGBITS, SHT_RELA, TargetProfile, ValidatedElfProcedureLinkageTemplatePlan,
    checked_sum, checked_u32, derive_contents,
    non_authoritative_descriptor_compatibility_fingerprint,
    plan_elf_procedure_linkage_section_descriptors, target, validate_candidate, validate_contents,
    validate_name,
};
use crate::{
    plan_elf_dynamic_link_inputs, plan_elf_dynamic_section_descriptors, plan_elf_dynamic_sections,
    plan_elf_procedure_linkage_relocations, plan_elf_procedure_linkage_templates,
    serialize_elf_dynamic_sections,
};
use arena::Handle;
use image::{
    FinalImage, FinalImageImport, FinalImageImportPlan, FinalImageMemory, FinalImageRelocation,
    FinalImageSection, FinalImageSymbol,
};
use object_file::{RelocationKind, SymbolKind};
use target::{ForeignLocatorCandidate, normalize_elf_interpreter_plan, normalize_foreign_locator};

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
        _ => unreachable!("descriptor fixture uses a Linux target"),
    }
}

fn templates(
    target: TargetProfile,
    imports: &[ImportFixture],
) -> ValidatedElfProcedureLinkageTemplatePlan {
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
            name: format!("__omega_linkage_descriptor_import_{index}"),
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
                .expect("valid descriptor locator"),
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
        .expect("valid descriptor interpreter");
    let inputs =
        plan_elf_dynamic_link_inputs(image, interpreter).expect("valid dynamic-link inputs");
    let sections = plan_elf_dynamic_sections(inputs).expect("valid dynamic sections");
    let payloads = serialize_elf_dynamic_sections(sections).expect("valid dynamic payloads");
    let descriptors =
        plan_elf_dynamic_section_descriptors(payloads).expect("valid base descriptors");
    let linkage = plan_elf_procedure_linkage_relocations(descriptors)
        .expect("valid semantic procedure linkage");
    plan_elf_procedure_linkage_templates(linkage).expect("valid target templates")
}

fn candidate(target: TargetProfile) -> Candidate {
    let templates = templates(target, &IMPORTS);
    let contents = derive_contents(&templates).expect("derived linkage descriptors");
    let non_authoritative_descriptor_compatibility_fingerprint =
        non_authoritative_descriptor_compatibility_fingerprint(&templates, &contents);
    Candidate {
        templates,
        contents,
        non_authoritative_descriptor_compatibility_fingerprint,
    }
}

fn row(
    contents: &ElfProcedureLinkageSectionDescriptorContents,
    kind: ElfProcedureLinkageSectionKind,
) -> &ElfProcedureLinkageSectionDescriptor {
    contents
        .descriptors
        .iter()
        .find(|row| row.kind == kind)
        .expect("descriptor row")
}

#[test]
fn both_targets_append_exact_names_and_address_free_metadata() {
    for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
        let plan = plan_elf_procedure_linkage_section_descriptors(templates(target, &IMPORTS))
            .expect("validated linkage descriptors");
        assert_eq!(plan.templates.linkage().descriptors().descriptor_count(), 7);
        assert_eq!(
            plan.templates
                .linkage()
                .descriptors()
                .section_name_seed_byte_count(),
            79,
        );
        assert_eq!(plan.descriptor_count(), 11);
        assert_eq!(plan.appended_descriptor_count(), 4);
        assert_eq!(plan.section_name_seed_byte_count(), 113);
        assert_eq!(
            &plan.contents.section_name_table_seed[79..],
            PROCEDURE_LINKAGE_NAME_SUFFIX,
        );

        let plt = row(
            &plan.contents,
            ElfProcedureLinkageSectionKind::ProcedureLinkage,
        );
        assert_eq!(plt.name_offset, 79);
        assert_eq!(plt.section_type, SHT_PROGBITS);
        assert_eq!(plt.alignment, 16);
        assert_eq!(plt.entry_size, 0);
        assert_eq!(plt.link, ElfProcedureLinkageSectionLink::None);
        assert_eq!(plt.info, ElfProcedureLinkageSectionInfo::None);
        assert_eq!(
            plt.flags,
            match target {
                TargetProfile::LinuxX64 => SHF_ALLOC | SHF_EXECINSTR,
                TargetProfile::LinuxArm64 => {
                    SHF_ALLOC | SHF_EXECINSTR | SHF_AARCH64_PURECODE
                }
                _ => unreachable!(),
            },
        );
        assert_eq!(
            plt.payload_size,
            match target {
                TargetProfile::LinuxX64 => 48,
                TargetProfile::LinuxArm64 => 64,
                _ => unreachable!(),
            },
        );

        assert_eq!(
            *row(&plan.contents, ElfProcedureLinkageSectionKind::ProcedureGot,),
            ElfProcedureLinkageSectionDescriptor {
                kind: ElfProcedureLinkageSectionKind::ProcedureGot,
                name_offset: 84,
                section_type: SHT_PROGBITS,
                flags: SHF_ALLOC | SHF_WRITE,
                payload_size: 40,
                alignment: 8,
                entry_size: 0,
                link: ElfProcedureLinkageSectionLink::None,
                info: ElfProcedureLinkageSectionInfo::None,
            },
        );
        assert_eq!(
            *row(
                &plan.contents,
                ElfProcedureLinkageSectionKind::ProcedureRelocation,
            ),
            ElfProcedureLinkageSectionDescriptor {
                kind: ElfProcedureLinkageSectionKind::ProcedureRelocation,
                name_offset: 93,
                section_type: SHT_RELA,
                flags: SHF_ALLOC | SHF_INFO_LINK,
                payload_size: 48,
                alignment: 8,
                entry_size: 24,
                link: ElfProcedureLinkageSectionLink::DynamicSymbol,
                info: ElfProcedureLinkageSectionInfo::RelocatedSection(
                    ElfProcedureLinkageSectionKind::ProcedureGot,
                ),
            },
        );
        assert_eq!(
            *row(
                &plan.contents,
                ElfProcedureLinkageSectionKind::GeneralRelocation,
            ),
            ElfProcedureLinkageSectionDescriptor {
                kind: ElfProcedureLinkageSectionKind::GeneralRelocation,
                name_offset: 103,
                section_type: SHT_RELA,
                flags: SHF_ALLOC | SHF_INFO_LINK,
                payload_size: 0,
                alignment: 8,
                entry_size: 24,
                link: ElfProcedureLinkageSectionLink::DynamicSymbol,
                info: ElfProcedureLinkageSectionInfo::None,
            },
        );
        assert_ne!(
            plan.non_authoritative_descriptor_compatibility_fingerprint(),
            0
        );
        validate_contents(plan.templates(), &plan.contents)
            .expect("independent linkage-descriptor replay");
    }
}

#[test]
fn name_seed_is_an_exact_append_only_version_of_the_seven_row_owner() {
    let plan = plan_elf_procedure_linkage_section_descriptors(templates(
        TargetProfile::LinuxX64,
        &IMPORTS,
    ))
    .unwrap();
    let base = plan
        .templates
        .linkage()
        .descriptors()
        .section_name_table_seed();
    assert_eq!(&plan.contents.section_name_table_seed[..base.len()], base);
    assert_eq!(
        &plan.contents.section_name_table_seed[59..69],
        b".shstrtab\0",
    );
    for descriptor in &plan.contents.descriptors {
        validate_name(&plan.contents.section_name_table_seed, descriptor).unwrap();
    }
}

#[test]
fn import_permutation_preserves_identity_and_target_policy_changes_it() {
    let forward = plan_elf_procedure_linkage_section_descriptors(templates(
        TargetProfile::LinuxX64,
        &IMPORTS,
    ))
    .unwrap();
    let reverse_imports = IMPORTS.iter().rev().copied().collect::<Vec<_>>();
    let reverse = plan_elf_procedure_linkage_section_descriptors(templates(
        TargetProfile::LinuxX64,
        &reverse_imports,
    ))
    .unwrap();
    let arm = plan_elf_procedure_linkage_section_descriptors(templates(
        TargetProfile::LinuxArm64,
        &IMPORTS,
    ))
    .unwrap();

    assert_eq!(
        forward.non_authoritative_descriptor_compatibility_fingerprint(),
        reverse.non_authoritative_descriptor_compatibility_fingerprint()
    );
    assert_eq!(forward.contents, reverse.contents);
    assert_ne!(
        forward.non_authoritative_descriptor_compatibility_fingerprint(),
        arm.non_authoritative_descriptor_compatibility_fingerprint()
    );
    assert_ne!(
        row(
            &forward.contents,
            ElfProcedureLinkageSectionKind::ProcedureLinkage,
        )
        .flags,
        row(
            &arm.contents,
            ElfProcedureLinkageSectionKind::ProcedureLinkage,
        )
        .flags,
    );
}

#[test]
fn every_appended_name_byte_is_bound_and_rejection_retains_templates() {
    let base_len = candidate(TargetProfile::LinuxX64)
        .templates
        .linkage()
        .descriptors()
        .section_name_seed_byte_count();
    for offset in base_len..base_len + PROCEDURE_LINKAGE_NAME_SUFFIX.len() {
        let mut candidate = candidate(TargetProfile::LinuxX64);
        let expected_identity = candidate
            .templates
            .non_authoritative_template_compatibility_fingerprint();
        candidate.contents.section_name_table_seed[offset] ^= 1;
        let error =
            validate_candidate(candidate).expect_err("mutated linkage name seed must reject");
        assert_eq!(
            error
                .candidate
                .templates
                .non_authoritative_template_compatibility_fingerprint(),
            expected_identity
        );
    }
}

#[test]
fn independent_replay_rejects_every_descriptor_field_and_identity_corruption() {
    let corruptions: Vec<Box<dyn Fn(&mut Candidate)>> = vec![
        Box::new(|candidate| candidate.contents.section_name_table_seed[0] ^= 1),
        Box::new(|candidate| {
            candidate.contents.section_name_table_seed.pop();
        }),
        Box::new(|candidate| candidate.contents.section_name_table_seed.push(0)),
        Box::new(|candidate| {
            candidate.contents.descriptors.pop();
        }),
        Box::new(|candidate| {
            candidate
                .contents
                .descriptors
                .push(candidate.contents.descriptors[0])
        }),
        Box::new(|candidate| candidate.contents.descriptors.swap(0, 1)),
        Box::new(|candidate| {
            candidate.contents.descriptors[0].kind = ElfProcedureLinkageSectionKind::ProcedureGot
        }),
        Box::new(|candidate| candidate.contents.descriptors[0].name_offset = u32::MAX),
        Box::new(|candidate| candidate.contents.descriptors[0].section_type = SHT_RELA),
        Box::new(|candidate| candidate.contents.descriptors[0].flags ^= SHF_WRITE),
        Box::new(|candidate| candidate.contents.descriptors[0].payload_size += 1),
        Box::new(|candidate| candidate.contents.descriptors[0].alignment = 3),
        Box::new(|candidate| candidate.contents.descriptors[0].entry_size = 16),
        Box::new(|candidate| {
            candidate.contents.descriptors[0].link = ElfProcedureLinkageSectionLink::DynamicSymbol
        }),
        Box::new(|candidate| {
            candidate.contents.descriptors[0].info =
                ElfProcedureLinkageSectionInfo::RelocatedSection(
                    ElfProcedureLinkageSectionKind::ProcedureGot,
                )
        }),
        Box::new(|candidate| {
            candidate.contents.descriptors[2].link = ElfProcedureLinkageSectionLink::None
        }),
        Box::new(|candidate| {
            candidate.contents.descriptors[2].info =
                ElfProcedureLinkageSectionInfo::RelocatedSection(
                    ElfProcedureLinkageSectionKind::ProcedureLinkage,
                )
        }),
        Box::new(|candidate| candidate.non_authoritative_descriptor_compatibility_fingerprint ^= 1),
    ];

    for corrupt in corruptions {
        let mut candidate = candidate(TargetProfile::LinuxX64);
        let expected_identity = candidate
            .templates
            .non_authoritative_template_compatibility_fingerprint();
        corrupt(&mut candidate);
        let error = validate_candidate(candidate)
            .expect_err("corrupt linkage descriptor candidate must reject");
        assert_eq!(
            error
                .candidate
                .templates
                .non_authoritative_template_compatibility_fingerprint(),
            expected_identity,
            "linkage-descriptor rejection retains exact template custody",
        );
    }
}

#[test]
fn aarch64_purecode_and_typed_relocation_semantics_are_exact() {
    let mut candidate = candidate(TargetProfile::LinuxArm64);
    let plt = row(
        &candidate.contents,
        ElfProcedureLinkageSectionKind::ProcedureLinkage,
    );
    assert_eq!(plt.flags, SHF_ALLOC | SHF_EXECINSTR | SHF_AARCH64_PURECODE,);
    let rela = row(
        &candidate.contents,
        ElfProcedureLinkageSectionKind::ProcedureRelocation,
    );
    assert_eq!(rela.link, ElfProcedureLinkageSectionLink::DynamicSymbol);
    assert_eq!(
        rela.info,
        ElfProcedureLinkageSectionInfo::RelocatedSection(
            ElfProcedureLinkageSectionKind::ProcedureGot,
        ),
    );

    candidate.contents.descriptors[0].flags &= !SHF_AARCH64_PURECODE;
    let error = validate_candidate(candidate)
        .expect_err("AArch64 PLT without pure-code identity must reject");
    assert_eq!(
        target(&error.candidate.templates),
        TargetProfile::LinuxArm64,
    );
}

#[test]
fn malformed_offsets_lengths_and_arithmetic_reject_without_panicking() {
    assert!(checked_sum(usize::MAX, 1, "sum").is_err());
    assert!(checked_u32(usize::MAX, "word").is_err());
    let seed = b"\0.plt\0";
    let mut row = ElfProcedureLinkageSectionDescriptor {
        kind: ElfProcedureLinkageSectionKind::ProcedureLinkage,
        name_offset: u32::MAX,
        section_type: SHT_PROGBITS,
        flags: SHF_ALLOC | SHF_EXECINSTR,
        payload_size: 16,
        alignment: 16,
        entry_size: 0,
        link: ElfProcedureLinkageSectionLink::None,
        info: ElfProcedureLinkageSectionInfo::None,
    };
    assert!(validate_name(seed, &row).is_err());
    row.name_offset = 1;
    assert!(validate_name(b"\0.plt", &row).is_err());
    row.name_offset = 2;
    assert!(validate_name(seed, &row).is_err());
}
