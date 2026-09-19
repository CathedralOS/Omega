//! Accepted native production for one already-prepared local project.

use crate::review::SemanticBindingReview;

use super::PreparedLocalProject;
use crate::admission::{
    AcceptedNativeInput, AcceptedNativeRealizationRequest, AcceptedOrdinaryEvidenceError,
    accept_ordinary_closure_evidence, realize_accepted_native_report,
};
use crate::review::{
    CanonicalPackageReconstructionQuestionLimits, CompileResolvedPackageReviewsError,
    ReviewOnlyCapabilityConflictLimits, compile_resolved_package_candidate_for_production,
};
use compiler::{CompileReport, OptimizationRollback, TrustAdmission};
use diagnostics::Diagnostic;
use native_realization::{
    TerminalAuthorityPermissionPolicy, TerminalAuthorityPolicy, current_terminal_authority_policy,
};
use std::fmt;
use std::path::PathBuf;

/// Complete policy and output input for one package-aware native production.
///
/// Construction defaults to the toolchain's mechanism-classification policy
/// and no receiving permission policy. An absent receiving policy is not a
/// deny-all or allow-all policy: production then makes no receiver-admission
/// claim, never fabricates receiver rows from accepted package evidence, and
/// the emitted artifact cannot satisfy an explicit admission replay. Callers
/// may replace the mechanism policy or explicitly supply a receiving policy;
/// package acceptance is checked against the prepared project's accepted lock
/// target regardless.
pub struct PreparedLocalProjectNativeRequest {
    prepared: PreparedLocalProject,
    build_dir: PathBuf,
    target_profile: target::TargetProfile,

    accepted_trust_admissions: Vec<TrustAdmission>,
    optimization_rollback: OptimizationRollback,
    build_snapshot: Option<build_evaluation::BuildSnapshotRequest>,
    terminal_authority_policy: TerminalAuthorityPolicy,
    receiving_terminal_authority_permission_policy: Option<TerminalAuthorityPermissionPolicy>,
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

            accepted_trust_admissions: Vec::new(),
            optimization_rollback: OptimizationRollback::default(),
            build_snapshot: None,
            terminal_authority_policy: current_terminal_authority_policy(),
            receiving_terminal_authority_permission_policy: None,
        }
    }

    pub fn with_accepted_trust_admissions(mut self, admissions: Vec<TrustAdmission>) -> Self {
        self.accepted_trust_admissions = admissions;
        self
    }

    pub fn with_optimization_rollback(mut self, rollback: OptimizationRollback) -> Self {
        self.optimization_rollback = rollback;
        self
    }

    /// Select the root build's input inventory and output obligations. Dependency
    /// builds retain their independently captured package inventories.
    pub fn with_build_snapshot(mut self, snapshot: build_evaluation::BuildSnapshotRequest) -> Self {
        self.build_snapshot = Some(snapshot);
        self
    }

    /// Supply the receiving mechanism-classification policy. Accepted package
    /// evidence never manufactures these rows; every demanded
    /// normalized-foreign, syscall, or checked-physical leaf still needs one
    /// exact explicit row from the receiving authority.
    pub fn with_terminal_authority_policy(mut self, policy: TerminalAuthorityPolicy) -> Self {
        self.terminal_authority_policy = policy;
        self
    }

    /// Supply an explicit receiving permission policy. Calling this is the
    /// only way package production claims receiver admission; the policy is
    /// adjudicated exactly as supplied, including an explicit empty policy.
    pub fn with_receiving_terminal_authority_permission_policy(
        mut self,
        policy: TerminalAuthorityPermissionPolicy,
    ) -> Self {
        self.receiving_terminal_authority_permission_policy = Some(policy);
        self
    }
}

#[derive(Debug)]
pub enum CompilePreparedLocalProjectNativeError {
    Review(CompileResolvedPackageReviewsError),
    Evidence(AcceptedOrdinaryEvidenceError),
    TrustAdmission(Vec<Diagnostic>),
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
            Self::TrustAdmission(diagnostics) => {
                write!(formatter, "cannot admit package trust: {diagnostics:?}")
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

/// Observe the exact checked program that native production consumes, without
/// repeating package acquisition or sponsored build execution. The observer must
/// only compute a private result: native realization can still reject afterward.
/// Neither the observation nor a successful report replaces the caller's exact
/// trust-admission settlement before publication or execution.
/// Check accepted project policy and realize one freshly compiled application.
/// `observe` sees the checked compilation before realization; a caller with
/// nothing to observe passes `|_| ()`.
///
/// This operation is the production CLI seam. It neither asks project
/// preparation to infer permissions nor treats decoded policy bytes as
/// evidence: all acceptance starts again from live resolver custody and the
/// exact final checked review pass.
pub fn compile_prepared_local_project_for_native<Observation>(
    request: PreparedLocalProjectNativeRequest,
    observe: impl FnOnce(&compiler::CheckedCompilation) -> Observation,
) -> Result<(CompileReport, Observation), CompilePreparedLocalProjectNativeError> {
    let PreparedLocalProjectNativeRequest {
        prepared,
        build_dir,
        target_profile,

        accepted_trust_admissions,
        optimization_rollback,
        build_snapshot,
        terminal_authority_policy,
        receiving_terminal_authority_permission_policy,
    } = request;
    let (_, source_closure, accepted_target) = prepared.into_review_parts();
    let target_closure = source_closure.for_exact_target(target_profile);
    let candidate = compile_resolved_package_candidate_for_production(
        &target_closure,
        &build_dir,
        SemanticBindingReview::Discover,
        build_snapshot.as_ref(),
    )
    .map_err(CompilePreparedLocalProjectNativeError::Review)?;
    let evidence = accept_ordinary_closure_evidence(
        &target_closure,
        candidate.reviews(),
        CanonicalPackageReconstructionQuestionLimits::default(),
        ReviewOnlyCapabilityConflictLimits::default(),
        accepted_target.as_ref(),
    )
    .map_err(CompilePreparedLocalProjectNativeError::Evidence)?;
    let admission =
        compiler::admit_checked_compilation(candidate.checked_root(), &accepted_trust_admissions)
            .map_err(CompilePreparedLocalProjectNativeError::TrustAdmission)?;
    let trust_settlement = admission.into_settlement();
    let observation = observe(candidate.checked_root());
    realize_accepted_native_report(
        AcceptedNativeInput::Reviewed {
            candidate: Box::new(candidate),
            optimization_rollback: &optimization_rollback,
        },
        AcceptedNativeRealizationRequest {
            evidence: &evidence,
            profile: &proof_admission::AdmissionProfile::default(),
            terminal_authority_policy,
            receiving_terminal_authority_permission_policy,
            imports: &[],
        },
    )
    .map(|report| {
        (
            report.with_trust_admission_settlement(trust_settlement),
            observation,
        )
    })
    .map_err(CompilePreparedLocalProjectNativeError::Native)
}

#[cfg(test)]
mod tests {
    use super::PathBuf;
    mod accepted_lock;
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
