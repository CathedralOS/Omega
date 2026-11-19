//! Compile one program from settled inputs. Project builds call this once per stage.

#![forbid(unsafe_code)]

/// Fingerprint of this compiler's sources, toolchain, target, and build profile.
pub const fn compiler_identity() -> [u8; 32] {
    *include_bytes!(concat!(env!("OUT_DIR"), "/compiler-identity.bin"))
}

mod request_outcome;
pub use request_outcome::CompilerRequestOutcome;

use diagnostics::Diagnostic;
use psi_authorization::{
    AuthorizedTerminalArtifact, HostProducerTrust, authorize_terminal_artifact,
};

pub use native_compiler::{
    NativeApplicationSettings, SavedProgramTarget as NativeTarget, TargetProviderInstallation,
    SourceNativeTarget, stage_validators,
};
use source_compiler as source_to_psi;
pub use source_compiler::{PublishedPsi, StageSource};

pub enum CompileRequest<'a> {
    Psi(StageSource<'a>),
    /// Admit the chosen entry and providers for execution on the Psi interpreter.
    Runtime {
        program: Program<'a>,
        entry_memory: &'a [psi_entry_memory::PrivateEntryRamInitialization],
    },
    Native {
        program: Program<'a>,
        entry_memory: &'a [psi_entry_memory::PrivateEntryRamInitialization],
    },
}

/// The source producer supplies its checked deployment; a saved program uses the
/// receiving host's installation already pinned to the authorized manifest.
pub enum Program<'a> {
    Source {
        source: StageSource<'a>,
        target: SourceNativeTarget<'a>,
    },
    Psi {
        program: AuthorizedTerminalArtifact,
        target: NativeTarget<'a>,
    },
}

pub enum Artifact {
    Psi(PublishedPsi),
    Runtime(product_publication::CompletedProduct),
    Native(product_publication::CompletedProduct),
}

pub fn compile(request: CompileRequest<'_>) -> Result<Artifact, Vec<Diagnostic>> {
    match request {
        CompileRequest::Psi(source) => source_to_psi::compile(source).map(Artifact::Psi),
        CompileRequest::Runtime {
            program,
            entry_memory,
        } => {
            let admitted = program
                .prepare()?
                .admit_runtime(entry_memory, &mut Vec::new())?;
            Ok(Artifact::Runtime(
                product_publication::CompletedProduct::from_runtime(admitted, None),
            ))
        }
        CompileRequest::Native {
            program,
            entry_memory,
        } => {
            let application = program.application().clone();
            let image = program.prepare()?.emit(entry_memory)?;
            Ok(Artifact::Native(
                product_publication::CompletedProduct::from_native(image, None, application),
            ))
        }
    }
}

/// Compile exactly the requested product, running stage checks only when it
/// emits a native image. Planning-stage Psi does not receive the final request.
pub fn compile_with_validators(
    request: CompileRequest<'_>,
    validators: &stage_validators::StageValidatorRequest,
) -> Result<
    (Artifact, Vec<stage_validators::StageValidationRow>),
    stage_validators::NativeValidationError,
> {
    use stage_validators::NativeValidationError;

    let rows = match request {
        CompileRequest::Native {
            program,
            entry_memory,
        } => {
            let application = program.application().clone();
            let (image, rows) = program
                .prepare()?
                .emit_with_validators(entry_memory, validators)?;
            let product =
                product_publication::CompletedProduct::from_native(image, None, application);
            return Ok((Artifact::Native(product), rows));
        }
        CompileRequest::Psi(ref source) => validators.stages_not_produced(
            source.selections.calling_targets,
            |_| "this product stops at Terminal Psi before target operations",
        )?,
        CompileRequest::Runtime { ref program, .. } => validators.stages_not_produced(
            &[program.profile().target_name()],
            |_| "runtime admission stops before target lowering",
        )?,
    };
    let product = compile(request).map_err(NativeValidationError::Compilation)?;
    Ok((product, rows))
}

impl Program<'_> {
    fn profile(&self) -> target::TargetProfile {
        match self {
            Self::Psi { target, .. } => target.profile,
            Self::Source { target, .. } => target.profile,
        }
    }

    fn application(&self) -> &target::NativeApplicationMetadata {
        match self {
            Self::Psi { target, .. } => &target.application,
            Self::Source { target, .. } => &target.settings.application,
        }
    }

    fn prepare(self) -> Result<native_compiler::NativeProgram, Vec<Diagnostic>> {
        match self {
            Self::Psi { program, target } => {
                native_compiler::prepare_saved_program(program, target)
            }
            Self::Source { source, target } => {
                let publication =
                    source_to_psi::compile_for_target(source, target.profile.target_name())?;
                let (artifact, providers, exclusions) = publication.into_parts();
                let producer = HostProducerTrust::accept("current compiler invocation");
                let authorized = authorize_terminal_artifact(artifact, Some(&producer))
                    .map_err(|reason| vec![Diagnostic::error(reason.to_string())])?;
                native_compiler::prepare_source_program(authorized, providers, &exclusions, target)
            }
        }
    }
}
