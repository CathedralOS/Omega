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
    SavedProgramTarget as NativeTarget, TargetProviderInstallation, stage_validators,
};
use source_compiler as source_to_psi;
pub use source_compiler::{PublishedPsi, StageSource};

pub enum CompileRequest<'a> {
    Psi(StageSource<'a>),
    /// Admit the chosen entry and providers for execution on the Psi interpreter.
    Runtime {
        program: Program<'a>,
        target: NativeTarget<'a>,
        entry_memory: &'a [psi_entry_memory::PrivateEntryRamInitialization],
    },
    Native {
        program: Program<'a>,
        target: NativeTarget<'a>,
        entry_memory: &'a [psi_entry_memory::PrivateEntryRamInitialization],
    },
}

pub enum Program<'a> {
    Source(StageSource<'a>),
    /// The receiving host admitted this exact published artifact and its producer.
    Psi(AuthorizedTerminalArtifact),
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
            target,
            entry_memory,
        } => {
            let admitted = native_compiler::admit_published_runtime(
                program.into_authorized_psi()?,
                target,
                entry_memory,
                &mut Vec::new(),
            )?;
            Ok(Artifact::Runtime(
                product_publication::CompletedProduct::from_runtime(admitted, None),
            ))
        }
        CompileRequest::Native {
            program,
            target,
            entry_memory,
        } => {
            let psi = program.into_authorized_psi()?;
            let application = target.application.clone();
            let image = native_compiler::emit(psi, target, entry_memory)?;
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
            target,
            entry_memory,
        } => {
            let application = target.application.clone();
            let authorized = program
                .into_authorized_psi()
                .map_err(NativeValidationError::Compilation)?;
            let (image, rows) = native_compiler::emit_with_validators(
                authorized,
                target,
                entry_memory,
                validators,
            )?;
            let product =
                product_publication::CompletedProduct::from_native(image, None, application);
            return Ok((Artifact::Native(product), rows));
        }
        CompileRequest::Psi(ref source) => validators.stages_not_produced(
            source.selections.calling_targets,
            |_| "this product stops at Terminal Psi before target operations",
        )?,
        CompileRequest::Runtime { ref target, .. } => validators.stages_not_produced(
            &[target.profile.target_name()],
            |_| "runtime admission stops before target lowering",
        )?,
    };
    let product = compile(request).map_err(NativeValidationError::Compilation)?;
    Ok((product, rows))
}

impl Program<'_> {
    fn into_authorized_psi(self) -> Result<AuthorizedTerminalArtifact, Vec<Diagnostic>> {
        match self {
            Self::Psi(authorized) => Ok(authorized),
            Self::Source(source) => {
                let published = source_to_psi::compile(source)?;
                // Trust applies only to the checked output this invocation just produced.
                let compiler = HostProducerTrust::accept("current compiler invocation");
                authorize_terminal_artifact(published, Some(&compiler))
                    .map_err(|reason| vec![Diagnostic::error(reason.to_string())])
            }
        }
    }
}
