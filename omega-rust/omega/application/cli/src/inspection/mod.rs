//! Inspect a source product through Build, or admit an existing Terminal envelope.
//! Neither route grants native installation or runtime authority.

use diagnostics::Diagnostic;
use omega_build::{BuildError, BuildRequest, BuildTargets, ProjectProduct};
use std::path::PathBuf;
use terminal_codec::CanonicalTerminalArtifact;

pub struct InspectTerminalRequest {
    pub root_path: PathBuf,
    pub machine: String,
    pub target_name: Option<String>,
}

pub struct TerminalInspection {
    pub artifact: CanonicalTerminalArtifact,
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
    let outcomes = omega_build::build(BuildRequest {
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
        artifact: artifact
            .into_artifact()
            .map_err(InspectTerminalError::Artifact)?,
    })
}

/// Inspect external envelope bytes through canonical admission. This route does
/// not prepare packages, compile source, establish bindings or execute code.
pub fn inspect_terminal_artifact(bytes: &[u8]) -> Result<TerminalInspection, InspectTerminalError> {
    let artifact =
        CanonicalTerminalArtifact::from_bytes(bytes).map_err(InspectTerminalError::Artifact)?;
    Ok(TerminalInspection { artifact })
}
