//! Production-emitter custody for already-admitted dynamic ELF bytes.
//!
//! This bridge consumes only the exact final-byte carrier produced by the ELF
//! owner and independently rejoins it to the source-free object artifact.  It
//! deliberately does not construct loader inputs, publish bytes, create an
//! installation receipt, or grant execution authority.

use diagnostics::Diagnostic;
use image::{
    EmittedImageOutput, FinalImageInput, emitted_direct_executable_output,
    final_image_symbol_digest,
};
use image_elf::{ValidatedElfDynamicExecutable, *};
use target::{Architecture, NormalizedElfInterpreterPlan, ObjectFormat};

use crate::ObjectArtifact;
use crate::final_image_validation::validate_terminal_dynamic_elf_image;

/// Exact image-emission inputs selected outside the source-free object owner.
///
/// A normalized interpreter is consumed only when the object itself retains
/// versioned ELF imports.  Supplying one cannot force an otherwise direct
/// image through the dynamic ELF path, and omitting one cannot make an
/// unresolved ELF import fall back to the direct writer.
#[derive(Debug, Clone)]
pub enum ExecutableImageEmissionRequest {
    Direct {
        subsystem: u16,
    },
    DynamicElf {
        interpreter: NormalizedElfInterpreterPlan,
    },
}

impl ExecutableImageEmissionRequest {
    pub const fn direct(subsystem: u16) -> Self {
        Self::Direct { subsystem }
    }

    pub const fn dynamic_elf(interpreter: NormalizedElfInterpreterPlan) -> Self {
        Self::DynamicElf { interpreter }
    }
}

/// Result of the image-bound request router.
///
/// Dynamic ELF output deliberately remains distinct from [`crate::ExecutableImage`]
/// and therefore cannot enter installation or publication APIs.
#[derive(Debug)]
#[must_use = "requested image emission retains its exact authority boundary"]
pub enum RequestedExecutableImage {
    Direct(crate::ExecutableImage),
    DynamicElf(RequestedDynamicElfImage),
}

impl RequestedExecutableImage {
    pub const fn output(&self) -> &EmittedImageOutput {
        match self {
            Self::Direct(image) => image.output(),
            Self::DynamicElf(image) => image.output(),
        }
    }
}

/// Non-installable dynamic output bound to the complete object artifact that
/// selected its writer path.
///
/// Retaining the whole artifact keeps Terminal PSI and every semantic/evidence
/// row in the replay boundary rather than treating byte-identical object
/// layouts as interchangeable.
#[derive(Debug)]
pub struct RequestedDynamicElfImage {
    artifact: ObjectArtifact,
    emission: DynamicElfImageEmission,
}

impl RequestedDynamicElfImage {
    pub const fn artifact(&self) -> &ObjectArtifact {
        &self.artifact
    }

    pub const fn emission(&self) -> &DynamicElfImageEmission {
        &self.emission
    }

    pub const fn output(&self) -> &EmittedImageOutput {
        self.emission.output()
    }

    pub fn into_emission(self) -> DynamicElfImageEmission {
        self.emission
    }
}

/// Rejected image-bound request with any consumed dynamic-loader input intact.
#[derive(Debug)]
#[must_use = "requested image-emission rejection may retain loader custody"]
pub enum RequestedExecutableImageError {
    MissingDynamicElfInterpreter {
        target: target::NativeTarget,
        subsystem: u16,
        diagnostic: Diagnostic,
    },
    UnexpectedDynamicElfInterpreter {
        interpreter: NormalizedElfInterpreterPlan,
        diagnostic: Diagnostic,
    },
    Direct(Diagnostic),
    DynamicElf(Box<DynamicElfOrchestrationError>),
}

impl RequestedExecutableImageError {
    pub const fn diagnostic(&self) -> &Diagnostic {
        match self {
            Self::MissingDynamicElfInterpreter { diagnostic, .. }
            | Self::UnexpectedDynamicElfInterpreter { diagnostic, .. }
            | Self::Direct(diagnostic) => diagnostic,
            Self::DynamicElf(error) => error.diagnostic(),
        }
    }

    pub fn into_unexpected_interpreter(self) -> Option<NormalizedElfInterpreterPlan> {
        match self {
            Self::UnexpectedDynamicElfInterpreter { interpreter, .. } => Some(interpreter),
            _ => None,
        }
    }
}

impl std::fmt::Display for RequestedExecutableImageError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic().fmt(formatter)
    }
}

impl std::error::Error for RequestedExecutableImageError {}

/// Select the only image-emission path justified by the exact object contents.
///
/// Import-bearing ELF objects require a consumed normalized interpreter and
/// produce non-installable dynamic custody.  Every other object must omit that
/// input and continues through the existing direct writer.
pub fn emit_requested_executable_image(
    artifact: &ObjectArtifact,
    request: ExecutableImageEmissionRequest,
) -> Result<RequestedExecutableImage, Box<RequestedExecutableImageError>> {
    let has_normalized_imports = !artifact.object().layout.normalized_imports.is_empty();
    let requires_dynamic_elf =
        artifact.target().object_format == ObjectFormat::Elf && has_normalized_imports;
    match (requires_dynamic_elf, request) {
        (true, ExecutableImageEmissionRequest::DynamicElf { interpreter }) => {
            emit_dynamic_elf_image(artifact, interpreter)
                .map(|emission| {
                    RequestedExecutableImage::DynamicElf(RequestedDynamicElfImage {
                        artifact: artifact.clone(),
                        emission,
                    })
                })
                .map_err(|error| Box::new(RequestedExecutableImageError::DynamicElf(error)))
        }
        (true, ExecutableImageEmissionRequest::Direct { subsystem }) => Err(Box::new(
            RequestedExecutableImageError::MissingDynamicElfInterpreter {
                target: artifact.target(),
                subsystem,
                diagnostic: Diagnostic::error(
                    "import-bearing ELF image emission requires an exact normalized interpreter input",
                ),
            },
        )),
        (false, ExecutableImageEmissionRequest::DynamicElf { interpreter }) => Err(Box::new(
            RequestedExecutableImageError::UnexpectedDynamicElfInterpreter {
                interpreter,
                diagnostic: Diagnostic::error(
                    "a normalized ELF interpreter cannot select the dynamic writer without normalized ELF imports",
                ),
            },
        )),
        (false, ExecutableImageEmissionRequest::Direct { subsystem }) => {
            crate::emit_executable_image(artifact, subsystem)
                .map(RequestedExecutableImage::Direct)
                .map_err(|diagnostic| Box::new(RequestedExecutableImageError::Direct(diagnostic)))
        }
    }
}

/// Independently replay the selected image path without collapsing dynamic
/// output into installable custody.
pub fn validate_requested_executable_image(
    artifact: &ObjectArtifact,
    image: &RequestedExecutableImage,
) -> Result<(), Diagnostic> {
    let has_normalized_imports = !artifact.object().layout.normalized_imports.is_empty();
    match image {
        RequestedExecutableImage::Direct(image) => {
            if artifact.target().object_format == ObjectFormat::Elf && has_normalized_imports {
                return Err(Diagnostic::error(
                    "import-bearing ELF object was substituted into direct image custody",
                ));
            }
            crate::validate_executable_image(artifact, image)
        }
        RequestedExecutableImage::DynamicElf(image) => {
            validate_requested_dynamic_elf_image(artifact, image)
        }
    }
}

/// Replay the dynamic branch of the request router without manufacturing an
/// owned sum carrier. This remains a route/custody check, not loader policy.
pub fn validate_requested_dynamic_elf_image(
    artifact: &ObjectArtifact,
    image: &RequestedDynamicElfImage,
) -> Result<(), Diagnostic> {
    if artifact.target().object_format != ObjectFormat::Elf
        || artifact.object().layout.normalized_imports.is_empty()
    {
        return Err(Diagnostic::error(
            "dynamic ELF image custody requires exact normalized ELF imports",
        ));
    }
    if image.artifact != *artifact {
        return Err(Diagnostic::error(
            "dynamic ELF image custody does not retain the exact source object artifact",
        ));
    }
    validate_dynamic_elf_image_emission(artifact, &image.emission)
}

/// Exact admitted dynamic ELF bytes after production-emitter reconciliation.
///
/// This is intentionally not [`crate::ExecutableImage`], so existing
/// installation and installed-artifact APIs cannot mistake final-byte custody
/// for publication or execution authority.
///
/// ```compile_fail
/// use image_emission::{
///     DynamicElfImageEmission, build_installation_record,
/// };
/// use semantic_vocabulary::ProfileDecisionId;
///
/// fn cannot_install(
///     emission: &DynamicElfImageEmission,
///     profile: ProfileDecisionId,
/// ) {
///     let _ = build_installation_record(emission, profile);
/// }
/// ```
#[derive(Debug)]
#[must_use = "dynamic ELF production emission retains admitted byte custody"]
pub struct DynamicElfImageEmission {
    admitted: ValidatedElfDynamicExecutable,
    output: EmittedImageOutput,
}

impl DynamicElfImageEmission {
    pub const fn admitted(&self) -> &ValidatedElfDynamicExecutable {
        &self.admitted
    }

    pub const fn output(&self) -> &EmittedImageOutput {
        &self.output
    }

    pub fn into_admitted(self) -> ValidatedElfDynamicExecutable {
        self.admitted
    }
}

/// Rejected production-emitter reconciliation with the admitted owner intact.
#[derive(Debug)]
#[must_use = "dynamic ELF emission rejection retains admitted byte custody"]
pub struct DynamicElfImageEmissionError {
    admitted: ValidatedElfDynamicExecutable,
    diagnostic: Diagnostic,
}

impl DynamicElfImageEmissionError {
    pub const fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (ValidatedElfDynamicExecutable, Diagnostic) {
        (self.admitted, self.diagnostic)
    }
}

impl std::fmt::Display for DynamicElfImageEmissionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for DynamicElfImageEmissionError {}

/// Exact failing owner from one stage of complete dynamic ELF orchestration.
///
/// Every variant preserves the stage-specific carrier supplied by the ELF
/// owner. Diagnostics are observations of that custody, never substitutes for
/// it.
#[derive(Debug)]
#[must_use = "dynamic ELF orchestration failure retains the exact failing owner"]
pub enum DynamicElfOrchestrationError {
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
    ProductionBridge(Box<DynamicElfImageEmissionError>),
}

impl DynamicElfOrchestrationError {
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
            Self::ProductionBridge(_) => "production-bridge",
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
            Self::ProductionBridge(error) => error.diagnostic(),
        }
    }
}

impl std::fmt::Display for DynamicElfOrchestrationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "dynamic ELF {}: {}",
            self.stage(),
            self.diagnostic()
        )
    }
}

impl std::error::Error for DynamicElfOrchestrationError {}

/// Run the complete existing dynamic ELF owner chain from one exact source-free
/// import-bearing object artifact and one normalized interpreter input through
/// production emission.
///
/// The artifact is borrowed, the interpreter is consumed into the first ELF
/// owner, and every rejection returns the exact stage carrier. Success remains
/// non-installable custody and grants no publication or execution authority.
/// The ordinary object builder now supplies the exact import-bearing input, but
/// this carrier still stops before native-artifact/installable integration.
pub fn emit_dynamic_elf_image(
    artifact: &ObjectArtifact,
    interpreter: NormalizedElfInterpreterPlan,
) -> Result<DynamicElfImageEmission, Box<DynamicElfOrchestrationError>> {
    let image = image::build_final_image(FinalImageInput {
        target: artifact.target(),
        object: artifact.object(),
        relocations: artifact.relocations(),
        text_bytes: artifact.text_bytes(),
        data_bytes: artifact.data_bytes(),
    });
    let inputs = plan_elf_dynamic_link_inputs(image, interpreter)
        .map_err(|error| Box::new(DynamicElfOrchestrationError::LinkInputs(error)))?;
    let sections = plan_elf_dynamic_sections(inputs)
        .map_err(|error| Box::new(DynamicElfOrchestrationError::DynamicSections(error)))?;
    let payloads = serialize_elf_dynamic_sections(sections)
        .map_err(|error| Box::new(DynamicElfOrchestrationError::DynamicSectionBytes(error)))?;
    let descriptors = plan_elf_dynamic_section_descriptors(payloads).map_err(|error| {
        Box::new(DynamicElfOrchestrationError::DynamicSectionDescriptors(
            error,
        ))
    })?;
    let linkage = plan_elf_procedure_linkage_relocations(descriptors).map_err(|error| {
        Box::new(DynamicElfOrchestrationError::ProcedureLinkageRelocations(
            error,
        ))
    })?;
    let templates = plan_elf_procedure_linkage_templates(linkage).map_err(|error| {
        Box::new(DynamicElfOrchestrationError::ProcedureLinkageTemplates(
            error,
        ))
    })?;
    let descriptors =
        plan_elf_procedure_linkage_section_descriptors(templates).map_err(|error| {
            Box::new(DynamicElfOrchestrationError::ProcedureLinkageDescriptors(
                error,
            ))
        })?;
    let tags = plan_elf_dynamic_tags(descriptors)
        .map_err(|error| Box::new(DynamicElfOrchestrationError::DynamicTags(error)))?;
    let dynamic = serialize_elf_dynamic_table(tags)
        .map_err(|error| Box::new(DynamicElfOrchestrationError::DynamicTableBytes(error)))?;
    let descriptor = plan_elf_dynamic_table_section_descriptor(dynamic)
        .map_err(|error| Box::new(DynamicElfOrchestrationError::DynamicTableDescriptor(error)))?;
    let names = plan_elf_section_name_table(descriptor)
        .map_err(|error| Box::new(DynamicElfOrchestrationError::SectionNames(error)))?;
    let roster = plan_elf_dynamic_section_roster(names)
        .map_err(|error| Box::new(DynamicElfOrchestrationError::SectionRoster(error)))?;
    let headers = serialize_elf_section_header_table(roster)
        .map_err(|error| Box::new(DynamicElfOrchestrationError::SectionHeaderBytes(error)))?;
    let payloads = plan_elf_indexed_section_payloads(headers)
        .map_err(|error| Box::new(DynamicElfOrchestrationError::IndexedPayloads(error)))?;
    let relative = plan_elf_relative_section_payload_layout(payloads)
        .map_err(|error| Box::new(DynamicElfOrchestrationError::RelativeLayout(error)))?;
    let load = plan_elf_dynamic_load_layout(relative)
        .map_err(|error| Box::new(DynamicElfOrchestrationError::LoadLayout(error)))?;
    let placed = apply_elf_section_header_placements(load)
        .map_err(|error| Box::new(DynamicElfOrchestrationError::PlacedSectionHeaders(error)))?;
    let resolved = apply_elf_dynamic_address_fixups(placed)
        .map_err(|error| Box::new(DynamicElfOrchestrationError::ResolvedDynamicTable(error)))?;
    let envelope = serialize_elf_dynamic_file_envelope(resolved)
        .map_err(|error| Box::new(DynamicElfOrchestrationError::FileEnvelope(error)))?;
    let linkage = apply_elf_procedure_linkage_fixups(envelope).map_err(|error| {
        Box::new(DynamicElfOrchestrationError::ProcedureLinkageApplication(
            error,
        ))
    })?;
    let assembled = assemble_elf_dynamic_file(linkage)
        .map_err(|error| Box::new(DynamicElfOrchestrationError::FileAssembly(error)))?;
    let admitted = admit_elf_dynamic_executable(assembled)
        .map_err(|error| Box::new(DynamicElfOrchestrationError::FinalByteAdmission(error)))?;
    emit_admitted_dynamic_elf_image(artifact, admitted)
        .map_err(|error| Box::new(DynamicElfOrchestrationError::ProductionBridge(error)))
}

/// Join exact admitted dynamic ELF bytes to the production image-output
/// surface without granting publication or execution authority.
pub fn emit_admitted_dynamic_elf_image(
    artifact: &ObjectArtifact,
    admitted: ValidatedElfDynamicExecutable,
) -> Result<DynamicElfImageEmission, Box<DynamicElfImageEmissionError>> {
    let output = match derive_output(artifact, &admitted) {
        Ok(output) => output,
        Err(diagnostic) => {
            return Err(Box::new(DynamicElfImageEmissionError {
                admitted,
                diagnostic,
            }));
        }
    };
    let emission = DynamicElfImageEmission { admitted, output };
    if let Err(diagnostic) = validate_dynamic_elf_image_emission(artifact, &emission) {
        return Err(Box::new(DynamicElfImageEmissionError {
            admitted: emission.admitted,
            diagnostic,
        }));
    }
    Ok(emission)
}

/// Independently replay one admitted dynamic ELF production-emission join.
pub fn validate_dynamic_elf_image_emission(
    artifact: &ObjectArtifact,
    emission: &DynamicElfImageEmission,
) -> Result<(), Diagnostic> {
    let expected = derive_output(artifact, &emission.admitted)?;
    if emission.output != expected {
        return Err(Diagnostic::error(
            "dynamic ELF production output drifted from exact admitted-byte custody",
        ));
    }
    Ok(())
}

fn derive_output(
    artifact: &ObjectArtifact,
    admitted: &ValidatedElfDynamicExecutable,
) -> Result<EmittedImageOutput, Diagnostic> {
    super::function_fragments::replay::validate(artifact)?;
    let target = artifact.target();
    if target != admitted.image().target
        || target.object_format != ObjectFormat::Elf
        || !matches!(
            target.architecture,
            Architecture::Aarch64 | Architecture::X86_64
        )
    {
        return Err(Diagnostic::error(
            "admitted dynamic ELF target does not match the exact Linux object artifact",
        ));
    }

    let mut replayed_image = image::build_final_image(FinalImageInput {
        target,
        object: artifact.object(),
        relocations: artifact.relocations(),
        text_bytes: artifact.text_bytes(),
        data_bytes: artifact.data_bytes(),
    });
    let symbol_digest = final_image_symbol_digest(&replayed_image);
    let mut output = emitted_direct_executable_output(admitted.output().clone());
    let validation = validate_terminal_dynamic_elf_image(artifact, &output)?;
    replayed_image.memory.text = output.final_text_bytes.clone();
    if replayed_image != *admitted.image()
        || symbol_digest != final_image_symbol_digest(admitted.image())
    {
        return Err(Diagnostic::error(
            "admitted dynamic ELF final image does not replay from the exact object artifact",
        ));
    }
    output.compiler_text_validation = Some(validation);
    Ok(output)
}
