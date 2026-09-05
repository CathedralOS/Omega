//! Checked reporting for one prepared package or application project.

use super::PreparedLocalProject;
use crate::review::{
    CompileResolvedPackageReviewsError, compile_resolved_package_candidate_for_check,
};
use compiler::{
    ArtifactEmissionPolicy, CompileOptions, CompileOutputKind, CompileReport, TrustAdmission,
};
use diagnostics::Diagnostic;
use std::fmt;
use std::path::PathBuf;
use target::TargetProfile;

#[cfg(test)]
mod tests;

pub struct PreparedLocalProjectCheckRequest {
    prepared: PreparedLocalProject,
    build_dir: PathBuf,
    target_profile: TargetProfile,
    artifact_policy: ArtifactEmissionPolicy,
    accepted_trust_admissions: Vec<TrustAdmission>,
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
            artifact_policy: ArtifactEmissionPolicy::Full,
            accepted_trust_admissions: Vec::new(),
        }
    }

    pub fn with_artifact_policy(mut self, artifact_policy: ArtifactEmissionPolicy) -> Self {
        self.artifact_policy = artifact_policy;
        self
    }

    pub fn with_accepted_trust_admissions(mut self, admissions: Vec<TrustAdmission>) -> Self {
        self.accepted_trust_admissions = admissions;
        self
    }
}

#[derive(Debug)]
pub enum CheckPreparedLocalProjectError {
    Review(CompileResolvedPackageReviewsError),
    CheckedObservations(Vec<Diagnostic>),
    Report(&'static str),
}

impl fmt::Display for CheckPreparedLocalProjectError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Review(error) => write!(formatter, "cannot check prepared project: {error}"),
            Self::CheckedObservations(diagnostics) => {
                write!(
                    formatter,
                    "cannot validate package trust observations: {diagnostics:?}"
                )
            }
            Self::Report(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for CheckPreparedLocalProjectError {}

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
        artifact_policy,
        accepted_trust_admissions,
    } = request;
    let (entry_path, source_closure) = prepared.into_review_parts();
    let checked = compile_resolved_package_candidate_for_check(
        &source_closure.for_exact_target(target_profile),
        &build_dir,
        &entry_path,
    )
    .map_err(CheckPreparedLocalProjectError::Review)?;
    let options = CompileOptions {
        root_path: entry_path,
        build_dir: Some(build_dir),
        target_name: Some(target_profile.target_name().to_owned()),
    };
    let settlement = compiler::report_checked_compilation_observations(
        &options,
        artifact_policy,
        &accepted_trust_admissions,
        &checked,
    )
    .map_err(CheckPreparedLocalProjectError::CheckedObservations)?;
    CompileReport::checked(
        options.root_path,
        checked.source_file_count(),
        false,
        CompileOutputKind::CheckOnly,
        None,
        None,
    )
    .map(|report| report.with_trust_admission_settlement(settlement))
    .map_err(CheckPreparedLocalProjectError::Report)
}
