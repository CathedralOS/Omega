//! Validating template candidates against the fixed PLT and RELA
//! templates.

use crate::dynamic_executable::procedure_linkage::dynamic_import_relocations::{ElfProcedureLinkageRelocationContents, ValidatedElfProcedureLinkageRelocationPlan};
use diagnostics::{Diagnostic};
use target::{TargetProfile};
use crate::dynamic_executable::procedure_linkage::dynamic_linkage_templates::{ValidatedElfProcedureLinkageTemplatePlan, AARCH64_PLT_ENTRY, AARCH64_PLT_ENTRY_SIZE, AARCH64_PLT_HEADER, AARCH64_PLT_HEADER_SIZE, ELF64_GOT_WORD_SIZE, ELF64_RELA_SIZE, GOT_PLT_HEADER_WORDS, X86_PLT_ENTRY_SIZE, X86_PLT_HEADER, X86_PLT_HEADER_SIZE};
use crate::dynamic_executable::procedure_linkage::dynamic_linkage_templates::candidates::{Candidate, CandidateValidationError, derive_constraints, derive_fixups, fixup_field, target};
use crate::dynamic_executable::procedure_linkage::dynamic_linkage_templates::compatibility_fingerprint::{non_authoritative_template_compatibility_fingerprint};
use crate::dynamic_executable::procedure_linkage::dynamic_linkage_templates::template_plans::{ElfProcedureLinkageFixup, ElfProcedureLinkageFixupStorage, ElfProcedureLinkageSemanticTarget, ElfProcedureLinkageTemplateBytes, ElfProcedureLinkageTemplateContents, ElfProcedureLinkageTemplatePolicy};
use crate::dynamic_executable::checked::{read_field, read_u32, read_u64, require};
use super::super::super::checked;

pub(crate) fn validate_candidate(
    candidate: Candidate,
) -> Result<ValidatedElfProcedureLinkageTemplatePlan, CandidateValidationError<Candidate>> {
    checked::validate_candidate(
        candidate,
        |candidate| validate_contents(&candidate.linkage, &candidate.contents),
        |candidate| {
            candidate.non_authoritative_template_compatibility_fingerprint
                == non_authoritative_template_compatibility_fingerprint(
                    &candidate.linkage,
                    &candidate.contents,
                )
        },
        "ELF GOT/PLT template compatibility fingerprint does not replay",
        |candidate| ValidatedElfProcedureLinkageTemplatePlan {
            linkage: candidate.linkage,
            contents: candidate.contents,
            non_authoritative_template_compatibility_fingerprint: candidate
                .non_authoritative_template_compatibility_fingerprint,
        },
    )
}

pub(crate) fn validate_contents(
    linkage: &ValidatedElfProcedureLinkageRelocationPlan,
    contents: &ElfProcedureLinkageTemplateContents,
) -> Result<(), Diagnostic> {
    let target = target(linkage);
    let expected_policy = match target {
        TargetProfile::LinuxX64 => ElfProcedureLinkageTemplatePolicy::X86_64SmallMediumLazy,
        TargetProfile::LinuxArm64 => ElfProcedureLinkageTemplatePolicy::Aarch64StandardLazy,
        _ => return Err(Diagnostic::error("unsupported validated GOT/PLT target")),
    };
    require(
        contents.policy == expected_policy,
        "ELF procedure-linkage template policy drifted from its exact target",
    )?;
    validate_fixed_template_bytes(target, linkage.contents(), &contents.bytes)?;
    let expected_fixups = derive_fixups(target, linkage.contents())?;
    require(
        contents.fixups == expected_fixups,
        "ELF procedure-linkage semantic fixups are missing, duplicated, or reordered",
    )?;
    validate_fixup_placeholders(linkage, contents)?;
    validate_fixup_coverage(&contents.fixups)?;
    require(
        contents.constraints == derive_constraints(&contents.fixups)?,
        "ELF procedure-linkage placement constraints drifted from exact fixups",
    )
}

fn validate_fixed_template_bytes(
    target: TargetProfile,
    linkage: &ElfProcedureLinkageRelocationContents,
    bytes: &ElfProcedureLinkageTemplateBytes,
) -> Result<(), Diagnostic> {
    let slot_count = linkage.slots.len();
    let expected_plt_size = match target {
        TargetProfile::LinuxX64 => crate::dynamic_executable::checked::checked_sum(
            X86_PLT_HEADER_SIZE,
            crate::dynamic_executable::checked::checked_product(
                slot_count,
                X86_PLT_ENTRY_SIZE,
                "validated x86-64 PLT entries",
            )?,
            "validated x86-64 PLT size",
        )?,
        TargetProfile::LinuxArm64 => crate::dynamic_executable::checked::checked_sum(
            AARCH64_PLT_HEADER_SIZE,
            crate::dynamic_executable::checked::checked_product(
                slot_count,
                AARCH64_PLT_ENTRY_SIZE,
                "validated AArch64 PLT entries",
            )?,
            "validated AArch64 PLT size",
        )?,
        _ => return Err(Diagnostic::error("unsupported fixed-template target")),
    };
    let expected_got_size = crate::dynamic_executable::checked::checked_product(
        crate::dynamic_executable::checked::checked_sum(
            GOT_PLT_HEADER_WORDS,
            slot_count,
            "validated GOT words",
        )?,
        ELF64_GOT_WORD_SIZE,
        "validated GOT size",
    )?;
    let expected_rela_size = crate::dynamic_executable::checked::checked_product(
        linkage.jump_slot_relocations.len(),
        ELF64_RELA_SIZE,
        "validated procedure RELA size",
    )?;
    let expected_rela_dyn_size = crate::dynamic_executable::checked::checked_product(
        linkage.general_relocations.len(),
        ELF64_RELA_SIZE,
        "validated general RELA size",
    )?;
    require(
        bytes.plt.len() == expected_plt_size
            && bytes.got_plt.len() == expected_got_size
            && bytes.rela_plt.len() == expected_rela_size
            && bytes.rela_dyn.len() == expected_rela_dyn_size,
        "ELF procedure-linkage template byte lengths do not match exact rows",
    )?;
    match target {
        TargetProfile::LinuxX64 => replay_x86_plt(linkage, &bytes.plt)?,
        TargetProfile::LinuxArm64 => replay_aarch64_plt(linkage, &bytes.plt)?,
        _ => unreachable!(),
    }
    require(
        bytes.got_plt.iter().all(|byte| *byte == 0),
        "ELF procedure GOT template contains a premature address or loader value",
    )?;
    replay_rela(linkage, &bytes.rela_plt)?;
    replay_general_rela(linkage, &bytes.rela_dyn)
}

fn replay_x86_plt(
    linkage: &ElfProcedureLinkageRelocationContents,
    bytes: &[u8],
) -> Result<(), Diagnostic> {
    require(
        bytes.get(..X86_PLT_HEADER_SIZE) == Some(&X86_PLT_HEADER),
        "x86-64 PLT0 template bytes drifted",
    )?;
    for (index, slot) in linkage.slots.iter().enumerate() {
        let start = crate::dynamic_executable::checked::checked_sum(
            X86_PLT_HEADER_SIZE,
            crate::dynamic_executable::checked::checked_product(
                index,
                X86_PLT_ENTRY_SIZE,
                "decoded x86-64 PLT entry",
            )?,
            "decoded x86-64 PLT entry",
        )?;
        let end = crate::dynamic_executable::checked::checked_sum(
            start,
            X86_PLT_ENTRY_SIZE,
            "decoded x86-64 PLT entry end",
        )?;
        let entry = bytes
            .get(start..end)
            .ok_or_else(|| Diagnostic::error("truncated x86-64 PLT entry"))?;
        require(
            entry.get(..7) == Some(&[0xff, 0x25, 0, 0, 0, 0, 0x68])
                && read_u32(entry, 7, "x86-64 PLT relocation ordinal")? == slot.logical_ordinal
                && entry.get(11..) == Some(&[0xe9, 0, 0, 0, 0]),
            "x86-64 PLT entry opcode, ordinal, or placeholder drifted",
        )?;
    }
    Ok(())
}

fn replay_aarch64_plt(
    linkage: &ElfProcedureLinkageRelocationContents,
    bytes: &[u8],
) -> Result<(), Diagnostic> {
    require(
        bytes.get(..AARCH64_PLT_HEADER_SIZE) == Some(&AARCH64_PLT_HEADER),
        "AArch64 PLT0 template bytes drifted",
    )?;
    for index in 0..linkage.slots.len() {
        let start = crate::dynamic_executable::checked::checked_sum(
            AARCH64_PLT_HEADER_SIZE,
            crate::dynamic_executable::checked::checked_product(
                index,
                AARCH64_PLT_ENTRY_SIZE,
                "decoded AArch64 PLT entry",
            )?,
            "decoded AArch64 PLT entry",
        )?;
        let end = crate::dynamic_executable::checked::checked_sum(
            start,
            AARCH64_PLT_ENTRY_SIZE,
            "decoded AArch64 PLT entry end",
        )?;
        require(
            bytes.get(start..end) == Some(&AARCH64_PLT_ENTRY),
            "AArch64 PLT entry opcode or zero placeholder drifted",
        )?;
    }
    Ok(())
}

macro_rules! replay_rela_rows {
    ($name:ident, $rows:ident, $domain:literal) => {
        fn $name(
            linkage: &ElfProcedureLinkageRelocationContents,
            bytes: &[u8],
        ) -> Result<(), Diagnostic> {
            for (index, relocation) in linkage.$rows.iter().enumerate() {
                let start = crate::dynamic_executable::checked::checked_product(
                    index,
                    ELF64_RELA_SIZE,
                    concat!("decoded ", $domain, " RELA row"),
                )?;
                let expected_info = (u64::from(relocation.dynamic_symbol_index) << 32)
                    | u64::from(relocation.relocation_type);
                require(
                    read_u64(bytes, start, concat!($domain, " RELA r_offset"))? == 0
                        && read_u64(
                            bytes,
                            crate::dynamic_executable::checked::checked_sum(
                                start,
                                8,
                                concat!($domain, " RELA r_info offset"),
                            )?,
                            concat!($domain, " RELA r_info"),
                        )? == expected_info
                        && read_u64(
                            bytes,
                            crate::dynamic_executable::checked::checked_sum(
                                start,
                                16,
                                concat!($domain, " RELA addend offset"),
                            )?,
                            concat!($domain, " RELA addend"),
                        )? == relocation.addend as u64,
                    concat!(
                        "ELF ",
                        $domain,
                        " RELA fixed row or address placeholder drifted"
                    ),
                )?;
            }
            Ok(())
        }
    };
}

replay_rela_rows!(replay_rela, jump_slot_relocations, "procedure");
replay_rela_rows!(replay_general_rela, general_relocations, "general");

fn validate_fixup_placeholders(
    linkage: &ValidatedElfProcedureLinkageRelocationPlan,
    contents: &ElfProcedureLinkageTemplateContents,
) -> Result<(), Diagnostic> {
    let source_text = &linkage
        .descriptors()
        .payloads()
        .plan()
        .inputs()
        .image()
        .memory
        .text;
    for fixup in &contents.fixups {
        let storage = match fixup.storage {
            ElfProcedureLinkageFixupStorage::SourceText => source_text,
            ElfProcedureLinkageFixupStorage::Plt => &contents.bytes.plt,
            ElfProcedureLinkageFixupStorage::GotPlt => &contents.bytes.got_plt,
            ElfProcedureLinkageFixupStorage::RelaPlt => &contents.bytes.rela_plt,
            ElfProcedureLinkageFixupStorage::RelaDyn => &contents.bytes.rela_dyn,
        };
        let field = read_field(
            storage,
            fixup.byte_offset,
            fixup.byte_width,
            "procedure-linkage fixup field",
        )?;
        require(
            field & fixup.mutable_mask == 0,
            "ELF procedure-linkage fixup field is not an exact zero placeholder",
        )?;
        require(
            fixup_field(fixup.kind) == (fixup.byte_width, fixup.mutable_mask),
            "ELF procedure-linkage fixup width or mutable mask drifted from its kind",
        )?;
        validate_semantic_target(linkage.contents(), fixup.target)?;
    }
    Ok(())
}

fn validate_semantic_target(
    linkage: &ElfProcedureLinkageRelocationContents,
    target: ElfProcedureLinkageSemanticTarget,
) -> Result<(), Diagnostic> {
    let valid_ordinal = |ordinal: u32| {
        linkage
            .slots
            .get(ordinal as usize)
            .is_some_and(|slot| slot.logical_ordinal == ordinal)
    };
    require(
        match target {
            ElfProcedureLinkageSemanticTarget::FutureDynamicSection
            | ElfProcedureLinkageSemanticTarget::PltHeader => true,
            ElfProcedureLinkageSemanticTarget::PltEntry { logical_ordinal }
            | ElfProcedureLinkageSemanticTarget::PltLazyTail { logical_ordinal }
            | ElfProcedureLinkageSemanticTarget::GotPltSlot { logical_ordinal } => {
                valid_ordinal(logical_ordinal)
            }
            ElfProcedureLinkageSemanticTarget::GotPltHeaderWord { word_index } => {
                usize::from(word_index) < GOT_PLT_HEADER_WORDS
            }
            ElfProcedureLinkageSemanticTarget::RelocatedImageSection {
                section,
                byte_offset,
            } => linkage.general_relocations.iter().any(|relocation| {
                relocation.source_section == section && relocation.source_offset == byte_offset
            }),
        },
        "ELF procedure-linkage fixup target does not resolve semantically",
    )
}

pub(crate) fn validate_fixup_coverage(
    fixups: &[ElfProcedureLinkageFixup],
) -> Result<(), Diagnostic> {
    for (index, fixup) in fixups.iter().enumerate() {
        for other in &fixups[index + 1..] {
            if fixup.storage != other.storage {
                continue;
            }
            let end = crate::dynamic_executable::checked::checked_sum(
                fixup.byte_offset,
                usize::from(fixup.byte_width),
                "procedure-linkage fixup end",
            )?;
            let other_end = crate::dynamic_executable::checked::checked_sum(
                other.byte_offset,
                usize::from(other.byte_width),
                "procedure-linkage fixup end",
            )?;
            require(
                end <= other.byte_offset || other_end <= fixup.byte_offset,
                "ELF procedure-linkage fixups overlap or duplicate one field",
            )?;
        }
    }
    Ok(())
}
