//! Exact application of dynamic ELF source relocations and procedure-linkage
//! fixups.
//!
//! This rung consumes the exact dynamic file-envelope owner, copies the
//! retained source `.text` and `.data` and the `.plt`, `.got.plt`, and
//! `.rela.plt` templates, patches every image-local source relocation (one
//! whose symbol the image defines) at the already-validated absolute load
//! layout, and then applies every indexed procedure fixup. A separate replay
//! re-derives the image-local relocated memory and checks every application,
//! target, encoding, mutable mask, unchanged byte, range, and alignment
//! against it and the complete upstream custody chain.
//!
//! These resolved byte regions remain fragments. This layer does not place
//! them into one file, mutate the retained `FinalImage`,
//! publish bytes, or grant loader or runnable-image authority.
//!
//! This file owns the linkage constants and the fixup entry point.
//! `image_local_relocations.rs` patches the image's own relocations,
//! `linkage_fixups.rs` carries applied fixups, the validated linkage and
//! its error, `candidates.rs` derives and validates candidates,
//! `storage_fields.rs` reads and writes storage fields,
//! `compatibility_fingerprint.rs` derives the report-only fingerprint and
//! `tests.rs` holds the linkage tests.

mod candidates;
mod compatibility_fingerprint;
mod image_local_relocations;
mod linkage_fixups;
mod storage_fields;
#[cfg(test)]
mod tests;

pub use linkage_fixups::{
    ElfAppliedProcedureLinkageFixup, ElfAppliedProcedureLinkageKind,
    ElfAppliedProcedureLinkageStorage, ElfAppliedProcedureLinkageTarget,
    ElfProcedureLinkageApplicationError, ValidatedElfResolvedProcedureLinkage,
};

use crate::dynamic_executable::file_assembly::dynamic_file_envelope::ValidatedElfDynamicFileEnvelope;
use crate::dynamic_executable::file_assembly::resolved_procedure_linkage::compatibility_fingerprint::non_authoritative_resolved_linkage_compatibility_fingerprint;
use crate::dynamic_executable::file_assembly::resolved_procedure_linkage::candidates::{derive_contents, Candidate, validate_candidate};

const X86_PLT_HEADER_SIZE: u64 = 16;

const AARCH64_PLT_HEADER_SIZE: u64 = 32;

const PROCEDURE_LINKAGE_ENTRY_SIZE: u64 = 16;

const X86_PLT_LAZY_TAIL_OFFSET: u64 = 6;

const GOT_PLT_HEADER_WORDS: u64 = 3;

const ELF64_GOT_WORD_SIZE: u64 = 8;

/// Patch every image-local source relocation, then apply every exact indexed
/// procedure/source fixup into copied fragments.
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
