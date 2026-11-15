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
