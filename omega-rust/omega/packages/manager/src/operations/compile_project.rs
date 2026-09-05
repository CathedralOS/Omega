//! Accepted native production for one already-prepared local project.

use super::PreparedLocalProject;
use crate::admission::{
    AcceptedOrdinaryEvidenceError, accept_ordinary_closure_evidence,
    realize_accepted_reviewed_package_candidate_report_with_source_evaluated_imports_and_policy,
};
use crate::review::{
    CanonicalPackageReconstructionQuestionLimits, CompileResolvedPackageReviewsError,
    ReviewOnlyCapabilityConflictError, ReviewOnlyCapabilityConflictLimits,
    ReviewOnlyRootPolicyDirectory, ReviewOnlyRootPolicyFileError, ReviewOnlyRootPolicyName,
    ReviewOnlyRootPolicyRecordLimits, compare_review_only_initial_capabilities,
    compile_resolved_package_candidate_for_production,
};
use compiler::{
    ArtifactEmissionPolicy, CompileOptions, CompileReport, OptimizationRollback, TrustAdmission,
};
use diagnostics::Diagnostic;
use std::fmt;
use std::path::PathBuf;
use terminal_psi_to_native_artifact::{
    TerminalAuthorityPermissionPolicy, current_terminal_authority_permission_policy,
    current_terminal_authority_policy,
};

/// One explicit root-owned policy record selected by command orchestration.
///
/// The prepared project cannot construct or discover this value. Recovery
/// still rebinds its bytes to the fresh compiler-derived conflict set.
#[derive(Debug, Clone, Copy)]
pub struct LocalProjectRootPolicy<'a> {
    directory: &'a ReviewOnlyRootPolicyDirectory,
    name: &'a ReviewOnlyRootPolicyName,
}

impl<'a> LocalProjectRootPolicy<'a> {
    pub const fn new(
        directory: &'a ReviewOnlyRootPolicyDirectory,
        name: &'a ReviewOnlyRootPolicyName,
    ) -> Self {
        Self { directory, name }
    }
}

/// Complete policy and output input for one package-aware native production.
///
/// Construction defaults to the toolchain's explicit deny-by-absence
/// receiving permission policy. Callers may replace that policy, but package
/// acceptance remains independently reconstructed from the root-policy file.
pub struct PreparedLocalProjectNativeRequest<'a> {
    prepared: PreparedLocalProject,
    build_dir: PathBuf,
    target_profile: target::TargetProfile,
    root_policy: Option<LocalProjectRootPolicy<'a>>,
    artifact_policy: ArtifactEmissionPolicy,
    accepted_trust_admissions: Vec<TrustAdmission>,
    optimization_rollback: OptimizationRollback,
    receiving_terminal_authority_permission_policy: TerminalAuthorityPermissionPolicy,
}

impl<'a> PreparedLocalProjectNativeRequest<'a> {
    pub fn new(
        prepared: PreparedLocalProject,
        build_dir: impl Into<PathBuf>,
        target_profile: target::TargetProfile,
    ) -> Self {
        Self {
            prepared,
            build_dir: build_dir.into(),
            target_profile,
            root_policy: None,
            artifact_policy: ArtifactEmissionPolicy::Full,
            accepted_trust_admissions: Vec::new(),
            optimization_rollback: OptimizationRollback::default(),
            receiving_terminal_authority_permission_policy:
                current_terminal_authority_permission_policy(),
        }
    }

    pub fn with_root_policy(mut self, root_policy: LocalProjectRootPolicy<'a>) -> Self {
        self.root_policy = Some(root_policy);
        self
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
    Conflict(ReviewOnlyCapabilityConflictError),
    MissingRootPolicy,
    UnexpectedRootPolicy,
    RootPolicyFile(ReviewOnlyRootPolicyFileError),
    Evidence(AcceptedOrdinaryEvidenceError),
    CheckedObservations(Vec<Diagnostic>),
    Native(Vec<Diagnostic>),
}

impl fmt::Display for CompilePreparedLocalProjectNativeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Review(error) => write!(formatter, "cannot compile fresh package review: {error}"),
            Self::Conflict(error) => {
                write!(formatter, "cannot reconstruct package root-policy conflicts: {error}")
            }
            Self::MissingRootPolicy => formatter.write_str(
                "fresh package review has blocking rows but no explicit --package-root-policy",
            ),
            Self::UnexpectedRootPolicy => formatter.write_str(
                "fresh package review has no blocking rows but an explicit package root policy was supplied",
            ),
            Self::RootPolicyFile(error) => write!(formatter, "cannot recover package root policy: {error}"),
            Self::Evidence(error) => {
                write!(formatter, "cannot accept fresh package review evidence: {error}")
            }
            Self::CheckedObservations(diagnostics) => {
                write!(formatter, "cannot validate package trust observations: {diagnostics:?}")
            }
            Self::Native(diagnostics) => {
                write!(formatter, "cannot realize accepted package production: {diagnostics:?}")
            }
        }
    }
}

impl std::error::Error for CompilePreparedLocalProjectNativeError {}

/// Compile, replay root policy, accept, and realize one prepared application.
///
/// This operation is the production CLI seam. It neither asks project
/// preparation to infer permissions nor treats decoded policy bytes as
/// evidence: all acceptance starts again from live resolver custody and the
/// exact final checked review pass.
pub fn compile_prepared_local_project_for_native(
    request: PreparedLocalProjectNativeRequest<'_>,
) -> Result<CompileReport, CompilePreparedLocalProjectNativeError> {
    let PreparedLocalProjectNativeRequest {
        prepared,
        build_dir,
        target_profile,
        root_policy,
        artifact_policy,
        accepted_trust_admissions,
        optimization_rollback,
        receiving_terminal_authority_permission_policy,
    } = request;
    let (entry_path, source_closure) = prepared.into_review_parts();
    let target_closure = source_closure.for_exact_target(target_profile);
    let candidate = compile_resolved_package_candidate_for_production(&target_closure, &build_dir)
        .map_err(CompilePreparedLocalProjectNativeError::Review)?;
    let conflicts = compare_review_only_initial_capabilities(
        candidate.reviews(),
        &target_closure,
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .map_err(CompilePreparedLocalProjectNativeError::Conflict)?;
    let root_policy = match (conflicts.packages().is_empty(), root_policy) {
        (true, None) => None,
        (true, Some(_)) => {
            return Err(CompilePreparedLocalProjectNativeError::UnexpectedRootPolicy);
        }
        (false, None) => return Err(CompilePreparedLocalProjectNativeError::MissingRootPolicy),
        (false, Some(root_policy)) => Some(
            root_policy
                .directory
                .recover_resolution(
                    root_policy.name,
                    &conflicts,
                    ReviewOnlyRootPolicyRecordLimits::default(),
                )
                .map_err(CompilePreparedLocalProjectNativeError::RootPolicyFile)?,
        ),
    };
    let evidence = accept_ordinary_closure_evidence(
        &target_closure,
        candidate.reviews(),
        CanonicalPackageReconstructionQuestionLimits::default(),
        ReviewOnlyCapabilityConflictLimits::default(),
        root_policy.as_ref(),
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
    use super::*;
    use crate::review::{
        ReviewOnlyRootPolicyDisposition, resolve_review_only_root_policy_decisions,
    };
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TREE: AtomicU64 = AtomicU64::new(0);

    struct TemporaryProject {
        source: PathBuf,
        policy: PathBuf,
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
            let policy = std::env::temp_dir().join(format!(
                "omega-cli-package-policy-{}-{}",
                std::process::id(),
                NEXT_TREE.fetch_add(1, Ordering::Relaxed),
            ));
            std::fs::create_dir(&policy).expect("create temporary policy directory");
            let workspace = std::env::temp_dir().join(format!(
                "omega-cli-package-workspace-{}-{}",
                std::process::id(),
                NEXT_TREE.fetch_add(1, Ordering::Relaxed),
            ));
            std::fs::create_dir(&workspace).expect("create temporary build workspace");
            Self {
                source: path,
                policy,
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
            let _ = std::fs::remove_dir_all(&self.policy);
            let _ = std::fs::remove_dir_all(&self.workspace);
        }
    }

    #[test]
    fn native_project_requires_and_replays_exact_explicit_root_policy() {
        let project = TemporaryProject::new();
        let target = target::TargetProfile::LinuxX64;
        let missing = compile_prepared_local_project_for_native(
            PreparedLocalProjectNativeRequest::new(
                super::super::prepare_local_project(&project.entry())
                    .expect("prepare project")
                    .expect("build project"),
                project.workspace.join("missing-policy-build"),
                target,
            )
            .with_artifact_policy(ArtifactEmissionPolicy::OutputOnly),
        );
        assert!(matches!(
            missing,
            Err(CompilePreparedLocalProjectNativeError::MissingRootPolicy)
        ));

        let prepared = super::super::prepare_local_project(&project.entry())
            .expect("prepare policy candidate")
            .expect("build project");
        let (_entry, closure) = prepared.into_review_parts();
        let target_closure = closure.for_exact_target(target);
        let candidate = compile_resolved_package_candidate_for_production(
            &target_closure,
            &project.workspace.join("policy-question-build"),
        )
        .expect("compile policy candidate");
        let conflicts = compare_review_only_initial_capabilities(
            candidate.reviews(),
            &target_closure,
            ReviewOnlyCapabilityConflictLimits::default(),
        )
        .expect("derive policy conflicts");
        assert!(!conflicts.is_empty());
        let decisions = conflicts
            .packages()
            .iter()
            .flat_map(|package| {
                package
                    .conflicts()
                    .iter()
                    .filter(|conflict| conflict.is_blocking())
                    .map(|conflict| {
                        package
                            .root_policy_decision(
                                conflict,
                                ReviewOnlyRootPolicyDisposition::AcceptCandidateChange,
                            )
                            .expect("accept exact blocking conflict")
                    })
            })
            .collect::<Vec<_>>();
        let resolution = resolve_review_only_root_policy_decisions(&conflicts, &decisions)
            .expect("resolve exact policy candidate");
        let policy_path = project.policy.clone();
        let directory =
            cap_std::fs::Dir::open_ambient_dir(&policy_path, cap_std::ambient_authority())
                .expect("open explicit policy directory");
        let directory = ReviewOnlyRootPolicyDirectory::from_capability(directory, &policy_path)
            .expect("bind policy directory");
        let name = ReviewOnlyRootPolicyName::parse("candidate.policy").expect("policy name");
        directory
            .persist_new_resolution(
                &name,
                &resolution,
                ReviewOnlyRootPolicyRecordLimits::default(),
            )
            .expect("persist exact policy");

        let report = compile_prepared_local_project_for_native(
            PreparedLocalProjectNativeRequest::new(
                super::super::prepare_local_project(&project.entry())
                    .expect("prepare accepted project")
                    .expect("build project"),
                project.workspace.join("accepted-build"),
                target,
            )
            .with_root_policy(LocalProjectRootPolicy::new(&directory, &name))
            .with_artifact_policy(ArtifactEmissionPolicy::OutputOnly),
        )
        .expect("exact recovered root policy admits native production");
        assert_eq!(
            report.output_kind(),
            compiler::CompileOutputKind::RetainedNativeArtifact
        );
        assert!(report.production_manifest().is_some());
        report
            .retained_native_artifact()
            .expect("native custody")
            .validate()
            .expect("valid native artifact");

        std::fs::write(
            project.source.join("main.omg"),
            "data Main { }\nmachine Main::main(&mut self) { }\n",
        )
        .expect("replace application with blocker-free source");
        let unexpected = compile_prepared_local_project_for_native(
            PreparedLocalProjectNativeRequest::new(
                super::super::prepare_local_project(&project.entry())
                    .expect("prepare blocker-free project")
                    .expect("build project"),
                project.workspace.join("unexpected-policy-build"),
                target,
            )
            .with_root_policy(LocalProjectRootPolicy::new(&directory, &name))
            .with_artifact_policy(ArtifactEmissionPolicy::OutputOnly),
        );
        assert!(matches!(
            unexpected,
            Err(CompilePreparedLocalProjectNativeError::UnexpectedRootPolicy)
        ));
    }
}
