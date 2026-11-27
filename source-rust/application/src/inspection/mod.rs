//! Inspect a selected source machine or decode a saved Psi artifact.

mod summary;

pub use summary::terminal_summary;

use diagnostics::Diagnostic;
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
}

impl std::fmt::Display for InspectTerminalError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Diagnostics(diagnostics) => {
                diagnostics::write_diagnostics(formatter, diagnostics)
            }
            Self::Artifact(error) => write!(formatter, "cannot inspect Psi artifact: {error}"),
        }
    }
}

impl std::error::Error for InspectTerminalError {}

pub fn inspect_terminal(
    request: &InspectTerminalRequest,
) -> Result<TerminalInspection, InspectTerminalError> {
    let profile = target::TargetProfile::from_omega_target_name(request.target_name.as_deref())
        .map_err(|error| InspectTerminalError::Diagnostics(vec![error]))?;
    let product = package_manager::project::open_product(
        &request.root_path,
        profile,
        false,
        &product_publication::output_directory::base(&request.root_path, None),
        None,
    )
    .map_err(InspectTerminalError::Diagnostics)?;
    let stage = product
        .product_stage(&[profile])
        .map_err(InspectTerminalError::Diagnostics)?;
    let artifact = compiler::inspect(stage.source(), &request.machine).map_err(|mut rows| {
        product.attribute_diagnostics(&mut rows);
        InspectTerminalError::Diagnostics(rows)
    })?;
    Ok(TerminalInspection { artifact })
}

/// Inspection decodes under the host's load limits but needs no trust.
pub fn inspect_terminal_artifact(
    bytes: &[u8],
    limits: terminal_codec::PsiLoadLimits,
) -> Result<TerminalInspection, InspectTerminalError> {
    let artifact = CanonicalTerminalArtifact::from_bytes_within(bytes, limits)
        .map_err(InspectTerminalError::Artifact)?;
    Ok(TerminalInspection {
        artifact: std::sync::Arc::new(artifact),
    })
}
