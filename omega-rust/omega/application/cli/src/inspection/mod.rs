//! Prepare package inputs and request a Terminal inspection. Grants no native authority.
//!
//! Source preparation follows the compile command rather than a focused-file
//! shortcut. A root beside a `build.omg` (or an `omega.lock`) is a package
//! project: the package manager resolves its declared closure and checks each
//! prerequisite build before its consumer, retaining generated source under
//! the producer's custody. Acquisition-only compiler inputs cannot provide
//! that handoff, even if a helper happens to have usable physical source.
//! A standalone root keeps the direct, optionally targetless check unchanged.
//! Inspection asks the compiler to publish the selected machine as Terminal Psi;
//! it neither admits runtime trust nor realizes native output. Candidate checking is shared with `--check`, but
//! trust admission and native acceptance/publication remain separate operations.

use compiler::{CheckedCompileRequest, CompileOptions};
use diagnostics::Diagnostic;
use package_manager::operations as packages;
use package_manager::operations::LocalProjectPreparationOptions;
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
    Preparation(packages::PrepareLocalProjectError),
    Review(package_manager::review::CompileResolvedPackageReviewsError),
}

impl std::fmt::Display for InspectTerminalError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Diagnostics(diagnostics) => crate::write_diagnostics(formatter, diagnostics),
            Self::Artifact(error) => write!(formatter, "cannot inspect Terminal artifact: {error}"),
            Self::Preparation(error) => write!(formatter, "{error}"),
            Self::Review(error) => write!(formatter, "cannot check inspection project: {error}"),
        }
    }
}

impl std::error::Error for InspectTerminalError {}

pub fn inspect_terminal(
    request: &InspectTerminalRequest,
) -> Result<TerminalInspection, InspectTerminalError> {
    let artifact = compile_inspection(request)?;
    Ok(TerminalInspection { artifact })
}

/// Inspect externally supplied envelope bytes through canonical admission.
/// This is a data inspection, independent of package preparation or runtime
/// installation. It provides no establishment, provider, or execution grant.
pub fn inspect_terminal_artifact(bytes: &[u8]) -> Result<TerminalInspection, InspectTerminalError> {
    let artifact =
        CanonicalTerminalArtifact::from_bytes(bytes).map_err(InspectTerminalError::Artifact)?;
    Ok(TerminalInspection { artifact })
}

/// Check package sources through the manager's dependency-first candidate
/// pipeline. The prepared entry routes through immutable source custody, while
/// build staging follows the authored root exactly as the compile command
/// places it, independently of that custody's memory or persisted backing.
/// Package preparation needs an exact
/// target, so the invocation resolves one first: an explicit `--target`,
/// else the compiler host's catalogued profile. A host that owns none
/// reports that absence as an ordinary diagnostic rather than panicking.
/// With the profile resolved, the checked compile keeps it so package
/// inputs and target attachments agree; a standalone root without a target
/// stays targetless, as before.
fn compile_inspection(
    request: &InspectTerminalRequest,
) -> Result<CanonicalTerminalArtifact, InspectTerminalError> {
    let target = crate::invocation_target_profile(request.target_name.as_deref())
        .map_err(|diagnostic| InspectTerminalError::Diagnostics(vec![diagnostic]))?;
    let prepared = packages::prepare_local_project(
        &request.root_path,
        LocalProjectPreparationOptions {
            target,
            offline: false,
        },
    )
    .map_err(InspectTerminalError::Preparation)?;
    let Some(prepared) = prepared else {
        return compiler::CompilationSession::default()
            .produce(
                CheckedCompileRequest::new(&request.root_path, request.target_name.as_deref()),
                compiler::CompilationProduct::Inspection(request.machine.clone()),
            )
            .and_then(compiler::CompilationOutput::into_inspection)
            .map_err(compiler::CompilationProductError::into_diagnostics)
            .map_err(InspectTerminalError::Diagnostics);
    };
    let build_dir = CompileOptions {
        root_path: request.root_path.clone(),
        build_dir: None,
        target_name: None,
    }
    .build_dir();
    packages::inspect_prepared_local_project(prepared, &build_dir, target, &request.machine)
        .map_err(InspectTerminalError::Review)
}
