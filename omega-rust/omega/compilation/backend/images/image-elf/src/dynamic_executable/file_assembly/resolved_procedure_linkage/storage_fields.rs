//! Storage addresses, encoded fields and storage byte access.

use crate::dynamic_executable::checked::require;
use crate::dynamic_executable::file_assembly::dynamic_file_envelope::ValidatedElfDynamicFileEnvelope;
use crate::dynamic_executable::file_assembly::resolved_procedure_linkage::candidates::ElfResolvedProcedureLinkageContents;
use crate::dynamic_executable::file_assembly::resolved_procedure_linkage::image_local_relocations::ElfImageLocalRelocatedMemory;
use crate::dynamic_executable::file_assembly::resolved_procedure_linkage::{
    AARCH64_PLT_HEADER_SIZE, ELF64_GOT_WORD_SIZE, ElfAppliedProcedureLinkageKind,
    ElfAppliedProcedureLinkageStorage, ElfAppliedProcedureLinkageTarget, GOT_PLT_HEADER_WORDS,
    PROCEDURE_LINKAGE_ENTRY_SIZE, X86_PLT_HEADER_SIZE, X86_PLT_LAZY_TAIL_OFFSET,
};
use crate::dynamic_executable::load_placement::load_layout::{
    ElfPlacedDynamicSection, ElfPlacedDynamicSectionKind, ValidatedElfDynamicLoadLayout,
};
use crate::dynamic_executable::procedure_linkage::dynamic_linkage_templates::{
    ElfProcedureLinkageFixupKind, ElfProcedureLinkageSemanticTarget,
};
use crate::dynamic_executable::section_headers::section_payload_roster::{
    ElfIndexedProcedureFixup, ElfIndexedProcedureFixupStorage, ElfIndexedSectionPayloadContents,
};
use crate::dynamic_executable::section_headers::section_roster::ElfDynamicRosterSectionKind;
use diagnostics::Diagnostic;

pub(crate) fn storage_address(
    layout: &ValidatedElfDynamicLoadLayout,
    fixup: &ElfIndexedProcedureFixup,
) -> Result<u64, Diagnostic> {
    let base = match fixup.storage {
        ElfIndexedProcedureFixupStorage::SourceText => layout.image_memory().text_virtual_address(),
        ElfIndexedProcedureFixupStorage::Section { index, kind } => {
            let section = exact_section(layout, index)?;
            require(
                section.kind() == public_section_kind(kind),
                "procedure fixup storage kind drifted from absolute layout",
            )?;
            section.virtual_address().ok_or_else(|| {
                Diagnostic::error("procedure fixup storage has no allocated address")
            })?
        }
    };
    crate::dynamic_executable::checked::checked_sum_u64(
        base,
        u64::try_from(fixup.byte_offset)
            .map_err(|_| Diagnostic::error("procedure fixup byte offset exceeds Elf64_Addr"))?,
        "procedure fixup source address",
        "Elf64_Addr",
    )
}

pub(crate) fn semantic_target_address(
    layout: &ValidatedElfDynamicLoadLayout,
    fixup: &ElfIndexedProcedureFixup,
) -> Result<u64, Diagnostic> {
    if let ElfProcedureLinkageSemanticTarget::RelocatedImageSection {
        section,
        byte_offset,
    } = fixup.target
    {
        let base = match section {
            image::FinalImageSection::Text => layout.image_memory().text_virtual_address(),
            image::FinalImageSection::Data => layout.image_memory().data_virtual_address(),
            image::FinalImageSection::Bss => layout.image_memory().bss_virtual_address(),
            image::FinalImageSection::None => {
                return Err(Diagnostic::error(
                    "general relocation fixup targets no materialized image section",
                ));
            }
        };
        return crate::dynamic_executable::checked::checked_sum_u64(
            base,
            u64::try_from(byte_offset).map_err(|_| {
                Diagnostic::error("general relocation slot offset exceeds Elf64_Addr")
            })?,
            "general relocation target slot address",
            "Elf64_Addr",
        );
    }
    let section = exact_section(layout, fixup.target_section_index)?;
    let (expected_kind, offset) = match fixup.target {
        ElfProcedureLinkageSemanticTarget::FutureDynamicSection => {
            (ElfPlacedDynamicSectionKind::DynamicTable, 0)
        }
        ElfProcedureLinkageSemanticTarget::PltHeader => {
            (ElfPlacedDynamicSectionKind::ProcedureLinkage, 0)
        }
        ElfProcedureLinkageSemanticTarget::PltEntry { logical_ordinal } => (
            ElfPlacedDynamicSectionKind::ProcedureLinkage,
            crate::dynamic_executable::checked::checked_sum_u64(
                procedure_linkage_header_size(layout)?,
                crate::dynamic_executable::checked::checked_product_u64(
                    u64::from(logical_ordinal),
                    PROCEDURE_LINKAGE_ENTRY_SIZE,
                    "procedure-linkage entry offset",
                    "Elf64_Xword",
                )?,
                "procedure-linkage entry offset",
                "Elf64_Addr",
            )?,
        ),
        ElfProcedureLinkageSemanticTarget::PltLazyTail { logical_ordinal } => (
            ElfPlacedDynamicSectionKind::ProcedureLinkage,
            crate::dynamic_executable::checked::checked_sum_u64(
                crate::dynamic_executable::checked::checked_sum_u64(
                    procedure_linkage_header_size(layout)?,
                    crate::dynamic_executable::checked::checked_product_u64(
                        u64::from(logical_ordinal),
                        PROCEDURE_LINKAGE_ENTRY_SIZE,
                        "procedure-linkage lazy-tail offset",
                        "Elf64_Xword",
                    )?,
                    "procedure-linkage lazy-tail offset",
                    "Elf64_Addr",
                )?,
                X86_PLT_LAZY_TAIL_OFFSET,
                "procedure-linkage lazy-tail offset",
                "Elf64_Addr",
            )?,
        ),
        ElfProcedureLinkageSemanticTarget::GotPltHeaderWord { word_index } => (
            ElfPlacedDynamicSectionKind::ProcedureGot,
            crate::dynamic_executable::checked::checked_product_u64(
                u64::from(word_index),
                ELF64_GOT_WORD_SIZE,
                "procedure GOT header-word offset",
                "Elf64_Xword",
            )?,
        ),
        ElfProcedureLinkageSemanticTarget::GotPltSlot { logical_ordinal } => (
            ElfPlacedDynamicSectionKind::ProcedureGot,
            crate::dynamic_executable::checked::checked_product_u64(
                crate::dynamic_executable::checked::checked_sum_u64(
                    GOT_PLT_HEADER_WORDS,
                    u64::from(logical_ordinal),
                    "procedure GOT slot index",
                    "Elf64_Addr",
                )?,
                ELF64_GOT_WORD_SIZE,
                "procedure GOT slot offset",
                "Elf64_Xword",
            )?,
        ),
        ElfProcedureLinkageSemanticTarget::RelocatedImageSection { .. } => {
            unreachable!("image-section targets resolve through image memory placement")
        }
    };
    require(
        section.kind() == expected_kind,
        "procedure fixup semantic target kind drifted from absolute layout",
    )?;
    require(
        crate::dynamic_executable::checked::checked_sum_u64(
            offset,
            1,
            "procedure fixup target extent",
            "Elf64_Addr",
        )? <= section.byte_size(),
        "procedure fixup semantic target lies outside its exact section",
    )?;
    crate::dynamic_executable::checked::checked_sum_u64(
        section.virtual_address().ok_or_else(|| {
            Diagnostic::error("procedure fixup semantic target has no allocated address")
        })?,
        offset,
        "procedure fixup target address",
        "Elf64_Addr",
    )
}

fn procedure_linkage_header_size(
    layout: &ValidatedElfDynamicLoadLayout,
) -> Result<u64, Diagnostic> {
    match layout.target() {
        target::TargetProfile::LinuxX64 => Ok(X86_PLT_HEADER_SIZE),
        target::TargetProfile::LinuxArm64 => Ok(AARCH64_PLT_HEADER_SIZE),
        _ => Err(Diagnostic::error(
            "procedure-linkage application requires Linux x86-64 or AArch64",
        )),
    }
}

fn exact_section(
    layout: &ValidatedElfDynamicLoadLayout,
    index: u32,
) -> Result<&ElfPlacedDynamicSection, Diagnostic> {
    layout
        .sections()
        .get(index as usize)
        .filter(|section| section.index() == index)
        .ok_or_else(|| Diagnostic::error("procedure fixup references a missing placed section"))
}

pub(crate) fn encode_field(
    kind: ElfProcedureLinkageFixupKind,
    original: u64,
    mutable_mask: u64,
    source_address: u64,
    target_address: u64,
) -> Result<u64, Diagnostic> {
    let mutable_bits = match kind {
        ElfProcedureLinkageFixupKind::X86PcRelative32 => {
            let next_instruction = crate::dynamic_executable::checked::checked_sum_u64(
                source_address,
                4,
                "x86-64 relocation PC",
                "Elf64_Addr",
            )?;
            let delta = i128::from(target_address) - i128::from(next_instruction);
            u64::from(u32::from_le_bytes(
                i32::try_from(delta)
                    .map_err(|_| {
                        Diagnostic::error(format!(
                            "x86-64 procedure relocation is out of signed-32 range: {delta} byte(s)"
                        ))
                    })?
                    .to_le_bytes(),
            ))
        }
        ElfProcedureLinkageFixupKind::Aarch64Page21 => {
            let source_page = source_address & !0xfff;
            let target_page = target_address & !0xfff;
            let page_delta = (i128::from(target_page) - i128::from(source_page)) / 4096;
            require(
                (-(1_i128 << 20)..(1_i128 << 20)).contains(&page_delta),
                "AArch64 procedure ADRP relocation is out of signed page range",
            )?;
            let immediate = (page_delta as u32) & 0x1f_ffff;
            u64::from(((immediate & 0b11) << 29) | (((immediate >> 2) & 0x7ffff) << 5))
        }
        ElfProcedureLinkageFixupKind::Aarch64Load64Low12 => {
            let page_offset = target_address & 0xfff;
            require(
                page_offset.is_multiple_of(8),
                "AArch64 procedure LDR target is not eight-byte aligned",
            )?;
            (page_offset / 8) << 10
        }
        ElfProcedureLinkageFixupKind::Aarch64AddLow12 => (target_address & 0xfff) << 10,
        ElfProcedureLinkageFixupKind::Aarch64Branch26 => {
            let delta = i128::from(target_address) - i128::from(source_address);
            require(
                delta % 4 == 0,
                "AArch64 procedure branch target is not instruction-aligned",
            )?;
            let immediate = delta / 4;
            require(
                (-(1_i128 << 25)..(1_i128 << 25)).contains(&immediate),
                "AArch64 procedure branch relocation is out of signed branch range",
            )?;
            (immediate as u64) & 0x03ff_ffff
        }
        ElfProcedureLinkageFixupKind::Absolute64
        | ElfProcedureLinkageFixupKind::Elf64RelaOffset => target_address,
    };
    require(
        mutable_bits & !mutable_mask == 0,
        "procedure relocation encoding exceeds its typed mutable mask",
    )?;
    Ok((original & !mutable_mask) | mutable_bits)
}

pub(crate) fn decode_rejoins_target(
    kind: ElfProcedureLinkageFixupKind,
    field: u64,
    source_address: u64,
    target_address: u64,
) -> Result<(), Diagnostic> {
    let rejoins = match kind {
        ElfProcedureLinkageFixupKind::X86PcRelative32 => {
            let displacement = i32::from_le_bytes((field as u32).to_le_bytes());
            checked_add_signed(
                crate::dynamic_executable::checked::checked_sum_u64(
                    source_address,
                    4,
                    "decoded x86-64 relocation PC",
                    "Elf64_Addr",
                )?,
                i128::from(displacement),
                "decoded x86-64 procedure target",
            )? == target_address
        }
        ElfProcedureLinkageFixupKind::Aarch64Page21 => {
            let word = field as u32;
            let immediate = ((word >> 29) & 0b11) | (((word >> 5) & 0x7ffff) << 2);
            let page_delta = sign_extend(u64::from(immediate), 21) * 4096;
            checked_add_signed(
                source_address & !0xfff,
                page_delta,
                "decoded AArch64 ADRP target page",
            )? == target_address & !0xfff
        }
        ElfProcedureLinkageFixupKind::Aarch64Load64Low12 => {
            (((field >> 10) & 0xfff) * 8) == target_address & 0xfff
        }
        ElfProcedureLinkageFixupKind::Aarch64AddLow12 => {
            ((field >> 10) & 0xfff) == target_address & 0xfff
        }
        ElfProcedureLinkageFixupKind::Aarch64Branch26 => {
            let immediate = sign_extend(field & 0x03ff_ffff, 26) * 4;
            checked_add_signed(source_address, immediate, "decoded AArch64 branch target")?
                == target_address
        }
        ElfProcedureLinkageFixupKind::Absolute64
        | ElfProcedureLinkageFixupKind::Elf64RelaOffset => field == target_address,
    };
    require(
        rejoins,
        "decoded procedure-linkage field does not rejoin its exact semantic target",
    )
}

const fn sign_extend(value: u64, bits: u32) -> i128 {
    let shift = 128 - bits;
    ((value as i128) << shift) >> shift
}

fn checked_add_signed(base: u64, delta: i128, context: &'static str) -> Result<u64, Diagnostic> {
    let value = i128::from(base)
        .checked_add(delta)
        .filter(|value| (0..=i128::from(u64::MAX)).contains(value))
        .ok_or_else(|| Diagnostic::error(format!("{context} overflows Elf64_Addr")))?;
    Ok(value as u64)
}

/// The exact bytes one storage holds before any procedure fixup: the indexed
/// linkage templates, or the image-locally relocated source `.text`.
pub(crate) fn upstream_storage_bytes<'a>(
    envelope: &'a ValidatedElfDynamicFileEnvelope,
    relocated: &'a ElfImageLocalRelocatedMemory,
    storage: ElfAppliedProcedureLinkageStorage,
) -> Result<&'a [u8], Diagnostic> {
    let payloads = indexed_payloads(envelope);
    match storage {
        ElfAppliedProcedureLinkageStorage::SourceText => Ok(&relocated.text),
        ElfAppliedProcedureLinkageStorage::ProcedureLinkage => {
            indexed_row_bytes(payloads, 8, ElfDynamicRosterSectionKind::ProcedureLinkage)
        }
        ElfAppliedProcedureLinkageStorage::ProcedureGot => {
            indexed_row_bytes(payloads, 9, ElfDynamicRosterSectionKind::ProcedureGot)
        }
        ElfAppliedProcedureLinkageStorage::ProcedureRelocation => indexed_row_bytes(
            payloads,
            10,
            ElfDynamicRosterSectionKind::ProcedureRelocation,
        ),
        ElfAppliedProcedureLinkageStorage::GeneralRelocation => {
            indexed_row_bytes(payloads, 11, ElfDynamicRosterSectionKind::GeneralRelocation)
        }
    }
}

pub(crate) fn indexed_row_bytes(
    payloads: &ElfIndexedSectionPayloadContents,
    index: u32,
    kind: ElfDynamicRosterSectionKind,
) -> Result<&[u8], Diagnostic> {
    payloads
        .rows
        .get(index as usize)
        .filter(|row| row.index == index && row.kind == kind)
        .map(|row| row.bytes.as_slice())
        .ok_or_else(|| Diagnostic::error("procedure-linkage storage row is missing"))
}

pub(crate) fn storage_bytes(
    contents: &ElfResolvedProcedureLinkageContents,
    storage: ElfAppliedProcedureLinkageStorage,
) -> &[u8] {
    match storage {
        ElfAppliedProcedureLinkageStorage::SourceText => &contents.source_text_bytes,
        ElfAppliedProcedureLinkageStorage::ProcedureLinkage => &contents.procedure_linkage_bytes,
        ElfAppliedProcedureLinkageStorage::ProcedureGot => &contents.procedure_got_bytes,
        ElfAppliedProcedureLinkageStorage::ProcedureRelocation => {
            &contents.procedure_relocation_bytes
        }
        ElfAppliedProcedureLinkageStorage::GeneralRelocation => &contents.general_relocation_bytes,
    }
}

pub(crate) fn storage_bytes_mut(
    contents: &mut ElfResolvedProcedureLinkageContents,
    storage: ElfAppliedProcedureLinkageStorage,
) -> &mut [u8] {
    match storage {
        ElfAppliedProcedureLinkageStorage::SourceText => &mut contents.source_text_bytes,
        ElfAppliedProcedureLinkageStorage::ProcedureLinkage => {
            &mut contents.procedure_linkage_bytes
        }
        ElfAppliedProcedureLinkageStorage::ProcedureGot => &mut contents.procedure_got_bytes,
        ElfAppliedProcedureLinkageStorage::ProcedureRelocation => {
            &mut contents.procedure_relocation_bytes
        }
        ElfAppliedProcedureLinkageStorage::GeneralRelocation => {
            &mut contents.general_relocation_bytes
        }
    }
}

pub(crate) fn write_field(
    bytes: &mut [u8],
    offset: usize,
    width: u8,
    value: u64,
) -> Result<(), Diagnostic> {
    let end = crate::dynamic_executable::checked::checked_sum(
        offset,
        usize::from(width),
        "procedure fixup field end",
    )?;
    let field = bytes
        .get_mut(offset..end)
        .ok_or_else(|| Diagnostic::error("procedure fixup field exceeds its exact storage"))?;
    match width {
        4 => field.copy_from_slice(
            &u32::try_from(value)
                .map_err(|_| Diagnostic::error("procedure fixup value exceeds 32-bit field"))?
                .to_le_bytes(),
        ),
        8 => field.copy_from_slice(&value.to_le_bytes()),
        _ => {
            return Err(Diagnostic::error(
                "procedure fixup has an unsupported field width",
            ));
        }
    }
    Ok(())
}

pub(crate) fn public_storage(
    storage: ElfIndexedProcedureFixupStorage,
) -> Result<ElfAppliedProcedureLinkageStorage, Diagnostic> {
    match storage {
        ElfIndexedProcedureFixupStorage::SourceText => {
            Ok(ElfAppliedProcedureLinkageStorage::SourceText)
        }
        ElfIndexedProcedureFixupStorage::Section { kind, .. } => match kind {
            ElfDynamicRosterSectionKind::ProcedureLinkage => {
                Ok(ElfAppliedProcedureLinkageStorage::ProcedureLinkage)
            }
            ElfDynamicRosterSectionKind::ProcedureGot => {
                Ok(ElfAppliedProcedureLinkageStorage::ProcedureGot)
            }
            ElfDynamicRosterSectionKind::ProcedureRelocation => {
                Ok(ElfAppliedProcedureLinkageStorage::ProcedureRelocation)
            }
            ElfDynamicRosterSectionKind::GeneralRelocation => {
                Ok(ElfAppliedProcedureLinkageStorage::GeneralRelocation)
            }
            _ => Err(Diagnostic::error(
                "procedure fixup names a non-procedure storage section",
            )),
        },
    }
}

pub(crate) const fn public_kind(
    kind: ElfProcedureLinkageFixupKind,
) -> ElfAppliedProcedureLinkageKind {
    kind
}

pub(crate) const fn public_target(
    target: ElfProcedureLinkageSemanticTarget,
) -> ElfAppliedProcedureLinkageTarget {
    match target {
        ElfProcedureLinkageSemanticTarget::FutureDynamicSection => {
            ElfAppliedProcedureLinkageTarget::DynamicSection
        }
        ElfProcedureLinkageSemanticTarget::PltHeader => {
            ElfAppliedProcedureLinkageTarget::ProcedureLinkageHeader
        }
        ElfProcedureLinkageSemanticTarget::PltEntry { logical_ordinal } => {
            ElfAppliedProcedureLinkageTarget::ProcedureLinkageEntry { logical_ordinal }
        }
        ElfProcedureLinkageSemanticTarget::PltLazyTail { logical_ordinal } => {
            ElfAppliedProcedureLinkageTarget::ProcedureLinkageLazyTail { logical_ordinal }
        }
        ElfProcedureLinkageSemanticTarget::GotPltHeaderWord { word_index } => {
            ElfAppliedProcedureLinkageTarget::ProcedureGotHeaderWord { word_index }
        }
        ElfProcedureLinkageSemanticTarget::GotPltSlot { logical_ordinal } => {
            ElfAppliedProcedureLinkageTarget::ProcedureGotSlot { logical_ordinal }
        }
        ElfProcedureLinkageSemanticTarget::RelocatedImageSection {
            section,
            byte_offset,
        } => ElfAppliedProcedureLinkageTarget::RelocatedImageSection {
            section,
            byte_offset,
        },
    }
}

const fn public_section_kind(kind: ElfDynamicRosterSectionKind) -> ElfPlacedDynamicSectionKind {
    kind
}

pub(crate) fn load_layout(
    envelope: &ValidatedElfDynamicFileEnvelope,
) -> &ValidatedElfDynamicLoadLayout {
    envelope
        .resolved_dynamic_table()
        .placed_section_headers()
        .load_layout()
}

pub(crate) fn indexed_payloads(
    envelope: &ValidatedElfDynamicFileEnvelope,
) -> &ElfIndexedSectionPayloadContents {
    load_layout(envelope).relative().payloads().contents()
}
