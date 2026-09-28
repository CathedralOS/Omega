//! Absolute file and virtual placement for the closed dynamic ELF roster.
//!
//! This layer consumes relative permission-domain packing, binds it to the
//! exact retained Linux target, and closes the current five-program-header
//! geometry (`PT_INTERP`, R/RX/RW `PT_LOAD`, and `PT_DYNAMIC`). It also
//! resolves all twenty-five section-header placement obligations as retained
//! values. It does not write those values into the section-header template,
//! resolve payload-internal fixups, mutate image bytes, serialize program
//! headers, or claim runnable-ELF authority. The section-header template gets
//! a file-only coordinate, but its bytes remain unchanged.
//!
//! This file owns the layout constants and the planning entry point.
//! `load_headers.rs` carries program headers, placed sections and memory
//! placements, `validated_layout.rs` the validated layout and its error,
//! `candidates.rs` candidate derivation and validation, `abi_validation.rs`
//! ABI and deferred constraint validation, `compatibility_fingerprint.rs`
//! the report-only compatibility fingerprint and `tests.rs` the layout
//! tests.

mod abi_validation;
mod candidates;
mod compatibility_fingerprint;
mod load_headers;
#[cfg(test)]
mod tests;
mod validated_layout;

pub use load_headers::{
    ElfLoadImageMemoryPlacement, ElfLoadProgramHeader, ElfLoadProgramHeaderKind,
    ElfPlacedDynamicSection, ElfPlacedDynamicSectionKind, ElfResolvedSectionHeaderPlacement,
    ElfSectionPlacementResolutionKind,
};
pub use validated_layout::{ElfDynamicLoadLayoutError, ValidatedElfDynamicLoadLayout};

use crate::constants::IMAGE_BASE;
use crate::dynamic_executable::load_placement::load_layout::abi_validation::retained_target;
use crate::dynamic_executable::load_placement::load_layout::candidates::Candidate;
use crate::dynamic_executable::load_placement::load_layout::candidates::{
    derive_contents, validate_candidate,
};
use crate::dynamic_executable::load_placement::load_layout::compatibility_fingerprint::non_authoritative_layout_compatibility_fingerprint;
use crate::dynamic_executable::section_headers::relative_section_layout::ValidatedElfRelativeSectionPayloadLayout;

const SECTION_COUNT: usize = 14;

const PLACEMENT_FIXUP_COUNT: usize = 25;

const DYNAMIC_PROGRAM_HEADER_COUNT: u64 = 5;

const DYNAMIC_MAX_PAGE_SIZE: u64 = 0x1_0000;

const DYNAMIC_LOAD_POLICY_TAG: u8 = 1;

const AARCH64_RELOCATION_PAGE_SIZE: u64 = 0x1000;

const PF_X: u32 = 1;

const PF_W: u32 = 2;

const PF_R: u32 = 4;

const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;

const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Close absolute load and section geometry for the retained Linux target.
///
/// Both admitted profiles use a target-derived 64 KiB maximum-page policy.
/// This is compatible with the AArch64 ELF ABI recommendation and with x86-64
/// Linux loaders, while preserving `p_offset == p_vaddr (mod p_align)`.
pub fn plan_elf_dynamic_load_layout(
    relative: ValidatedElfRelativeSectionPayloadLayout,
) -> Result<ValidatedElfDynamicLoadLayout, Box<ElfDynamicLoadLayoutError>> {
    let target = retained_target(&relative);
    let (
        program_headers,
        image_memory,
        sections,
        section_header_table_file_offset,
        section_header_resolutions,
    ) = match derive_contents(&relative, target) {
        Ok(contents) => contents,
        Err(diagnostic) => {
            return Err(Box::new(ElfDynamicLoadLayoutError {
                relative,
                diagnostic,
            }));
        }
    };
    let image_base = IMAGE_BASE;
    let max_page_alignment = DYNAMIC_MAX_PAGE_SIZE;
    let non_authoritative_layout_compatibility_fingerprint =
        non_authoritative_layout_compatibility_fingerprint(
            &relative,
            target,
            image_base,
            max_page_alignment,
            &program_headers,
            &image_memory,
            &sections,
            section_header_table_file_offset,
            &section_header_resolutions,
        );
    let candidate = Candidate {
        relative,
        target,
        image_base,
        max_page_alignment,
        program_headers,
        image_memory,
        sections,
        section_header_table_file_offset,
        section_header_resolutions,
        non_authoritative_layout_compatibility_fingerprint,
    };
    validate_candidate(candidate).map_err(|error| {
        Box::new(ElfDynamicLoadLayoutError {
            relative: error.candidate.relative,
            diagnostic: error.diagnostic,
        })
    })
}
