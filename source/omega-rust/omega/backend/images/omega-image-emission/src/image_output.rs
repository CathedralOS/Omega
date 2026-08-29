//! Object-container and direct executable-image output dispatch.
//!
//! This module selects the target writer, retains the sealed terminal-Psi
//! image carrier, and invokes independent final-image replay before returning.
//! It does not construct machine code or installation authority.

use omega_image::{EmittedImageOutput, FinalImageInput, emitted_direct_executable_output};
use omega_object_file::{ObjectContainerInput, ObjectContainerOutput, emit_omega_object_container};
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use psi_diagnostics::Diagnostic;
use psi_terminal::TerminalPsiIdentity;

use super::final_image_validation::{
    validate_terminal_image, validate_terminal_native_fuel_image,
    validate_terminal_native_fuel_transfer_runtime_image,
};
use super::{
    LINUX_X86_SCALAR_EXIT_SHIM_BYTES, LinuxX86ScalarExitShim, ObjectArtifact,
    ObjectBoundarySettlement, ObjectFuelAttribution, ObjectFunction, ObjectPortEffect,
    SCALAR_CALL_REFERENCE_FINGERPRINT, ValidatedNativeFuelArtifact, ValidatedNativeFuelFunction,
    ValidatedNativeFuelTransferRuntimeArtifact,
};
use omega_installation_evidence::NativeFuelTargetPlanProjection;

pub fn emit_object_container(artifact: &ObjectArtifact) -> ObjectContainer {
    ObjectContainer {
        psi: artifact.psi,
        output: emit_omega_object_container(ObjectContainerInput {
            target: artifact.target,
            object: &artifact.object,
            relocations: &artifact.relocations,
            text_bytes: &artifact.text_bytes,
            data_bytes: &[],
        }),
    }
}

pub fn emit_native_fuel_object_container(
    artifact: &ValidatedNativeFuelArtifact,
) -> ObjectContainer {
    ObjectContainer {
        psi: artifact.semantic_artifact().psi(),
        output: emit_omega_object_container(ObjectContainerInput {
            target: artifact.semantic_artifact().target(),
            object: artifact.object(),
            relocations: artifact.relocations(),
            text_bytes: artifact.text_bytes(),
            data_bytes: &[],
        }),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectContainer {
    pub psi: TerminalPsiIdentity,
    pub output: ObjectContainerOutput,
}

pub fn can_emit_executable_image(target: NativeTarget) -> bool {
    target.pointer_size == 8
        && target.pointer_alignment == 8
        && matches!(
            (target.object_format, target.architecture),
            (ObjectFormat::Elf, Architecture::Aarch64)
                | (ObjectFormat::Elf, Architecture::X86_64)
                | (ObjectFormat::MachO, Architecture::Aarch64)
                | (ObjectFormat::Coff, Architecture::X86_64)
        )
}

/// Emit and validate one direct executable image.
///
/// The clean lane admits only typed internal-call relocations. Final-text
/// mutation outside their architecture-specific immediate bits, imports,
/// appended thunks, overlapping/missing function spans, and unclassified
/// executable bytes are hard failures.
pub fn emit_executable_image(
    artifact: &ObjectArtifact,
    subsystem: u16,
) -> Result<ExecutableImage, Diagnostic> {
    if !can_emit_executable_image(artifact.target) {
        return Err(Diagnostic::error(format!(
            "cannot emit terminal-Psi executable image for {:?}",
            artifact.target
        )));
    }
    let image = omega_image::build_final_image(FinalImageInput {
        target: artifact.target,
        object: &artifact.object,
        relocations: &artifact.relocations,
        text_bytes: &artifact.text_bytes,
        data_bytes: &[],
    });
    let final_image_symbol_digest = omega_image::final_image_symbol_digest(&image);
    let output = match (artifact.target.object_format, artifact.target.architecture) {
        (ObjectFormat::Elf, Architecture::Aarch64) => {
            omega_image_elf::emit_elf_aarch64_executable(image)
        }
        (ObjectFormat::Elf, Architecture::X86_64) => {
            omega_image_elf::emit_elf_x86_64_executable(image)
        }
        (ObjectFormat::MachO, Architecture::Aarch64) => {
            omega_image_macho::emit_macho_aarch64_executable(image)
        }
        (ObjectFormat::Coff, Architecture::X86_64) => {
            omega_image_pe::emit_pe_x86_64_executable(image, subsystem)
        }
        _ => {
            return Err(Diagnostic::error(format!(
                "cannot emit terminal-Psi executable image for {:?}",
                artifact.target
            )));
        }
    }?;
    let mut output = emitted_direct_executable_output(output);
    output.compiler_text_validation = Some(validate_terminal_image(
        artifact,
        &artifact.object,
        &artifact.relocations,
        &artifact.text_bytes,
        None,
        &output,
    )?);
    Ok(ExecutableImage {
        psi: artifact.psi,
        target: artifact.target,
        subsystem: matches!(artifact.target.object_format, ObjectFormat::Coff).then_some(subsystem),
        functions: artifact.functions.clone(),
        fuel_attribution: artifact.fuel_attribution.clone(),
        port_effects: artifact.port_effects.clone(),
        boundary_settlements: artifact.boundary_settlements.clone(),
        final_image_symbol_digest,
        output,
    })
}

/// Independently replay the complete object-to-executable-image join retained
/// by a source-free native artifact.
///
/// This repeats final-text relocation-envelope validation and requires the
/// recomputed evidence to equal the evidence sealed by image construction.
pub fn validate_executable_image(
    artifact: &ObjectArtifact,
    image: &ExecutableImage,
) -> Result<(), Diagnostic> {
    if artifact.psi() != image.psi()
        || artifact.target() != image.target()
        || artifact.functions() != image.functions()
        || artifact.fuel_attribution() != image.fuel_attribution()
        || artifact.port_effects() != image.port_effects()
        || artifact.boundary_settlements() != image.boundary_settlements()
    {
        return Err(Diagnostic::error(
            "terminal object and executable image have different semantic or evidence identity",
        ));
    }
    let replayed_final_image = omega_image::build_final_image(FinalImageInput {
        target: artifact.target(),
        object: artifact.object(),
        relocations: artifact.relocations(),
        text_bytes: artifact.text_bytes(),
        data_bytes: &[],
    });
    if image.final_image_symbol_digest
        != omega_image::final_image_symbol_digest(&replayed_final_image)
    {
        return Err(Diagnostic::error(
            "terminal executable image symbol evidence does not match its exact object entry/data-symbol table",
        ));
    }
    let recomputed = validate_terminal_image(
        artifact,
        artifact.object(),
        artifact.relocations(),
        artifact.text_bytes(),
        None,
        image.output(),
    )?;
    if image.output().compiler_text_validation != Some(recomputed) {
        return Err(Diagnostic::error(
            "terminal executable image retained stale final-text validation evidence",
        ));
    }
    Ok(())
}

/// Emit the runnable Linux x86-64 image for the exact published proof-free i32
/// scalar-call reference.
///
/// This fixture/profile-specific API is deliberately not a general scalar
/// process adapter. `ObjectFunction` does not retain ordinary scalar
/// arity, and an unused entry parameter can produce byte-identical machine code.
/// Exact semantic-identity binding prevents such an entry from silently
/// acquiring zero-argument process-entry semantics.
pub fn emit_scalar_call_reference_linux_x86_64_image(
    artifact: &ObjectArtifact,
) -> Result<ScalarCallReferenceImage, Diagnostic> {
    if artifact.target != NativeTarget::linux_x64() {
        return Err(Diagnostic::error(format!(
            "Linux x86-64 scalar entry shim cannot target {:?}",
            artifact.target
        )));
    }
    if artifact.psi.vocabulary_marker != psi_terminal::VocabularyMarker::CURRENT
        || artifact.psi.program_fingerprint.as_bytes() != &SCALAR_CALL_REFERENCE_FINGERPRINT
    {
        return Err(Diagnostic::error(format!(
            "Linux x86-64 scalar-call reference image requires the exact published semantic identity; got {}:{}, expected {}:{:02x?}",
            artifact.psi.vocabulary_marker.get(),
            artifact.psi.program_fingerprint,
            psi_terminal::VocabularyMarker::CURRENT.get(),
            SCALAR_CALL_REFERENCE_FINGERPRINT,
        )));
    }
    let entry = artifact.entry_function();
    if entry.scalar_stack.is_none() || entry.bytes(artifact).last() != Some(&0xc3) {
        return Err(Diagnostic::error(format!(
            "terminal entry {} is not a completely accounted returning scalar function",
            artifact.entry
        )));
    }

    let mut object = artifact.object.clone();
    let mut relocations = artifact.relocations.clone();
    let mut text_bytes = artifact.text_bytes.clone();
    let text_offset = text_bytes.len();
    text_bytes.extend_from_slice(&LINUX_X86_SCALAR_EXIT_SHIM_BYTES);

    let text_section = object
        .layout
        .sections
        .iter()
        .find(|(_, section)| section.kind == omega_object_file::SectionKind::Text)
        .map(|(handle, _)| handle)
        .ok_or_else(|| Diagnostic::error("terminal object has no text section"))?;
    object.layout.sections.get_mut(text_section).size = text_bytes.len();

    let symbol = object.layout.symbols.insert(omega_object_file::SymbolPlan {
        name: "omega_terminal_linux_x86_64_scalar_exit_entry".into(),
        section: omega_object_file::SymbolSection::Section(omega_object_file::SectionKind::Text),
        offset: text_offset,
        size: LINUX_X86_SCALAR_EXIT_SHIM_BYTES.len(),
        kind: omega_object_file::SymbolKind::Function,
        import_library: String::new(),
    });
    object.layout.entry_symbol = symbol;
    let relocation_offset = text_offset
        .checked_add(1)
        .ok_or_else(|| Diagnostic::error("terminal scalar entry relocation offset overflows"))?;
    relocations.push_record(omega_object_file::RelocationRecord {
        origin: omega_object_file::RelocationOrigin::Instruction {
            function_symbol_handle: symbol,
            selected_instruction_index: 0,
        },
        section: omega_object_file::SectionKind::Text,
        offset: relocation_offset,
        byte_width: 4,
        symbol_handle: entry.symbol,
        addend: 0,
        kind: omega_object_file::RelocationKind::X86_64Relative32,
    });
    let shim = LinuxX86ScalarExitShim {
        symbol,
        target_symbol: entry.symbol,
        text_offset,
        byte_count: LINUX_X86_SCALAR_EXIT_SHIM_BYTES.len(),
        relocation_offset,
    };

    let image = omega_image::build_final_image(FinalImageInput {
        target: artifact.target,
        object: &object,
        relocations: &relocations,
        text_bytes: &text_bytes,
        data_bytes: &[],
    });
    let output = omega_image_elf::emit_elf_x86_64_executable(image)?;
    let mut output = emitted_direct_executable_output(output);
    output.compiler_text_validation = Some(validate_terminal_image(
        artifact,
        &object,
        &relocations,
        &text_bytes,
        Some(shim),
        &output,
    )?);
    Ok(ScalarCallReferenceImage {
        psi: artifact.psi,
        target: artifact.target,
        shim,
        output,
    })
}

pub fn emit_native_fuel_executable_image(
    artifact: &ValidatedNativeFuelArtifact,
    subsystem: u16,
) -> Result<NativeFuelExecutableImage, Diagnostic> {
    let target = artifact.semantic_artifact().target();
    if !can_emit_executable_image(target) {
        return Err(Diagnostic::error(format!(
            "cannot emit metered terminal-Psi executable image for {target:?}"
        )));
    }
    let image = omega_image::build_final_image(FinalImageInput {
        target,
        object: artifact.object(),
        relocations: artifact.relocations(),
        text_bytes: artifact.text_bytes(),
        data_bytes: &[],
    });
    let output = match (target.object_format, target.architecture) {
        (ObjectFormat::Elf, Architecture::Aarch64) => {
            omega_image_elf::emit_elf_aarch64_executable(image)
        }
        (ObjectFormat::Elf, Architecture::X86_64) => {
            omega_image_elf::emit_elf_x86_64_executable(image)
        }
        (ObjectFormat::MachO, Architecture::Aarch64) => {
            omega_image_macho::emit_macho_aarch64_executable(image)
        }
        (ObjectFormat::Coff, Architecture::X86_64) => {
            omega_image_pe::emit_pe_x86_64_executable(image, subsystem)
        }
        _ => {
            return Err(Diagnostic::error(format!(
                "cannot emit metered terminal-Psi executable image for {target:?}"
            )));
        }
    }?;
    let mut output = emitted_direct_executable_output(output);
    output.compiler_text_validation = Some(validate_terminal_native_fuel_image(artifact, &output)?);
    Ok(NativeFuelExecutableImage {
        artifact: artifact.clone(),
        subsystem: matches!(target.object_format, ObjectFormat::Coff).then_some(subsystem),
        output,
    })
}

/// Emit and replay-validate an image containing the exact compiler-owned
/// transfer and resume entries. The returned runtime evidence is read-only;
/// this operation does not grant installation or executable custody.
pub fn emit_native_fuel_transfer_runtime_executable_image(
    artifact: &ValidatedNativeFuelTransferRuntimeArtifact,
    subsystem: u16,
) -> Result<NativeFuelTransferRuntimeExecutableImage, Diagnostic> {
    let target = artifact.metered_artifact().semantic_artifact().target();
    if !can_emit_executable_image(target) {
        return Err(Diagnostic::error(format!(
            "cannot emit native fuel transfer image for {target:?}"
        )));
    }
    let image = omega_image::build_final_image(FinalImageInput {
        target,
        object: artifact.object(),
        relocations: artifact.relocations(),
        text_bytes: artifact.text_bytes(),
        data_bytes: &[],
    });
    let output = match (target.object_format, target.architecture) {
        (ObjectFormat::Elf, Architecture::X86_64) => {
            omega_image_elf::emit_elf_x86_64_executable(image)
        }
        (ObjectFormat::Elf, Architecture::Aarch64) => {
            omega_image_elf::emit_elf_aarch64_executable(image)
        }
        _ => {
            return Err(Diagnostic::error(format!(
                "native fuel transfer runtime has no terminal image emitter for {target:?}"
            )));
        }
    }?;
    let mut output = emitted_direct_executable_output(output);
    let (compiler_text_validation, transfer_runtime_evidence) =
        validate_terminal_native_fuel_transfer_runtime_image(artifact, &output)?;
    output.compiler_text_validation = Some(compiler_text_validation);
    Ok(NativeFuelTransferRuntimeExecutableImage {
        artifact: artifact.clone(),
        subsystem: matches!(target.object_format, ObjectFormat::Coff).then_some(subsystem),
        output,
        transfer_runtime_evidence,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeFuelTransferRuntimeExecutableImage {
    artifact: ValidatedNativeFuelTransferRuntimeArtifact,
    subsystem: Option<u16>,
    output: EmittedImageOutput,
    transfer_runtime_evidence: omega_installation_evidence::NativeFuelTransferRuntimeEvidence,
}

impl NativeFuelTransferRuntimeExecutableImage {
    pub const fn psi(&self) -> TerminalPsiIdentity {
        self.artifact.metered_artifact().semantic_artifact().psi()
    }

    pub const fn target(&self) -> NativeTarget {
        self.artifact
            .metered_artifact()
            .semantic_artifact()
            .target()
    }

    pub const fn subsystem(&self) -> Option<u16> {
        self.subsystem
    }

    pub const fn artifact(&self) -> &ValidatedNativeFuelTransferRuntimeArtifact {
        &self.artifact
    }

    pub const fn output(&self) -> &EmittedImageOutput {
        &self.output
    }

    pub const fn transfer_runtime_evidence(
        &self,
    ) -> &omega_installation_evidence::NativeFuelTransferRuntimeEvidence {
        &self.transfer_runtime_evidence
    }

    pub(crate) fn metered_installation_view(&self) -> NativeFuelExecutableImage {
        NativeFuelExecutableImage {
            artifact: self.artifact.metered_artifact().clone(),
            subsystem: self.subsystem,
            output: self.output.clone(),
        }
    }
}

impl omega_installation_evidence::NativeFuelTransferRuntimeImageEvidence
    for NativeFuelTransferRuntimeExecutableImage
{
    fn psi(&self) -> TerminalPsiIdentity {
        self.psi()
    }

    fn target(&self) -> NativeTarget {
        self.target()
    }

    fn unrelocated_text_bytes(&self) -> &[u8] {
        self.artifact.text_bytes()
    }

    fn final_text_bytes(&self) -> &[u8] {
        &self.output.final_text_bytes
    }

    fn sponsor_text_offset(&self) -> usize {
        self.artifact
            .object()
            .layout
            .symbols
            .get(self.artifact.sponsor_symbol())
            .offset
    }

    fn transfer_runtime_evidence(
        &self,
    ) -> &omega_installation_evidence::NativeFuelTransferRuntimeEvidence {
        &self.transfer_runtime_evidence
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeFuelExecutableImage {
    artifact: ValidatedNativeFuelArtifact,
    subsystem: Option<u16>,
    output: EmittedImageOutput,
}

impl NativeFuelExecutableImage {
    pub const fn psi(&self) -> TerminalPsiIdentity {
        self.artifact.semantic_artifact().psi()
    }

    pub const fn target(&self) -> NativeTarget {
        self.artifact.semantic_artifact().target()
    }

    pub const fn subsystem(&self) -> Option<u16> {
        self.subsystem
    }

    pub const fn target_policy(&self) -> NativeFuelTargetPlanProjection {
        self.artifact.target_policy()
    }

    pub fn functions(&self) -> &[ValidatedNativeFuelFunction] {
        self.artifact.functions()
    }

    /// Immutable semantic/source evidence retained beside the metered
    /// realization. Installation records keep its coordinates distinct from
    /// the executable metered coordinates.
    pub const fn semantic_artifact(&self) -> &ObjectArtifact {
        self.artifact.semantic_artifact()
    }

    pub fn metered_text_bytes(&self) -> &[u8] {
        self.artifact.text_bytes()
    }

    pub fn charges(&self) -> Vec<omega_installation_evidence::NativeFuelChargeEvidence> {
        omega_installation_evidence::NativeFuelImageEvidence::charges(self)
    }

    pub(crate) fn semantic_installation_view(&self) -> ExecutableImage {
        let semantic = self.artifact.semantic_artifact();
        let semantic_final_image = omega_image::build_final_image(FinalImageInput {
            target: semantic.target(),
            object: semantic.object(),
            relocations: semantic.relocations(),
            text_bytes: semantic.text_bytes(),
            data_bytes: &[],
        });
        ExecutableImage {
            psi: semantic.psi(),
            target: semantic.target(),
            subsystem: self.subsystem,
            functions: semantic.functions().to_vec(),
            fuel_attribution: semantic.fuel_attribution().to_vec(),
            port_effects: semantic.port_effects().to_vec(),
            boundary_settlements: semantic.boundary_settlements().to_vec(),
            final_image_symbol_digest: omega_image::final_image_symbol_digest(
                &semantic_final_image,
            ),
            output: self.output.clone(),
        }
    }

    pub const fn output(&self) -> &EmittedImageOutput {
        &self.output
    }
}

impl omega_installation_evidence::NativeFuelImageEvidence for NativeFuelExecutableImage {
    fn psi(&self) -> TerminalPsiIdentity {
        self.psi()
    }

    fn target(&self) -> NativeTarget {
        self.target()
    }

    fn target_policy(&self) -> NativeFuelTargetPlanProjection {
        self.target_policy()
    }

    fn source_text_bytes(&self) -> &[u8] {
        self.artifact.semantic_artifact().text_bytes()
    }

    fn metered_text_bytes(&self) -> &[u8] {
        self.artifact.text_bytes()
    }

    fn final_text_bytes(&self) -> &[u8] {
        &self.output.final_text_bytes
    }

    fn function_text_offset(&self, machine: psi_core::MachineId) -> Option<usize> {
        self.artifact
            .functions()
            .iter()
            .find(|function| function.machine == machine)
            .map(|function| function.text_offset)
    }

    fn charges(&self) -> Vec<omega_installation_evidence::NativeFuelChargeEvidence> {
        self.artifact
            .functions()
            .iter()
            .flat_map(|metered| {
                let source = self
                    .artifact
                    .semantic_artifact()
                    .functions()
                    .iter()
                    .find(|function| function.machine == metered.machine)
                    .expect("validated metered function retains its semantic source");
                metered.charges.iter().map(move |charge| {
                    let site = match charge.attribution.site {
                        omega_machine_code::NativeFuelSite::Operation(operation) => {
                            omega_installation_evidence::FuelAttributionSite::Operation(operation)
                        }
                        omega_machine_code::NativeFuelSite::Edge(edge) => {
                            omega_installation_evidence::FuelAttributionSite::Edge(edge)
                        }
                    };
                    omega_installation_evidence::NativeFuelChargeEvidence {
                        attribution: omega_installation_evidence::FuelAttributionEvidence {
                            machine: metered.machine,
                            schedule: charge.attribution.schedule,
                            site,
                            units: charge.attribution.units,
                            operation_ordinal: charge.attribution.operation_ordinal,
                            text_offset: source.text_offset + charge.attribution.code_offset,
                            byte_count: charge.attribution.byte_count,
                        },
                        charge_text_offset: metered.text_offset + charge.charge_code_offset,
                        charge_byte_count: charge.charge_byte_count,
                        semantic_text_offset: metered.text_offset + charge.semantic_code_offset,
                        cold_dispatch_text_offset: metered.text_offset
                            + charge.cold_dispatch_code_offset,
                        cold_dispatch_byte_count: charge.cold_dispatch_byte_count,
                    }
                })
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutableImage {
    psi: TerminalPsiIdentity,
    target: NativeTarget,
    subsystem: Option<u16>,
    functions: Vec<ObjectFunction>,
    fuel_attribution: Vec<ObjectFuelAttribution>,
    port_effects: Vec<ObjectPortEffect>,
    boundary_settlements: Vec<ObjectBoundarySettlement>,
    final_image_symbol_digest: omega_image::FinalImageSymbolDigest,
    output: EmittedImageOutput,
}

impl ExecutableImage {
    pub const fn psi(&self) -> TerminalPsiIdentity {
        self.psi
    }

    pub const fn target(&self) -> NativeTarget {
        self.target
    }

    /// PE/COFF subsystem selected by the writer. Other formats carry no
    /// subsystem fact because the argument is not interpreted by their writer.
    pub const fn subsystem(&self) -> Option<u16> {
        self.subsystem
    }

    pub const fn output(&self) -> &EmittedImageOutput {
        &self.output
    }

    pub fn boundary_settlements(&self) -> &[ObjectBoundarySettlement] {
        &self.boundary_settlements
    }

    pub fn functions(&self) -> &[ObjectFunction] {
        &self.functions
    }

    pub fn port_effects(&self) -> &[ObjectPortEffect] {
        &self.port_effects
    }

    pub fn fuel_attribution(&self) -> &[ObjectFuelAttribution] {
        &self.fuel_attribution
    }

    pub const fn final_image_symbol_digest(&self) -> omega_image::FinalImageSymbolDigest {
        self.final_image_symbol_digest
    }
}

/// Differential-only runnable image for the exact published scalar-call
/// reference. This deliberately is not `ExecutableImage`, so it cannot
/// be passed to installation-record or installed-artifact APIs that account
/// only semantic terminal functions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarCallReferenceImage {
    psi: TerminalPsiIdentity,
    target: NativeTarget,
    shim: LinuxX86ScalarExitShim,
    output: EmittedImageOutput,
}

impl ScalarCallReferenceImage {
    pub const fn psi(&self) -> TerminalPsiIdentity {
        self.psi
    }

    pub const fn target(&self) -> NativeTarget {
        self.target
    }

    pub const fn linux_x86_scalar_exit_shim(&self) -> LinuxX86ScalarExitShim {
        self.shim
    }

    pub const fn output(&self) -> &EmittedImageOutput {
        &self.output
    }
}
