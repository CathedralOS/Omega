//! Validated procedure linkage template plans, contents, fixups and
//! placement constraints.

use crate::dynamic_executable::procedure_linkage::dynamic_import_relocations::ValidatedElfProcedureLinkageRelocationPlan;
use diagnostics::Diagnostic;
use image::FinalImageSection;

/// Independently validated fixed template bytes and semantic fixups for the
/// exact address-free procedure-linkage plan.
///
/// The retained upstream plan remains the sole owner of import semantics and
/// source-call custody. No template field is a final address, section index,
/// placed byte, or image-mutation capability.
#[derive(Debug)]
#[must_use = "validated GOT/PLT templates retain the exact linkage plan"]
pub struct ValidatedElfProcedureLinkageTemplatePlan {
    pub(crate) linkage: ValidatedElfProcedureLinkageRelocationPlan,
    pub(crate) contents: ElfProcedureLinkageTemplateContents,
    pub(crate) non_authoritative_template_compatibility_fingerprint: u64,
}

impl ValidatedElfProcedureLinkageTemplatePlan {
    pub const fn linkage(&self) -> &ValidatedElfProcedureLinkageRelocationPlan {
        &self.linkage
    }

    pub fn procedure_linkage_byte_count(&self) -> usize {
        self.contents.bytes.plt.len()
    }

    pub fn procedure_got_byte_count(&self) -> usize {
        self.contents.bytes.got_plt.len()
    }

    pub fn procedure_relocation_byte_count(&self) -> usize {
        self.contents.bytes.rela_plt.len()
    }

    pub fn general_relocation_byte_count(&self) -> usize {
        self.contents.bytes.rela_dyn.len()
    }

    pub fn fixup_count(&self) -> usize {
        self.contents.fixups.len()
    }

    pub fn placement_constraint_count(&self) -> usize {
        self.contents.constraints.len()
    }

    /// Compatibility fingerprint of the upstream linkage identity, exact
    /// target policy, fixed template bytes, semantic fixups, and deferred
    /// placement constraints. This is not final-byte or loader identity.
    pub const fn non_authoritative_template_compatibility_fingerprint(&self) -> u64 {
        self.non_authoritative_template_compatibility_fingerprint
    }

    pub(crate) const fn contents(&self) -> &ElfProcedureLinkageTemplateContents {
        &self.contents
    }

    #[allow(dead_code)]
    pub(crate) fn into_parts(
        self,
    ) -> (
        ValidatedElfProcedureLinkageRelocationPlan,
        ElfProcedureLinkageTemplateContents,
    ) {
        (self.linkage, self.contents)
    }
}

/// Rejected GOT/PLT template planning with exact upstream custody.
#[derive(Debug)]
#[must_use = "ELF GOT/PLT template rejection retains the procedure-linkage plan"]
pub struct ElfProcedureLinkageTemplatePlanningError {
    pub(crate) linkage: ValidatedElfProcedureLinkageRelocationPlan,
    pub(crate) diagnostic: Diagnostic,
}

impl ElfProcedureLinkageTemplatePlanningError {
    pub const fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (ValidatedElfProcedureLinkageRelocationPlan, Diagnostic) {
        (self.linkage, self.diagnostic)
    }
}

impl std::fmt::Display for ElfProcedureLinkageTemplatePlanningError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for ElfProcedureLinkageTemplatePlanningError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ElfProcedureLinkageTemplateContents {
    pub(crate) policy: ElfProcedureLinkageTemplatePolicy,
    pub(crate) bytes: ElfProcedureLinkageTemplateBytes,
    pub(crate) fixups: Vec<ElfProcedureLinkageFixup>,
    pub(crate) constraints: Vec<ElfProcedureLinkagePlacementConstraint>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum ElfProcedureLinkageTemplatePolicy {
    X86_64SmallMediumLazy = 1,
    Aarch64StandardLazy = 2,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ElfProcedureLinkageTemplateBytes {
    pub(crate) plt: Vec<u8>,
    pub(crate) got_plt: Vec<u8>,
    pub(crate) rela_plt: Vec<u8>,
    pub(crate) rela_dyn: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ElfProcedureLinkageFixup {
    pub(crate) storage: ElfProcedureLinkageFixupStorage,
    pub(crate) byte_offset: usize,
    pub(crate) byte_width: u8,
    pub(crate) mutable_mask: u64,
    pub(crate) kind: ElfProcedureLinkageFixupKind,
    pub(crate) target: ElfProcedureLinkageSemanticTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum ElfProcedureLinkageFixupStorage {
    SourceText = 1,
    Plt = 2,
    GotPlt = 3,
    RelaPlt = 4,
    RelaDyn = 5,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum ElfProcedureLinkageFixupKind {
    X86PcRelative32 = 1,
    Aarch64Page21 = 2,
    Aarch64Load64Low12 = 3,
    Aarch64AddLow12 = 4,
    Aarch64Branch26 = 5,
    Absolute64 = 6,
    Elf64RelaOffset = 7,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ElfProcedureLinkageSemanticTarget {
    FutureDynamicSection,
    PltHeader,
    PltEntry {
        logical_ordinal: u32,
    },
    PltLazyTail {
        logical_ordinal: u32,
    },
    GotPltHeaderWord {
        word_index: u8,
    },
    GotPltSlot {
        logical_ordinal: u32,
    },
    /// One retained source-image section plus its section-relative slot
    /// offset: the future `r_offset` of a general `.rela.dyn` row.
    RelocatedImageSection {
        section: FinalImageSection,
        byte_offset: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ElfProcedureLinkagePlacementConstraint {
    pub(crate) fixup_ordinal: u32,
    pub(crate) kind: ElfProcedureLinkagePlacementConstraintKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum ElfProcedureLinkagePlacementConstraintKind {
    X86Signed32 = 1,
    Aarch64PageDelta21 = 2,
    Aarch64Load64Low12Aligned = 3,
    Aarch64Branch26 = 4,
}
