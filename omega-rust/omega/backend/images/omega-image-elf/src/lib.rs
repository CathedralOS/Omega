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
//! Everything else in the crate is the dynamic lane, and it is a linear chain
//! of 22 stages driven from `omega-image-emission/src/dynamic_elf.rs`:
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

//! `omega-image-emission/src/dynamic_elf.rs` is the only driver of the dynamic
//! lane; `omega-image` supplies `FinalImage`, `place_executable_regions` and the
//! relocation appliers both lanes use.
//!
//! @Note: do not decide what is dead in this crate by grepping for type names.
//! The driver binds every stage result with an inferred `let` and never spells a
//! carrier type, so 21 of the 22 carriers have zero occurrences of their names
//! anywhere outside this crate while being entirely load-bearing -
//! `ValidatedElfDynamicLoadLayout` among them, which is the return type of a
//! function the driver calls in production. A scouting pass over this crate
//! called 18 such types dead on exactly that evidence.
//!
//! @Incomplete: the static lane's own error message is out of date and says so
//! in the most misleading possible place - the text a user sees. It claims
//! "no target-owned ELF loader plan carries the exact PT_INTERP bytes". One
//! does: `omega_target::NormalizedElfInterpreterPlan::interpreter_path` carries
//! exactly those bytes, `plan_elf_dynamic_link_inputs` takes that plan as its
//! second argument, and `dynamic_file_envelope.rs` maps
//! `ElfLoadProgramHeaderKind::Interpreter` to `PT_INTERP` (3). The claim in the
//! other direction is stale too: `omega-target/src/elf_loader.rs` says dynamic
//! ELF emission "remains unavailable until a later owner joins this input to the
//! complete dynamic-link structures", and that owner is this crate. Each side
//! documents the other's absence while the other is present. Fix them together
//! or not at all.

use omega_image::{
    ExecutableImageOutput, FinalImage, FinalImageLayout, apply_aarch64_relocations,
    apply_x86_64_relocations, place_executable_regions,
};
use psi_diagnostics::Diagnostic;

mod assembled_dynamic_file;
mod bytes;
mod constants;
mod dynamic_file_envelope;
mod dynamic_import_relocations;
mod dynamic_link;
mod dynamic_linkage_descriptors;
mod dynamic_linkage_templates;
mod dynamic_section_bytes;
mod dynamic_section_descriptors;
mod dynamic_sections;
mod dynamic_table_descriptor;
mod dynamic_tag_bytes;
mod dynamic_tags;
mod entry;
mod headers;
mod imports;
mod layout;
mod load_layout;
mod placed_section_headers;
mod relative_section_layout;
mod resolved_dynamic_table;
mod resolved_procedure_linkage;
mod section_header_bytes;
mod section_name_table;
mod section_payload_roster;
mod section_roster;
mod sections;
#[cfg(test)]
mod tests;

pub use assembled_dynamic_file::{
    ElfDynamicExecutableAdmissionError, ElfDynamicFileAssemblyError, ElfDynamicFileFragmentKind,
    ElfDynamicFileFragmentPlacement, ValidatedElfAssembledDynamicFile,
    ValidatedElfDynamicExecutable, admit_elf_dynamic_executable, assemble_elf_dynamic_file,
};
pub use dynamic_file_envelope::{
    ElfDynamicFileEnvelopeSerializationError, ValidatedElfDynamicFileEnvelope,
    serialize_elf_dynamic_file_envelope,
};
pub use dynamic_import_relocations::{
    ElfProcedureLinkageRelocationPlanningError, ValidatedElfProcedureLinkageRelocationPlan,
    plan_elf_procedure_linkage_relocations,
};
pub use dynamic_link::{
    ElfDynamicLinkInputPlanningError, PlannedElfDynamicLinkInputs, plan_elf_dynamic_link_inputs,
};
pub use dynamic_linkage_descriptors::{
    ElfProcedureLinkageSectionDescriptorPlanningError,
    ValidatedElfProcedureLinkageSectionDescriptorPlan,
    plan_elf_procedure_linkage_section_descriptors,
};
pub use dynamic_linkage_templates::{
    ElfProcedureLinkageTemplatePlanningError, ValidatedElfProcedureLinkageTemplatePlan,
    plan_elf_procedure_linkage_templates,
};
pub use dynamic_section_bytes::{
    ElfDynamicSectionSerializationError, ValidatedElfDynamicSectionPayloads,
    serialize_elf_dynamic_sections,
};
pub use dynamic_section_descriptors::{
    ElfDynamicSectionDescriptorPlanningError, ValidatedElfDynamicSectionDescriptorPlan,
    plan_elf_dynamic_section_descriptors,
};
pub use dynamic_sections::{
    ElfDynamicSectionPlanningError, ValidatedElfDynamicSectionPlan, plan_elf_dynamic_sections,
};
pub use dynamic_table_descriptor::{
    ElfDynamicTableSectionDescriptorPlanningError, ValidatedElfDynamicTableSectionDescriptorPlan,
    plan_elf_dynamic_table_section_descriptor,
};
pub use dynamic_tag_bytes::{
    ElfDynamicTableSerializationError, ValidatedElfDynamicTablePayload, serialize_elf_dynamic_table,
};
pub use dynamic_tags::{
    ElfDynamicTagPlanningError, ValidatedElfDynamicTagPlan, plan_elf_dynamic_tags,
};
pub use load_layout::{
    ElfDynamicLoadLayoutError, ElfLoadImageMemoryPlacement, ElfLoadProgramHeader,
    ElfLoadProgramHeaderKind, ElfPlacedDynamicSection, ElfPlacedDynamicSectionKind,
    ElfResolvedSectionHeaderPlacement, ElfSectionPlacementResolutionKind,
    ValidatedElfDynamicLoadLayout, plan_elf_dynamic_load_layout,
};
pub use placed_section_headers::{
    ElfAppliedSectionHeaderPlacement, ElfSectionHeaderPlacementApplicationError,
    ValidatedElfPlacedSectionHeaderTable, apply_elf_section_header_placements,
};
pub use relative_section_layout::{
    ElfRelativeSectionPayloadLayoutError, ElfRelativeSectionPayloadRegion,
    ValidatedElfRelativeSectionPayloadLayout, plan_elf_relative_section_payload_layout,
};
pub use resolved_dynamic_table::{
    ElfAppliedDynamicAddress, ElfDynamicAddressApplicationError, ElfDynamicAddressApplicationKind,
    ElfDynamicAddressApplicationTarget, ValidatedElfResolvedDynamicTable,
    apply_elf_dynamic_address_fixups,
};
pub use resolved_procedure_linkage::{
    ElfAppliedProcedureLinkageFixup, ElfAppliedProcedureLinkageKind,
    ElfAppliedProcedureLinkageStorage, ElfAppliedProcedureLinkageTarget,
    ElfProcedureLinkageApplicationError, ValidatedElfResolvedProcedureLinkage,
    apply_elf_procedure_linkage_fixups,
};
pub use section_header_bytes::{
    ElfSectionHeaderTableSerializationError, ValidatedElfSectionHeaderTableTemplate,
    serialize_elf_section_header_table,
};
pub use section_name_table::{
    ElfSectionNameTablePlanningError, ValidatedElfSectionNameTablePlan, plan_elf_section_name_table,
};
pub use section_payload_roster::{
    ElfIndexedSectionPayloadPlanningError, ValidatedElfIndexedSectionPayloadPlan,
    plan_elf_indexed_section_payloads,
};
pub use section_roster::{
    ElfDynamicSectionRosterPlanningError, ValidatedElfDynamicSectionRoster,
    plan_elf_dynamic_section_roster,
};

use entry::elf_entry_address;
use headers::{write_data_program_header, write_elf_header, write_text_program_header};
use imports::{ElfImportLocator, canonical_referenced_imports};
use sections::plan_elf_sections;

// ELF e_machine values.
const EM_X86_64: u16 = 62;
const EM_AARCH64: u16 = 183;

pub fn emit_elf_aarch64_executable(image: FinalImage) -> Result<ExecutableImageOutput, Diagnostic> {
    emit_elf_executable(
        image,
        EM_AARCH64,
        "elf64-aarch64-executable",
        apply_aarch64_relocations,
    )
}

pub fn emit_elf_x86_64_executable(image: FinalImage) -> Result<ExecutableImageOutput, Diagnostic> {
    emit_elf_executable(
        image,
        EM_X86_64,
        "elf64-x86-64-executable",
        apply_x86_64_relocations,
    )
}

/// Shared ELF64 executable emitter. The ELF container is architecture-agnostic
/// apart from `e_machine` and the relocation application, both passed in.
fn emit_elf_executable(
    mut image: FinalImage,
    machine: u16,
    format: &str,
    apply_relocations: fn(&mut FinalImage, &FinalImageLayout, &str) -> Result<(), Diagnostic>,
) -> Result<ExecutableImageOutput, Diagnostic> {
    let imports = canonical_referenced_imports(&image)?;
    if let Some(import) = imports.first() {
        let message = match &import.locator {
            ElfImportLocator::StringBackedBootstrap { library, symbol } => format!(
                "ELF direct image relocation references unknown symbol `{symbol}`; canonical dynamic import request names library `{library}` at {} relocation site(s), but ELF loader binding is not implemented",
                import.relocations.len(),
            ),
            ElfImportLocator::Versioned {
                target_profile,
                compatibility_report_identity,
                object,
                symbol,
                version,
            } => format!(
                "versioned ELF foreign locator report 0x{compatibility_report_identity:016x} for target `{}` reached final emission with object {}, symbol {}, version {}, and {} exact relocation site(s); runnable dynamic ELF emission remains fail-closed before image mutation because no target-owned ELF loader plan carries the exact PT_INTERP bytes",
                target_profile.target_name(),
                hex_bytes(object),
                hex_bytes(symbol),
                hex_bytes(version),
                import.relocations.len(),
            ),
        };
        return Err(Diagnostic::error(message));
    }
    let sections = plan_elf_sections(&image);
    let layout = sections.final_image_layout();
    let entry_address = elf_entry_address(&image, sections.text_address)?;

    apply_relocations(&mut image, &layout, "ELF direct image")?;
    let executable_regions = place_executable_regions(&image, layout)?;

    let mut bytes = Vec::with_capacity(sections.data_offset + image.memory.data.len());
    write_elf_header(
        &mut bytes,
        machine,
        entry_address,
        sections.text_offset,
        sections.data_offset,
    );
    write_text_program_header(&mut bytes, sections.text_offset, image.memory.text.len());
    write_data_program_header(
        &mut bytes,
        sections.data_offset,
        image.memory.data.len(),
        sections.data_memory_size,
    );
    bytes.resize(sections.text_offset, 0);
    bytes.extend(&image.memory.text);
    bytes.resize(sections.data_offset, 0);
    bytes.extend(&image.memory.data);

    Ok(ExecutableImageOutput {
        final_image_layout: layout,
        final_text_bytes: image.memory.text.clone(),
        final_data_bytes: image.memory.data.clone(),
        bytes,
        file_name: "omega-program".to_owned(),
        format: format.to_owned(),
        text_bytes: image.memory.text.len(),
        data_bytes: image.memory.data.len(),
        bss_bytes: image.memory.bss_size,
        symbols: image.symbol_table.symbols.len(),
        imports: image.symbol_table.imports.len(),
        relocations: image.relocation_table.relocations.len(),
        executable_regions,
    })
}

fn hex_bytes(bytes: &[u8]) -> String {
    let mut rendered = String::with_capacity(bytes.len().saturating_mul(2).saturating_add(2));
    rendered.push_str("0x");
    for byte in bytes {
        use std::fmt::Write;
        write!(&mut rendered, "{byte:02x}").expect("writing to String cannot fail");
    }
    rendered
}
