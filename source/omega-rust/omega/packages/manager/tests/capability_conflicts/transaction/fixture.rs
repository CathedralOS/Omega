use super::*;

pub(super) struct ExactCompilerRowScenario {
    pub(super) live: PathBuf,
    pub(super) baseline_cache: PathBuf,
    pub(super) stale_baseline_cache: PathBuf,
    pub(super) candidate_cache: PathBuf,
    pub(super) representation_cache: PathBuf,
    pub(super) dangerous_slack_cache: PathBuf,
    pub(super) accepted_claim_baseline_cache: PathBuf,
    pub(super) accepted_claim_candidate_cache: PathBuf,
    pub(super) build_root: PathBuf,
    pub(super) policy_root: PathBuf,
    pub(super) policy_outside: PathBuf,
    pub(super) baseline_sources: ResolvedPackageSourceClosure,
    pub(super) candidate_sources: ResolvedPackageSourceClosure,
    pub(super) baseline_reviews: omega_package_manager::CompilerIssuedPackageReviewSet,
    pub(super) stale_baseline_reviews: omega_package_manager::CompilerIssuedPackageReviewSet,
    pub(super) candidate_reviews: omega_package_manager::CompilerIssuedPackageReviewSet,
}

impl ExactCompilerRowScenario {
    pub(super) fn establish() -> Self {
        let live = temp_root("live");
        let baseline_cache = temp_root("baseline-cache");
        let stale_baseline_cache = temp_root("stale-baseline-cache");
        let candidate_cache = temp_root("candidate-cache");
        let representation_cache = temp_root("representation-cache");
        let dangerous_slack_cache = temp_root("dangerous-slack-cache");
        let accepted_claim_baseline_cache = temp_root("accepted-claim-baseline-cache");
        let accepted_claim_candidate_cache = temp_root("accepted-claim-candidate-cache");
        let build_root = temp_root("build");
        let policy_root = temp_root("capability-conflict-root-policy");
        let policy_outside = temp_root("capability-conflict-root-policy-outside");
        let context = ExternalSourceContext::derive(b"capability-conflict-test-lock");
        write_package(
            &live,
            r#"pub machine add_u64(left: u64, right: u64) -> u64 {
    left + right
}
"#,
        );
        let baseline_sources = resolve_external_local_package_closure(
            &live,
            context.clone(),
            &baseline_cache,
            LocalSourceLimits::default(),
            PackageSourceClosureLimits::default(),
        )
        .expect("resolve baseline custody");
        let baseline_reviews =
            compile_resolved_package_reviews(&baseline_sources, "windows_x64", &build_root)
                .expect("compile baseline review");

        write_package(
            &live,
            r#"pub machine add_u64(left: u64, right: u64) -> u64 {
    left + right
}

pub proposition ready();
"#,
        );
        let stale_baseline_sources = resolve_external_local_package_closure(
            &live,
            context.clone(),
            &stale_baseline_cache,
            LocalSourceLimits::default(),
            PackageSourceClosureLimits::default(),
        )
        .expect("resolve alternate baseline custody");
        let stale_baseline_reviews =
            compile_resolved_package_reviews(&stale_baseline_sources, "windows_x64", &build_root)
                .expect("compile alternate baseline review");

        write_package(
            &live,
            r#"pub machine add_u64(left: u64, right: u64) -> u64 {
    left + right
}

pub proposition ready();
pub proposition settled();
"#,
        );
        let candidate_sources = resolve_external_local_package_closure(
            &live,
            context,
            &candidate_cache,
            LocalSourceLimits::default(),
            PackageSourceClosureLimits::default(),
        )
        .expect("resolve candidate custody");
        let candidate_reviews =
            compile_resolved_package_reviews(&candidate_sources, "windows_x64", &build_root)
                .expect("compile candidate review");

        Self {
            live,
            baseline_cache,
            stale_baseline_cache,
            candidate_cache,
            representation_cache,
            dangerous_slack_cache,
            accepted_claim_baseline_cache,
            accepted_claim_candidate_cache,
            build_root,
            policy_root,
            policy_outside,
            baseline_sources,
            candidate_sources,
            baseline_reviews,
            stale_baseline_reviews,
            candidate_reviews,
        }
    }
}

impl Drop for ExactCompilerRowScenario {
    fn drop(&mut self) {
        for path in [
            &self.live,
            &self.baseline_cache,
            &self.stale_baseline_cache,
            &self.candidate_cache,
            &self.representation_cache,
            &self.dangerous_slack_cache,
            &self.accepted_claim_baseline_cache,
            &self.accepted_claim_candidate_cache,
            &self.build_root,
            &self.policy_root,
            &self.policy_outside,
        ] {
            let _ = std::fs::remove_dir_all(path);
        }
    }
}
