//! Checked inspection and reporting for one prepared package or application project.

use super::accepted_compilation::{
    AcceptedCompilationRequest, compile_accepted_project, compile_accepted_projects,
};
use super::{PrepareLocalProjectError, PreparedLocalProject};
use crate::build_session::PackageBuildSession;
use crate::review::CompileResolvedPackageReviewsError;
use compiler::CompileReport;
use diagnostics::Diagnostic;
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
    build_snapshot: Option<build_evaluation::BuildSnapshotRequest>,
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
    pub fn with_build_snapshot(mut self, snapshot: build_evaluation::BuildSnapshotRequest) -> Self {
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
                crate::review::render_diagnostics(formatter, diagnostics)
            }
            Self::ProofProduct(diagnostics) => {
                write!(formatter, "cannot satisfy the requested proof product:")?;
                crate::review::render_diagnostics(formatter, diagnostics)
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
) -> Result<terminal_codec::CanonicalTerminalArtifact, CompileResolvedPackageReviewsError> {
    let (entry_path, source_closure, targets) = prepared.into_target_review_parts();
    let failure = |message: String| CompileResolvedPackageReviewsError::Compilation {
        package: source_closure.graph().root().clone(),
        diagnostics: vec![Diagnostic::error(message)],
    };
    let target = targets
        .into_iter()
        .find(|target| target.profile == target_profile)
        .ok_or_else(|| failure("inspection target was not prepared".to_owned()))?;
    let accepted = target
        .accepted
        .map_err(|error| failure(error.to_string()))?;
    // Inspection consumes accepted compilation inputs, but never arms the
    // lock's restricted-action grants or turns observed trust into admission.
    let mut session = PackageBuildSession::create(build_dir)?;
    let result = compile_accepted_project(
        AcceptedCompilationRequest {
            closure: source_closure.for_exact_target(target_profile),
            accepted: accepted.as_ref(),
            entry: &entry_path,
            snapshot: None,
            verify: true,
            timings: false,
            product: compiler::CompilationProduct::Inspection(machine.to_owned()),
        },
        &mut session,
    );
    session.dispose(result.and_then(|output| {
        output
            .into_inspection()
            .map_err(|error| CompileResolvedPackageReviewsError::Compilation {
                package: source_closure.graph().root().clone(),
                diagnostics: error.into_diagnostics(),
            })
    }))
}

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
    let (entry_path, source_closure, prepared_targets) = prepared.into_target_review_parts();
    let targets: Vec<_> = prepared_targets
        .into_iter()
        .filter(|target| target_profiles.contains(&target.profile))
        .collect();
    // The whole batch borrows one invocation: sponsors, staging and the pure
    // Build store persist across targets rather than replenishing per check.
    let mut session = match PackageBuildSession::create(&build_dir) {
        Ok(session) => session,
        Err(error) => {
            let message = error.to_string();
            return targets
                .iter()
                .map(|target| CheckedProjectTargetOutcome {
                    target: target.profile,
                    result: Err(CheckPreparedLocalProjectError::Review(
                        CompileResolvedPackageReviewsError::Compilation {
                            package: source_closure.graph().root().clone(),
                            diagnostics: vec![Diagnostic::error(message.clone())],
                        },
                    )),
                })
                .collect();
        }
    };
    let requests = targets
        .iter()
        .filter_map(|target| {
            let accepted = target.accepted.as_ref().ok()?;
            Some(AcceptedCompilationRequest {
                closure: source_closure.for_exact_target(target.profile),
                accepted: accepted.as_ref(),
                entry: &entry_path,
                snapshot: build_snapshot.as_ref(),
                verify,
                timings,
                product: compiler::CompilationProduct::Check,
            })
        })
        .collect();
    let mut checked = compile_accepted_projects(requests, &mut session).into_iter();
    let outcomes = targets
        .into_iter()
        .map(|prepared_target| {
            let profile = prepared_target.profile;
            let result = prepared_target
                .accepted
                .map_err(CheckPreparedLocalProjectError::Prepare)
                .and_then(|_| {
                    checked
                        .next()
                        .ok_or(CheckPreparedLocalProjectError::Report(
                            "compiler lost a package target outcome",
                        ))?
                        .map_err(CheckPreparedLocalProjectError::Review)
                })
                .and_then(|product| product.into_report().map_err(product_failure));
            CheckedProjectTargetOutcome {
                target: profile,
                result,
            }
        })
        .collect::<Vec<_>>();
    // Staging cleanup is disposal work, not a target outcome; the shared
    // Drop would otherwise swallow it silently.
    let _ = session.close();

    outcomes
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
