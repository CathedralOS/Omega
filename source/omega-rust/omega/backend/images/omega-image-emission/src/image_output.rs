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

use super::final_image_validation::validate_terminal_image;
use super::{
    LINUX_X86_SCALAR_EXIT_SHIM_BYTES, LinuxX86ScalarExitShim, ObjectArtifact,
    ObjectBoundarySettlement, ObjectCodeAttribution, ObjectCompilerPrivateFunction, ObjectFunction,
    ObjectPortEffect, SCALAR_CALL_REFERENCE_FINGERPRINT,
};

fn validate_x86_scalar_fma_provider(artifact: &ObjectArtifact) -> Result<(), Diagnostic> {
    let fragments = artifact
        .functions
        .iter()
        .flat_map(|function| function.x86_scalar_fma.iter())
        .collect::<Vec<_>>();
    if fragments.is_empty() {
        if artifact.x86_scalar_fma_provider.is_some() {
            return Err(Diagnostic::error(
                "x86 scalar FMA provider admission has no retained instruction custody",
            ));
        }
        return Ok(());
    }
    let provider = artifact.x86_scalar_fma_provider.ok_or_else(|| {
        Diagnostic::error(
            "x86 scalar FMA feature requirements have no admitted executable provider",
        )
    })?;
    if !provider.has_canonical_identity()
        || Some(provider.profile()) != artifact.x86_feature_profile
        || provider.profile().native_target() != artifact.target
    {
        return Err(Diagnostic::error(
            "x86 scalar FMA executable provider admission does not match its exact object target",
        ));
    }
    for fragment in fragments {
        let slot = match fragment.format {
            omega_machine_code::X86ScalarFmaFormat::Binary32 => {
                omega_target::X86ScalarFmaSlot::Binary32
            }
            omega_machine_code::X86ScalarFmaFormat::Binary64 => {
                omega_target::X86ScalarFmaSlot::Binary64
            }
        };
        if !provider.admits(fragment.requirement, slot) {
            return Err(Diagnostic::error(format!(
                "x86 scalar FMA provider does not admit exact generic slot `{}`",
                slot.requirement_identity(),
            )));
        }
    }
    Ok(())
}

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
/// The direct lane admits typed internal calls and exactly retained imports
/// supported by its target writer. Final-text mutation outside their
/// architecture-specific immediate bits, unaccounted imports or thunks,
/// overlapping/missing function spans, and unclassified executable bytes are
/// hard failures.
pub fn emit_executable_image(
    artifact: &ObjectArtifact,
    subsystem: u16,
) -> Result<ExecutableImage, Diagnostic> {
    validate_x86_scalar_fma_provider(artifact)?;
    super::ranked_u32_countdown::replay_ranked_u32_countdown_final_image(artifact)?;
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
        x86_scalar_fma_provider: artifact.x86_scalar_fma_provider,
        subsystem: matches!(artifact.target.object_format, ObjectFormat::Coff).then_some(subsystem),
        functions: artifact.functions.clone(),
        private_functions: artifact.private_functions.clone(),
        semantic_code_attribution: artifact.semantic_code_attribution.clone(),
        port_effects: artifact.port_effects.clone(),
        boundary_settlements: artifact.boundary_settlements.clone(),
        foreign_calls: artifact.foreign_calls.clone(),
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
    validate_x86_scalar_fma_provider(artifact)?;
    super::ranked_u32_countdown::replay_ranked_u32_countdown_final_image(artifact)?;
    if artifact.psi() != image.psi()
        || artifact.target() != image.target()
        || artifact.x86_scalar_fma_provider() != image.x86_scalar_fma_provider()
        || artifact.functions() != image.functions()
        || artifact.private_functions() != image.private_functions()
        || artifact.semantic_code_attribution() != image.semantic_code_attribution()
        || artifact.port_effects() != image.port_effects()
        || artifact.boundary_settlements() != image.boundary_settlements()
        || artifact.foreign_calls() != image.foreign_calls()
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutableImage {
    psi: TerminalPsiIdentity,
    target: NativeTarget,
    x86_scalar_fma_provider: Option<omega_target::AdmittedX86ScalarFmaProvider>,
    subsystem: Option<u16>,
    functions: Vec<ObjectFunction>,
    private_functions: Vec<ObjectCompilerPrivateFunction>,
    semantic_code_attribution: Vec<ObjectCodeAttribution>,
    port_effects: Vec<ObjectPortEffect>,
    boundary_settlements: Vec<ObjectBoundarySettlement>,
    foreign_calls: Vec<super::ObjectForeignCall>,
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

    pub const fn x86_scalar_fma_provider(
        &self,
    ) -> Option<omega_target::AdmittedX86ScalarFmaProvider> {
        self.x86_scalar_fma_provider
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

    pub fn foreign_calls(&self) -> &[super::ObjectForeignCall] {
        &self.foreign_calls
    }

    pub fn functions(&self) -> &[ObjectFunction] {
        &self.functions
    }

    pub fn private_functions(&self) -> &[ObjectCompilerPrivateFunction] {
        &self.private_functions
    }

    pub fn port_effects(&self) -> &[ObjectPortEffect] {
        &self.port_effects
    }

    pub fn semantic_code_attribution(&self) -> &[ObjectCodeAttribution] {
        &self.semantic_code_attribution
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
