/*

Terminal Psi inspection entry points (`omega psi inspect`).

We support two inspection routes, neither of which emits a native image or
grants runtime execution authority:
- `inspect_terminal`: compiles a source root through `orchestration::build`
  with `ProjectProduct::Inspection { machine }` and `verify: true`, returning
  the selected machine's `CanonicalTerminalArtifact`.
- `inspect_terminal_artifact`: decodes and validates an existing `.psi`
  envelope from raw bytes (`CanonicalTerminalArtifact::from_bytes`) without
  preparing packages, compiling source, or establishing bindings.

*/

mod summary;

pub use summary::terminal_summary;

use diagnostics::Diagnostic;
use orchestration::{BuildError, BuildRequest, BuildTargets, ProjectProduct};
use std::path::PathBuf;
use terminal_codec::CanonicalTerminalArtifact;

pub struct InspectTerminalRequest {
    pub root_path: PathBuf,
    pub machine: String,
    pub target_name: Option<String>,
}

pub struct TerminalInspection {
    pub artifact: std::sync::Arc<CanonicalTerminalArtifact>,
}

#[derive(Debug)]
pub enum InspectTerminalError {
    Diagnostics(Vec<Diagnostic>),
    Artifact(terminal_codec::CanonicalTerminalArtifactError),
    Build(BuildError),
}

impl std::fmt::Display for InspectTerminalError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Diagnostics(diagnostics) => {
                diagnostics::write_diagnostics(formatter, diagnostics)
            }
            Self::Artifact(error) => write!(formatter, "cannot inspect Terminal artifact: {error}"),
            Self::Build(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for InspectTerminalError {}

pub fn inspect_terminal(
    request: &InspectTerminalRequest,
) -> Result<TerminalInspection, InspectTerminalError> {
    let targets = match request.target_name.as_deref() {
        Some(name) => BuildTargets::Single(
            target::TargetProfile::from_omega_target_name(Some(name))
                .map_err(|error| InspectTerminalError::Diagnostics(vec![error]))?,
        ),
        None => BuildTargets::Host,
    };
    let outcomes = orchestration::build(BuildRequest {
        stage_validators: Default::default(),
        root_path: request.root_path.clone(),
        build_directory: None,
        targets,
        product: ProjectProduct::Inspection {
            machine: request.machine.clone(),
        },
        timings: false,
        offline: false,
        verify: true,
        require_package_project: false,
        optimization_rollback: Default::default(),
        build_snapshot: None,
    })
    .map_err(InspectTerminalError::Build)?;
    let [outcome]: [_; 1] = outcomes.try_into().map_err(|_| {
        InspectTerminalError::Diagnostics(vec![Diagnostic::error(
            "inspection requires exactly one project Build outcome",
        )])
    })?;
    let outcome = outcome.result.map_err(InspectTerminalError::Build)?;
    let artifact = outcome
        .output
        .into_inspection()
        .map_err(|error| InspectTerminalError::Diagnostics(error.into_diagnostics()))?;
    Ok(TerminalInspection {
        artifact: std::sync::Arc::clone(artifact.artifact()),
    })
}

pub fn inspect_terminal_artifact(bytes: &[u8]) -> Result<TerminalInspection, InspectTerminalError> {
    let artifact =
        CanonicalTerminalArtifact::from_bytes(bytes).map_err(InspectTerminalError::Artifact)?;
    Ok(TerminalInspection {
        artifact: std::sync::Arc::new(artifact),
    })
}

const MAX_ARTIFACT_BYTES: u64 = 64 * 1024 * 1024;

pub(crate) fn read_artifact(path: &std::path::Path) -> std::io::Result<Vec<u8>> {
    use std::io::Read;
    let file = std::fs::File::open(path)?;
    let mut bytes = Vec::new();
    file.take(MAX_ARTIFACT_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_ARTIFACT_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Psi artifact exceeds 64 MiB",
        ));
    }
    Ok(bytes)
}

pub(crate) fn open_trusted_program(
    path: &std::path::Path,
    trusted_artifact: Option<&str>,
) -> Result<terminal_installation::AuthorizedTerminalArtifact, Vec<Diagnostic>> {
    let expected = trusted_artifact.ok_or_else(|| vec![Diagnostic::error("saved Psi execution requires --trusted-artifact with the manifest identity supplied by the trusted producer")])?;
    let bytes = read_artifact(path).map_err(|error| {
        vec![Diagnostic::error(format!(
            "cannot read Psi artifact {}: {error}",
            path.display()
        ))]
    })?;
    let decoded = inspect_terminal_artifact(&bytes)
        .map_err(|error| vec![Diagnostic::error(error.to_string())])?;
    if decoded.artifact.manifest().identity().to_string() != expected {
        return Err(vec![Diagnostic::error(
            "saved Psi manifest differs from the artifact authorized by this host",
        )]);
    }
    let producer = terminal_installation::HostProducerTrust::accept(
        "trusted producer of the host-pinned artifact",
    );
    terminal_installation::authorize_terminal_artifact(decoded.artifact, Some(&producer))
        .map_err(|reason| vec![Diagnostic::error(reason.to_string())])
}
