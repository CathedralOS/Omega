//! Accepted native production for one already-prepared local project.

use super::PreparedLocalProject;
use crate::admission::{
    AcceptedOrdinaryEvidenceError, accept_ordinary_closure_evidence,
    realize_accepted_reviewed_package_candidate_report_with_source_evaluated_imports_and_policy,
};
use crate::review::{
    CanonicalPackageReconstructionQuestionLimits, CompileResolvedPackageReviewsError,
    ReviewOnlyCapabilityConflictLimits, compile_resolved_package_candidate_for_production,
};
use compiler::{
    ArtifactEmissionPolicy, CompileOptions, CompileReport, OptimizationRollback, TrustAdmission,
};
use diagnostics::Diagnostic;
use native_realization::{
    TerminalAuthorityPermissionPolicy, current_terminal_authority_permission_policy,
    current_terminal_authority_policy,
};
use std::fmt;
use std::path::PathBuf;

/// Complete policy and output input for one package-aware native production.
///
/// Construction defaults to the toolchain's explicit deny-by-absence
/// receiving permission policy. Callers may replace that policy, but package
/// acceptance is checked against the prepared project's accepted lock target.
pub struct PreparedLocalProjectNativeRequest {
    prepared: PreparedLocalProject,
    build_dir: PathBuf,
    target_profile: target::TargetProfile,
    artifact_policy: ArtifactEmissionPolicy,
    accepted_trust_admissions: Vec<TrustAdmission>,
    optimization_rollback: OptimizationRollback,
    receiving_terminal_authority_permission_policy: TerminalAuthorityPermissionPolicy,
}

impl PreparedLocalProjectNativeRequest {
    pub fn new(
        prepared: PreparedLocalProject,
        build_dir: impl Into<PathBuf>,
        target_profile: target::TargetProfile,
    ) -> Self {
        Self {
            prepared,
            build_dir: build_dir.into(),
            target_profile,
            artifact_policy: ArtifactEmissionPolicy::Full,
            accepted_trust_admissions: Vec::new(),
            optimization_rollback: OptimizationRollback::default(),
            receiving_terminal_authority_permission_policy:
                current_terminal_authority_permission_policy(),
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

    pub fn with_optimization_rollback(mut self, rollback: OptimizationRollback) -> Self {
        self.optimization_rollback = rollback;
        self
    }

    pub fn with_receiving_terminal_authority_permission_policy(
        mut self,
        policy: TerminalAuthorityPermissionPolicy,
    ) -> Self {
        self.receiving_terminal_authority_permission_policy = policy;
        self
    }
}

#[derive(Debug)]
pub enum CompilePreparedLocalProjectNativeError {
    Review(CompileResolvedPackageReviewsError),
    Evidence(AcceptedOrdinaryEvidenceError),
    CheckedObservations(Vec<Diagnostic>),
    Native(Vec<Diagnostic>),
}

impl fmt::Display for CompilePreparedLocalProjectNativeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Review(error) => {
                write!(formatter, "cannot compile fresh package review: {error}")
            }
            Self::Evidence(error) => {
                write!(
                    formatter,
                    "cannot accept fresh package review evidence: {error}"
                )
            }
            Self::CheckedObservations(diagnostics) => {
                write!(
                    formatter,
                    "cannot validate package trust observations: {diagnostics:?}"
                )
            }
            Self::Native(diagnostics) => {
                write!(
                    formatter,
                    "cannot realize accepted package production: {diagnostics:?}"
                )
            }
        }
    }
}

impl std::error::Error for CompilePreparedLocalProjectNativeError {}

/// Check accepted project policy and realize one freshly compiled application.
///
/// This operation is the production CLI seam. It neither asks project
/// preparation to infer permissions nor treats decoded policy bytes as
/// evidence: all acceptance starts again from live resolver custody and the
/// exact final checked review pass.
pub fn compile_prepared_local_project_for_native(
    request: PreparedLocalProjectNativeRequest,
) -> Result<CompileReport, CompilePreparedLocalProjectNativeError> {
    let PreparedLocalProjectNativeRequest {
        prepared,
        build_dir,
        target_profile,
        artifact_policy,
        accepted_trust_admissions,
        optimization_rollback,
        receiving_terminal_authority_permission_policy,
    } = request;
    let (entry_path, source_closure, accepted_target) = prepared.into_review_parts();
    let target_closure = source_closure.for_exact_target(target_profile);
    let candidate = compile_resolved_package_candidate_for_production(&target_closure, &build_dir)
        .map_err(CompilePreparedLocalProjectNativeError::Review)?;
    let evidence = accept_ordinary_closure_evidence(
        &target_closure,
        candidate.reviews(),
        CanonicalPackageReconstructionQuestionLimits::default(),
        ReviewOnlyCapabilityConflictLimits::default(),
        accepted_target.as_ref(),
    )
    .map_err(CompilePreparedLocalProjectNativeError::Evidence)?;
    let options = CompileOptions {
        root_path: entry_path,
        build_dir: Some(build_dir),
        target_name: Some(target_profile.target_name().to_owned()),
    };
    let trust_settlement = compiler::report_checked_compilation_observations(
        &options,
        artifact_policy,
        &accepted_trust_admissions,
        candidate.checked_root(),
    )
    .map_err(CompilePreparedLocalProjectNativeError::CheckedObservations)?;
    realize_accepted_reviewed_package_candidate_report_with_source_evaluated_imports_and_policy(
        candidate,
        &evidence,
        &proof_admission::AdmissionProfile::default(),
        &optimization_rollback,
        current_terminal_authority_policy(),
        receiving_terminal_authority_permission_policy,
        &[],
    )
    .map(|report| report.with_trust_admission_settlement(trust_settlement))
    .map_err(CompilePreparedLocalProjectNativeError::Native)
}

#[cfg(test)]
mod tests {
    mod accepted_lock;

    use super::*;
    use crate::review::ReviewOnlyRootPolicyDisposition;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TREE: AtomicU64 = AtomicU64::new(0);

    struct TemporaryProject {
        source: PathBuf,
        workspace: PathBuf,
    }

    impl TemporaryProject {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "omega-cli-package-native-{}-{}",
                std::process::id(),
                NEXT_TREE.fetch_add(1, Ordering::Relaxed),
            ));
            std::fs::create_dir(&path).expect("create temporary package project");
            std::fs::write(
                path.join("build.omg"),
                r#"
machine build(builder: &mut Build) {
    builder.application("accepted-claim-app");
    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
}
"#,
            )
            .expect("write package build declaration");
            std::fs::write(
                path.join("main.omg"),
                r#"pub proposition accepted(value: u64);

boundary machine trusted_value() -> u64
ensures accepted(result);

data Main { }

machine Main::main(&mut self) { }
"#,
            )
            .expect("write package application");
            let workspace = std::env::temp_dir().join(format!(
                "omega-cli-package-workspace-{}-{}",
                std::process::id(),
                NEXT_TREE.fetch_add(1, Ordering::Relaxed),
            ));
            std::fs::create_dir(&workspace).expect("create temporary build workspace");
            Self {
                source: path,
                workspace,
            }
        }

        fn entry(&self) -> PathBuf {
            self.source.join("main.omg")
        }
    }

    impl Drop for TemporaryProject {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.source);
            let _ = std::fs::remove_dir_all(&self.workspace);
        }
    }
}
