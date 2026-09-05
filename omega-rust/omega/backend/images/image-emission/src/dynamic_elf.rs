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
    super::ranked_u32_countdown::replay_ranked_u32_countdown_final_image(artifact)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build_object_artifact;
    use image::FinalImageInput;
    use image_elf::{
        admit_elf_dynamic_executable, apply_elf_dynamic_address_fixups,
        apply_elf_procedure_linkage_fixups, apply_elf_section_header_placements,
        assemble_elf_dynamic_file, plan_elf_dynamic_link_inputs, plan_elf_dynamic_load_layout,
        plan_elf_dynamic_section_descriptors, plan_elf_dynamic_section_roster,
        plan_elf_dynamic_sections, plan_elf_dynamic_table_section_descriptor,
        plan_elf_dynamic_tags, plan_elf_indexed_section_payloads,
        plan_elf_procedure_linkage_relocations, plan_elf_procedure_linkage_section_descriptors,
        plan_elf_procedure_linkage_templates, plan_elf_relative_section_payload_layout,
        plan_elf_section_name_table, serialize_elf_dynamic_file_envelope,
        serialize_elf_dynamic_sections, serialize_elf_dynamic_table,
        serialize_elf_section_header_table,
    };
    use machine_code::MachineCodePlan;
    use semantic_vocabulary::{MachineId, OperationId};
    use target::{
        ForeignLocatorCandidate, TargetProfile, normalize_elf_interpreter_plan,
        normalize_foreign_locator,
    };
    use target_operations::TerminalPsiProvenance;
    use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

    fn machine_code_plan(target: TargetProfile) -> MachineCodePlan {
        let native = target.native_target();
        let locators = [b"alpha_call".as_slice(), b"beta_call"]
            .into_iter()
            .map(|name| {
                normalize_foreign_locator(
                    ForeignLocatorCandidate::ElfVersioned {
                        object: b"libproduction-emitter.so".to_vec(),
                        symbol: name.to_vec(),
                        version: b"PRODUCTION_EMITTER_1".to_vec(),
                    },
                    target,
                )
                .unwrap()
            })
            .collect::<Vec<_>>();

        let signature = calling_conventions::CallSignature {
            parameters: Vec::new(),
            result: None,
        };
        let boundary_entry_plan = calling_conventions::evaluate_ordinary_boundary_entry_plan(
            calling_conventions::CallingPolicy::native_for_target(native),
            &signature,
        )
        .unwrap()
        .plan()
        .clone();
        let provider_plan_commitment =
            task_plans::SameStackProviderPlanCommitment::from_digest([0x5a; 32]);
        let same_stack_contribution = task_plans::admit_same_stack_contribution(
            task_plans::SameStackContributionAdmissionCandidate {
                provider_plan_report_identity: 1,
                provider_plan_commitment,
                requirement_identity: "production-emitter-import".into(),
                receipt:
                    task_plans::SameStackContributionAdmissionReceiptId::from_normalized_identity(1)
                        .unwrap(),
                bytes: 64,
                alignment: 16,
            },
            1,
            provider_plan_commitment,
            "production-emitter-import",
        )
        .unwrap();
        let machine = MachineId::new(1).unwrap();
        let return_edge = semantic_vocabulary::EdgeId::new(1).unwrap();
        let operations = (1..=3)
            .map(|identity| OperationId::new(identity).unwrap())
            .collect::<Vec<_>>();
        let provider_execution =
            target_operations::ProviderExecutionBinding::from_execution_record(
                target_operations::ProviderPlanReportIdentity::new(1).unwrap(),
                2,
                3,
                4,
                5,
            )
            .unwrap();
        let target_plan = target_operations::TargetOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([0x5a; 32]),
            },
            target: native,
            entry: machine,
            functions: vec![target_operations::TargetFunction {
                fixed_integer_scalar_abi: None,
                mixed_structural_scalar_abi: None,
                machine,
                attachment: None,
                provenance: TerminalPsiProvenance {
                    operations: operations.clone(),
                    edges: vec![return_edge],
                },
                operation: target_operations::TargetOperation::UnitBody(
                    target_operations::TargetUnitBody {
                        structural_types: Vec::new(),
                        call_plan: calling_conventions::evaluate_call_plan(
                            calling_conventions::CallingPolicy::native_for_target(native),
                            &signature,
                        )
                        .unwrap(),
                        scalar_parameters: Vec::new(),
                        parameters: Vec::new(),
                        operations: operations
                            .iter()
                            .copied()
                            .zip([0, 0, 1])
                            .map(|(psi_operation, locator_index)| {
                                target_operations::TargetUnitOperation::NormalizedForeignCall {
                                    psi_operation,
                                    boundary: semantic_vocabulary::BoundaryMachineId::new(
                                        psi_operation.get(),
                                    )
                                    .unwrap(),
                                    provider_execution,
                                    binding: target_operations::NormalizedForeignCallBinding {
                                        locator: locators[locator_index].clone(),
                                        boundary_entry_plan: boundary_entry_plan.clone(),
                                        same_stack_contribution: same_stack_contribution.clone(),
                                    },
                                    scalar_arguments: Vec::new(),
                                    result_home: None,
                                }
                            })
                            .chain(std::iter::once(
                                target_operations::TargetUnitOperation::Return {
                                    psi_edge: return_edge,
                                    cleanup_actions: Vec::new(),
                                },
                            ))
                            .collect(),
                    },
                ),
            }],
        };
        let assigned =
            target_operations_to_assigned_target_operations::assign_registers(&target_plan)
                .unwrap();
        machine_emission::emit_machine_code(&assigned).unwrap()
    }

    fn artifact(target: TargetProfile) -> ObjectArtifact {
        build_object_artifact(&machine_code_plan(target)).expect("normalized foreign-call object")
    }

    fn direct_artifact(target: TargetProfile) -> ObjectArtifact {
        let native = target.native_target();
        let machine = MachineId::new(1).unwrap();
        let return_edge = semantic_vocabulary::EdgeId::new(1).unwrap();
        let call_plan = calling_conventions::evaluate_call_plan(
            calling_conventions::CallingPolicy::native_for_target(native),
            &calling_conventions::CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .unwrap();
        let target_plan = target_operations::TargetOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([0x6b; 32]),
            },
            target: native,
            entry: machine,
            functions: vec![target_operations::TargetFunction {
                fixed_integer_scalar_abi: None,
                mixed_structural_scalar_abi: None,
                machine,
                attachment: None,
                provenance: TerminalPsiProvenance {
                    operations: Vec::new(),
                    edges: vec![return_edge],
                },
                operation: target_operations::TargetOperation::UnitBody(
                    target_operations::TargetUnitBody {
                        structural_types: Vec::new(),
                        call_plan,
                        scalar_parameters: Vec::new(),
                        parameters: Vec::new(),
                        operations: vec![target_operations::TargetUnitOperation::Return {
                            psi_edge: return_edge,
                            cleanup_actions: Vec::new(),
                        }],
                    },
                ),
            }],
        };
        let assigned =
            target_operations_to_assigned_target_operations::assign_registers(&target_plan)
                .unwrap();
        let machine_code = machine_emission::emit_machine_code(&assigned).unwrap();
        build_object_artifact(&machine_code).expect("import-free direct object")
    }

    fn admitted(artifact: &ObjectArtifact, target: TargetProfile) -> ValidatedElfDynamicExecutable {
        let image = image::build_final_image(FinalImageInput {
            target: artifact.target(),
            object: artifact.object(),
            relocations: artifact.relocations(),
            text_bytes: artifact.text_bytes(),
            data_bytes: artifact.data_bytes(),
        });
        let inputs = plan_elf_dynamic_link_inputs(image, interpreter(target)).unwrap();
        let sections = plan_elf_dynamic_sections(inputs).unwrap();
        let payloads = serialize_elf_dynamic_sections(sections).unwrap();
        let descriptors = plan_elf_dynamic_section_descriptors(payloads).unwrap();
        let linkage = plan_elf_procedure_linkage_relocations(descriptors).unwrap();
        let templates = plan_elf_procedure_linkage_templates(linkage).unwrap();
        let descriptors = plan_elf_procedure_linkage_section_descriptors(templates).unwrap();
        let tags = plan_elf_dynamic_tags(descriptors).unwrap();
        let dynamic = serialize_elf_dynamic_table(tags).unwrap();
        let descriptor = plan_elf_dynamic_table_section_descriptor(dynamic).unwrap();
        let names = plan_elf_section_name_table(descriptor).unwrap();
        let roster = plan_elf_dynamic_section_roster(names).unwrap();
        let headers = serialize_elf_section_header_table(roster).unwrap();
        let payloads = plan_elf_indexed_section_payloads(headers).unwrap();
        let relative = plan_elf_relative_section_payload_layout(payloads).unwrap();
        let load = plan_elf_dynamic_load_layout(relative).unwrap();
        let placed = apply_elf_section_header_placements(load).unwrap();
        let resolved = apply_elf_dynamic_address_fixups(placed).unwrap();
        let envelope = serialize_elf_dynamic_file_envelope(resolved).unwrap();
        let linkage = apply_elf_procedure_linkage_fixups(envelope).unwrap();
        let assembled = assemble_elf_dynamic_file(linkage).unwrap();
        admit_elf_dynamic_executable(assembled).unwrap()
    }

    fn interpreter(target: TargetProfile) -> NormalizedElfInterpreterPlan {
        let path = match target {
            TargetProfile::LinuxX64 => b"/lib64/ld-linux-x86-64.so.2".as_slice(),
            TargetProfile::LinuxArm64 => b"/lib/ld-linux-aarch64.so.1".as_slice(),
            _ => unreachable!(),
        };
        normalize_elf_interpreter_plan(path.to_vec(), target).unwrap()
    }

    #[test]
    fn import_bearing_object_orchestration_is_exact_for_both_linux_targets() {
        for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
            let artifact = artifact(target);
            assert_eq!(artifact.object().layout.normalized_imports.len(), 2);
            assert_eq!(
                artifact
                    .object()
                    .layout
                    .symbols
                    .iter()
                    .filter(|(_, symbol)| symbol.kind == object_file::SymbolKind::Import)
                    .count(),
                2,
            );
            assert_eq!(artifact.relocations().record_count(), 3);
            match target {
                TargetProfile::LinuxX64 => {
                    let controls = artifact
                        .foreign_calls()
                        .iter()
                        .map(|call| call.x86_floating_control.expect("x86 MXCSR custody"))
                        .collect::<Vec<_>>();
                    assert!(controls.windows(2).all(|pair| {
                        pair[0].saved_slot_byte_offset == pair[1].saved_slot_byte_offset
                            && pair[0].restore_offset + pair[0].restore_byte_count
                                <= pair[1].save_offset
                    }));
                }
                TargetProfile::LinuxArm64 => {
                    let controls = artifact
                        .foreign_calls()
                        .iter()
                        .map(|call| {
                            assert_eq!(call.x86_floating_control, None);
                            call.aarch64_floating_control.expect("AArch64 FPCR custody")
                        })
                        .collect::<Vec<_>>();
                    assert!(controls.windows(2).all(|pair| {
                        pair[0].saved_slot_byte_offset == pair[1].saved_slot_byte_offset
                            && pair[0].restore_offset + pair[0].restore_byte_count
                                <= pair[1].save_offset
                    }));
                }
                _ => unreachable!(),
            }
            let first = emit_dynamic_elf_image(&artifact, interpreter(target)).unwrap();
            let replay = emit_dynamic_elf_image(&artifact, interpreter(target)).unwrap();

            validate_dynamic_elf_image_emission(&artifact, &first).unwrap();
            assert_eq!(first.output(), replay.output());
            assert_eq!(first.output().final_image_imports, 2);
            assert_eq!(first.output().final_image_relocations, 3);
            assert!(first.output().bytes.starts_with(b"\x7fELF"));
            assert!(first.output().format.contains("dynamic-executable"));
        }
    }

    #[test]
    fn image_request_router_selects_dynamic_elf_only_from_exact_object_custody() {
        for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
            let artifact = artifact(target);
            let requested = emit_requested_executable_image(
                &artifact,
                ExecutableImageEmissionRequest::dynamic_elf(interpreter(target)),
            )
            .expect("import-bearing ELF request selects the dynamic owner");
            let RequestedExecutableImage::DynamicElf(emission) = &requested else {
                panic!("normalized ELF imports cannot enter the direct writer")
            };
            assert_eq!(emission.output().final_image_imports, 2);
            assert_eq!(emission.output().final_image_relocations, 3);
            validate_requested_executable_image(&artifact, &requested)
                .expect("selected dynamic image replays independently");
        }
    }

    #[test]
    fn image_request_router_rejects_psi_only_dynamic_artifact_substitution() {
        let artifact = artifact(TargetProfile::LinuxX64);
        let requested = emit_requested_executable_image(
            &artifact,
            ExecutableImageEmissionRequest::dynamic_elf(interpreter(TargetProfile::LinuxX64)),
        )
        .expect("exact import-bearing ELF request");
        let RequestedExecutableImage::DynamicElf(image) = &requested else {
            unreachable!("fixture has normalized ELF imports")
        };
        assert_eq!(image.artifact(), &artifact);

        let mut substituted = artifact.clone();
        substituted.psi = TerminalPsiIdentity {
            vocabulary_marker: artifact.psi().vocabulary_marker,
            program_fingerprint: SemanticFingerprint::from_bytes([0x7c; 32]),
        };
        assert_eq!(substituted.object(), artifact.object());
        assert_eq!(substituted.relocations(), artifact.relocations());
        assert_eq!(substituted.text_bytes(), artifact.text_bytes());
        assert_eq!(substituted.data_bytes(), artifact.data_bytes());
        assert_ne!(substituted.psi(), artifact.psi());
        assert!(validate_dynamic_elf_image_emission(&substituted, image.emission()).is_ok());
        assert!(validate_requested_executable_image(&substituted, &requested).is_err());
    }

    #[test]
    fn image_request_router_preserves_direct_authority_for_import_free_objects() {
        for target in [
            TargetProfile::LinuxX64,
            TargetProfile::LinuxArm64,
            TargetProfile::MacosArm64,
            TargetProfile::WindowsX64,
        ] {
            let artifact = direct_artifact(target);
            assert!(artifact.object().layout.normalized_imports.is_empty());
            let requested = emit_requested_executable_image(
                &artifact,
                ExecutableImageEmissionRequest::direct(73),
            )
            .expect("import-free object selects the direct writer");
            let RequestedExecutableImage::Direct(image) = &requested else {
                panic!("import-free object cannot acquire dynamic ELF custody")
            };
            assert_eq!(image.output().final_image_imports, 0);
            assert_eq!(
                image.subsystem(),
                (target == TargetProfile::WindowsX64).then_some(73),
            );
            validate_requested_executable_image(&artifact, &requested)
                .expect("selected direct image replays independently");
        }
    }

    #[test]
    fn image_request_router_rejects_missing_unused_and_mismatched_interpreters() {
        let artifact = artifact(TargetProfile::LinuxX64);
        let missing =
            emit_requested_executable_image(&artifact, ExecutableImageEmissionRequest::direct(73))
                .expect_err("normalized ELF imports cannot fall back to the direct writer");
        assert!(matches!(
            *missing,
            RequestedExecutableImageError::MissingDynamicElfInterpreter {
                target,
                subsystem: 73,
                ..
            } if target == TargetProfile::LinuxX64.native_target()
        ));

        let wrong_path = interpreter(TargetProfile::LinuxArm64);
        let expected_path = wrong_path.interpreter_path().to_vec();
        let mismatched = emit_requested_executable_image(
            &artifact,
            ExecutableImageEmissionRequest::dynamic_elf(wrong_path),
        )
        .expect_err("cross-target interpreter substitution must fail in the ELF owner");
        let RequestedExecutableImageError::DynamicElf(error) = *mismatched else {
            panic!("target mismatch must retain the dynamic orchestration failure")
        };
        assert_eq!(error.stage(), "link-inputs");
        let DynamicElfOrchestrationError::LinkInputs(error) = *error else {
            unreachable!()
        };
        let (_, recovered_interpreter, _) = error.into_parts();
        assert_eq!(recovered_interpreter.interpreter_path(), expected_path);

        let mut no_imports = artifact;
        no_imports.object.layout.normalized_imports.clear();
        let exact_interpreter = interpreter(TargetProfile::LinuxX64);
        let expected_path = exact_interpreter.interpreter_path().to_vec();
        let unexpected = emit_requested_executable_image(
            &no_imports,
            ExecutableImageEmissionRequest::dynamic_elf(exact_interpreter),
        )
        .expect_err("loader input cannot select dynamic ELF without normalized imports");
        let recovered = unexpected
            .into_unexpected_interpreter()
            .expect("unused normalized interpreter is returned intact");
        assert_eq!(recovered.interpreter_path(), expected_path);
    }

    #[test]
    fn admitted_foreign_stack_leaves_compose_by_max_with_strong_provenance() {
        for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
            let artifact = artifact(target);
            assert_eq!(artifact.foreign_calls().len(), 3);
            let contribution = artifact.foreign_calls()[0].same_stack_contribution.clone();
            assert!(
                artifact
                    .foreign_calls()
                    .iter()
                    .all(|call| call.same_stack_contribution == contribution)
            );
            let local_peak = u64::from(
                artifact.functions()[0]
                    .unit_stack
                    .expect("foreign Unit body retains its local stack")
                    .local_peak_bytes,
            );
            let foreign_peak = artifact
                .foreign_calls()
                .iter()
                .map(|call| {
                    u64::from(call.caller_live_bytes)
                        .checked_add(call.same_stack_contribution.bytes())
                        .unwrap()
                })
                .max()
                .unwrap();
            let demand = crate::derive_stack_demand(&artifact, artifact.entry()).unwrap();
            assert_eq!(demand.ceiling_bytes(), local_peak.max(foreign_peak));
            assert!(
                demand.ceiling_bytes()
                    < artifact
                        .foreign_calls()
                        .iter()
                        .map(|call| call.same_stack_contribution.bytes())
                        .sum::<u64>(),
                "sequential foreign leaves share one maximum stack extent",
            );
            assert_eq!(
                demand.admitted_contribution_report_identities(),
                &std::collections::BTreeSet::from([contribution.report_identity()]),
            );
            assert_eq!(
                demand.admitted_contribution_commitments(),
                &std::collections::BTreeSet::from([contribution.commitment()]),
            );
        }
    }

    #[test]
    fn foreign_stack_composition_overflow_rejects() {
        let mut plan = machine_code_plan(TargetProfile::LinuxX64);
        let provider_plan_commitment =
            task_plans::SameStackProviderPlanCommitment::from_digest([0x5a; 32]);
        plan.functions[0].foreign_calls[0].same_stack_contribution =
            task_plans::admit_same_stack_contribution(
                task_plans::SameStackContributionAdmissionCandidate {
                    provider_plan_report_identity: 1,
                    provider_plan_commitment,
                    requirement_identity: "production-emitter-import".into(),
                    receipt: task_plans::SameStackContributionAdmissionReceiptId::from_normalized_identity(2)
                        .unwrap(),
                    bytes: u64::MAX,
                    alignment: 16,
                },
                1,
                provider_plan_commitment,
                "production-emitter-import",
            )
            .unwrap();
        let artifact = build_object_artifact(&plan).unwrap();
        assert!(matches!(
            crate::derive_stack_demand(&artifact, artifact.entry()),
            Err(crate::ObjectError::TerminalStackCompositionOverflow { .. })
        ));
    }

    #[test]
    fn object_builder_rejects_foreign_placeholder_and_target_drift() {
        let mut malformed = machine_code_plan(TargetProfile::LinuxX64);
        let opcode = malformed.functions[0].foreign_calls[0].offset - 1;
        malformed.functions[0].bytes[opcode] = 0x90;
        assert!(matches!(
            build_object_artifact(&malformed),
            Err(crate::ObjectError::InvalidForeignCallSite { .. })
        ));

        let mut wrong_target = machine_code_plan(TargetProfile::LinuxX64);
        wrong_target.target = TargetProfile::LinuxArm64.native_target();
        assert!(matches!(
            build_object_artifact(&wrong_target),
            Err(crate::ObjectError::ForeignCallTargetMismatch { .. })
        ));

        let mut missing_control = machine_code_plan(TargetProfile::LinuxX64);
        missing_control.functions[0].foreign_calls[0].x86_floating_control = None;
        assert!(matches!(
            build_object_artifact(&missing_control),
            Err(crate::ObjectError::InvalidForeignCallFloatingControl { .. })
        ));

        let mut wrong_slot = machine_code_plan(TargetProfile::LinuxX64);
        wrong_slot.functions[0].foreign_calls[0]
            .x86_floating_control
            .as_mut()
            .unwrap()
            .saved_slot_byte_offset = u32::MAX;
        assert!(matches!(
            build_object_artifact(&wrong_slot),
            Err(crate::ObjectError::InvalidForeignCallFloatingControl { .. })
        ));

        let mut mutated_save = machine_code_plan(TargetProfile::LinuxX64);
        let save_offset = mutated_save.functions[0].foreign_calls[0]
            .x86_floating_control
            .unwrap()
            .save_offset;
        mutated_save.functions[0].bytes[save_offset] ^= 1;
        assert!(matches!(
            build_object_artifact(&mutated_save),
            Err(crate::ObjectError::InvalidForeignCallFloatingControl { .. })
        ));

        let mut substituted_interval = machine_code_plan(TargetProfile::LinuxX64);
        substituted_interval.functions[0].foreign_calls[0].x86_floating_control =
            substituted_interval.functions[0].foreign_calls[1].x86_floating_control;
        assert!(matches!(
            build_object_artifact(&substituted_interval),
            Err(crate::ObjectError::InvalidForeignCallFloatingControl { .. })
        ));

        let mut missing_aarch64_control = machine_code_plan(TargetProfile::LinuxArm64);
        missing_aarch64_control.functions[0].foreign_calls[0].aarch64_floating_control = None;
        assert!(matches!(
            build_object_artifact(&missing_aarch64_control),
            Err(crate::ObjectError::InvalidForeignCallFloatingControl { .. })
        ));

        let mut wrong_aarch64_slot = machine_code_plan(TargetProfile::LinuxArm64);
        let control = {
            let control = wrong_aarch64_slot.functions[0].foreign_calls[0]
                .aarch64_floating_control
                .as_mut()
                .unwrap();
            control.saved_slot_byte_offset = 8;
            *control
        };
        wrong_aarch64_slot.functions[0].bytes
            [control.save_offset..control.save_offset + control.save_byte_count]
            .copy_from_slice(&isa_aarch64::encode_save_fpcr_to_sp_displacement(8).unwrap());
        wrong_aarch64_slot.functions[0].bytes
            [control.restore_offset..control.restore_offset + control.restore_byte_count]
            .copy_from_slice(&isa_aarch64::encode_restore_fpcr_from_sp_displacement(8).unwrap());
        assert!(matches!(
            build_object_artifact(&wrong_aarch64_slot),
            Err(crate::ObjectError::InvalidForeignCallFloatingControl { .. })
        ));

        let mut mutated_aarch64_save = machine_code_plan(TargetProfile::LinuxArm64);
        let save_offset = mutated_aarch64_save.functions[0].foreign_calls[0]
            .aarch64_floating_control
            .unwrap()
            .save_offset;
        mutated_aarch64_save.functions[0].bytes[save_offset] ^= 1;
        assert!(matches!(
            build_object_artifact(&mutated_aarch64_save),
            Err(crate::ObjectError::InvalidForeignCallFloatingControl { .. })
        ));

        let mut substituted_aarch64_interval = machine_code_plan(TargetProfile::LinuxArm64);
        substituted_aarch64_interval.functions[0].foreign_calls[0].aarch64_floating_control =
            substituted_aarch64_interval.functions[0].foreign_calls[1].aarch64_floating_control;
        assert!(matches!(
            build_object_artifact(&substituted_aarch64_interval),
            Err(crate::ObjectError::InvalidForeignCallFloatingControl { .. })
        ));
    }

    #[test]
    fn tampered_call_placeholder_rejects_mid_chain_with_exact_stage_custody() {
        let mut artifact = artifact(TargetProfile::LinuxX64);
        let opcode = artifact
            .text_bytes
            .iter()
            .position(|byte| *byte == 0xe8)
            .expect("x86 CALL opcode");
        artifact.text_bytes[opcode] = 0x90;
        let exact_text = artifact.text_bytes.clone();
        let exact_interpreter = interpreter(TargetProfile::LinuxX64);
        let exact_interpreter_path = exact_interpreter.interpreter_path().to_vec();

        let error = emit_dynamic_elf_image(&artifact, exact_interpreter).unwrap_err();
        assert_eq!(error.stage(), "procedure-linkage-relocations");
        let DynamicElfOrchestrationError::ProcedureLinkageRelocations(error) = *error else {
            panic!("tampered call must reject at procedure-linkage relocation planning")
        };
        let (descriptors, diagnostic) = error.into_parts();
        let inputs = descriptors.payloads().plan().inputs();
        assert!(diagnostic.to_string().contains("CALL rel32"));
        assert_eq!(inputs.image().memory.text, exact_text);
        assert_eq!(
            inputs.interpreter().interpreter_path(),
            exact_interpreter_path
        );
        assert_eq!(inputs.referenced_import_count(), 2);
    }

    #[test]
    fn mismatched_interpreter_rejects_with_both_initial_inputs_recoverable() {
        let artifact = artifact(TargetProfile::LinuxX64);
        let exact_text = artifact.text_bytes.clone();
        let interpreter = interpreter(TargetProfile::LinuxArm64);
        let exact_interpreter_path = interpreter.interpreter_path().to_vec();

        let error = emit_dynamic_elf_image(&artifact, interpreter).unwrap_err();
        assert_eq!(error.stage(), "link-inputs");
        let DynamicElfOrchestrationError::LinkInputs(error) = *error else {
            panic!("target mismatch must reject at link-input planning")
        };
        let (image, interpreter, diagnostic) = error.into_parts();
        assert!(diagnostic.to_string().contains("profile does not match"));
        assert_eq!(image.memory.text, exact_text);
        assert_eq!(
            interpreter.interpreter_path(),
            exact_interpreter_path.as_slice()
        );
        assert_eq!(interpreter.target(), TargetProfile::LinuxArm64);
    }

    #[test]
    fn both_linux_targets_rejoin_exact_admitted_bytes_without_installation_authority() {
        for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
            let artifact = artifact(target);
            let admitted = admitted(&artifact, target);
            let exact_bytes = admitted.output().bytes.clone();
            let report_fingerprint =
                admitted.non_authoritative_assembled_file_compatibility_fingerprint();

            let emission = emit_admitted_dynamic_elf_image(&artifact, admitted).unwrap();
            validate_dynamic_elf_image_emission(&artifact, &emission).unwrap();
            assert_eq!(emission.output().bytes, exact_bytes);
            assert_eq!(
                emission
                    .admitted()
                    .non_authoritative_assembled_file_compatibility_fingerprint(),
                report_fingerprint,
            );
            assert_eq!(emission.output().final_image_imports, 2);
            assert!(emission.output().format.contains("dynamic-executable"));
        }
    }

    #[test]
    fn target_drift_rejects_with_exact_admitted_custody() {
        let source = artifact(TargetProfile::LinuxX64);
        let admitted = admitted(&source, TargetProfile::LinuxX64);
        let exact_bytes = admitted.output().bytes.clone();
        let report_fingerprint =
            admitted.non_authoritative_assembled_file_compatibility_fingerprint();
        let wrong_artifact = artifact(TargetProfile::LinuxArm64);

        let error = emit_admitted_dynamic_elf_image(&wrong_artifact, admitted).unwrap_err();
        let (admitted, diagnostic) = error.into_parts();
        assert!(diagnostic.to_string().contains("target does not match"));
        assert_eq!(admitted.output().bytes, exact_bytes);
        assert_eq!(
            admitted.non_authoritative_assembled_file_compatibility_fingerprint(),
            report_fingerprint,
        );
    }

    #[test]
    fn artifact_drift_rejects_with_exact_admitted_custody() {
        let mut artifact = artifact(TargetProfile::LinuxX64);
        let admitted = admitted(&artifact, TargetProfile::LinuxX64);
        let exact_bytes = admitted.output().bytes.clone();
        let report_fingerprint =
            admitted.non_authoritative_assembled_file_compatibility_fingerprint();
        artifact.text_bytes[0] ^= 1;

        let error = emit_admitted_dynamic_elf_image(&artifact, admitted).unwrap_err();
        let (admitted, diagnostic) = error.into_parts();
        assert!(
            diagnostic
                .to_string()
                .contains("changed outside its declared relocation field")
        );
        assert_eq!(admitted.output().bytes, exact_bytes);
        assert_eq!(
            admitted.non_authoritative_assembled_file_compatibility_fingerprint(),
            report_fingerprint,
        );
    }

    #[test]
    fn import_drift_rejects_with_exact_admitted_custody() {
        let mut artifact = artifact(TargetProfile::LinuxX64);
        let admitted = admitted(&artifact, TargetProfile::LinuxX64);
        let exact_bytes = admitted.output().bytes.clone();
        let report_fingerprint =
            admitted.non_authoritative_assembled_file_compatibility_fingerprint();
        artifact.object.layout.normalized_imports[0].locator = normalize_foreign_locator(
            ForeignLocatorCandidate::ElfVersioned {
                object: b"libproduction-emitter.so".to_vec(),
                symbol: b"mutated_call".to_vec(),
                version: b"PRODUCTION_EMITTER_1".to_vec(),
            },
            TargetProfile::LinuxX64,
        )
        .unwrap();

        let error = emit_admitted_dynamic_elf_image(&artifact, admitted).unwrap_err();
        let (admitted, diagnostic) = error.into_parts();
        assert!(
            diagnostic
                .to_string()
                .contains("does not replay from the exact object artifact")
        );
        assert_eq!(admitted.output().bytes, exact_bytes);
        assert_eq!(
            admitted.non_authoritative_assembled_file_compatibility_fingerprint(),
            report_fingerprint,
        );
    }

    #[test]
    fn independent_replay_rejects_production_output_mutation() {
        let artifact = artifact(TargetProfile::LinuxX64);
        let admitted = admitted(&artifact, TargetProfile::LinuxX64);
        let mut emission = emit_admitted_dynamic_elf_image(&artifact, admitted).unwrap();
        emission.output.bytes[0] ^= 1;
        assert!(validate_dynamic_elf_image_emission(&artifact, &emission).is_err());
    }
}
