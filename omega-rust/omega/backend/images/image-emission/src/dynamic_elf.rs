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
use image_elf::{
    ElfDynamicExecutableEmissionError, ValidatedElfDynamicExecutable, emit_elf_dynamic_executable,
};
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

    /// Bind `code_signature_identifier` as the Mach-O CodeDirectory identity.
    /// The caller supplies the build-validated authored application
    /// identifier; emission writes it verbatim and rejects an empty string
    /// rather than silently falling back to the leaf.
    pub fn with_code_signature_identifier(self, code_signature_identifier: Option<String>) -> Self {
        match self {
            Self::Direct { subsystem, .. } => Self::Direct {
                subsystem,
                code_signature_identifier,
            },
            Self::DynamicElf { .. } => self,
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
        ) => crate::image_output::emit_executable_image_signed(
            artifact,
            subsystem,
            code_signature_identifier.as_deref(),
        )
        .map(RequestedExecutableImage::Direct)
        .map_err(|diagnostic| Box::new(RequestedExecutableImageError::Direct(diagnostic))),
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

/// Exact failing owner from one step of dynamic ELF orchestration: the
/// hosted-entry preparation, the ELF owner's chain, or the production bridge.
///
/// Every variant preserves the carrier the step refused. Diagnostics are
/// observations of that custody, never substitutes for it.
#[derive(Debug)]
#[must_use = "dynamic ELF orchestration failure retains the exact failing owner"]
pub enum DynamicElfOrchestrationError {
    /// The hosted-entry owner rejected the artifact before any ELF stage ran:
    /// an import-bearing object still carries its exact entry custody, so a
    /// receiver bridge cannot be dropped merely because imports selected the
    /// dynamic writer.
    HostedEntryPreparation(Diagnostic),
    /// One of the ELF owner's 22 stages refused; the exact stage carrier is
    /// inside.
    Elf(Box<ElfDynamicExecutableEmissionError>),
    ProductionBridge(Box<DynamicElfImageEmissionError>),
}

impl DynamicElfOrchestrationError {
    pub const fn stage(&self) -> &'static str {
        match self {
            Self::HostedEntryPreparation(_) => "hosted-entry-preparation",
            Self::Elf(error) => error.stage(),
            Self::ProductionBridge(_) => "production-bridge",
        }
    }

    pub const fn diagnostic(&self) -> &Diagnostic {
        match self {
            Self::HostedEntryPreparation(diagnostic) => diagnostic,
            Self::Elf(error) => error.diagnostic(),
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

/// Prepare the hosted entry, build the final image, run the ELF owner's
/// dynamic lane over it, and rejoin the admitted bytes to the production
/// image-output surface.
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
    // An import-bearing object still enters through the exact hosted-entry
    // owner: a receiver binding appends its bridge text, private BSS
    // partitions, symbols, and relocations before the dynamic writer plans
    // sections, and the prepared entry symbol becomes e_entry.
    let prepared_entry = crate::hosted_unit_entry::prepare(artifact).map_err(|diagnostic| {
        Box::new(DynamicElfOrchestrationError::HostedEntryPreparation(
            diagnostic,
        ))
    })?;
    let (object, text_bytes, relocations) = prepared_entry.as_ref().map_or(
        (
            artifact.object(),
            artifact.text_bytes(),
            artifact.relocations(),
        ),
        |prepared| {
            (
                &prepared.object,
                prepared.text.as_slice(),
                &prepared.relocations,
            )
        },
    );
    let image = image::build_final_image(FinalImageInput {
        target: artifact.target(),
        object,
        relocations,
        text_bytes,
        data_bytes: artifact.data_bytes(),
    });
    let admitted = emit_elf_dynamic_executable(image, interpreter)
        .map_err(|error| Box::new(DynamicElfOrchestrationError::Elf(error)))?;
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

    // Replay the same hosted-entry preparation the production path consumed;
    // the admitted image must match the prepared object/text/relocations
    // exactly, including any receiver bridge suffix and its BSS partitions.
    let prepared_entry = crate::hosted_unit_entry::prepare(artifact)?;
    let (object, text_bytes, relocations, entry_shim) = prepared_entry.as_ref().map_or(
        (
            artifact.object(),
            artifact.text_bytes(),
            artifact.relocations(),
            None,
        ),
        |prepared| {
            (
                &prepared.object,
                prepared.text.as_slice(),
                &prepared.relocations,
                Some(prepared.shim),
            )
        },
    );
    let mut replayed_image = image::build_final_image(FinalImageInput {
        target,
        object,
        relocations,
        text_bytes,
        data_bytes: artifact.data_bytes(),
    });
    let symbol_digest = final_image_symbol_digest(&replayed_image);
    let mut output = emitted_direct_executable_output(admitted.output().clone());
    let validation = validate_terminal_dynamic_elf_image(
        artifact,
        object,
        relocations,
        text_bytes,
        entry_shim,
        &output,
    )?;
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
