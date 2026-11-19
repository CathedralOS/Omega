//! Compile one program from settled inputs. Project builds call this once per stage.

#![forbid(unsafe_code)]

mod request_outcome;
pub use request_outcome::CompilerRequestOutcome;

use diagnostics::Diagnostic;
use psi_authorization::{
    AuthorizedTerminalArtifact, HostProducerTrust, authorize_terminal_artifact,
};

pub use native_compiler::{SavedProgramTarget as NativeTarget, TargetProviderInstallation};
use source_compiler as source_to_psi;
pub use source_compiler::{PublishedPsi, StageSource};

pub enum CompileRequest<'a> {
    Psi(StageSource<'a>),
    Native {
        program: Program<'a>,
        target: NativeTarget<'a>,
        entry_memory: &'a [terminal_installation::PrivateEntryRamInitialization],
    },
}

pub enum Program<'a> {
    Source(StageSource<'a>),
    /// The receiving host admitted this exact published artifact and its producer.
    Psi(AuthorizedTerminalArtifact),
}

pub enum Artifact {
    Psi(PublishedPsi),
    Native(product_publication::CompletedProduct),
}

pub fn compile(request: CompileRequest<'_>) -> Result<Artifact, Vec<Diagnostic>> {
    match request {
        CompileRequest::Psi(source) => source_to_psi::compile(source).map(Artifact::Psi),
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
