//! Checked inspection and reporting for one prepared package or application project.

use super::accepted_compilation::{AcceptedCompilationRequest, compile_accepted_project};
use super::prepare_project::PreparedProjectTarget;
use super::{PrepareLocalProjectError, PreparedLocalProject};
use crate::build_session::PackageSourcePreparation;
use crate::review::CompileResolvedPackageReviewsError;
use compiler::{CompileOptions, CompileReport};
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
                write!(formatter, "cannot admit package trust: {diagnostics:?}")
            }
            Self::ProofProduct(diagnostics) => {
                write!(
                    formatter,
                    "cannot satisfy the requested proof product: {diagnostics:?}"
                )
            }
            Self::Report(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for CheckPreparedLocalProjectError {}

/// Check the exact requested entry after its dependency builds, retaining
/// generated source and checked semantics for an inspecting consumer. This is
/// the same accepted-input pipeline used by check reporting, but stops before trust
/// admission or product publication. Inspection may describe unresolved trust;
/// it cannot use the returned program as evidence of accepted native authority.
pub fn check_prepared_local_project_for_inspection(
    prepared: PreparedLocalProject,
    build_dir: &Path,
    target_profile: TargetProfile,
) -> Result<compiler::CheckedCompilation, CompileResolvedPackageReviewsError> {
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
    compile_accepted_project(
        AcceptedCompilationRequest {
            closure: &source_closure.for_exact_target(target_profile),
            accepted: accepted.as_ref(),
            entry: &entry_path,
            build_root: build_dir,
            snapshot: None,
            verify: true,
            timings: false,
        },
        &mut PackageSourcePreparation::for_closure(&source_closure),
    )
}

/// One target's check result inside a shared multi-target request. A failing
/// target never stops or hides a sibling.
pub struct CheckedProjectTargetOutcome {
    pub target: TargetProfile,
    pub result: Result<CompileReport, CheckPreparedLocalProjectError>,
}

/// Run scoped candidate checking and report its final checked root. Generated
/// source and semantic bindings enter through the candidate pipeline; reporting
/// uses the retained result after staging disposal without another build run.
/// This is the per-target route of [`check_prepared_local_project_for_targets`]
/// for one target, staging directly into `build_dir`.
pub fn check_prepared_local_project(
    request: PreparedLocalProjectCheckRequest,
) -> Result<CompileReport, CheckPreparedLocalProjectError> {
    let PreparedLocalProjectCheckRequest {
        prepared,
        build_dir,
        target_profiles,
        verify,
        build_snapshot,
        timings,
    } = request;
    let [target_profile] = target_profiles.as_slice() else {
        return Err(CheckPreparedLocalProjectError::Report(
            "check_prepared_local_project needs a single-target request",
        ));
    };
    let target_profile = *target_profile;
    let (entry_path, source_closure, accepted_target) = prepared
        .into_review_parts(target_profile)
        .map_err(CheckPreparedLocalProjectError::Prepare)?;
    let mut preparation = PackageSourcePreparation::for_closure(&source_closure);
    check_one_prepared_target(
        PreparedProjectTarget {
            profile: target_profile,
            accepted: Ok(accepted_target),
        },
        &source_closure,
        &entry_path,
        &build_dir,
        build_snapshot.as_ref(),
        verify,
        timings,
        &mut preparation,
    )
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
    let mut preparation = PackageSourcePreparation::for_closure(&source_closure);
    prepared_targets
        .into_iter()
        .filter(|target| target_profiles.contains(&target.profile))
        .map(|prepared_target| {
            let profile = prepared_target.profile;
            let result = check_one_prepared_target(
                prepared_target,
                &source_closure,
                &entry_path,
                &build_dir.join(profile.target_name()),
                build_snapshot.as_ref(),
                verify,
                timings,
                &mut preparation,
            );
            CheckedProjectTargetOutcome {
                target: profile,
                result,
            }
        })
        .collect()
}

/// Check one prepared target, staging into `target_build_dir`. Checked
/// reporting consumes the compile's generated sources, so it is an executor
/// of the accepted policy, not an observational review: the target's own
/// restricted-request checkpoint arms the pass, and a projected request the
/// accepted rows do not grant rejects before its own build effect executes.
fn check_one_prepared_target(
    prepared_target: PreparedProjectTarget,
    source_closure: &crate::resolution::graph::ResolvedPackageSourceClosure,
    entry_path: &Path,
    target_build_dir: &Path,
    build_snapshot: Option<&build_evaluation::BuildSnapshotRequest>,
    verify: bool,
    timings: bool,
    preparation: &mut PackageSourcePreparation,
) -> Result<CompileReport, CheckPreparedLocalProjectError> {
    let profile = prepared_target.profile;
    let accepted_target = prepared_target
        .accepted
        .map_err(CheckPreparedLocalProjectError::Prepare)?;
    let checked = compile_accepted_project(
        AcceptedCompilationRequest {
            closure: &source_closure.for_exact_target(profile),
            accepted: accepted_target.as_ref(),
            entry: entry_path,
            build_root: target_build_dir,
            snapshot: build_snapshot,
            verify,
            timings,
        },
        preparation,
    )
    .map_err(CheckPreparedLocalProjectError::Review)?;
    let options = CompileOptions {
        root_path: entry_path.to_path_buf(),
        build_dir: Some(target_build_dir.to_path_buf()),
        target_name: Some(profile.target_name().to_owned()),
    };
    checked
        .admit_trust()
        .map_err(CheckPreparedLocalProjectError::TrustAdmission)?;
    // The checked project reaches product admission through the same
    // settlement call the standalone compiler uses, so a check-only stop
    // refuses an incompatible proof-product or release-rollback request here
    // rather than through a second check-only helper.
    match checked
        .prepare_product_report(
            options.root_path,
            compiler::RequestedCompileProduct::Check,
            &compiler::OptimizationRollback::default(),
        )
        .map_err(CheckPreparedLocalProjectError::ProofProduct)?
    {
        compilation_products::CheckedProductReport::Complete(report) => Ok(report),
        compilation_products::CheckedProductReport::Terminal(_)
        | compilation_products::CheckedProductReport::Native(_) => Err(
            CheckPreparedLocalProjectError::Report("check admission settled a producing product"),
        ),
    }
}
