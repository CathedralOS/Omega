//! Two ELF emitters in one crate. About 800 lines of static emitter that ships,
//! and 22,000 lines of dynamic-linking pipeline that a different caller drives.
//!
//! `emit_elf_aarch64_executable` and `emit_elf_x86_64_executable` are the whole
//! static lane, and they are deliberately small. The output has a 64-byte ELF
//! header, exactly two program headers, and NO section headers at all:
//!
//! ```text
//!   0x000000  ELF header (64) + 2 program headers (56 each) = 176 bytes
//!   0x001000  .text   at the first PAGE_SIZE boundary, vaddr 0x401000
//!   0x00N000  .data   at the next boundary, .bss aligned after it
//!   IMAGE_BASE = 0x400000, PAGE_SIZE = 0x1000, PROGRAM_HEADER_COUNT = 2
//! ```
//!
//! The container is architecture-agnostic apart from `e_machine` (62 for
//! x86-64, 183 for AArch64) and which relocation applier is passed in, which is
//! why one private `emit_elf_executable` serves both.
//!
//! That lane FAILS CLOSED the moment it sees a referenced import: any surviving
//! entry from `canonical_referenced_imports` is an immediate error before the
//! image is mutated at all. A statically emitted ELF here is one that needs no
//! loader.
//!
//! Everything else in the crate is the dynamic lane: `emit_elf_dynamic_executable`
//! in `dynamic_executable.rs` drives a linear chain of 22 stages:
//!
//! ```text
//!   plan_elf_dynamic_link_inputs -> plan_elf_dynamic_sections
//!   -> serialize_elf_dynamic_sections -> plan_elf_dynamic_section_descriptors
//!   -> plan_elf_procedure_linkage_relocations
//!   -> plan_elf_procedure_linkage_templates
//!   -> plan_elf_procedure_linkage_section_descriptors
//!   -> plan_elf_dynamic_tags -> serialize_elf_dynamic_table
//!   -> plan_elf_dynamic_table_section_descriptor -> plan_elf_section_name_table
//!   -> plan_elf_dynamic_section_roster -> serialize_elf_section_header_table
//!   -> plan_elf_indexed_section_payloads
//!   -> plan_elf_relative_section_payload_layout -> plan_elf_dynamic_load_layout
//!   -> apply_elf_section_header_placements -> apply_elf_dynamic_address_fixups
//!   -> serialize_elf_dynamic_file_envelope -> apply_elf_procedure_linkage_fixups
//!   -> assemble_elf_dynamic_file -> admit_elf_dynamic_executable
//! ```
//!
//! Each stage takes the previous stage's `ValidatedElf*` carrier as its only
//! input. Not by convention - there is no way to call stage N without holding a
//! value that only stage N-1 can construct.
//!
//! The three page-size constants look redundant and are not. The static lane
//! writes `p_align` of `PAGE_SIZE` (0x1000). The dynamic lane aligns segments to
//! `DYNAMIC_MAX_PAGE_SIZE` (0x1_0000), because AArch64 permits translation
//! granules up to 64 KiB and a segment aligned only to 4 KiB is not portable
//! across them. `AARCH64_RELOCATION_PAGE_SIZE` stays 0x1000 in the same file
//! because ADRP's page is 4 KiB regardless of what the loader maps with. Two of
//! the three are numerically equal and mean different things.

//! One carrier type and one error enum per stage - 22 of each - instead of one
//! `ElfDynamicImage` struct that every stage fills in a little more. The cost is
//! roughly forty types that exist only to be passed once. What it buys is that
//! running the stages out of order, or running one on half-planned input, is not
//! something a caller can express: there is no constructor for stage N's input
//! except stage N-1's success. A mutable shared struct would move all 22
//! ordering constraints into review comments.
//!
//! The static emitter refuses an import rather than emitting an image that might
//! load. A best-effort static link - resolve what we can, leave the rest - would
//! produce a file that runs until it reaches the unbound call, which is the
//! failure mode hardest to attribute back to the compiler.

//! `image-emission/src/dynamic_elf.rs` is the only caller of the dynamic
//! lane's entry; `image` supplies `FinalImage`, `place_executable_regions`
//! and the relocation appliers both lanes use.
//!
//! @Note: do not decide what is dead in this crate by grepping for type names.
//! The chain binds every stage result with an inferred `let` and never spells a
//! carrier type, so 21 of the 22 carriers have zero occurrences of their names
//! anywhere outside this crate while being entirely load-bearing -
//! `ValidatedElfDynamicLoadLayout` among them, which is the return type of a
//! stage the chain calls in production. A scouting pass over this crate
//! called 18 such types dead on exactly that evidence.
//!
//! @Incomplete: the static lane's own error message is out of date and says so
//! in the most misleading possible place - the text a user sees. It claims
//! "no target-owned ELF loader plan carries the exact PT_INTERP bytes". One
//! does: `target::NormalizedElfInterpreterPlan::interpreter_path` carries
//! exactly those bytes, `plan_elf_dynamic_link_inputs` takes that plan as its
//! second argument, and `dynamic_file_envelope.rs` maps
//! `ElfLoadProgramHeaderKind::Interpreter` to `PT_INTERP` (3). The claim in the
//! other direction is stale too: `target/src/elf_loader.rs` says dynamic
//! ELF emission "remains unavailable until a later owner joins this input to the
//! complete dynamic-link structures", and that owner is this crate. Each side
//! documents the other's absence while the other is present. Fix them together
//! or not at all.
//!
//! On disk the crate splits the same way: `static_executable.rs` is the
//! static lane, `dynamic_executable.rs` owns the 22-stage dynamic lane's
//! entry and groups its stages by phase, and `bytes.rs`, `constants.rs`,
//! `entry_symbol.rs` and `imports.rs` hold what both lanes share.

mod bytes;
mod constants;
mod dynamic_executable;
mod entry_symbol;
mod imports;
mod static_executable;

pub use dynamic_executable::{ElfDynamicExecutableEmissionError, emit_elf_dynamic_executable};
pub use static_executable::{emit_elf_aarch64_executable, emit_elf_x86_64_executable};

pub use dynamic_executable::dynamic_table::dynamic_table_descriptor::{
    ElfDynamicTableSectionDescriptorPlanningError, ValidatedElfDynamicTableSectionDescriptorPlan,
    plan_elf_dynamic_table_section_descriptor,
};
pub use dynamic_executable::dynamic_table::dynamic_tag_bytes::{
    ElfDynamicTableSerializationError, ValidatedElfDynamicTablePayload, serialize_elf_dynamic_table,
};
pub use dynamic_executable::dynamic_table::dynamic_tags::{
    ElfDynamicTagPlanningError, ValidatedElfDynamicTagPlan, plan_elf_dynamic_tags,
};
pub use dynamic_executable::file_assembly::assembled_dynamic_file::{
    ElfDynamicExecutableAdmissionError, ElfDynamicFileAssemblyError, ElfDynamicFileFragmentKind,
    ElfDynamicFileFragmentPlacement, ValidatedElfAssembledDynamicFile,
    ValidatedElfDynamicExecutable, admit_elf_dynamic_executable, assemble_elf_dynamic_file,
};
pub use dynamic_executable::file_assembly::dynamic_file_envelope::{
    ElfDynamicFileEnvelopeSerializationError, ValidatedElfDynamicFileEnvelope,
    serialize_elf_dynamic_file_envelope,
};
pub use dynamic_executable::file_assembly::resolved_procedure_linkage::{
    ElfAppliedProcedureLinkageFixup, ElfAppliedProcedureLinkageKind,
    ElfAppliedProcedureLinkageStorage, ElfAppliedProcedureLinkageTarget,
    ElfProcedureLinkageApplicationError, ValidatedElfResolvedProcedureLinkage,
    apply_elf_procedure_linkage_fixups,
};
pub use dynamic_executable::import_sections::dynamic_link::{
    ElfDynamicLinkInputPlanningError, PlannedElfDynamicLinkInputs, plan_elf_dynamic_link_inputs,
};
pub use dynamic_executable::import_sections::dynamic_section_bytes::{
    ElfDynamicSectionSerializationError, ValidatedElfDynamicSectionPayloads,
    serialize_elf_dynamic_sections,
};
pub use dynamic_executable::import_sections::dynamic_section_descriptors::{
    ElfDynamicSectionDescriptorPlanningError, ValidatedElfDynamicSectionDescriptorPlan,
    plan_elf_dynamic_section_descriptors,
};
pub use dynamic_executable::import_sections::dynamic_sections::{
    ElfDynamicSectionPlanningError, ValidatedElfDynamicSectionPlan, plan_elf_dynamic_sections,
};
pub use dynamic_executable::load_placement::load_layout::{
    ElfDynamicLoadLayoutError, ElfLoadImageMemoryPlacement, ElfLoadProgramHeader,
    ElfLoadProgramHeaderKind, ElfPlacedDynamicSection, ElfPlacedDynamicSectionKind,
    ElfResolvedSectionHeaderPlacement, ElfSectionPlacementResolutionKind,
    ValidatedElfDynamicLoadLayout, plan_elf_dynamic_load_layout,
};
pub use dynamic_executable::load_placement::placed_section_headers::{
    ElfAppliedSectionHeaderPlacement, ElfSectionHeaderPlacementApplicationError,
    ValidatedElfPlacedSectionHeaderTable, apply_elf_section_header_placements,
};
pub use dynamic_executable::load_placement::resolved_dynamic_table::{
    ElfAppliedDynamicAddress, ElfDynamicAddressApplicationError, ElfDynamicAddressApplicationKind,
    ElfDynamicAddressApplicationTarget, ValidatedElfResolvedDynamicTable,
    apply_elf_dynamic_address_fixups,
};
pub use dynamic_executable::procedure_linkage::dynamic_import_relocations::{
    ElfProcedureLinkageRelocationPlanningError, ValidatedElfProcedureLinkageRelocationPlan,
    plan_elf_procedure_linkage_relocations,
};
pub use dynamic_executable::procedure_linkage::dynamic_linkage_descriptors::{
    ElfProcedureLinkageSectionDescriptorPlanningError,
    ValidatedElfProcedureLinkageSectionDescriptorPlan,
    plan_elf_procedure_linkage_section_descriptors,
};
pub use dynamic_executable::procedure_linkage::dynamic_linkage_templates::{
    ElfProcedureLinkageTemplatePlanningError, ValidatedElfProcedureLinkageTemplatePlan,
    plan_elf_procedure_linkage_templates,
};
pub use dynamic_executable::section_headers::relative_section_layout::{
    ElfRelativeSectionPayloadLayoutError, ElfRelativeSectionPayloadRegion,
    ValidatedElfRelativeSectionPayloadLayout, plan_elf_relative_section_payload_layout,
};
pub use dynamic_executable::section_headers::section_header_bytes::{
    ElfSectionHeaderTableSerializationError, ValidatedElfSectionHeaderTableTemplate,
    serialize_elf_section_header_table,
};
pub use dynamic_executable::section_headers::section_name_table::{
    ElfSectionNameTablePlanningError, ValidatedElfSectionNameTablePlan, plan_elf_section_name_table,
};
pub use dynamic_executable::section_headers::section_payload_roster::{
    ElfIndexedSectionPayloadPlanningError, ValidatedElfIndexedSectionPayloadPlan,
    plan_elf_indexed_section_payloads,
};
pub use dynamic_executable::section_headers::section_roster::{
    ElfDynamicSectionRosterPlanningError, ValidatedElfDynamicSectionRoster,
    plan_elf_dynamic_section_roster,
};
