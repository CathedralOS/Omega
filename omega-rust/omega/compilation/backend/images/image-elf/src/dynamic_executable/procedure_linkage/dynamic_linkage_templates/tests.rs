//! Procedure linkage template tests.

use super::{AARCH64_PLT_ENTRY, AARCH64_PLT_HEADER, ELF64_RELA_SIZE, ElfProcedureLinkageFixup, ElfProcedureLinkageFixupKind, ElfProcedureLinkageFixupStorage, ElfProcedureLinkagePlacementConstraintKind, ElfProcedureLinkageSemanticTarget, ElfProcedureLinkageTemplatePolicy, ValidatedElfProcedureLinkageRelocationPlan, non_authoritative_template_compatibility_fingerprint, plan_elf_procedure_linkage_templates};
use crate::{
    plan_elf_dynamic_link_inputs, plan_elf_dynamic_section_descriptors,
    plan_elf_dynamic_sections, plan_elf_procedure_linkage_relocations,
    serialize_elf_dynamic_sections,
};
use arena::Handle;
use image::{
    FinalImage, FinalImageMemory,
};
use target::normalize_elf_interpreter_plan;
use crate::dynamic_executable::procedure_linkage::dynamic_linkage_templates::candidates::{Candidate, derive_contents};
use crate::dynamic_executable::procedure_linkage::dynamic_linkage_templates::template_validation::{validate_candidate, validate_contents, validate_fixup_coverage};
use target::TargetProfile;
use crate::dynamic_executable::checked::{checked_product, checked_sum, checked_u32, read_field, read_u32, read_u64};

use crate::test_support::{IMPORTS, ImportFixture, import_fixture_interpreter_path};

fn linkage(
    target: TargetProfile,
    imports: &[ImportFixture],
) -> ValidatedElfProcedureLinkageRelocationPlan {
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
        let symbol_handle = crate::test_support::insert_import_fixture(
            &mut image,
            "__omega_template_import_",
            index,
            fixture.object,
            fixture.symbol,
            fixture.version,
            target,
            "valid template locator",
        );
        crate::test_support::insert_text_call_relocations(
            &mut image,
            target,
            fixture
                .instruction_offsets
                .iter()
                .map(|offset| (*offset, symbol_handle)),
        );
    }
    let interpreter =
        normalize_elf_interpreter_plan(import_fixture_interpreter_path(target).to_vec(), target)
            .expect("valid template interpreter");
    let inputs =
        plan_elf_dynamic_link_inputs(image, interpreter).expect("valid dynamic-link inputs");
    let sections = plan_elf_dynamic_sections(inputs).expect("valid dynamic sections");
    let payloads = serialize_elf_dynamic_sections(sections).expect("valid dynamic payloads");
    let descriptors =
        plan_elf_dynamic_section_descriptors(payloads).expect("valid dynamic descriptors");
    plan_elf_procedure_linkage_relocations(descriptors).expect("valid semantic procedure linkage")
}

fn candidate(target: TargetProfile) -> Candidate {
    let linkage = linkage(target, &IMPORTS);
    let contents = derive_contents(&linkage).expect("derived templates");
    let non_authoritative_template_compatibility_fingerprint =
        non_authoritative_template_compatibility_fingerprint(&linkage, &contents);
    Candidate {
        linkage,
        contents,
        non_authoritative_template_compatibility_fingerprint,
    }
}

#[test]
fn both_targets_emit_exact_fixed_templates_fixups_and_constraints() {
    for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
        let plan = plan_elf_procedure_linkage_templates(linkage(target, &IMPORTS))
            .expect("validated linkage templates");
        assert_eq!(plan.procedure_got_byte_count(), 48);
        assert_eq!(plan.procedure_relocation_byte_count(), 72);
        assert_eq!(
            plan.procedure_linkage_byte_count(),
            match target {
                TargetProfile::LinuxX64 => 64,
                TargetProfile::LinuxArm64 => 80,
                _ => unreachable!(),
            }
        );
        assert_eq!(
            plan.fixup_count(),
            match target {
                TargetProfile::LinuxX64 => 19,
                TargetProfile::LinuxArm64 => 22,
                _ => unreachable!(),
            }
        );
        assert_eq!(plan.placement_constraint_count(), 12);
        assert_ne!(
            plan.non_authoritative_template_compatibility_fingerprint(),
            0
        );
        assert!(plan.contents.bytes.got_plt.iter().all(|byte| *byte == 0));

        for (index, relocation) in plan
            .linkage
            .contents()
            .jump_slot_relocations
            .iter()
            .enumerate()
        {
            let start = index * ELF64_RELA_SIZE;
            assert_eq!(
                read_u64(&plan.contents.bytes.rela_plt, start, "offset").unwrap(),
                0
            );
            assert_eq!(
                read_u64(&plan.contents.bytes.rela_plt, start + 8, "info").unwrap(),
                (u64::from(relocation.dynamic_symbol_index) << 32)
                    | u64::from(relocation.relocation_type),
            );
            assert_eq!(
                read_u64(&plan.contents.bytes.rela_plt, start + 16, "addend").unwrap(),
                0,
            );
        }
        validate_contents(plan.linkage(), &plan.contents).expect("independent template replay");
    }
}

#[test]
fn aarch64_keeps_all_three_header_words_zero_and_targets_plt_zero() {
    let plan = plan_elf_procedure_linkage_templates(linkage(TargetProfile::LinuxArm64, &IMPORTS))
        .expect("validated AArch64 templates");
    assert_eq!(&plan.contents.bytes.plt[..32], &AARCH64_PLT_HEADER);
    assert_eq!(&plan.contents.bytes.plt[32..48], &AARCH64_PLT_ENTRY);
    assert_eq!(&plan.contents.bytes.got_plt[..24], &[0; 24]);
    assert!(!plan.contents.fixups.iter().any(|fixup| {
        fixup.storage == ElfProcedureLinkageFixupStorage::GotPlt && fixup.byte_offset < 24
    }));
    assert_eq!(
        plan.contents
            .fixups
            .iter()
            .filter(|fixup| {
                fixup.storage == ElfProcedureLinkageFixupStorage::GotPlt
                    && matches!(fixup.target, ElfProcedureLinkageSemanticTarget::PltHeader)
            })
            .count(),
        3,
    );
    assert!(!plan.contents.fixups.iter().any(|fixup| matches!(
        fixup.target,
        ElfProcedureLinkageSemanticTarget::FutureDynamicSection
    )));
}

#[test]
fn import_permutation_and_repeated_calls_preserve_canonical_template_identity() {
    let forward =
        plan_elf_procedure_linkage_templates(linkage(TargetProfile::LinuxX64, &IMPORTS)).unwrap();
    let reverse_imports = IMPORTS.iter().rev().copied().collect::<Vec<_>>();
    let reverse =
        plan_elf_procedure_linkage_templates(linkage(TargetProfile::LinuxX64, &reverse_imports))
            .unwrap();
    let arm =
        plan_elf_procedure_linkage_templates(linkage(TargetProfile::LinuxArm64, &IMPORTS)).unwrap();

    assert_eq!(
        forward.non_authoritative_template_compatibility_fingerprint(),
        reverse.non_authoritative_template_compatibility_fingerprint()
    );
    assert_eq!(forward.contents, reverse.contents);
    assert_ne!(
        forward.non_authoritative_template_compatibility_fingerprint(),
        arm.non_authoritative_template_compatibility_fingerprint()
    );
    assert_eq!(forward.linkage().logical_slot_count(), 3);
    assert_eq!(forward.linkage().direct_call_site_count(), 4);
}

#[test]
fn independent_replay_rejects_every_template_fixup_constraint_and_identity_corruption() {
    let corruptions: Vec<Box<dyn Fn(&mut Candidate)>> = vec![
        Box::new(|candidate| {
            candidate.contents.policy = ElfProcedureLinkageTemplatePolicy::Aarch64StandardLazy
        }),
        Box::new(|candidate| candidate.contents.bytes.plt[0] ^= 1),
        Box::new(|candidate| candidate.contents.bytes.plt[2] = 1),
        Box::new(|candidate| {
            candidate.contents.bytes.plt.pop();
        }),
        Box::new(|candidate| candidate.contents.bytes.plt.push(0)),
        Box::new(|candidate| candidate.contents.bytes.got_plt[0] = 1),
        Box::new(|candidate| {
            candidate.contents.bytes.got_plt.pop();
        }),
        Box::new(|candidate| candidate.contents.bytes.got_plt.push(0)),
        Box::new(|candidate| candidate.contents.bytes.rela_plt[0] = 1),
        Box::new(|candidate| candidate.contents.bytes.rela_plt[8] ^= 1),
        Box::new(|candidate| candidate.contents.bytes.rela_plt[16] = 1),
        Box::new(|candidate| {
            candidate.contents.bytes.rela_plt.pop();
        }),
        Box::new(|candidate| candidate.contents.bytes.rela_plt.push(0)),
        Box::new(|candidate| {
            candidate.contents.fixups.pop();
        }),
        Box::new(|candidate| candidate.contents.fixups.push(candidate.contents.fixups[0])),
        Box::new(|candidate| candidate.contents.fixups.swap(0, 1)),
        Box::new(|candidate| {
            candidate.contents.fixups[0].storage = ElfProcedureLinkageFixupStorage::RelaPlt
        }),
        Box::new(|candidate| candidate.contents.fixups[0].byte_offset = usize::MAX),
        Box::new(|candidate| candidate.contents.fixups[0].byte_width = 8),
        Box::new(|candidate| candidate.contents.fixups[0].mutable_mask ^= 1),
        Box::new(|candidate| {
            candidate.contents.fixups[0].kind = ElfProcedureLinkageFixupKind::Aarch64Page21
        }),
        Box::new(|candidate| {
            candidate.contents.fixups[0].target = ElfProcedureLinkageSemanticTarget::GotPltSlot {
                logical_ordinal: u32::MAX,
            }
        }),
        Box::new(|candidate| {
            candidate.contents.constraints.pop();
        }),
        Box::new(|candidate| candidate.contents.constraints[0].fixup_ordinal = u32::MAX),
        Box::new(|candidate| {
            candidate.contents.constraints[0].kind =
                ElfProcedureLinkagePlacementConstraintKind::Aarch64Branch26
        }),
        Box::new(|candidate| candidate.non_authoritative_template_compatibility_fingerprint ^= 1),
    ];

    for corrupt in corruptions {
        let mut candidate = candidate(TargetProfile::LinuxX64);
        let expected_identity = candidate
            .linkage
            .non_authoritative_linkage_compatibility_fingerprint();
        corrupt(&mut candidate);
        let error = validate_candidate(candidate)
            .expect_err("corrupt linkage template candidate must reject");
        assert_eq!(
            error
                .candidate
                .linkage
                .non_authoritative_linkage_compatibility_fingerprint(),
            expected_identity,
            "template rejection retains exact linkage custody",
        );
    }
}

#[test]
fn aarch64_opcode_placeholder_and_reserved_header_corruption_rejects() {
    let corruptions: Vec<Box<dyn Fn(&mut Candidate)>> = vec![
        Box::new(|candidate| candidate.contents.bytes.plt[0] ^= 1),
        Box::new(|candidate| candidate.contents.bytes.plt[4] |= 0x20),
        Box::new(|candidate| candidate.contents.bytes.plt[8] |= 0x04),
        Box::new(|candidate| candidate.contents.bytes.plt[12] |= 0x04),
        Box::new(|candidate| candidate.contents.bytes.plt[32] |= 0x20),
        Box::new(|candidate| candidate.contents.bytes.got_plt[0] = 1),
        Box::new(|candidate| candidate.contents.bytes.got_plt[8] = 1),
        Box::new(|candidate| candidate.contents.bytes.got_plt[16] = 1),
        Box::new(|candidate| {
            candidate.contents.fixups[0].target =
                ElfProcedureLinkageSemanticTarget::FutureDynamicSection
        }),
    ];
    for corrupt in corruptions {
        let mut candidate = candidate(TargetProfile::LinuxArm64);
        let expected_identity = candidate
            .linkage
            .non_authoritative_linkage_compatibility_fingerprint();
        corrupt(&mut candidate);
        let error =
            validate_candidate(candidate).expect_err("corrupt AArch64 template must reject");
        assert_eq!(
            error
                .candidate
                .linkage
                .non_authoritative_linkage_compatibility_fingerprint(),
            expected_identity
        );
    }
}

#[test]
fn malformed_lengths_offsets_masks_and_arithmetic_reject_without_panicking() {
    assert!(checked_sum(usize::MAX, 1, "sum").is_err());
    assert!(checked_product(usize::MAX, 2, "product").is_err());
    assert!(checked_u32(usize::MAX, "word").is_err());
    assert!(read_u32(&[], usize::MAX, "u32").is_err());
    assert!(read_u64(&[0; 7], 0, "u64").is_err());
    assert!(read_field(&[0; 8], 0, 3, "test fixup field").is_err());

    let overflowing = ElfProcedureLinkageFixup {
        storage: ElfProcedureLinkageFixupStorage::Plt,
        byte_offset: usize::MAX,
        byte_width: 8,
        mutable_mask: u64::MAX,
        kind: ElfProcedureLinkageFixupKind::Absolute64,
        target: ElfProcedureLinkageSemanticTarget::PltHeader,
    };
    assert!(validate_fixup_coverage(&[overflowing, overflowing]).is_err());
}
