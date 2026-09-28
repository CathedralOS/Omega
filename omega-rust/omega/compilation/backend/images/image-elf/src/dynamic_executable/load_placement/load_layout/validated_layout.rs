//! The validated dynamic load layout and its error.

use crate::dynamic_executable::load_placement::load_layout::abi_validation::retained_image;
use crate::dynamic_executable::load_placement::load_layout::{
    ElfLoadImageMemoryPlacement, ElfLoadProgramHeader, ElfPlacedDynamicSection,
    ElfPlacedDynamicSectionKind, ElfResolvedSectionHeaderPlacement,
};
use crate::dynamic_executable::section_headers::relative_section_layout::ValidatedElfRelativeSectionPayloadLayout;
use diagnostics::Diagnostic;
use target::TargetProfile;

/// Replayed absolute geometry retaining the exact relative-layout owner.
#[derive(Debug)]
#[must_use = "validated dynamic ELF load layout retains all upstream payload custody"]
pub struct ValidatedElfDynamicLoadLayout {
    pub(crate) relative: ValidatedElfRelativeSectionPayloadLayout,
    pub(crate) target: TargetProfile,
    pub(crate) image_base: u64,
    pub(crate) max_page_alignment: u64,
    pub(crate) program_headers: Vec<ElfLoadProgramHeader>,
    pub(crate) image_memory: ElfLoadImageMemoryPlacement,
    pub(crate) sections: Vec<ElfPlacedDynamicSection>,
    pub(crate) section_header_table_file_offset: u64,
    pub(crate) section_header_resolutions: Vec<ElfResolvedSectionHeaderPlacement>,
    pub(crate) non_authoritative_layout_compatibility_fingerprint: u64,
}

impl ValidatedElfDynamicLoadLayout {
    pub const fn relative(&self) -> &ValidatedElfRelativeSectionPayloadLayout {
        &self.relative
    }

    pub const fn target(&self) -> TargetProfile {
        self.target
    }

    pub const fn image_base(&self) -> u64 {
        self.image_base
    }

    pub const fn max_page_alignment(&self) -> u64 {
        self.max_page_alignment
    }

    pub fn program_headers(&self) -> &[ElfLoadProgramHeader] {
        &self.program_headers
    }

    pub const fn image_memory(&self) -> &ElfLoadImageMemoryPlacement {
        &self.image_memory
    }

    /// Address observation required by later source relocation replay.
    pub const fn final_image_layout(&self) -> image::FinalImageLayout {
        image::FinalImageLayout {
            text_address: self.image_memory.text_virtual_address,
            data_address: self.image_memory.data_virtual_address,
            bss_address: self.image_memory.bss_virtual_address,
        }
    }

    /// Crate-private custody access for later exact ELF serialization rungs.
    /// Public consumers must not bypass the validated layout carrier to recover
    /// the mutable source image.
    pub(crate) fn retained_image(&self) -> &image::FinalImage {
        retained_image(&self.relative)
    }

    pub fn sections(&self) -> &[ElfPlacedDynamicSection] {
        &self.sections
    }

    pub fn section_header_resolutions(&self) -> &[ElfResolvedSectionHeaderPlacement] {
        &self.section_header_resolutions
    }

    pub const fn section_header_table_file_offset(&self) -> u64 {
        self.section_header_table_file_offset
    }

    pub fn section_header_table_byte_size(&self) -> usize {
        self.relative.payloads().section_headers().byte_count()
    }

    pub fn dynamic_section(&self) -> &ElfPlacedDynamicSection {
        &self.sections[ElfPlacedDynamicSectionKind::DynamicTable as usize]
    }

    /// Compatibility fingerprint only; this is not a runnable-image identity.
    pub const fn non_authoritative_layout_compatibility_fingerprint(&self) -> u64 {
        self.non_authoritative_layout_compatibility_fingerprint
    }

    pub(crate) fn into_relative(self) -> ValidatedElfRelativeSectionPayloadLayout {
        self.relative
    }
}

/// Rejected absolute placement with exact relative-layout custody.
#[derive(Debug)]
#[must_use = "dynamic ELF load-layout rejection retains the relative layout"]
pub struct ElfDynamicLoadLayoutError {
    pub(crate) relative: ValidatedElfRelativeSectionPayloadLayout,
    pub(crate) diagnostic: Diagnostic,
}

impl ElfDynamicLoadLayoutError {
    pub const fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (ValidatedElfRelativeSectionPayloadLayout, Diagnostic) {
        (self.relative, self.diagnostic)
    }
}

impl std::fmt::Display for ElfDynamicLoadLayoutError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for ElfDynamicLoadLayoutError {}
