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
    ElfDynamicFileAssemblyError, ElfDynamicFileFragmentKind, ElfDynamicFileFragmentPlacement,
    ValidatedElfAssembledDynamicFile, assemble_elf_dynamic_file,
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
                normalized_identity,
                object,
                symbol,
                version,
            } => format!(
                "versioned ELF foreign locator 0x{normalized_identity:016x} for target `{}` reached final emission with object {}, symbol {}, version {}, and {} exact relocation site(s); runnable dynamic ELF emission remains fail-closed before image mutation because no target-owned ELF loader plan carries the exact PT_INTERP bytes",
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
        final_text_bytes: image.memory.text.clone(),
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
