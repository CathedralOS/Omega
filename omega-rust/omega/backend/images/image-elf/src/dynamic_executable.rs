//! The dynamic lane: [`emit_elf_dynamic_executable`] runs the 22-stage chain
//! over one final image and one normalized interpreter plan, grouped below by
//! phase in chain order.
//!
//! ```text
//!   import_sections    plan_elf_dynamic_link_inputs -> plan_elf_dynamic_sections
//!                      -> serialize_elf_dynamic_sections
//!                      -> plan_elf_dynamic_section_descriptors
//!   procedure_linkage  plan_elf_procedure_linkage_relocations
//!                      -> plan_elf_procedure_linkage_templates
//!                      -> plan_elf_procedure_linkage_section_descriptors
//!   dynamic_table      plan_elf_dynamic_tags -> serialize_elf_dynamic_table
//!                      -> plan_elf_dynamic_table_section_descriptor
//!   section_headers    plan_elf_section_name_table -> plan_elf_dynamic_section_roster
//!                      -> serialize_elf_section_header_table
//!                      -> plan_elf_indexed_section_payloads
//!                      -> plan_elf_relative_section_payload_layout
//!   load_placement     plan_elf_dynamic_load_layout
//!                      -> apply_elf_section_header_placements
//!                      -> apply_elf_dynamic_address_fixups
//!   file_assembly      serialize_elf_dynamic_file_envelope
//!                      -> apply_elf_procedure_linkage_fixups
//!                      -> assemble_elf_dynamic_file -> admit_elf_dynamic_executable
//! ```
//!
//! Each stage takes the previous stage's `ValidatedElf*` carrier as its only
//! input: there is no way to call stage N without holding a value that only
//! stage N-1 can construct. The chain below is the one place that sequence is
//! spelled; every rejection returns the exact stage carrier inside one
//! variant of [`ElfDynamicExecutableEmissionError`]. The crate root also
//! re-exports every stage's entry function, carrier and error for a caller
//! that needs to hold an intermediate.

mod checked;
pub(crate) mod dynamic_table;
pub(crate) mod file_assembly;
pub(crate) mod import_sections;
pub(crate) mod load_placement;
pub(crate) mod procedure_linkage;
pub(crate) mod section_headers;

use diagnostics::Diagnostic;
use image::FinalImage;
use target::NormalizedElfInterpreterPlan;

use dynamic_table::dynamic_table_descriptor::{
    ElfDynamicTableSectionDescriptorPlanningError, plan_elf_dynamic_table_section_descriptor,
};
use dynamic_table::dynamic_tag_bytes::{
    ElfDynamicTableSerializationError, serialize_elf_dynamic_table,
};
use dynamic_table::dynamic_tags::{ElfDynamicTagPlanningError, plan_elf_dynamic_tags};
use file_assembly::assembled_dynamic_file::{
    ElfDynamicExecutableAdmissionError, ElfDynamicFileAssemblyError, ValidatedElfDynamicExecutable,
    admit_elf_dynamic_executable, assemble_elf_dynamic_file,
};
use file_assembly::dynamic_file_envelope::{
    ElfDynamicFileEnvelopeSerializationError, serialize_elf_dynamic_file_envelope,
};
use file_assembly::resolved_procedure_linkage::{
    ElfProcedureLinkageApplicationError, apply_elf_procedure_linkage_fixups,
};
use import_sections::dynamic_link::{
    ElfDynamicLinkInputPlanningError, plan_elf_dynamic_link_inputs,
};
use import_sections::dynamic_section_bytes::{
    ElfDynamicSectionSerializationError, serialize_elf_dynamic_sections,
};
use import_sections::dynamic_section_descriptors::{
    ElfDynamicSectionDescriptorPlanningError, plan_elf_dynamic_section_descriptors,
};
use import_sections::dynamic_sections::{
    ElfDynamicSectionPlanningError, plan_elf_dynamic_sections,
};
use load_placement::load_layout::{ElfDynamicLoadLayoutError, plan_elf_dynamic_load_layout};
use load_placement::placed_section_headers::{
    ElfSectionHeaderPlacementApplicationError, apply_elf_section_header_placements,
};
use load_placement::resolved_dynamic_table::{
    ElfDynamicAddressApplicationError, apply_elf_dynamic_address_fixups,
};
use procedure_linkage::dynamic_import_relocations::{
    ElfProcedureLinkageRelocationPlanningError, plan_elf_procedure_linkage_relocations,
};
use procedure_linkage::dynamic_linkage_descriptors::{
    ElfProcedureLinkageSectionDescriptorPlanningError,
    plan_elf_procedure_linkage_section_descriptors,
};
use procedure_linkage::dynamic_linkage_templates::{
    ElfProcedureLinkageTemplatePlanningError, plan_elf_procedure_linkage_templates,
};
use section_headers::relative_section_layout::{
    ElfRelativeSectionPayloadLayoutError, plan_elf_relative_section_payload_layout,
};
use section_headers::section_header_bytes::{
    ElfSectionHeaderTableSerializationError, serialize_elf_section_header_table,
};
use section_headers::section_name_table::{
    ElfSectionNameTablePlanningError, plan_elf_section_name_table,
};
use section_headers::section_payload_roster::{
    ElfIndexedSectionPayloadPlanningError, plan_elf_indexed_section_payloads,
};
use section_headers::section_roster::{
    ElfDynamicSectionRosterPlanningError, plan_elf_dynamic_section_roster,
};

/// The exact failing owner from one stage of the dynamic lane. Every variant
/// preserves the stage-specific carrier that stage refused; diagnostics are
/// observations of that custody, never substitutes for it.
#[derive(Debug)]
#[must_use = "dynamic ELF emission failure retains the exact failing owner"]
pub enum ElfDynamicExecutableEmissionError {
    LinkInputs(Box<ElfDynamicLinkInputPlanningError>),
    DynamicSections(Box<ElfDynamicSectionPlanningError>),
    DynamicSectionBytes(Box<ElfDynamicSectionSerializationError>),
    DynamicSectionDescriptors(Box<ElfDynamicSectionDescriptorPlanningError>),
    ProcedureLinkageRelocations(Box<ElfProcedureLinkageRelocationPlanningError>),
    ProcedureLinkageTemplates(Box<ElfProcedureLinkageTemplatePlanningError>),
    ProcedureLinkageDescriptors(Box<ElfProcedureLinkageSectionDescriptorPlanningError>),
    DynamicTags(Box<ElfDynamicTagPlanningError>),
    DynamicTableBytes(Box<ElfDynamicTableSerializationError>),
    DynamicTableDescriptor(Box<ElfDynamicTableSectionDescriptorPlanningError>),
    SectionNames(Box<ElfSectionNameTablePlanningError>),
    SectionRoster(Box<ElfDynamicSectionRosterPlanningError>),
    SectionHeaderBytes(Box<ElfSectionHeaderTableSerializationError>),
    IndexedPayloads(Box<ElfIndexedSectionPayloadPlanningError>),
    RelativeLayout(Box<ElfRelativeSectionPayloadLayoutError>),
    LoadLayout(Box<ElfDynamicLoadLayoutError>),
    PlacedSectionHeaders(Box<ElfSectionHeaderPlacementApplicationError>),
    ResolvedDynamicTable(Box<ElfDynamicAddressApplicationError>),
    FileEnvelope(Box<ElfDynamicFileEnvelopeSerializationError>),
    ProcedureLinkageApplication(Box<ElfProcedureLinkageApplicationError>),
    FileAssembly(Box<ElfDynamicFileAssemblyError>),
    FinalByteAdmission(Box<ElfDynamicExecutableAdmissionError>),
}

impl ElfDynamicExecutableEmissionError {
    pub const fn stage(&self) -> &'static str {
        match self {
            Self::LinkInputs(_) => "link-inputs",
            Self::DynamicSections(_) => "dynamic-sections",
            Self::DynamicSectionBytes(_) => "dynamic-section-bytes",
            Self::DynamicSectionDescriptors(_) => "dynamic-section-descriptors",
            Self::ProcedureLinkageRelocations(_) => "procedure-linkage-relocations",
            Self::ProcedureLinkageTemplates(_) => "procedure-linkage-templates",
            Self::ProcedureLinkageDescriptors(_) => "procedure-linkage-descriptors",
            Self::DynamicTags(_) => "dynamic-tags",
            Self::DynamicTableBytes(_) => "dynamic-table-bytes",
            Self::DynamicTableDescriptor(_) => "dynamic-table-descriptor",
            Self::SectionNames(_) => "section-names",
            Self::SectionRoster(_) => "section-roster",
            Self::SectionHeaderBytes(_) => "section-header-bytes",
            Self::IndexedPayloads(_) => "indexed-payloads",
            Self::RelativeLayout(_) => "relative-layout",
            Self::LoadLayout(_) => "load-layout",
            Self::PlacedSectionHeaders(_) => "placed-section-headers",
            Self::ResolvedDynamicTable(_) => "resolved-dynamic-table",
            Self::FileEnvelope(_) => "file-envelope",
            Self::ProcedureLinkageApplication(_) => "procedure-linkage-application",
            Self::FileAssembly(_) => "file-assembly",
            Self::FinalByteAdmission(_) => "final-byte-admission",
        }
    }

    pub const fn diagnostic(&self) -> &Diagnostic {
        match self {
            Self::LinkInputs(error) => error.diagnostic(),
            Self::DynamicSections(error) => error.diagnostic(),
            Self::DynamicSectionBytes(error) => error.diagnostic(),
            Self::DynamicSectionDescriptors(error) => error.diagnostic(),
            Self::ProcedureLinkageRelocations(error) => error.diagnostic(),
            Self::ProcedureLinkageTemplates(error) => error.diagnostic(),
            Self::ProcedureLinkageDescriptors(error) => error.diagnostic(),
            Self::DynamicTags(error) => error.diagnostic(),
            Self::DynamicTableBytes(error) => error.diagnostic(),
            Self::DynamicTableDescriptor(error) => error.diagnostic(),
            Self::SectionNames(error) => error.diagnostic(),
            Self::SectionRoster(error) => error.diagnostic(),
            Self::SectionHeaderBytes(error) => error.diagnostic(),
            Self::IndexedPayloads(error) => error.diagnostic(),
            Self::RelativeLayout(error) => error.diagnostic(),
            Self::LoadLayout(error) => error.diagnostic(),
            Self::PlacedSectionHeaders(error) => error.diagnostic(),
            Self::ResolvedDynamicTable(error) => error.diagnostic(),
            Self::FileEnvelope(error) => error.diagnostic(),
            Self::ProcedureLinkageApplication(error) => error.diagnostic(),
            Self::FileAssembly(error) => error.diagnostic(),
            Self::FinalByteAdmission(error) => error.diagnostic(),
        }
    }
}

impl std::fmt::Display for ElfDynamicExecutableEmissionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "dynamic ELF {}: {}",
            self.stage(),
            self.diagnostic()
        )
    }
}

impl std::error::Error for ElfDynamicExecutableEmissionError {}

/// Emit one dynamically linked ELF executable from a final image and the
/// normalized interpreter plan its imports require: plan the dynamic-link
/// inputs and import sections, the procedure linkage, the dynamic table and
/// the section headers; place the load layout and resolve the addresses it
/// fixes; then serialize the file envelope, apply the procedure-linkage
/// fixups, assemble the file and admit its final bytes.
///
/// The interpreter is consumed into the first stage; every rejection returns
/// the exact stage carrier. The admitted result is final-byte custody only --
/// it grants no publication or execution authority.
pub fn emit_elf_dynamic_executable(
    image: FinalImage,
    interpreter: NormalizedElfInterpreterPlan,
) -> Result<ValidatedElfDynamicExecutable, Box<ElfDynamicExecutableEmissionError>> {
    use ElfDynamicExecutableEmissionError as Stage;
    let inputs = plan_elf_dynamic_link_inputs(image, interpreter)
        .map_err(|error| Box::new(Stage::LinkInputs(error)))?;
    let sections = plan_elf_dynamic_sections(inputs)
        .map_err(|error| Box::new(Stage::DynamicSections(error)))?;
    let payloads = serialize_elf_dynamic_sections(sections)
        .map_err(|error| Box::new(Stage::DynamicSectionBytes(error)))?;
    let descriptors = plan_elf_dynamic_section_descriptors(payloads)
        .map_err(|error| Box::new(Stage::DynamicSectionDescriptors(error)))?;
    let linkage = plan_elf_procedure_linkage_relocations(descriptors)
        .map_err(|error| Box::new(Stage::ProcedureLinkageRelocations(error)))?;
    let templates = plan_elf_procedure_linkage_templates(linkage)
        .map_err(|error| Box::new(Stage::ProcedureLinkageTemplates(error)))?;
    let descriptors = plan_elf_procedure_linkage_section_descriptors(templates)
        .map_err(|error| Box::new(Stage::ProcedureLinkageDescriptors(error)))?;
    let tags =
        plan_elf_dynamic_tags(descriptors).map_err(|error| Box::new(Stage::DynamicTags(error)))?;
    let dynamic = serialize_elf_dynamic_table(tags)
        .map_err(|error| Box::new(Stage::DynamicTableBytes(error)))?;
    let descriptor = plan_elf_dynamic_table_section_descriptor(dynamic)
        .map_err(|error| Box::new(Stage::DynamicTableDescriptor(error)))?;
    let names = plan_elf_section_name_table(descriptor)
        .map_err(|error| Box::new(Stage::SectionNames(error)))?;
    let roster = plan_elf_dynamic_section_roster(names)
        .map_err(|error| Box::new(Stage::SectionRoster(error)))?;
    let headers = serialize_elf_section_header_table(roster)
        .map_err(|error| Box::new(Stage::SectionHeaderBytes(error)))?;
    let payloads = plan_elf_indexed_section_payloads(headers)
        .map_err(|error| Box::new(Stage::IndexedPayloads(error)))?;
    let relative = plan_elf_relative_section_payload_layout(payloads)
        .map_err(|error| Box::new(Stage::RelativeLayout(error)))?;
    let load = plan_elf_dynamic_load_layout(relative)
        .map_err(|error| Box::new(Stage::LoadLayout(error)))?;
    let placed = apply_elf_section_header_placements(load)
        .map_err(|error| Box::new(Stage::PlacedSectionHeaders(error)))?;
    let resolved = apply_elf_dynamic_address_fixups(placed)
        .map_err(|error| Box::new(Stage::ResolvedDynamicTable(error)))?;
    let envelope = serialize_elf_dynamic_file_envelope(resolved)
        .map_err(|error| Box::new(Stage::FileEnvelope(error)))?;
    let linkage = apply_elf_procedure_linkage_fixups(envelope)
        .map_err(|error| Box::new(Stage::ProcedureLinkageApplication(error)))?;
    let assembled =
        assemble_elf_dynamic_file(linkage).map_err(|error| Box::new(Stage::FileAssembly(error)))?;
    admit_elf_dynamic_executable(assembled)
        .map_err(|error| Box::new(Stage::FinalByteAdmission(error)))
}
