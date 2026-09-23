//! Dynamic tag tests.

use super::{
    Candidate, ElfDynamicAddressObligation, ElfDynamicAddressTarget, ElfDynamicSemanticRow,
    ElfDynamicTag, ElfDynamicValue, TargetProfile,
    ValidatedElfProcedureLinkageSectionDescriptorPlan, checked_product, checked_sum, checked_u32,
    derive_contents, dynamic_payloads, dynamic_string,
    non_authoritative_tag_compatibility_fingerprint, plan_elf_dynamic_tags, structural_contents,
    validate_candidate, validate_contents,
};
use crate::{
    plan_elf_dynamic_link_inputs, plan_elf_dynamic_section_descriptors, plan_elf_dynamic_sections,
    plan_elf_procedure_linkage_relocations, plan_elf_procedure_linkage_section_descriptors,
    plan_elf_procedure_linkage_templates, serialize_elf_dynamic_sections,
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
        _ => unreachable!("dynamic-tag fixture uses a Linux target"),
    }
}

fn descriptors(
    target: TargetProfile,
    imports: &[ImportFixture],
) -> ValidatedElfProcedureLinkageSectionDescriptorPlan {
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
            name: format!("__omega_dynamic_tag_import_{index}"),
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
                .expect("valid dynamic-tag locator"),
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
        .expect("valid dynamic-tag interpreter");
    let inputs =
        plan_elf_dynamic_link_inputs(image, interpreter).expect("valid dynamic-link inputs");
    let sections = plan_elf_dynamic_sections(inputs).expect("valid dynamic sections");
    let payloads = serialize_elf_dynamic_sections(sections).expect("valid dynamic payloads");
    let base = plan_elf_dynamic_section_descriptors(payloads).expect("valid base descriptors");
    let linkage = plan_elf_procedure_linkage_relocations(base).expect("valid procedure linkage");
    let templates = plan_elf_procedure_linkage_templates(linkage).expect("valid linkage templates");
    plan_elf_procedure_linkage_section_descriptors(templates).expect("valid linkage descriptors")
}

fn candidate(target: TargetProfile) -> Candidate {
    let descriptors = descriptors(target, &IMPORTS);
    let contents = derive_contents(&descriptors).expect("derived dynamic tags");
    let non_authoritative_tag_compatibility_fingerprint =
        non_authoritative_tag_compatibility_fingerprint(&descriptors, &contents);
    Candidate {
        descriptors,
        contents,
        non_authoritative_tag_compatibility_fingerprint,
    }
}

#[test]
fn both_targets_plan_exact_needed_prefix_fixed_tags_and_address_obligations() {
    for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
        let plan = plan_elf_dynamic_tags(descriptors(target, &IMPORTS))
            .expect("validated semantic dynamic tags");
        assert_eq!(plan.descriptors().descriptor_count(), 11);
        assert_eq!(plan.needed_row_count(), 2);
        assert_eq!(plan.row_count(), 19);
        assert_eq!(plan.address_obligation_count(), 9);
        assert_ne!(plan.non_authoritative_tag_compatibility_fingerprint(), 0);

        let structural = structural_contents(&plan.descriptors);
        assert_eq!(
            &plan.contents.rows[..2],
            &[
                ElfDynamicSemanticRow {
                    tag: ElfDynamicTag::Needed,
                    value: ElfDynamicValue::NeededStringOffset(structural.needed[0]),
                },
                ElfDynamicSemanticRow {
                    tag: ElfDynamicTag::Needed,
                    value: ElfDynamicValue::NeededStringOffset(structural.needed[1]),
                },
            ],
        );
        assert_eq!(
            plan.contents
                .rows
                .iter()
                .map(|row| row.tag)
                .collect::<Vec<_>>(),
            [
                ElfDynamicTag::Needed,
                ElfDynamicTag::Needed,
                ElfDynamicTag::ProcedureRelocationSize,
                ElfDynamicTag::ProcedureGot,
                ElfDynamicTag::SystemVHash,
                ElfDynamicTag::GnuHash,
                ElfDynamicTag::DynamicString,
                ElfDynamicTag::DynamicSymbol,
                ElfDynamicTag::DynamicStringSize,
                ElfDynamicTag::DynamicSymbolEntrySize,
                ElfDynamicTag::ProcedureRelocationKind,
                ElfDynamicTag::ProcedureRelocation,
                ElfDynamicTag::Rela,
                ElfDynamicTag::GeneralRelocationSize,
                ElfDynamicTag::GeneralRelocationEntrySize,
                ElfDynamicTag::GnuSymbolVersion,
                ElfDynamicTag::GnuVersionRequirement,
                ElfDynamicTag::GnuVersionRequirementCount,
                ElfDynamicTag::Null,
            ],
        );
        assert_eq!(
            plan.contents.address_obligations,
            [
                ElfDynamicAddressObligation {
                    row_ordinal: 3,
                    byte_width: 8,
                    target: ElfDynamicAddressTarget::ProcedureGot,
                },
                ElfDynamicAddressObligation {
                    row_ordinal: 4,
                    byte_width: 8,
                    target: ElfDynamicAddressTarget::SystemVHash,
                },
                ElfDynamicAddressObligation {
                    row_ordinal: 5,
                    byte_width: 8,
                    target: ElfDynamicAddressTarget::GnuHash,
                },
                ElfDynamicAddressObligation {
                    row_ordinal: 6,
                    byte_width: 8,
                    target: ElfDynamicAddressTarget::DynamicString,
                },
                ElfDynamicAddressObligation {
                    row_ordinal: 7,
                    byte_width: 8,
                    target: ElfDynamicAddressTarget::DynamicSymbol,
                },
                ElfDynamicAddressObligation {
                    row_ordinal: 11,
                    byte_width: 8,
                    target: ElfDynamicAddressTarget::ProcedureRelocation,
                },
                ElfDynamicAddressObligation {
                    row_ordinal: 12,
                    byte_width: 8,
                    target: ElfDynamicAddressTarget::GeneralRelocation,
                },
                ElfDynamicAddressObligation {
                    row_ordinal: 15,
                    byte_width: 8,
                    target: ElfDynamicAddressTarget::GnuSymbolVersion,
                },
                ElfDynamicAddressObligation {
                    row_ordinal: 16,
                    byte_width: 8,
                    target: ElfDynamicAddressTarget::GnuVersionRequirement,
                },
            ],
        );
        assert_eq!(
            plan.contents.rows[2].value,
            ElfDynamicValue::ProcedureRelocationByteCount(48),
        );
        assert_eq!(
            plan.contents.rows[9].value,
            ElfDynamicValue::DynamicSymbolEntryByteCount(24),
        );
        assert_eq!(
            plan.contents.rows[10].value,
            ElfDynamicValue::RelocationTag(ElfDynamicTag::Rela),
        );
        assert_eq!(
            plan.contents.rows[12].value,
            ElfDynamicValue::AddressPlaceholder,
        );
        assert_eq!(
            plan.contents.rows[13].value,
            ElfDynamicValue::GeneralRelocationByteCount(0),
        );
        assert_eq!(
            plan.contents.rows[14].value,
            ElfDynamicValue::GeneralRelocationEntryByteCount(24),
        );
        assert_eq!(
            plan.contents.rows[17].value,
            ElfDynamicValue::VersionRequirementRecordCount(2),
        );
        assert_eq!(plan.contents.rows[18].value, ElfDynamicValue::Null);
        validate_contents(plan.descriptors(), &plan.contents)
            .expect("independent dynamic-tag replay");
    }
}

#[test]
fn needed_rows_preserve_raw_non_utf8_names_and_significant_order() {
    let plan = plan_elf_dynamic_tags(descriptors(TargetProfile::LinuxX64, &IMPORTS)).unwrap();
    let dynstr = &dynamic_payloads(plan.descriptors()).payloads().dynstr;
    let names = structural_contents(plan.descriptors())
        .needed
        .iter()
        .map(|offset| dynamic_string(dynstr, *offset).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(names, [b"liba\xff.so".as_slice(), b"libb.so".as_slice()]);
    assert!(names[0].contains(&0xff));
}

#[test]
fn import_permutation_preserves_rows_and_identity_while_target_stays_bound() {
    let forward = plan_elf_dynamic_tags(descriptors(TargetProfile::LinuxX64, &IMPORTS)).unwrap();
    let reverse_imports = IMPORTS.iter().rev().copied().collect::<Vec<_>>();
    let reverse =
        plan_elf_dynamic_tags(descriptors(TargetProfile::LinuxX64, &reverse_imports)).unwrap();
    let arm = plan_elf_dynamic_tags(descriptors(TargetProfile::LinuxArm64, &IMPORTS)).unwrap();
    assert_eq!(forward.contents, reverse.contents);
    assert_eq!(
        forward.non_authoritative_tag_compatibility_fingerprint(),
        reverse.non_authoritative_tag_compatibility_fingerprint()
    );
    assert_ne!(
        forward.non_authoritative_tag_compatibility_fingerprint(),
        arm.non_authoritative_tag_compatibility_fingerprint()
    );
}

#[test]
fn exact_tag_set_binds_general_relocation_and_omits_optional_policies() {
    let plan = plan_elf_dynamic_tags(descriptors(TargetProfile::LinuxX64, &IMPORTS)).unwrap();
    assert_eq!(
        plan.contents
            .rows
            .iter()
            .filter(|row| row.tag == ElfDynamicTag::Rela)
            .count(),
        1,
    );
    assert_eq!(
        plan.descriptors
            .templates()
            .linkage()
            .general_dynamic_relocation_count(),
        0,
    );
    assert_eq!(
        plan.contents
            .rows
            .iter()
            .filter(|row| row.tag == ElfDynamicTag::GeneralRelocationEntrySize)
            .count(),
        1,
    );
    assert_eq!(
        plan.contents
            .rows
            .iter()
            .filter(|row| row.tag == ElfDynamicTag::Null)
            .count(),
        1,
    );
    assert_eq!(plan.contents.rows.last().unwrap().tag, ElfDynamicTag::Null);
}

#[test]
fn independent_replay_rejects_every_value_family_order_and_identity_corruption() {
    let corruptions: Vec<Box<dyn Fn(&mut Candidate)>> = vec![
        Box::new(|candidate| candidate.contents.rows.swap(0, 1)),
        Box::new(|candidate| candidate.contents.rows.swap(2, 3)),
        Box::new(|candidate| {
            candidate.contents.rows.pop();
        }),
        Box::new(|candidate| candidate.contents.rows.push(candidate.contents.rows[18])),
        Box::new(|candidate| candidate.contents.rows[0].tag = ElfDynamicTag::Null),
        Box::new(|candidate| {
            candidate.contents.rows[0].value = ElfDynamicValue::NeededStringOffset(0)
        }),
        Box::new(|candidate| {
            candidate.contents.rows[2].value = ElfDynamicValue::ProcedureRelocationByteCount(47)
        }),
        Box::new(|candidate| candidate.contents.rows[3].value = ElfDynamicValue::Null),
        Box::new(|candidate| {
            candidate.contents.rows[8].value = ElfDynamicValue::DynamicStringByteCount(0)
        }),
        Box::new(|candidate| {
            candidate.contents.rows[9].value = ElfDynamicValue::DynamicSymbolEntryByteCount(16)
        }),
        Box::new(|candidate| {
            candidate.contents.rows[10].value = ElfDynamicValue::RelocationTag(ElfDynamicTag::Null)
        }),
        Box::new(|candidate| {
            candidate.contents.rows[14].value = ElfDynamicValue::GeneralRelocationEntryByteCount(16)
        }),
        Box::new(|candidate| {
            candidate.contents.rows[17].value = ElfDynamicValue::VersionRequirementRecordCount(1)
        }),
        Box::new(|candidate| {
            candidate.contents.rows[18].value = ElfDynamicValue::AddressPlaceholder
        }),
        Box::new(|candidate| candidate.non_authoritative_tag_compatibility_fingerprint ^= 1),
    ];
    for corrupt in corruptions {
        let mut candidate = candidate(TargetProfile::LinuxX64);
        let expected_identity = candidate
            .descriptors
            .non_authoritative_descriptor_compatibility_fingerprint();
        corrupt(&mut candidate);
        let error =
            validate_candidate(candidate).expect_err("corrupt semantic dynamic tags must reject");
        assert_eq!(
            error
                .candidate
                .descriptors
                .non_authoritative_descriptor_compatibility_fingerprint(),
            expected_identity,
            "dynamic-tag rejection retains exact descriptor custody",
        );
    }
}

#[test]
fn independent_replay_rejects_missing_duplicate_or_misdirected_address_obligations() {
    let corruptions: Vec<Box<dyn Fn(&mut Candidate)>> = vec![
        Box::new(|candidate| {
            candidate.contents.address_obligations.pop();
        }),
        Box::new(|candidate| {
            candidate
                .contents
                .address_obligations
                .push(candidate.contents.address_obligations[0])
        }),
        Box::new(|candidate| candidate.contents.address_obligations.swap(0, 1)),
        Box::new(|candidate| candidate.contents.address_obligations[0].row_ordinal = u32::MAX),
        Box::new(|candidate| candidate.contents.address_obligations[0].byte_width = 4),
        Box::new(|candidate| {
            candidate.contents.address_obligations[0].target =
                ElfDynamicAddressTarget::DynamicString
        }),
    ];
    for corrupt in corruptions {
        let mut candidate = candidate(TargetProfile::LinuxArm64);
        let expected_identity = candidate
            .descriptors
            .non_authoritative_descriptor_compatibility_fingerprint();
        corrupt(&mut candidate);
        let error = validate_candidate(candidate)
            .expect_err("corrupt dynamic address obligations must reject");
        assert_eq!(
            error
                .candidate
                .descriptors
                .non_authoritative_descriptor_compatibility_fingerprint(),
            expected_identity,
        );
    }
}

#[test]
fn malformed_offsets_counts_and_arithmetic_reject_without_panicking() {
    assert!(checked_sum(usize::MAX, 1, "sum").is_err());
    assert!(checked_product(usize::MAX, 2, "product").is_err());
    assert!(checked_u32(usize::MAX, "word").is_err());
    assert!(dynamic_string(&[], 0).is_err());
    assert!(dynamic_string(b"unterminated", 0).is_err());
    assert!(dynamic_string(b"\0", u32::MAX).is_err());

    let mut candidate = candidate(TargetProfile::LinuxX64);
    candidate.contents.address_obligations[0].row_ordinal = u32::MAX;
    assert!(validate_candidate(candidate).is_err());
}
