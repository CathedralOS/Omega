//! Target-specific address-free GOT/PLT and procedure-relocation templates.
//!
//! This module selects the baseline lazy-binding sequences specified by the
//! [x86-64 psABI] and the AArch64 [System V ABI]. It serializes only fixed
//! opcodes, ordinals, symbol/type coordinates, and zero placeholders. Every
//! placement-dependent field remains covered by a typed fixup and constraint;
//! these are templates, not final section payloads or runnable-image bytes.
//!
//! [x86-64 psABI]: https://gitlab.com/x86-psABIs/x86-64-ABI/-/blob/master/x86-64-ABI/dl.tex
//! [System V ABI]: https://github.com/ARM-software/abi-aa/blob/main/sysvabi64/sysvabi64.rst#procedure-linkage-table
//!
//! This file owns the template constants and the planning entry point.
//! `template_plans.rs` carries the validated plan, contents, fixups and
//! constraints, `candidates.rs` derives candidate contents, bytes, fixups
//! and constraints, `template_validation.rs` validates candidates against
//! the fixed templates, `compatibility_fingerprint.rs` derives the
//! report-only fingerprint and `tests.rs` holds the template tests.

mod candidates;
mod compatibility_fingerprint;
mod template_plans;
mod template_validation;
#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) use template_plans::{ElfProcedureLinkageFixup, ElfProcedureLinkageTemplatePolicy};
pub(crate) use template_plans::{
    ElfProcedureLinkageFixupKind, ElfProcedureLinkageFixupStorage,
    ElfProcedureLinkagePlacementConstraint, ElfProcedureLinkagePlacementConstraintKind,
    ElfProcedureLinkageSemanticTarget,
};
pub use template_plans::{
    ElfProcedureLinkageTemplatePlanningError, ValidatedElfProcedureLinkageTemplatePlan,
};

use crate::dynamic_executable::procedure_linkage::dynamic_import_relocations::{ValidatedElfProcedureLinkageRelocationPlan};
use crate::dynamic_executable::procedure_linkage::dynamic_linkage_templates::compatibility_fingerprint::non_authoritative_template_compatibility_fingerprint;
use crate::dynamic_executable::procedure_linkage::dynamic_linkage_templates::candidates::{Candidate, derive_contents};
use crate::dynamic_executable::procedure_linkage::dynamic_linkage_templates::template_validation::validate_candidate;

const ELF64_RELA_SIZE: usize = 24;

const ELF64_GOT_WORD_SIZE: usize = 8;

const X86_PLT_HEADER_SIZE: usize = 16;

const X86_PLT_ENTRY_SIZE: usize = 16;

const AARCH64_PLT_HEADER_SIZE: usize = 32;

const AARCH64_PLT_ENTRY_SIZE: usize = 16;

const GOT_PLT_HEADER_WORDS: usize = 3;

const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;

const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

const X86_PLT_HEADER: [u8; X86_PLT_HEADER_SIZE] = [
    0xff, 0x35, 0, 0, 0, 0, // pushq GOT[1](%rip)
    0xff, 0x25, 0, 0, 0, 0, // jmpq *GOT[2](%rip)
    0x0f, 0x1f, 0x40, 0x00, // nopl 0(%rax)
];

const AARCH64_PLT_HEADER: [u8; AARCH64_PLT_HEADER_SIZE] = [
    0xf0, 0x7b, 0xbf, 0xa9, // stp x16, x30, [sp,#-16]!
    0x10, 0x00, 0x00, 0x90, // adrp x16, .got.plt[2]
    0x11, 0x02, 0x40, 0xf9, // ldr x17, [x16, :lo12:.got.plt[2]]
    0x10, 0x02, 0x00, 0x91, // add x16, x16, :lo12:.got.plt[2]
    0x20, 0x02, 0x1f, 0xd6, // br x17
    0x1f, 0x20, 0x03, 0xd5, // nop
    0x1f, 0x20, 0x03, 0xd5, // nop
    0x1f, 0x20, 0x03, 0xd5, // nop
];

const AARCH64_PLT_ENTRY: [u8; AARCH64_PLT_ENTRY_SIZE] = [
    0x10, 0x00, 0x00, 0x90, // adrp x16, .got.plt[N + 3]
    0x11, 0x02, 0x40, 0xf9, // ldr x17, [x16, :lo12:.got.plt[N + 3]]
    0x10, 0x02, 0x00, 0x91, // add x16, x16, :lo12:.got.plt[N + 3]
    0x20, 0x02, 0x1f, 0xd6, // br x17
];

/// Consume exact semantic procedure linkage into fixed target template bytes,
/// typed fixups, and deferred placement constraints.
///
/// All placement-dependent fields remain exact zero placeholders. This does
/// not serialize final GOT/PLT/RELA payloads, resolve a fixup, assign a section
/// index or address, mutate the image, or grant runnable-image authority.
pub fn plan_elf_procedure_linkage_templates(
    linkage: ValidatedElfProcedureLinkageRelocationPlan,
) -> Result<ValidatedElfProcedureLinkageTemplatePlan, Box<ElfProcedureLinkageTemplatePlanningError>>
{
    let contents = match derive_contents(&linkage) {
        Ok(contents) => contents,
        Err(diagnostic) => {
            return Err(Box::new(ElfProcedureLinkageTemplatePlanningError {
                linkage,
                diagnostic,
            }));
        }
    };
    let non_authoritative_template_compatibility_fingerprint =
        non_authoritative_template_compatibility_fingerprint(&linkage, &contents);
    let candidate = Candidate {
        linkage,
        contents,
        non_authoritative_template_compatibility_fingerprint,
    };
    match validate_candidate(candidate) {
        Ok(validated) => Ok(validated),
        Err(error) => Err(Box::new(ElfProcedureLinkageTemplatePlanningError {
            linkage: error.candidate.linkage,
            diagnostic: error.diagnostic,
        })),
    }
}
