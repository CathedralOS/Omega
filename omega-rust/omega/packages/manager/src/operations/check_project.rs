//! Checked inspection and reporting for one prepared package or application project.

use super::PreparedLocalProject;
use crate::review::{
    CompileResolvedPackageReviewsError, compile_resolved_package_candidate_for_check,
};
use compiler::{CompileOptions, CompileOutputKind, CompileReport, TrustAdmission};
use diagnostics::Diagnostic;
use std::fmt;
use std::path::{Path, PathBuf};
use target::TargetProfile;

#[cfg(test)]
mod tests;

pub struct PreparedLocalProjectCheckRequest {
    prepared: PreparedLocalProject,
    build_dir: PathBuf,
    target_profile: TargetProfile,

    accepted_trust_admissions: Vec<TrustAdmission>,
    build_snapshot: Option<build_evaluation::BuildSnapshotRequest>,
}

impl PreparedLocalProjectCheckRequest {
    pub fn new(
        prepared: PreparedLocalProject,
        build_dir: impl Into<PathBuf>,
        target_profile: TargetProfile,
    ) -> Self {
        Self {
            prepared,
            build_dir: build_dir.into(),
            target_profile,

            accepted_trust_admissions: Vec::new(),
            build_snapshot: None,
        }
    }

    pub fn with_accepted_trust_admissions(mut self, admissions: Vec<TrustAdmission>) -> Self {
        self.accepted_trust_admissions = admissions;
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
/// the same candidate pipeline used by check reporting, but stops before trust
/// admission or product publication. Inspection may describe unresolved trust;
/// it cannot use the returned program as evidence of accepted native authority.
pub fn check_prepared_local_project_for_inspection(
    prepared: PreparedLocalProject,
    build_dir: &Path,
    target_profile: TargetProfile,
) -> Result<compiler::CheckedCompilation, CompileResolvedPackageReviewsError> {
    let (entry_path, source_closure, _) = prepared.into_review_parts();
    compile_resolved_package_candidate_for_check(
        &source_closure.for_exact_target(target_profile),
        build_dir,
        &entry_path,
        None,
    )
}

/// Run scoped candidate checking and report its final checked root. Generated
/// source and semantic bindings enter through the candidate pipeline; reporting
/// uses the retained result after staging disposal without another build run.
pub fn check_prepared_local_project(
    request: PreparedLocalProjectCheckRequest,
) -> Result<CompileReport, CheckPreparedLocalProjectError> {
    let PreparedLocalProjectCheckRequest {
        prepared,
        build_dir,
        target_profile,

        accepted_trust_admissions,
        build_snapshot,
    } = request;
    let (entry_path, source_closure, _) = prepared.into_review_parts();
    let checked = compile_resolved_package_candidate_for_check(
        &source_closure.for_exact_target(target_profile),
        &build_dir,
        &entry_path,
        build_snapshot.as_ref(),
    )
    .map_err(CheckPreparedLocalProjectError::Review)?;
    let options = CompileOptions {
        root_path: entry_path,
        build_dir: Some(build_dir),
        target_name: Some(target_profile.target_name().to_owned()),
    };
    let admission = compiler::admit_checked_compilation(&checked, &accepted_trust_admissions)
        .map_err(CheckPreparedLocalProjectError::TrustAdmission)?;
    let settlement = admission.into_settlement();
    // A check-only stop publishes no artifact pair, so a checked root whose
    // build requested an optional proof product can never be satisfied here.
    // The standalone compiler's `RequestedCompileProduct::Check` arm applies
    // the same admission fence.
    if checked.pcc_requests().any() {
        return Err(CheckPreparedLocalProjectError::ProofProduct(vec![
            Diagnostic::error("a check-only stop cannot satisfy an optional proof-product request"),
        ]));
    }
    CompileReport::checked(
        options.root_path,
        checked.source_file_count(),
        false,
        CompileOutputKind::CheckOnly,
        None,
    )
    .map(|report| report.with_trust_admission_settlement(settlement))
    .map_err(CheckPreparedLocalProjectError::Report)
}
