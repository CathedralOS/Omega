//! Applied procedure linkage fixups, the validated linkage and its
//! application error.

use crate::dynamic_executable::file_assembly::dynamic_file_envelope::ValidatedElfDynamicFileEnvelope;
use crate::dynamic_executable::file_assembly::resolved_procedure_linkage::candidates::ElfResolvedProcedureLinkageContents;
use diagnostics::Diagnostic;

/// Exact byte owner receiving one applied procedure-linkage fixup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ElfAppliedProcedureLinkageStorage {
    SourceText = 1,
    ProcedureLinkage = 2,
    ProcedureGot = 3,
    ProcedureRelocation = 4,
    GeneralRelocation = 5,
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
    ProcedureLinkageEntry {
        logical_ordinal: u32,
    },
    ProcedureLinkageLazyTail {
        logical_ordinal: u32,
    },
    ProcedureGotHeaderWord {
        word_index: u8,
    },
    ProcedureGotSlot {
        logical_ordinal: u32,
    },
    /// Placed address of one retained source-image section slot: the resolved
    /// `r_offset` written into a `.rela.dyn` row.
    RelocatedImageSection {
        section: image::FinalImageSection,
        byte_offset: usize,
    },
}

/// One exact application retained beside the resulting fragment bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ElfAppliedProcedureLinkageFixup {
    pub(crate) ordinal: u32,
    pub(crate) storage: ElfAppliedProcedureLinkageStorage,
    pub(crate) byte_offset: usize,
    pub(crate) byte_width: u8,
    pub(crate) mutable_mask: u64,
    pub(crate) kind: ElfAppliedProcedureLinkageKind,
    pub(crate) target: ElfAppliedProcedureLinkageTarget,
    pub(crate) source_address: u64,
    pub(crate) target_address: u64,
    pub(crate) encoded_field: u64,
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
    pub(crate) envelope: ValidatedElfDynamicFileEnvelope,
    pub(crate) contents: ElfResolvedProcedureLinkageContents,
    pub(crate) non_authoritative_resolved_linkage_compatibility_fingerprint: u64,
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

    pub fn general_relocation_bytes(&self) -> &[u8] {
        &self.contents.general_relocation_bytes
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
    pub(crate) envelope: ValidatedElfDynamicFileEnvelope,
    pub(crate) diagnostic: Diagnostic,
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
