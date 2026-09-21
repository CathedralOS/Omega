//! Template candidates: derived contents, encoded bytes, fixups and
//! constraints.

use crate::bytes::write_u64;
use crate::dynamic_executable::checked::{checked_product, checked_sum, checked_u32};
use crate::dynamic_executable::procedure_linkage::dynamic_import_relocations::{
    ElfProcedureLinkageRelocationContents, ValidatedElfProcedureLinkageRelocationPlan,
};
use crate::dynamic_executable::procedure_linkage::dynamic_linkage_templates::template_plans::{
    ElfProcedureLinkageFixup, ElfProcedureLinkageFixupKind, ElfProcedureLinkageFixupStorage,
    ElfProcedureLinkagePlacementConstraint, ElfProcedureLinkagePlacementConstraintKind,
    ElfProcedureLinkageSemanticTarget, ElfProcedureLinkageTemplateBytes,
    ElfProcedureLinkageTemplateContents, ElfProcedureLinkageTemplatePolicy,
};
use crate::dynamic_executable::procedure_linkage::dynamic_linkage_templates::{
    AARCH64_PLT_ENTRY, AARCH64_PLT_ENTRY_SIZE, AARCH64_PLT_HEADER, AARCH64_PLT_HEADER_SIZE,
    ELF64_GOT_WORD_SIZE, ELF64_RELA_SIZE, GOT_PLT_HEADER_WORDS, X86_PLT_ENTRY_SIZE, X86_PLT_HEADER,
    X86_PLT_HEADER_SIZE,
};
use diagnostics::Diagnostic;
use target::TargetProfile;

pub(crate) struct Candidate {
    pub(super) linkage: ValidatedElfProcedureLinkageRelocationPlan,
    pub(super) contents: ElfProcedureLinkageTemplateContents,
    pub(super) non_authoritative_template_compatibility_fingerprint: u64,
}

pub(crate) struct CandidateValidationError {
    pub(super) candidate: Candidate,
    pub(super) diagnostic: Diagnostic,
}

pub(crate) fn derive_contents(
    linkage: &ValidatedElfProcedureLinkageRelocationPlan,
) -> Result<ElfProcedureLinkageTemplateContents, Diagnostic> {
    let target = target(linkage);
    let policy = match target {
        TargetProfile::LinuxX64 => ElfProcedureLinkageTemplatePolicy::X86_64SmallMediumLazy,
        TargetProfile::LinuxArm64 => ElfProcedureLinkageTemplatePolicy::Aarch64StandardLazy,
        _ => {
            return Err(Diagnostic::error(
                "ELF GOT/PLT templates require an exact Linux x86-64 or AArch64 profile",
            ));
        }
    };
    let bytes = encode_template_bytes(target, linkage.contents())?;
    let fixups = derive_fixups(target, linkage.contents())?;
    let constraints = derive_constraints(&fixups)?;
    Ok(ElfProcedureLinkageTemplateContents {
        policy,
        bytes,
        fixups,
        constraints,
    })
}

pub(crate) fn target(linkage: &ValidatedElfProcedureLinkageRelocationPlan) -> TargetProfile {
    linkage
        .descriptors()
        .payloads()
        .plan()
        .inputs()
        .interpreter()
        .target()
}

fn encode_template_bytes(
    target: TargetProfile,
    linkage: &ElfProcedureLinkageRelocationContents,
) -> Result<ElfProcedureLinkageTemplateBytes, Diagnostic> {
    let slot_count = linkage.slots.len();
    let mut plt = match target {
        TargetProfile::LinuxX64 => Vec::with_capacity(checked_sum(
            X86_PLT_HEADER_SIZE,
            checked_product(slot_count, X86_PLT_ENTRY_SIZE, "x86-64 PLT entries")?,
            "x86-64 PLT template size",
        )?),
        TargetProfile::LinuxArm64 => Vec::with_capacity(checked_sum(
            AARCH64_PLT_HEADER_SIZE,
            checked_product(slot_count, AARCH64_PLT_ENTRY_SIZE, "AArch64 PLT entries")?,
            "AArch64 PLT template size",
        )?),
        _ => return Err(Diagnostic::error("unsupported ELF GOT/PLT template target")),
    };
    match target {
        TargetProfile::LinuxX64 => {
            plt.extend(X86_PLT_HEADER);
            for slot in &linkage.slots {
                let relocation_index = i32::try_from(slot.logical_ordinal).map_err(|_| {
                    Diagnostic::error("x86-64 lazy-binding relocation index exceeds signed imm32")
                })?;
                plt.extend([0xff, 0x25, 0, 0, 0, 0, 0x68]);
                plt.extend(relocation_index.to_le_bytes());
                plt.extend([0xe9, 0, 0, 0, 0]);
            }
        }
        TargetProfile::LinuxArm64 => {
            plt.extend(AARCH64_PLT_HEADER);
            for _ in &linkage.slots {
                plt.extend(AARCH64_PLT_ENTRY);
            }
        }
        _ => unreachable!("target was checked above"),
    }

    let got_word_count = checked_sum(GOT_PLT_HEADER_WORDS, slot_count, "procedure GOT word count")?;
    let got_plt = vec![
        0;
        checked_product(
            got_word_count,
            ELF64_GOT_WORD_SIZE,
            "procedure GOT template size",
        )?
    ];
    let mut rela_plt = Vec::with_capacity(checked_product(
        linkage.jump_slot_relocations.len(),
        ELF64_RELA_SIZE,
        "procedure RELA template size",
    )?);
    for relocation in &linkage.jump_slot_relocations {
        write_u64(&mut rela_plt, 0);
        let info = (u64::from(relocation.dynamic_symbol_index) << 32)
            | u64::from(relocation.relocation_type);
        write_u64(&mut rela_plt, info);
        write_u64(&mut rela_plt, relocation.addend as u64);
    }
    let mut rela_dyn = Vec::with_capacity(checked_product(
        linkage.general_relocations.len(),
        ELF64_RELA_SIZE,
        "general RELA template size",
    )?);
    for relocation in &linkage.general_relocations {
        write_u64(&mut rela_dyn, 0);
        let info = (u64::from(relocation.dynamic_symbol_index) << 32)
            | u64::from(relocation.relocation_type);
        write_u64(&mut rela_dyn, info);
        write_u64(&mut rela_dyn, relocation.addend as u64);
    }
    Ok(ElfProcedureLinkageTemplateBytes {
        plt,
        got_plt,
        rela_plt,
        rela_dyn,
    })
}

pub(crate) fn derive_fixups(
    target: TargetProfile,
    linkage: &ElfProcedureLinkageRelocationContents,
) -> Result<Vec<ElfProcedureLinkageFixup>, Diagnostic> {
    let mut fixups = Vec::new();
    match target {
        TargetProfile::LinuxX64 => {
            push_fixup(
                &mut fixups,
                ElfProcedureLinkageFixupStorage::Plt,
                2,
                ElfProcedureLinkageFixupKind::X86PcRelative32,
                ElfProcedureLinkageSemanticTarget::GotPltHeaderWord { word_index: 1 },
            );
            push_fixup(
                &mut fixups,
                ElfProcedureLinkageFixupStorage::Plt,
                8,
                ElfProcedureLinkageFixupKind::X86PcRelative32,
                ElfProcedureLinkageSemanticTarget::GotPltHeaderWord { word_index: 2 },
            );
            for slot in &linkage.slots {
                let entry = checked_sum(
                    X86_PLT_HEADER_SIZE,
                    checked_product(
                        slot.logical_ordinal as usize,
                        X86_PLT_ENTRY_SIZE,
                        "x86-64 PLT entry offset",
                    )?,
                    "x86-64 PLT entry offset",
                )?;
                push_fixup(
                    &mut fixups,
                    ElfProcedureLinkageFixupStorage::Plt,
                    checked_sum(entry, 2, "x86-64 GOT displacement offset")?,
                    ElfProcedureLinkageFixupKind::X86PcRelative32,
                    ElfProcedureLinkageSemanticTarget::GotPltSlot {
                        logical_ordinal: slot.logical_ordinal,
                    },
                );
                push_fixup(
                    &mut fixups,
                    ElfProcedureLinkageFixupStorage::Plt,
                    checked_sum(entry, 12, "x86-64 PLT0 displacement offset")?,
                    ElfProcedureLinkageFixupKind::X86PcRelative32,
                    ElfProcedureLinkageSemanticTarget::PltHeader,
                );
            }
            push_fixup(
                &mut fixups,
                ElfProcedureLinkageFixupStorage::GotPlt,
                0,
                ElfProcedureLinkageFixupKind::Absolute64,
                ElfProcedureLinkageSemanticTarget::FutureDynamicSection,
            );
            for slot in &linkage.slots {
                push_fixup(
                    &mut fixups,
                    ElfProcedureLinkageFixupStorage::GotPlt,
                    got_slot_byte_offset(slot.logical_ordinal)?,
                    ElfProcedureLinkageFixupKind::Absolute64,
                    ElfProcedureLinkageSemanticTarget::PltLazyTail {
                        logical_ordinal: slot.logical_ordinal,
                    },
                );
            }
        }
        TargetProfile::LinuxArm64 => {
            for (offset, kind) in [
                (4, ElfProcedureLinkageFixupKind::Aarch64Page21),
                (8, ElfProcedureLinkageFixupKind::Aarch64Load64Low12),
                (12, ElfProcedureLinkageFixupKind::Aarch64AddLow12),
            ] {
                push_fixup(
                    &mut fixups,
                    ElfProcedureLinkageFixupStorage::Plt,
                    offset,
                    kind,
                    ElfProcedureLinkageSemanticTarget::GotPltHeaderWord { word_index: 2 },
                );
            }
            for slot in &linkage.slots {
                let entry = checked_sum(
                    AARCH64_PLT_HEADER_SIZE,
                    checked_product(
                        slot.logical_ordinal as usize,
                        AARCH64_PLT_ENTRY_SIZE,
                        "AArch64 PLT entry offset",
                    )?,
                    "AArch64 PLT entry offset",
                )?;
                for (field, kind) in [
                    (0, ElfProcedureLinkageFixupKind::Aarch64Page21),
                    (4, ElfProcedureLinkageFixupKind::Aarch64Load64Low12),
                    (8, ElfProcedureLinkageFixupKind::Aarch64AddLow12),
                ] {
                    push_fixup(
                        &mut fixups,
                        ElfProcedureLinkageFixupStorage::Plt,
                        checked_sum(entry, field, "AArch64 PLT fixup offset")?,
                        kind,
                        ElfProcedureLinkageSemanticTarget::GotPltSlot {
                            logical_ordinal: slot.logical_ordinal,
                        },
                    );
                }
            }
            for slot in &linkage.slots {
                push_fixup(
                    &mut fixups,
                    ElfProcedureLinkageFixupStorage::GotPlt,
                    got_slot_byte_offset(slot.logical_ordinal)?,
                    ElfProcedureLinkageFixupKind::Absolute64,
                    ElfProcedureLinkageSemanticTarget::PltHeader,
                );
            }
        }
        _ => return Err(Diagnostic::error("unsupported ELF GOT/PLT fixup target")),
    }

    for (index, relocation) in linkage.jump_slot_relocations.iter().enumerate() {
        push_fixup(
            &mut fixups,
            ElfProcedureLinkageFixupStorage::RelaPlt,
            checked_product(index, ELF64_RELA_SIZE, "procedure RELA r_offset")?,
            ElfProcedureLinkageFixupKind::Elf64RelaOffset,
            ElfProcedureLinkageSemanticTarget::GotPltSlot {
                logical_ordinal: relocation.logical_got_slot_ordinal,
            },
        );
    }
    for (index, relocation) in linkage.general_relocations.iter().enumerate() {
        push_fixup(
            &mut fixups,
            ElfProcedureLinkageFixupStorage::RelaDyn,
            checked_product(index, ELF64_RELA_SIZE, "general RELA r_offset")?,
            ElfProcedureLinkageFixupKind::Elf64RelaOffset,
            ElfProcedureLinkageSemanticTarget::RelocatedImageSection {
                section: relocation.source_section,
                byte_offset: relocation.source_offset,
            },
        );
    }
    for slot in &linkage.slots {
        for call_site in &slot.call_sites {
            push_fixup(
                &mut fixups,
                ElfProcedureLinkageFixupStorage::SourceText,
                call_site.relocation_offset,
                match target {
                    TargetProfile::LinuxX64 => ElfProcedureLinkageFixupKind::X86PcRelative32,
                    TargetProfile::LinuxArm64 => ElfProcedureLinkageFixupKind::Aarch64Branch26,
                    _ => unreachable!("target was checked above"),
                },
                ElfProcedureLinkageSemanticTarget::PltEntry {
                    logical_ordinal: slot.logical_ordinal,
                },
            );
        }
    }
    Ok(fixups)
}

fn push_fixup(
    fixups: &mut Vec<ElfProcedureLinkageFixup>,
    storage: ElfProcedureLinkageFixupStorage,
    byte_offset: usize,
    kind: ElfProcedureLinkageFixupKind,
    target: ElfProcedureLinkageSemanticTarget,
) {
    let (byte_width, mutable_mask) = fixup_field(kind);
    fixups.push(ElfProcedureLinkageFixup {
        storage,
        byte_offset,
        byte_width,
        mutable_mask,
        kind,
        target,
    });
}

pub(crate) const fn fixup_field(kind: ElfProcedureLinkageFixupKind) -> (u8, u64) {
    match kind {
        ElfProcedureLinkageFixupKind::X86PcRelative32 => (4, 0xffff_ffff),
        ElfProcedureLinkageFixupKind::Aarch64Page21 => (4, 0x60ff_ffe0),
        ElfProcedureLinkageFixupKind::Aarch64Load64Low12
        | ElfProcedureLinkageFixupKind::Aarch64AddLow12 => (4, 0x003f_fc00),
        ElfProcedureLinkageFixupKind::Aarch64Branch26 => (4, 0x03ff_ffff),
        ElfProcedureLinkageFixupKind::Absolute64
        | ElfProcedureLinkageFixupKind::Elf64RelaOffset => (8, u64::MAX),
    }
}

pub(crate) fn derive_constraints(
    fixups: &[ElfProcedureLinkageFixup],
) -> Result<Vec<ElfProcedureLinkagePlacementConstraint>, Diagnostic> {
    let mut constraints = Vec::new();
    for (index, fixup) in fixups.iter().enumerate() {
        let kind = match fixup.kind {
            ElfProcedureLinkageFixupKind::X86PcRelative32 => {
                Some(ElfProcedureLinkagePlacementConstraintKind::X86Signed32)
            }
            ElfProcedureLinkageFixupKind::Aarch64Page21 => {
                Some(ElfProcedureLinkagePlacementConstraintKind::Aarch64PageDelta21)
            }
            ElfProcedureLinkageFixupKind::Aarch64Load64Low12 => {
                Some(ElfProcedureLinkagePlacementConstraintKind::Aarch64Load64Low12Aligned)
            }
            ElfProcedureLinkageFixupKind::Aarch64Branch26 => {
                Some(ElfProcedureLinkagePlacementConstraintKind::Aarch64Branch26)
            }
            ElfProcedureLinkageFixupKind::Aarch64AddLow12
            | ElfProcedureLinkageFixupKind::Absolute64
            | ElfProcedureLinkageFixupKind::Elf64RelaOffset => None,
        };
        if let Some(kind) = kind {
            constraints.push(ElfProcedureLinkagePlacementConstraint {
                fixup_ordinal: checked_u32(index, "procedure-linkage fixup ordinal")?,
                kind,
            });
        }
    }
    Ok(constraints)
}

fn got_slot_byte_offset(logical_ordinal: u32) -> Result<usize, Diagnostic> {
    checked_product(
        checked_sum(
            GOT_PLT_HEADER_WORDS,
            logical_ordinal as usize,
            "procedure GOT slot word index",
        )?,
        ELF64_GOT_WORD_SIZE,
        "procedure GOT slot byte offset",
    )
}
