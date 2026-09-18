//! Storage addresses, encoded fields and storage byte access.

use crate::dynamic_executable::checked::require;
use crate::dynamic_executable::file_assembly::dynamic_file_envelope::ValidatedElfDynamicFileEnvelope;
use crate::dynamic_executable::file_assembly::resolved_procedure_linkage::candidates::ElfResolvedProcedureLinkageContents;
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
    checked_sum_u64(
        base,
        u64::try_from(fixup.byte_offset)
            .map_err(|_| Diagnostic::error("procedure fixup byte offset exceeds Elf64_Addr"))?,
        "procedure fixup source address",
    )
}

pub(crate) fn semantic_target_address(
    layout: &ValidatedElfDynamicLoadLayout,
    fixup: &ElfIndexedProcedureFixup,
) -> Result<u64, Diagnostic> {
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
            checked_sum_u64(
                procedure_linkage_header_size(layout)?,
                checked_product_u64(
                    u64::from(logical_ordinal),
                    PROCEDURE_LINKAGE_ENTRY_SIZE,
                    "procedure-linkage entry offset",
                )?,
                "procedure-linkage entry offset",
            )?,
        ),
        ElfProcedureLinkageSemanticTarget::PltLazyTail { logical_ordinal } => (
            ElfPlacedDynamicSectionKind::ProcedureLinkage,
            checked_sum_u64(
                checked_sum_u64(
                    procedure_linkage_header_size(layout)?,
                    checked_product_u64(
                        u64::from(logical_ordinal),
                        PROCEDURE_LINKAGE_ENTRY_SIZE,
                        "procedure-linkage lazy-tail offset",
                    )?,
                    "procedure-linkage lazy-tail offset",
                )?,
                X86_PLT_LAZY_TAIL_OFFSET,
                "procedure-linkage lazy-tail offset",
            )?,
        ),
        ElfProcedureLinkageSemanticTarget::GotPltHeaderWord { word_index } => (
            ElfPlacedDynamicSectionKind::ProcedureGot,
            checked_product_u64(
                u64::from(word_index),
                ELF64_GOT_WORD_SIZE,
                "procedure GOT header-word offset",
            )?,
        ),
        ElfProcedureLinkageSemanticTarget::GotPltSlot { logical_ordinal } => (
            ElfPlacedDynamicSectionKind::ProcedureGot,
            checked_product_u64(
                checked_sum_u64(
                    GOT_PLT_HEADER_WORDS,
                    u64::from(logical_ordinal),
                    "procedure GOT slot index",
                )?,
                ELF64_GOT_WORD_SIZE,
                "procedure GOT slot offset",
            )?,
        ),
    };
    require(
        section.kind() == expected_kind,
        "procedure fixup semantic target kind drifted from absolute layout",
    )?;
    require(
        checked_sum_u64(offset, 1, "procedure fixup target extent")? <= section.byte_size(),
        "procedure fixup semantic target lies outside its exact section",
    )?;
    checked_sum_u64(
        section.virtual_address().ok_or_else(|| {
            Diagnostic::error("procedure fixup semantic target has no allocated address")
        })?,
        offset,
        "procedure fixup target address",
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
            let next_instruction = checked_sum_u64(source_address, 4, "x86-64 relocation PC")?;
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
                checked_sum_u64(source_address, 4, "decoded x86-64 relocation PC")?,
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

pub(crate) fn upstream_storage_bytes(
    envelope: &ValidatedElfDynamicFileEnvelope,
    storage: ElfAppliedProcedureLinkageStorage,
) -> Result<&[u8], Diagnostic> {
    let payloads = indexed_payloads(envelope);
    match storage {
        ElfAppliedProcedureLinkageStorage::SourceText => {
            Ok(&load_layout(envelope).retained_image().memory.text)
        }
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
    }
}

pub(crate) fn read_field(bytes: &[u8], offset: usize, width: u8) -> Result<u64, Diagnostic> {
    let end = checked_sum_usize(offset, usize::from(width), "procedure fixup field end")?;
    let field = bytes
        .get(offset..end)
        .ok_or_else(|| Diagnostic::error("procedure fixup field exceeds its exact storage"))?;
    match width {
        4 => Ok(u64::from(u32::from_le_bytes(
            field.try_into().expect("four-byte slice"),
        ))),
        8 => Ok(u64::from_le_bytes(
            field.try_into().expect("eight-byte slice"),
        )),
        _ => Err(Diagnostic::error(
            "procedure fixup has an unsupported field width",
        )),
    }
}

pub(crate) fn write_field(
    bytes: &mut [u8],
    offset: usize,
    width: u8,
    value: u64,
) -> Result<(), Diagnostic> {
    let end = checked_sum_usize(offset, usize::from(width), "procedure fixup field end")?;
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
            _ => Err(Diagnostic::error(
                "procedure fixup names a non-procedure storage section",
            )),
        },
    }
}

pub(crate) const fn public_kind(
    kind: ElfProcedureLinkageFixupKind,
) -> ElfAppliedProcedureLinkageKind {
    match kind {
        ElfProcedureLinkageFixupKind::X86PcRelative32 => {
            ElfAppliedProcedureLinkageKind::X86PcRelative32
        }
        ElfProcedureLinkageFixupKind::Aarch64Page21 => {
            ElfAppliedProcedureLinkageKind::Aarch64Page21
        }
        ElfProcedureLinkageFixupKind::Aarch64Load64Low12 => {
            ElfAppliedProcedureLinkageKind::Aarch64Load64Low12
        }
        ElfProcedureLinkageFixupKind::Aarch64AddLow12 => {
            ElfAppliedProcedureLinkageKind::Aarch64AddLow12
        }
        ElfProcedureLinkageFixupKind::Aarch64Branch26 => {
            ElfAppliedProcedureLinkageKind::Aarch64Branch26
        }
        ElfProcedureLinkageFixupKind::Absolute64 => ElfAppliedProcedureLinkageKind::Absolute64,
        ElfProcedureLinkageFixupKind::Elf64RelaOffset => {
            ElfAppliedProcedureLinkageKind::Elf64RelaOffset
        }
    }
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
    }
}

const fn public_section_kind(kind: ElfDynamicRosterSectionKind) -> ElfPlacedDynamicSectionKind {
    match kind {
        ElfDynamicRosterSectionKind::Null => ElfPlacedDynamicSectionKind::Null,
        ElfDynamicRosterSectionKind::Interpreter => ElfPlacedDynamicSectionKind::Interpreter,
        ElfDynamicRosterSectionKind::DynamicString => ElfPlacedDynamicSectionKind::DynamicString,
        ElfDynamicRosterSectionKind::DynamicSymbol => ElfPlacedDynamicSectionKind::DynamicSymbol,
        ElfDynamicRosterSectionKind::SystemVHash => ElfPlacedDynamicSectionKind::SystemVHash,
        ElfDynamicRosterSectionKind::GnuHash => ElfPlacedDynamicSectionKind::GnuHash,
        ElfDynamicRosterSectionKind::GnuSymbolVersion => {
            ElfPlacedDynamicSectionKind::GnuSymbolVersion
        }
        ElfDynamicRosterSectionKind::GnuVersionRequirement => {
            ElfPlacedDynamicSectionKind::GnuVersionRequirement
        }
        ElfDynamicRosterSectionKind::ProcedureLinkage => {
            ElfPlacedDynamicSectionKind::ProcedureLinkage
        }
        ElfDynamicRosterSectionKind::ProcedureGot => ElfPlacedDynamicSectionKind::ProcedureGot,
        ElfDynamicRosterSectionKind::ProcedureRelocation => {
            ElfPlacedDynamicSectionKind::ProcedureRelocation
        }
        ElfDynamicRosterSectionKind::DynamicTable => ElfPlacedDynamicSectionKind::DynamicTable,
        ElfDynamicRosterSectionKind::SectionNameTable => {
            ElfPlacedDynamicSectionKind::SectionNameTable
        }
    }
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

pub(crate) fn checked_sum_usize(
    left: usize,
    right: usize,
    context: &'static str,
) -> Result<usize, Diagnostic> {
    left.checked_add(right)
        .ok_or_else(|| Diagnostic::error(format!("{context} overflows usize")))
}

fn checked_sum_u64(left: u64, right: u64, context: &'static str) -> Result<u64, Diagnostic> {
    left.checked_add(right)
        .ok_or_else(|| Diagnostic::error(format!("{context} overflows Elf64_Addr")))
}

fn checked_product_u64(left: u64, right: u64, context: &'static str) -> Result<u64, Diagnostic> {
    left.checked_mul(right)
        .ok_or_else(|| Diagnostic::error(format!("{context} overflows Elf64_Xword")))
}
