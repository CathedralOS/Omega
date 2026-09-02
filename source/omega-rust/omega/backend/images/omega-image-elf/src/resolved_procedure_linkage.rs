//! Exact application of dynamic ELF procedure-linkage and source-call fixups.
//!
//! This rung consumes the exact dynamic file-envelope owner, copies the
//! retained source `.text`, `.plt`, `.got.plt`, and `.rela.plt` templates, and
//! applies every indexed procedure fixup from the already-validated absolute
//! load layout. A separate replay checks every application, target, encoding,
//! mutable mask, unchanged byte, range, and alignment against the complete
//! upstream custody chain.
//!
//! These resolved byte regions remain fragments. This layer does not place
//! them into one file, mutate the retained `FinalImage`,
//! publish bytes, or grant loader or runnable-image authority.

use crate::dynamic_file_envelope::ValidatedElfDynamicFileEnvelope;
use crate::dynamic_linkage_templates::{
    ElfProcedureLinkageFixupKind, ElfProcedureLinkageSemanticTarget,
};
use crate::load_layout::{
    ElfPlacedDynamicSection, ElfPlacedDynamicSectionKind, ValidatedElfDynamicLoadLayout,
};
use crate::section_payload_roster::{
    ElfIndexedProcedureFixup, ElfIndexedProcedureFixupStorage, ElfIndexedSectionPayloadContents,
};
use crate::section_roster::ElfDynamicRosterSectionKind;
use psi_diagnostics::Diagnostic;

const X86_PLT_HEADER_SIZE: u64 = 16;
const AARCH64_PLT_HEADER_SIZE: u64 = 32;
const PROCEDURE_LINKAGE_ENTRY_SIZE: u64 = 16;
const X86_PLT_LAZY_TAIL_OFFSET: u64 = 6;
const GOT_PLT_HEADER_WORDS: u64 = 3;
const ELF64_GOT_WORD_SIZE: u64 = 8;
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Exact byte owner receiving one applied procedure-linkage fixup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ElfAppliedProcedureLinkageStorage {
    SourceText = 1,
    ProcedureLinkage = 2,
    ProcedureGot = 3,
    ProcedureRelocation = 4,
}

/// Target-specific encoding used for one applied fixup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ElfAppliedProcedureLinkageKind {
    X86PcRelative32 = 1,
    Aarch64Page21 = 2,
    Aarch64Load64Low12 = 3,
    Aarch64AddLow12 = 4,
    Aarch64Branch26 = 5,
    Absolute64 = 6,
    Elf64RelaOffset = 7,
}

/// Exact semantic target selected by one applied fixup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElfAppliedProcedureLinkageTarget {
    DynamicSection,
    ProcedureLinkageHeader,
    ProcedureLinkageEntry { logical_ordinal: u32 },
    ProcedureLinkageLazyTail { logical_ordinal: u32 },
    ProcedureGotHeaderWord { word_index: u8 },
    ProcedureGotSlot { logical_ordinal: u32 },
}

/// One exact application retained beside the resulting fragment bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ElfAppliedProcedureLinkageFixup {
    ordinal: u32,
    storage: ElfAppliedProcedureLinkageStorage,
    byte_offset: usize,
    byte_width: u8,
    mutable_mask: u64,
    kind: ElfAppliedProcedureLinkageKind,
    target: ElfAppliedProcedureLinkageTarget,
    source_address: u64,
    target_address: u64,
    encoded_field: u64,
}

impl ElfAppliedProcedureLinkageFixup {
    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }

    pub const fn storage(&self) -> ElfAppliedProcedureLinkageStorage {
        self.storage
    }

    pub const fn byte_offset(&self) -> usize {
        self.byte_offset
    }

    pub const fn byte_width(&self) -> u8 {
        self.byte_width
    }

    pub const fn mutable_mask(&self) -> u64 {
        self.mutable_mask
    }

    pub const fn kind(&self) -> ElfAppliedProcedureLinkageKind {
        self.kind
    }

    pub const fn target(&self) -> ElfAppliedProcedureLinkageTarget {
        self.target
    }

    pub const fn source_address(&self) -> u64 {
        self.source_address
    }

    pub const fn target_address(&self) -> u64 {
        self.target_address
    }

    pub const fn encoded_field(&self) -> u64 {
        self.encoded_field
    }
}

/// Independently replayed procedure-linkage fragments retaining the complete
/// dynamic ELF file-envelope custody chain.
#[derive(Debug)]
#[must_use = "resolved ELF procedure linkage retains exact non-runnable envelope custody"]
pub struct ValidatedElfResolvedProcedureLinkage {
    envelope: ValidatedElfDynamicFileEnvelope,
    contents: ElfResolvedProcedureLinkageContents,
    non_authoritative_resolved_linkage_compatibility_fingerprint: u64,
}

impl ValidatedElfResolvedProcedureLinkage {
    pub const fn envelope(&self) -> &ValidatedElfDynamicFileEnvelope {
        &self.envelope
    }

    pub fn source_text_bytes(&self) -> &[u8] {
        &self.contents.source_text_bytes
    }

    pub fn procedure_linkage_bytes(&self) -> &[u8] {
        &self.contents.procedure_linkage_bytes
    }

    pub fn procedure_got_bytes(&self) -> &[u8] {
        &self.contents.procedure_got_bytes
    }

    pub fn procedure_relocation_bytes(&self) -> &[u8] {
        &self.contents.procedure_relocation_bytes
    }

    pub fn applied_fixups(&self) -> &[ElfAppliedProcedureLinkageFixup] {
        &self.contents.applications
    }

    /// Compatibility/report coordinate only. Later file assembly must retain
    /// and replay the exact envelope, fragment bytes, and application ledger.
    pub const fn non_authoritative_resolved_linkage_compatibility_fingerprint(&self) -> u64 {
        self.non_authoritative_resolved_linkage_compatibility_fingerprint
    }

    pub(crate) fn into_envelope(self) -> ValidatedElfDynamicFileEnvelope {
        self.envelope
    }
}

/// Rejected fixup application retaining the exact file-envelope owner.
#[derive(Debug)]
#[must_use = "procedure-linkage rejection retains dynamic ELF envelope custody"]
pub struct ElfProcedureLinkageApplicationError {
    envelope: ValidatedElfDynamicFileEnvelope,
    diagnostic: Diagnostic,
}

impl ElfProcedureLinkageApplicationError {
    pub const fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (ValidatedElfDynamicFileEnvelope, Diagnostic) {
        (self.envelope, self.diagnostic)
    }
}

impl std::fmt::Display for ElfProcedureLinkageApplicationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for ElfProcedureLinkageApplicationError {}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ElfResolvedProcedureLinkageContents {
    source_text_bytes: Vec<u8>,
    procedure_linkage_bytes: Vec<u8>,
    procedure_got_bytes: Vec<u8>,
    procedure_relocation_bytes: Vec<u8>,
    applications: Vec<ElfAppliedProcedureLinkageFixup>,
}

struct Candidate {
    envelope: ValidatedElfDynamicFileEnvelope,
    contents: ElfResolvedProcedureLinkageContents,
    non_authoritative_resolved_linkage_compatibility_fingerprint: u64,
}

struct CandidateValidationError {
    candidate: Candidate,
    diagnostic: Diagnostic,
}

/// Apply every exact indexed procedure/source fixup into copied fragments.
///
/// Success remains non-runnable and does not mutate the retained final image.
pub fn apply_elf_procedure_linkage_fixups(
    envelope: ValidatedElfDynamicFileEnvelope,
) -> Result<ValidatedElfResolvedProcedureLinkage, Box<ElfProcedureLinkageApplicationError>> {
    let contents = match derive_contents(&envelope) {
        Ok(contents) => contents,
        Err(diagnostic) => {
            return Err(Box::new(ElfProcedureLinkageApplicationError {
                envelope,
                diagnostic,
            }));
        }
    };
    let non_authoritative_resolved_linkage_compatibility_fingerprint =
        non_authoritative_resolved_linkage_compatibility_fingerprint(&envelope, &contents);
    let candidate = Candidate {
        envelope,
        contents,
        non_authoritative_resolved_linkage_compatibility_fingerprint,
    };
    validate_candidate(candidate).map_err(|error| {
        Box::new(ElfProcedureLinkageApplicationError {
            envelope: error.candidate.envelope,
            diagnostic: error.diagnostic,
        })
    })
}

fn derive_contents(
    envelope: &ValidatedElfDynamicFileEnvelope,
) -> Result<ElfResolvedProcedureLinkageContents, Diagnostic> {
    let payloads = indexed_payloads(envelope);
    let mut contents = ElfResolvedProcedureLinkageContents {
        source_text_bytes: load_layout(envelope).retained_image().memory.text.clone(),
        procedure_linkage_bytes: indexed_row_bytes(
            payloads,
            8,
            ElfDynamicRosterSectionKind::ProcedureLinkage,
        )?
        .to_vec(),
        procedure_got_bytes: indexed_row_bytes(
            payloads,
            9,
            ElfDynamicRosterSectionKind::ProcedureGot,
        )?
        .to_vec(),
        procedure_relocation_bytes: indexed_row_bytes(
            payloads,
            10,
            ElfDynamicRosterSectionKind::ProcedureRelocation,
        )?
        .to_vec(),
        applications: Vec::with_capacity(payloads.procedure_fixups.len()),
    };
    for fixup in &payloads.procedure_fixups {
        let application = derive_application(envelope, &contents, fixup)?;
        write_field(
            storage_bytes_mut(&mut contents, application.storage),
            application.byte_offset,
            application.byte_width,
            application.encoded_field,
        )?;
        contents.applications.push(application);
    }
    Ok(contents)
}

fn derive_application(
    envelope: &ValidatedElfDynamicFileEnvelope,
    contents: &ElfResolvedProcedureLinkageContents,
    fixup: &ElfIndexedProcedureFixup,
) -> Result<ElfAppliedProcedureLinkageFixup, Diagnostic> {
    let storage = public_storage(fixup.storage)?;
    let source_address = storage_address(load_layout(envelope), fixup)?;
    let target_address = semantic_target_address(load_layout(envelope), fixup)?;
    let original = read_field(
        storage_bytes(contents, storage),
        fixup.byte_offset,
        fixup.byte_width,
    )?;
    require(
        original & fixup.mutable_mask == 0,
        "procedure-linkage fixup source is not an exact zero placeholder",
    )?;
    let encoded_field = encode_field(
        fixup.kind,
        original,
        fixup.mutable_mask,
        source_address,
        target_address,
    )?;
    Ok(ElfAppliedProcedureLinkageFixup {
        ordinal: fixup.upstream_ordinal,
        storage,
        byte_offset: fixup.byte_offset,
        byte_width: fixup.byte_width,
        mutable_mask: fixup.mutable_mask,
        kind: public_kind(fixup.kind),
        target: public_target(fixup.target),
        source_address,
        target_address,
        encoded_field,
    })
}

fn validate_candidate(
    candidate: Candidate,
) -> Result<ValidatedElfResolvedProcedureLinkage, CandidateValidationError> {
    if let Err(diagnostic) = validate_contents(&candidate.envelope, &candidate.contents) {
        return Err(CandidateValidationError {
            candidate,
            diagnostic,
        });
    }
    let expected = non_authoritative_resolved_linkage_compatibility_fingerprint(
        &candidate.envelope,
        &candidate.contents,
    );
    if candidate.non_authoritative_resolved_linkage_compatibility_fingerprint == 0
        || candidate.non_authoritative_resolved_linkage_compatibility_fingerprint != expected
    {
        return Err(CandidateValidationError {
            candidate,
            diagnostic: Diagnostic::error(
                "resolved procedure-linkage compatibility fingerprint does not replay",
            ),
        });
    }
    Ok(ValidatedElfResolvedProcedureLinkage {
        envelope: candidate.envelope,
        contents: candidate.contents,
        non_authoritative_resolved_linkage_compatibility_fingerprint: candidate
            .non_authoritative_resolved_linkage_compatibility_fingerprint,
    })
}

fn validate_contents(
    envelope: &ValidatedElfDynamicFileEnvelope,
    contents: &ElfResolvedProcedureLinkageContents,
) -> Result<(), Diagnostic> {
    let payloads = indexed_payloads(envelope);
    require(
        contents.source_text_bytes.len()
            == load_layout(envelope).retained_image().memory.text.len()
            && contents.procedure_linkage_bytes.len()
                == indexed_row_bytes(payloads, 8, ElfDynamicRosterSectionKind::ProcedureLinkage)?
                    .len()
            && contents.procedure_got_bytes.len()
                == indexed_row_bytes(payloads, 9, ElfDynamicRosterSectionKind::ProcedureGot)?.len()
            && contents.procedure_relocation_bytes.len()
                == indexed_row_bytes(
                    payloads,
                    10,
                    ElfDynamicRosterSectionKind::ProcedureRelocation,
                )?
                .len(),
        "resolved procedure-linkage fragment length drifted from exact upstream storage",
    )?;
    require(
        contents.applications.len() == payloads.procedure_fixups.len(),
        "resolved procedure-linkage application coverage is incomplete or duplicated",
    )?;
    validate_nonoverlapping_applications(&contents.applications)?;

    for (ordinal, (application, fixup)) in contents
        .applications
        .iter()
        .zip(&payloads.procedure_fixups)
        .enumerate()
    {
        require(
            application.ordinal == checked_u32(ordinal, "procedure application ordinal")?
                && application.ordinal == fixup.upstream_ordinal
                && application.storage == public_storage(fixup.storage)?
                && application.byte_offset == fixup.byte_offset
                && application.byte_width == fixup.byte_width
                && application.mutable_mask == fixup.mutable_mask
                && application.kind == public_kind(fixup.kind)
                && application.target == public_target(fixup.target),
            "resolved procedure-linkage application drifted from its indexed fixup",
        )?;
        let expected_source = storage_address(load_layout(envelope), fixup)?;
        let expected_target = semantic_target_address(load_layout(envelope), fixup)?;
        require(
            application.source_address == expected_source
                && application.target_address == expected_target,
            "resolved procedure-linkage source or target address drifted from absolute layout",
        )?;
        let original = read_field(
            upstream_storage_bytes(envelope, application.storage)?,
            application.byte_offset,
            application.byte_width,
        )?;
        require(
            original & application.mutable_mask == 0,
            "retained procedure-linkage template no longer has an exact zero placeholder",
        )?;
        let expected_field = encode_field(
            fixup.kind,
            original,
            application.mutable_mask,
            application.source_address,
            application.target_address,
        )?;
        let actual_field = read_field(
            storage_bytes(contents, application.storage),
            application.byte_offset,
            application.byte_width,
        )?;
        require(
            application.encoded_field == expected_field && actual_field == expected_field,
            "resolved procedure-linkage encoded field does not replay",
        )?;
        require(
            (actual_field & !application.mutable_mask) == (original & !application.mutable_mask),
            "resolved procedure-linkage application changed fixed opcode bits",
        )?;
        decode_rejoins_target(
            fixup.kind,
            actual_field,
            application.source_address,
            application.target_address,
        )?;
    }

    for storage in [
        ElfAppliedProcedureLinkageStorage::SourceText,
        ElfAppliedProcedureLinkageStorage::ProcedureLinkage,
        ElfAppliedProcedureLinkageStorage::ProcedureGot,
        ElfAppliedProcedureLinkageStorage::ProcedureRelocation,
    ] {
        validate_unchanged_bytes(
            upstream_storage_bytes(envelope, storage)?,
            storage_bytes(contents, storage),
            storage,
            &contents.applications,
        )?;
    }
    Ok(())
}

fn validate_nonoverlapping_applications(
    applications: &[ElfAppliedProcedureLinkageFixup],
) -> Result<(), Diagnostic> {
    for (index, application) in applications.iter().enumerate() {
        let end = checked_sum_usize(
            application.byte_offset,
            usize::from(application.byte_width),
            "procedure application end",
        )?;
        for other in &applications[index + 1..] {
            if application.storage != other.storage {
                continue;
            }
            let other_end = checked_sum_usize(
                other.byte_offset,
                usize::from(other.byte_width),
                "procedure application end",
            )?;
            require(
                end <= other.byte_offset || other_end <= application.byte_offset,
                "resolved procedure-linkage applications overlap or duplicate one field",
            )?;
        }
    }
    Ok(())
}

fn validate_unchanged_bytes(
    original: &[u8],
    resolved: &[u8],
    storage: ElfAppliedProcedureLinkageStorage,
    applications: &[ElfAppliedProcedureLinkageFixup],
) -> Result<(), Diagnostic> {
    require(
        original.len() == resolved.len(),
        "resolved procedure-linkage storage length drifted",
    )?;
    for (offset, (before, after)) in original.iter().zip(resolved).enumerate() {
        let mutable = applications.iter().any(|application| {
            application.storage == storage
                && offset >= application.byte_offset
                && offset
                    < application
                        .byte_offset
                        .saturating_add(usize::from(application.byte_width))
        });
        require(
            mutable || before == after,
            "resolved procedure-linkage application changed an unowned byte",
        )?;
    }
    Ok(())
}

fn storage_address(
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

fn semantic_target_address(
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
        omega_target::TargetProfile::LinuxX64 => Ok(X86_PLT_HEADER_SIZE),
        omega_target::TargetProfile::LinuxArm64 => Ok(AARCH64_PLT_HEADER_SIZE),
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

fn encode_field(
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
                page_offset % 8 == 0,
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

fn decode_rejoins_target(
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

fn upstream_storage_bytes(
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

fn indexed_row_bytes(
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

fn storage_bytes(
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

fn storage_bytes_mut(
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

fn read_field(bytes: &[u8], offset: usize, width: u8) -> Result<u64, Diagnostic> {
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

fn write_field(bytes: &mut [u8], offset: usize, width: u8, value: u64) -> Result<(), Diagnostic> {
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

fn public_storage(
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

const fn public_kind(kind: ElfProcedureLinkageFixupKind) -> ElfAppliedProcedureLinkageKind {
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

const fn public_target(
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

fn load_layout(envelope: &ValidatedElfDynamicFileEnvelope) -> &ValidatedElfDynamicLoadLayout {
    envelope
        .resolved_dynamic_table()
        .placed_section_headers()
        .load_layout()
}

fn indexed_payloads(
    envelope: &ValidatedElfDynamicFileEnvelope,
) -> &ElfIndexedSectionPayloadContents {
    load_layout(envelope).relative().payloads().contents()
}

fn checked_u32(value: usize, context: &'static str) -> Result<u32, Diagnostic> {
    u32::try_from(value).map_err(|_| Diagnostic::error(format!("{context} exceeds Elf64_Word")))
}

fn checked_sum_usize(
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

fn require(condition: bool, message: &'static str) -> Result<(), Diagnostic> {
    condition
        .then_some(())
        .ok_or_else(|| Diagnostic::error(message))
}

fn non_authoritative_resolved_linkage_compatibility_fingerprint(
    envelope: &ValidatedElfDynamicFileEnvelope,
    contents: &ElfResolvedProcedureLinkageContents,
) -> u64 {
    let mut hash = Fnv1a::new();
    hash.bytes(b"omega.elf.resolved-procedure-linkage.v1");
    hash.bytes(
        &envelope
            .non_authoritative_envelope_compatibility_fingerprint()
            .to_le_bytes(),
    );
    hash.bytes(&contents.source_text_bytes);
    hash.bytes(&contents.procedure_linkage_bytes);
    hash.bytes(&contents.procedure_got_bytes);
    hash.bytes(&contents.procedure_relocation_bytes);
    hash.bytes(&(contents.applications.len() as u64).to_le_bytes());
    for application in &contents.applications {
        hash.bytes(&application.ordinal.to_le_bytes());
        hash.byte(application.storage as u8);
        hash.bytes(&(application.byte_offset as u64).to_le_bytes());
        hash.byte(application.byte_width);
        hash.bytes(&application.mutable_mask.to_le_bytes());
        hash.byte(application.kind as u8);
        hash_target(&mut hash, application.target);
        hash.bytes(&application.source_address.to_le_bytes());
        hash.bytes(&application.target_address.to_le_bytes());
        hash.bytes(&application.encoded_field.to_le_bytes());
    }
    hash.finish()
}

fn hash_target(hash: &mut Fnv1a, target: ElfAppliedProcedureLinkageTarget) {
    match target {
        ElfAppliedProcedureLinkageTarget::DynamicSection => hash.byte(1),
        ElfAppliedProcedureLinkageTarget::ProcedureLinkageHeader => hash.byte(2),
        ElfAppliedProcedureLinkageTarget::ProcedureLinkageEntry { logical_ordinal } => {
            hash.byte(3);
            hash.bytes(&logical_ordinal.to_le_bytes());
        }
        ElfAppliedProcedureLinkageTarget::ProcedureLinkageLazyTail { logical_ordinal } => {
            hash.byte(4);
            hash.bytes(&logical_ordinal.to_le_bytes());
        }
        ElfAppliedProcedureLinkageTarget::ProcedureGotHeaderWord { word_index } => {
            hash.byte(5);
            hash.byte(word_index);
        }
        ElfAppliedProcedureLinkageTarget::ProcedureGotSlot { logical_ordinal } => {
            hash.byte(6);
            hash.bytes(&logical_ordinal.to_le_bytes());
        }
    }
}

struct Fnv1a(u64);

impl Fnv1a {
    const fn new() -> Self {
        Self(FNV_OFFSET_BASIS)
    }

    fn byte(&mut self, byte: u8) {
        self.0 ^= u64::from(byte);
        self.0 = self.0.wrapping_mul(FNV_PRIME);
    }

    fn bytes(&mut self, bytes: &[u8]) {
        for byte in (bytes.len() as u64)
            .to_le_bytes()
            .into_iter()
            .chain(bytes.iter().copied())
        {
            self.byte(byte);
        }
    }

    const fn finish(self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        apply_elf_dynamic_address_fixups, apply_elf_section_header_placements,
        plan_elf_dynamic_link_inputs, plan_elf_dynamic_load_layout,
        plan_elf_dynamic_section_descriptors, plan_elf_dynamic_section_roster,
        plan_elf_dynamic_sections, plan_elf_dynamic_table_section_descriptor,
        plan_elf_dynamic_tags, plan_elf_indexed_section_payloads,
        plan_elf_procedure_linkage_relocations, plan_elf_procedure_linkage_section_descriptors,
        plan_elf_procedure_linkage_templates, plan_elf_relative_section_payload_layout,
        plan_elf_section_name_table, serialize_elf_dynamic_file_envelope,
        serialize_elf_dynamic_sections, serialize_elf_dynamic_table,
        serialize_elf_section_header_table,
    };
    use omega_image::{
        FinalImage, FinalImageImport, FinalImageImportPlan, FinalImageMemory, FinalImageRelocation,
        FinalImageSection, FinalImageSymbol,
    };
    use omega_object_file::{RelocationKind, SymbolKind};
    use omega_target::{
        ForeignLocatorCandidate, TargetProfile, normalize_elf_interpreter_plan,
        normalize_foreign_locator,
    };
    use psi_arena::Handle;

    fn standard_envelope(target: TargetProfile) -> ValidatedElfDynamicFileEnvelope {
        envelope(target, [b"alpha_call".as_slice(), b"beta_call".as_slice()])
    }

    fn envelope(
        target: TargetProfile,
        imported_symbols: [&[u8]; 2],
    ) -> ValidatedElfDynamicFileEnvelope {
        let mut image = FinalImage::with_capacity(
            target.native_target(),
            FinalImageMemory {
                text: vec![0; 64],
                data: vec![0x5a; 13],
                bss_size: 23,
                bss_alignment: 16,
            },
            Handle::invalid(),
            3,
            2,
            3,
        );
        let entry = image.symbol_table.symbols.insert(FinalImageSymbol {
            name: "_start".to_owned(),
            section: FinalImageSection::Text,
            offset: 24,
            size: 4,
            kind: SymbolKind::Function,
        });
        image.symbol_table.entry_symbol = entry;

        let mut imports = Vec::new();
        for (index, symbol) in imported_symbols.into_iter().enumerate() {
            let handle = image.symbol_table.symbols.insert(FinalImageSymbol {
                name: format!("__omega_resolved_import_{index}"),
                section: FinalImageSection::None,
                offset: 0,
                size: 0,
                kind: SymbolKind::Import,
            });
            image.symbol_table.imports.insert(FinalImageImport {
                symbol_handle: handle,
                import: FinalImageImportPlan::Normalized(
                    normalize_foreign_locator(
                        ForeignLocatorCandidate::ElfVersioned {
                            object: b"libresolved-procedure.so".to_vec(),
                            symbol: symbol.to_vec(),
                            version: b"RESOLVED_PROCEDURE_1".to_vec(),
                        },
                        target,
                    )
                    .unwrap(),
                ),
            });
            imports.push(handle);
        }
        for (instruction_offset, symbol_handle) in
            [(0, imports[0]), (8, imports[0]), (16, imports[1])]
        {
            let (relocation_offset, kind) = match target {
                TargetProfile::LinuxX64 => {
                    image.memory.text[instruction_offset] = 0xe8;
                    (instruction_offset + 1, RelocationKind::X86_64Relative32)
                }
                TargetProfile::LinuxArm64 => {
                    image.memory.text[instruction_offset..instruction_offset + 4]
                        .copy_from_slice(&[0, 0, 0, 0x94]);
                    (instruction_offset, RelocationKind::Aarch64Branch26)
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

        let interpreter_path = match target {
            TargetProfile::LinuxX64 => b"/lib64/ld-linux-x86-64.so.2".as_slice(),
            TargetProfile::LinuxArm64 => b"/lib/ld-linux-aarch64.so.1".as_slice(),
            _ => unreachable!(),
        };
        let interpreter =
            normalize_elf_interpreter_plan(interpreter_path.to_vec(), target).unwrap();
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
        let relative = plan_elf_relative_section_payload_layout(payloads).unwrap();
        let load = plan_elf_dynamic_load_layout(relative).unwrap();
        let placed = apply_elf_section_header_placements(load).unwrap();
        let resolved = apply_elf_dynamic_address_fixups(placed).unwrap();
        serialize_elf_dynamic_file_envelope(resolved).unwrap()
    }

    fn candidate(target: TargetProfile) -> Candidate {
        let envelope = standard_envelope(target);
        let contents = derive_contents(&envelope).unwrap();
        let non_authoritative_resolved_linkage_compatibility_fingerprint =
            non_authoritative_resolved_linkage_compatibility_fingerprint(&envelope, &contents);
        Candidate {
            envelope,
            contents,
            non_authoritative_resolved_linkage_compatibility_fingerprint,
        }
    }

    fn refresh_fingerprint(candidate: &mut Candidate) {
        candidate.non_authoritative_resolved_linkage_compatibility_fingerprint =
            non_authoritative_resolved_linkage_compatibility_fingerprint(
                &candidate.envelope,
                &candidate.contents,
            );
    }

    fn assert_rejected_with_custody(candidate: Candidate) {
        let upstream = candidate
            .envelope
            .non_authoritative_envelope_compatibility_fingerprint();
        let error = validate_candidate(candidate).unwrap_err();
        assert_eq!(
            error
                .candidate
                .envelope
                .non_authoritative_envelope_compatibility_fingerprint(),
            upstream,
        );
        assert!(!error.diagnostic.to_string().is_empty());
    }

    #[test]
    fn both_linux_targets_apply_every_exact_procedure_and_source_fixup() {
        for (target, expected_count) in [
            (TargetProfile::LinuxX64, 14),
            (TargetProfile::LinuxArm64, 16),
        ] {
            let envelope = standard_envelope(target);
            let original_text = load_layout(&envelope).retained_image().memory.text.clone();
            let resolved = apply_elf_procedure_linkage_fixups(envelope).unwrap();
            assert_eq!(resolved.applied_fixups().len(), expected_count);
            assert_ne!(
                resolved.non_authoritative_resolved_linkage_compatibility_fingerprint(),
                0,
            );
            assert_eq!(resolved.source_text_bytes().len(), original_text.len());
            assert_ne!(resolved.source_text_bytes(), original_text);
            assert_eq!(
                resolved
                    .applied_fixups()
                    .iter()
                    .map(ElfAppliedProcedureLinkageFixup::ordinal)
                    .collect::<Vec<_>>(),
                (0..expected_count as u32).collect::<Vec<_>>(),
            );
            for storage in [
                ElfAppliedProcedureLinkageStorage::SourceText,
                ElfAppliedProcedureLinkageStorage::ProcedureLinkage,
                ElfAppliedProcedureLinkageStorage::ProcedureGot,
                ElfAppliedProcedureLinkageStorage::ProcedureRelocation,
            ] {
                assert!(
                    resolved
                        .applied_fixups()
                        .iter()
                        .any(|application| application.storage() == storage),
                );
            }
            assert_eq!(
                load_layout(resolved.envelope())
                    .retained_image()
                    .memory
                    .text,
                original_text,
                "the retained FinalImage must remain immutable",
            );
        }
    }

    #[test]
    fn exact_replay_is_deterministic_and_target_or_import_bound() {
        let first =
            apply_elf_procedure_linkage_fixups(standard_envelope(TargetProfile::LinuxX64)).unwrap();
        let replay =
            apply_elf_procedure_linkage_fixups(standard_envelope(TargetProfile::LinuxX64)).unwrap();
        let target_change =
            apply_elf_procedure_linkage_fixups(standard_envelope(TargetProfile::LinuxArm64))
                .unwrap();
        let import_change = apply_elf_procedure_linkage_fixups(envelope(
            TargetProfile::LinuxX64,
            [b"alpha_call", b"gamma_call"],
        ))
        .unwrap();
        assert_eq!(first.source_text_bytes(), replay.source_text_bytes());
        assert_eq!(
            first.procedure_linkage_bytes(),
            replay.procedure_linkage_bytes(),
        );
        assert_eq!(first.applied_fixups(), replay.applied_fixups());
        assert_eq!(
            first.non_authoritative_resolved_linkage_compatibility_fingerprint(),
            replay.non_authoritative_resolved_linkage_compatibility_fingerprint(),
        );
        assert_ne!(
            first.non_authoritative_resolved_linkage_compatibility_fingerprint(),
            target_change.non_authoritative_resolved_linkage_compatibility_fingerprint(),
        );
        assert_ne!(
            first.non_authoritative_resolved_linkage_compatibility_fingerprint(),
            import_change.non_authoritative_resolved_linkage_compatibility_fingerprint(),
        );
    }

    #[test]
    fn byte_opcode_address_and_ledger_substitutions_reject_with_custody() {
        let mut mutable_field = candidate(TargetProfile::LinuxX64);
        mutable_field.contents.source_text_bytes[1] ^= 1;
        refresh_fingerprint(&mut mutable_field);
        assert_rejected_with_custody(mutable_field);

        let mut fixed_opcode = candidate(TargetProfile::LinuxX64);
        fixed_opcode.contents.source_text_bytes[0] ^= 1;
        refresh_fingerprint(&mut fixed_opcode);
        assert_rejected_with_custody(fixed_opcode);

        let mut target = candidate(TargetProfile::LinuxX64);
        target.contents.applications[0].target_address += 4;
        refresh_fingerprint(&mut target);
        assert_rejected_with_custody(target);

        let mut source = candidate(TargetProfile::LinuxX64);
        source.contents.applications[0].source_address += 4;
        refresh_fingerprint(&mut source);
        assert_rejected_with_custody(source);

        let mut kind = candidate(TargetProfile::LinuxX64);
        kind.contents.applications[0].kind = ElfAppliedProcedureLinkageKind::Absolute64;
        refresh_fingerprint(&mut kind);
        assert_rejected_with_custody(kind);
    }

    #[test]
    fn missing_duplicate_reordered_truncated_and_report_drift_reject_with_custody() {
        let mut missing = candidate(TargetProfile::LinuxArm64);
        missing.contents.applications.pop();
        refresh_fingerprint(&mut missing);
        assert_rejected_with_custody(missing);

        let mut duplicate = candidate(TargetProfile::LinuxArm64);
        duplicate.contents.applications[1] = duplicate.contents.applications[0];
        refresh_fingerprint(&mut duplicate);
        assert_rejected_with_custody(duplicate);

        let mut reordered = candidate(TargetProfile::LinuxArm64);
        reordered.contents.applications.swap(0, 1);
        refresh_fingerprint(&mut reordered);
        assert_rejected_with_custody(reordered);

        let mut truncated = candidate(TargetProfile::LinuxArm64);
        truncated.contents.procedure_linkage_bytes.pop();
        refresh_fingerprint(&mut truncated);
        assert_rejected_with_custody(truncated);

        let mut zero = candidate(TargetProfile::LinuxArm64);
        zero.non_authoritative_resolved_linkage_compatibility_fingerprint = 0;
        assert_rejected_with_custody(zero);

        let mut drifted = candidate(TargetProfile::LinuxArm64);
        drifted.non_authoritative_resolved_linkage_compatibility_fingerprint ^= 1;
        assert_rejected_with_custody(drifted);
    }

    #[test]
    fn range_alignment_mask_and_overlap_helpers_fail_closed() {
        assert!(
            encode_field(
                ElfProcedureLinkageFixupKind::X86PcRelative32,
                0,
                0xffff_ffff,
                0,
                i32::MAX as u64 + 5,
            )
            .is_err(),
        );
        assert!(
            encode_field(
                ElfProcedureLinkageFixupKind::Aarch64Branch26,
                0x9400_0000,
                0x03ff_ffff,
                0x1000,
                0x1002,
            )
            .is_err(),
        );
        assert!(
            encode_field(
                ElfProcedureLinkageFixupKind::Aarch64Branch26,
                0x9400_0000,
                0x03ff_ffff,
                0,
                1 << 27,
            )
            .is_err(),
        );
        assert!(
            encode_field(
                ElfProcedureLinkageFixupKind::Aarch64Page21,
                0x9000_0010,
                0x60ff_ffe0,
                0,
                1_u64 << 32,
            )
            .is_err(),
        );
        assert!(
            encode_field(
                ElfProcedureLinkageFixupKind::Aarch64Load64Low12,
                0xf940_0211,
                0x003f_fc00,
                0,
                3,
            )
            .is_err(),
        );
        assert!(
            encode_field(
                ElfProcedureLinkageFixupKind::Absolute64,
                0,
                0xffff,
                0,
                1 << 20,
            )
            .is_err(),
        );

        let first = candidate(TargetProfile::LinuxX64).contents.applications[0];
        let mut overlapping = first;
        overlapping.ordinal += 1;
        assert!(validate_nonoverlapping_applications(&[first, overlapping]).is_err());
    }
}
