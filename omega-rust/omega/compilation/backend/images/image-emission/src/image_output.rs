//! Object-container and executable-image output: the emission entry.
//!
//! [`emit_executable_image`] is the single entry: the object's contents and
//! the caller's [`ExecutableImageEmissionRequest`] select the direct writer
//! (ELF, Mach-O or PE over a static image, owned here) or the dynamic ELF
//! lane (`dynamic_elf`). The direct lane retains the
//! sealed terminal-Psi image carrier and builds the final image once; it
//! does not replay the object it was just handed (verification placement in
//! `omega-rust/pipeline.md`). Nothing here constructs machine code or
//! installation authority.

mod application_metadata;
pub use application_metadata::ImageApplicationMetadataError;

use diagnostics::Diagnostic;
use image::{EmittedImageOutput, FinalImageInput, emitted_direct_executable_output};
use installation_evidence::ObjectEvidence;
use object_file::ObjectContainerOutput;
use target::{Architecture, NativeTarget, NormalizedElfInterpreterPlan, ObjectFormat};
use terminal_psi::TerminalPsiIdentity;

use super::dynamic_elf::{
    DynamicElfOrchestrationError, RequestedDynamicElfImage, emit_dynamic_elf_image,
};
use super::final_image_validation::validate_terminal_image;
use super::{
    ObjectArtifact, ObjectBoundarySettlement, ObjectCodeAttribution, ObjectCompilerPrivateFunction,
    ObjectFunction, ObjectPortEffect,
};

/// Exact image-emission inputs selected outside the source-free object owner.
///
/// A normalized interpreter is consumed only when the object itself retains
/// versioned ELF imports.  Supplying one cannot force an otherwise direct
/// image through the dynamic ELF path, and omitting one cannot make an
/// unresolved ELF import fall back to the direct writer. Build output
/// configuration's effective loader reaches a direct request only through
/// [`Self::settle_elf_loader`], once the emitted object is known.
#[derive(Debug, Clone)]
pub enum ExecutableImageEmissionRequest {
    Direct {
        subsystem: u16,
        /// The build-bound Mach-O CodeDirectory signing identity
        /// (wiki/spec/build/macos_application.md). `None` selects the
        /// validated executable leaf as the ad-hoc label; the ELF and PE
        /// writers do not interpret this value.
        code_signature_identifier: Option<String>,
    },
    DynamicElf {
        interpreter: NormalizedElfInterpreterPlan,
    },
}

impl ExecutableImageEmissionRequest {
    pub const fn direct(subsystem: u16) -> Self {
        Self::Direct {
            subsystem,
            code_signature_identifier: None,
        }
    }

    /// The identity a direct request binds into Mach-O signing, if any.
    pub fn code_signature_identifier(&self) -> Option<&str> {
        match self {
            Self::Direct {
                code_signature_identifier,
                ..
            } => code_signature_identifier.as_deref(),
            Self::DynamicElf { .. } => None,
        }
    }

    pub const fn dynamic_elf(interpreter: NormalizedElfInterpreterPlan) -> Self {
        Self::DynamicElf { interpreter }
    }

    /// Settle build output configuration's effective ELF loader against the
    /// exact emitted object
    /// (wiki/spec/build/configuration.md#elf-executable-loader-selection).
    ///
    /// An object that retains normalized ELF imports links dynamically, so a
    /// direct request becomes [`Self::DynamicElf`] carrying that loader; with
    /// no loader the output lacks required configuration and rejects. Every
    /// other object keeps the request unchanged: a loader alone never selects
    /// dynamic linking, and static output carries no `PT_INTERP`. A caller's
    /// explicit dynamic request is retained as supplied. The loader is borrowed
    /// and cloned only into the selected request, so this reads no host state.
    pub fn settle_elf_loader(
        self,
        artifact: &ObjectArtifact,
        elf_interpreter: Option<&NormalizedElfInterpreterPlan>,
    ) -> Result<Self, Box<RequestedExecutableImageError>> {
        if !requires_dynamic_elf(artifact) {
            return Ok(self);
        }
        let Self::Direct { subsystem, .. } = self else {
            return Ok(self);
        };
        match elf_interpreter {
            Some(interpreter) if interpreter.target().native_target() == artifact.target() => {
                Ok(Self::dynamic_elf(interpreter.clone()))
            }
            Some(interpreter) => Err(Box::new(
                RequestedExecutableImageError::UnexpectedDynamicElfInterpreter {
                    interpreter: interpreter.clone(),
                    diagnostic: Diagnostic::error(format!(
                        "ELF interpreter selected for target `{}` cannot load an object emitted for {:?}",
                        interpreter.target().target_name(),
                        artifact.target(),
                    )),
                },
            )),
            None => Err(Box::new(
                RequestedExecutableImageError::MissingDynamicElfInterpreter {
                    target: artifact.target(),
                    subsystem,
                    diagnostic: Diagnostic::error(
                        "missing output configuration: dynamically linked ELF output requires an interpreter pathname; select `Build.elf_interpreter` for the target, whose environment declares no default",
                    ),
                },
            )),
        }
    }
}

/// Whether the object's own contents select the dynamic ELF writer: an ELF
/// object that retains normalized imports. Request routing, loader settlement
/// and replay all read this one predicate.
fn requires_dynamic_elf(artifact: &ObjectArtifact) -> bool {
    artifact.target().object_format == ObjectFormat::Elf
        && !artifact.object().layout.normalized_imports.is_empty()
}

/// Result of the image-bound request router.
///
/// Dynamic ELF output deliberately remains distinct from [`crate::ExecutableImage`]
/// and therefore cannot enter installation or publication APIs.
#[derive(Debug)]
#[must_use = "requested image emission retains its exact authority boundary"]
pub enum RequestedExecutableImage {
    Direct(ExecutableImage),
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
}

impl std::fmt::Display for RequestedExecutableImageError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic().fmt(formatter)
    }
}

impl std::error::Error for RequestedExecutableImageError {}

/// Emit one executable image: the single entry. The object's contents select
/// the only justified path -- an import-bearing ELF object requires the
/// request's normalized interpreter and takes the dynamic ELF lane
/// (`dynamic_elf`), producing non-installable custody; every other object
/// must omit that input and takes the direct writer
/// ([`emit_direct_executable_image_signed`]). A request that disagrees with
/// the object is refused with the consumed loader input intact.
pub fn emit_executable_image(
    artifact: &ObjectArtifact,
    request: ExecutableImageEmissionRequest,
) -> Result<RequestedExecutableImage, Box<RequestedExecutableImageError>> {
    crate::private_object_composition::require_definitions(&artifact.object).map_err(|error| {
        Box::new(RequestedExecutableImageError::Direct(Diagnostic::error(
            error.to_string(),
        )))
    })?;
    match (requires_dynamic_elf(artifact), request) {
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
        (true, ExecutableImageEmissionRequest::Direct { subsystem, .. }) => Err(Box::new(
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
        (
            false,
            ExecutableImageEmissionRequest::Direct {
                subsystem,
                code_signature_identifier,
            },
        ) => emit_direct_executable_image_signed(
            artifact,
            subsystem,
            code_signature_identifier.as_deref(),
        )
        .map(RequestedExecutableImage::Direct)
        .map_err(|diagnostic| Box::new(RequestedExecutableImageError::Direct(diagnostic))),
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
                | (ObjectFormat::MachO, Architecture::X86_64)
                | (ObjectFormat::Coff, Architecture::X86_64)
        )
}

/// Emit and validate one direct executable image: the static lane
/// [`emit_executable_image`] takes for an object without normalized ELF
/// imports.
///
/// The direct lane admits typed internal calls and exactly retained imports
/// supported by its target writer. Final-text mutation outside their
/// architecture-specific immediate bits, unaccounted imports or thunks,
/// overlapping/missing function spans, and unclassified executable bytes are
/// hard failures.
///
/// This entry carries no bound signing identity: Mach-O output falls back to
/// the validated executable leaf as its ad-hoc CodeDirectory label. The
/// build-bound identity enters only through
/// [`crate::ExecutableImageEmissionRequest::Direct`]
/// (wiki/spec/build/macos_application.md).

/// `code_signature_identifier` is the build-bound Mach-O signing identity
/// carried by the image request. It is written verbatim into the emitted
/// CodeDirectory and participates in `LC_CODE_SIGNATURE` extent planning, so
/// it is fixed before any load command is emitted.
pub(crate) fn emit_direct_executable_image_signed(
    artifact: &ObjectArtifact,
    subsystem: u16,
    code_signature_identifier: Option<&str>,
) -> Result<ExecutableImage, Diagnostic> {
    if !can_emit_executable_image(artifact.target) {
        return Err(Diagnostic::error(format!(
            "cannot emit terminal-Psi executable image for {:?}",
            artifact.target
        )));
    }
    let prepared_entry = super::hosted_unit_entry::prepare(artifact)?;
    let prepared = super::hosted_unit_entry::prepared_parts(artifact, prepared_entry.as_ref());
    let image = image::build_final_image(FinalImageInput {
        target: artifact.target,
        object: prepared.object,
        relocations: prepared.relocations,
        text_bytes: prepared.text_bytes,
        data_bytes: artifact.data_bytes(),
    });
    let final_image_symbol_digest = image::final_image_symbol_digest(&image);
    let output = match artifact.target.object_format {
        // The architecture axis is deliberately not a dispatch key here: each
        // emitter reads the ABI off the image's own target and refuses a target
        // it has no container for, so no call site can name one architecture
        // while handing over another's image.
        ObjectFormat::Elf => image_elf::emit_elf_executable(image),
        ObjectFormat::MachO => match code_signature_identifier {
            Some(identifier) => image_macho::emit_macho_executable_signed(image, identifier),
            None => image_macho::emit_macho_executable(image),
        },
        ObjectFormat::Coff => image_pe::emit_pe_x86_64_executable(image, subsystem),
    }?;
    let mut output = emitted_direct_executable_output(output);
    let text_validation = validate_terminal_image(
        artifact,
        prepared.object,
        prepared.relocations,
        prepared.text_bytes,
        prepared.entry_shim,
        &output,
    )?;
    output.compiler_function_validation =
        super::function_fragments::reporting::summarize(artifact, &output, &text_validation)?;
    output.compiler_text_validation = Some(text_validation);
    Ok(ExecutableImage {
        psi: artifact.psi,
        target: artifact.target,
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
/// Retained-artifact readers (installation binding, native artifact replay)
/// call it; emission does not.
pub fn validate_direct_executable_image(
    artifact: &ObjectArtifact,
    image: &ExecutableImage,
) -> Result<(), Diagnostic> {
    super::function_fragments::replay::validate(artifact)?;
    if artifact.psi() != image.psi()
        || artifact.target() != image.target()
        || artifact.functions() != image.functions()
        || artifact.private_functions() != image.private_functions()
        || artifact.semantic_code_attribution() != image.semantic_code_attribution()
        || artifact.port_effects() != image.port_effects()
        || artifact.boundary_settlements() != image.boundary_settlements()
        || !crate::object_artifact::image_foreign_calls_match_object(
            artifact,
            image.foreign_calls(),
        )
    {
        return Err(Diagnostic::error(
            "terminal object and executable image have different semantic or evidence identity",
        ));
    }
    let prepared_entry = super::hosted_unit_entry::prepare(artifact)?;
    let prepared = super::hosted_unit_entry::prepared_parts(artifact, prepared_entry.as_ref());
    let replayed_final_image = image::build_final_image(FinalImageInput {
        target: artifact.target(),
        object: prepared.object,
        relocations: prepared.relocations,
        text_bytes: prepared.text_bytes,
        data_bytes: artifact.data_bytes(),
    });
    if image.final_image_symbol_digest != image::final_image_symbol_digest(&replayed_final_image) {
        return Err(Diagnostic::error(
            "terminal executable image symbol evidence does not match its exact object entry/data-symbol table",
        ));
    }
    let recomputed = validate_terminal_image(
        artifact,
        prepared.object,
        prepared.relocations,
        prepared.text_bytes,
        prepared.entry_shim,
        image.output(),
    )?;
    let function_validation =
        super::function_fragments::reporting::summarize(artifact, image.output(), &recomputed)?;
    if image.output().compiler_text_validation != Some(recomputed) {
        return Err(Diagnostic::error(
            "terminal executable image retained stale final-text validation evidence",
        ));
    }
    if image.output().compiler_function_validation != function_validation {
        return Err(Diagnostic::error(
            "terminal executable image retained stale compiler-function validation evidence",
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutableImage {
    psi: TerminalPsiIdentity,
    target: NativeTarget,
    subsystem: Option<u16>,
    functions: Vec<ObjectFunction>,
    private_functions: Vec<ObjectCompilerPrivateFunction>,
    semantic_code_attribution: Vec<ObjectCodeAttribution>,
    port_effects: Vec<ObjectPortEffect>,
    boundary_settlements: Vec<ObjectBoundarySettlement>,
    foreign_calls: Vec<super::ObjectForeignCall>,
    final_image_symbol_digest: image::FinalImageSymbolDigest,
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

    pub const fn final_image_symbol_digest(&self) -> image::FinalImageSymbolDigest {
        self.final_image_symbol_digest
    }
}
