//! Checked inspection and reporting for one prepared package or application project.

use compiler::CompileReport;
use diagnostics::Diagnostic;
use package_manager::operations::{
    PrepareLocalProjectError, PreparedLocalProject, ProjectCompilationError,
};
use package_manager::review::CompileResolvedPackageReviewsError;
use std::fmt;
use std::path::{Path, PathBuf};
use target::TargetProfile;

pub struct PreparedLocalProjectCheckRequest {
    prepared: PreparedLocalProject,
    build_dir: PathBuf,
    target_profiles: Vec<TargetProfile>,
    /// `--check --verify`: compile under the retained pins without the
    /// accepted lock's restricted-request grants, so findings the lock
    /// previously settled surface again.
    verify: bool,
    build_snapshot: Option<compiler::BuildSnapshotRequest>,
    timings: bool,
}

impl PreparedLocalProjectCheckRequest {
    pub fn new(
        prepared: PreparedLocalProject,
        build_dir: impl Into<PathBuf>,
        target_profile: TargetProfile,
    ) -> Self {
        Self::for_targets(prepared, build_dir, vec![target_profile])
    }

    /// One check request covering several exact targets. Their prepared source
    /// shares one candidate preparation: each package's binding-independent
    /// parse frontier is built once for the union while each target still
    /// checks under its own staging root and accepted consent.
    pub fn for_targets(
        prepared: PreparedLocalProject,
        build_dir: impl Into<PathBuf>,
        target_profiles: Vec<TargetProfile>,
    ) -> Self {
        Self {
            prepared,
            build_dir: build_dir.into(),
            target_profiles,
            verify: false,
            build_snapshot: None,
            timings: false,
        }
    }

    /// Re-verify under the lock's retained source pins: the restricted-request
    /// checkpoint is not armed. Acceptance rows still describe dependencies; they no longer
    /// grant this run's projected requests.
    pub fn with_verify(mut self, verify: bool) -> Self {
        self.verify = verify;
        self
    }

    /// Record per-stage timings on the produced report. Off by default: the
    /// ladder measures nothing until a caller asks for it.
    pub fn with_timings(mut self, timings: bool) -> Self {
        self.timings = timings;
        self
    }

    /// Select the root build's input inventory and output obligations. Dependency
    /// builds retain their independently captured package inventories.
    pub fn with_build_snapshot(mut self, snapshot: compiler::BuildSnapshotRequest) -> Self {
        self.build_snapshot = Some(snapshot);
        self
    }
}

#[derive(Debug)]
pub enum CheckPreparedLocalProjectError {
    /// One target's own prepared section failed while sibling targets carried
    /// on; reported per target in the multi-target route.
    Prepare(PrepareLocalProjectError),
    Review(CompileResolvedPackageReviewsError),
    TrustAdmission(Vec<Diagnostic>),
    /// The retained checked root carries an optional proof-product request a
    /// check-only stop cannot publish, so the request rejects rather than
    /// reporting a success that silently dropped it. The standalone
    /// compiler's `RequestedCompileProduct::Check` arm applies the same
    /// admission fence.
    ProofProduct(Vec<Diagnostic>),
    Report(&'static str),
}

impl fmt::Display for CheckPreparedLocalProjectError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Prepare(error) => write!(formatter, "{error}"),
            Self::Review(error) => write!(formatter, "cannot check prepared project: {error}"),
            Self::TrustAdmission(diagnostics) => {
                write!(formatter, "cannot admit package trust:")?;
                diagnostics::write_bounded_diagnostics(formatter, diagnostics)
            }
            Self::ProofProduct(diagnostics) => {
                write!(formatter, "cannot satisfy the requested proof product:")?;
                diagnostics::write_bounded_diagnostics(formatter, diagnostics)
            }
            Self::Report(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for CheckPreparedLocalProjectError {}

/// Produce a named machine's Terminal inspection through the accepted-input
/// pipeline. Checked source stays inside the compiler. The returned description
/// grants no native authority and may describe unresolved trust.
pub fn inspect_prepared_local_project(
    prepared: PreparedLocalProject,
    build_dir: &Path,
    target_profile: TargetProfile,
    machine: &str,
) -> Result<compiler::ProducedTerminalProgram, ProjectInspectionError> {
    let outcome = crate::project_compilation::compile(
        crate::project_compilation::ProjectCompilationRequest {
            prepared,
            build_directory: build_dir,
            target_profiles: &[target_profile],
            snapshot: None,
            verify: true,
            timings: false,
            product: compiler::CompilationProduct::Inspection(machine.to_owned()),
        },
    )
    .pop()
    .ok_or(ProjectInspectionError::MissingTarget)?;
    outcome
        .result
        .map_err(ProjectInspectionError::Project)?
        .into_inspection()
        .map_err(ProjectInspectionError::Product)
}

#[derive(Debug)]
pub enum ProjectInspectionError {
    Project(ProjectCompilationError),
    Product(compiler::CompilationProductError),
    MissingTarget,
}

impl fmt::Display for ProjectInspectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Project(error) => write!(formatter, "{error}"),
            Self::Product(
                compiler::CompilationProductError::TrustAdmission(diagnostics)
                | compiler::CompilationProductError::Production(diagnostics),
            ) => diagnostics::write_bounded_diagnostics(formatter, diagnostics),
            Self::MissingTarget => formatter.write_str("inspection target was not prepared"),
        }
    }
}

impl std::error::Error for ProjectInspectionError {}

/// One target's check result inside a shared multi-target request. A failing
/// target never stops or hides a sibling.
pub struct CheckedProjectTargetOutcome {
    pub target: TargetProfile,
    pub result: Result<CompileReport, CheckPreparedLocalProjectError>,
}

/// The same check for every requested target under one prepared source union.
/// All targets share the prepared project's resolved closure and one
/// source custody, so a package's binding-independent parse
/// frontier — and any typed frontier whose inputs two targets carry
/// identically — assembles exactly once for the request. Each target still
/// stages into `build_dir/<target>`, arms its own accepted checkpoint, and
/// resolves its own trust grants and report.
pub fn check_prepared_local_project_for_targets(
    request: PreparedLocalProjectCheckRequest,
) -> Vec<CheckedProjectTargetOutcome> {
    let PreparedLocalProjectCheckRequest {
        prepared,
        build_dir,
        target_profiles,
        verify,
        build_snapshot,
        timings,
    } = request;
    crate::project_compilation::compile(crate::project_compilation::ProjectCompilationRequest {
        prepared,
        build_directory: &build_dir,
        target_profiles: &target_profiles,
        snapshot: build_snapshot.as_ref(),
        verify,
        timings,
        product: compiler::CompilationProduct::Check,
    })
    .into_iter()
    .map(|outcome| CheckedProjectTargetOutcome {
        target: outcome.target,
        result: outcome
            .result
            .map_err(|error| match error {
                ProjectCompilationError::Prepare(error) => {
                    CheckPreparedLocalProjectError::Prepare(error)
                }
                ProjectCompilationError::Review(error) => {
                    CheckPreparedLocalProjectError::Review(error)
                }
            })
            .and_then(|product| product.into_report().map_err(product_failure)),
    })
    .collect()
}

fn product_failure(error: compiler::CompilationProductError) -> CheckPreparedLocalProjectError {
    match error {
        compiler::CompilationProductError::TrustAdmission(diagnostics) => {
            CheckPreparedLocalProjectError::TrustAdmission(diagnostics)
        }
        compiler::CompilationProductError::Production(diagnostics) => {
            CheckPreparedLocalProjectError::ProofProduct(diagnostics)
        }
    }
}
