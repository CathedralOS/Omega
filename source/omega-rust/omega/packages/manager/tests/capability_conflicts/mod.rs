use omega_package_evidence::record::{
    PackageReviewCanonicalRowKind, PackageReviewCanonicalRowRisk, PackageReviewCanonicalRowSource,
    PackageReviewSourceLocationRole,
};
use omega_package_manager::resolution::graph::{
    PackageSourceClosureLimits, ResolveExternalLocalPackageClosureError,
    ResolvedPackageSourceClosure, resolve_external_local_package_closure_with_storage,
};
use omega_package_manager::resolution::source::ResolvePackageSourceError;
use omega_package_manager::review::{
    PackageTriageDisposition, PackageTriageReason, ReviewOnlyBaselineCapsule,
    ReviewOnlyBaselineLimits, ReviewOnlyCapabilityConflictBaseline,
    ReviewOnlyCapabilityConflictChange, ReviewOnlyCapabilityConflictError,
    ReviewOnlyCapabilityConflictLimits, ReviewOnlyRootPolicyDirectory,
    ReviewOnlyRootPolicyDisposition, ReviewOnlyRootPolicyFileError, ReviewOnlyRootPolicyName,
    ReviewOnlyRootPolicyNameError, ReviewOnlyRootPolicyRecordError,
    ReviewOnlyRootPolicyRecordLimits, ReviewOnlyRootPolicyResolutionError,
    compare_review_only_capabilities, compare_review_only_capabilities_from_baseline,
    compare_review_only_initial_capabilities, compile_resolved_package_reviews,
    recover_review_only_root_policy_resolution, resolve_review_only_root_policy_decisions,
    triage_initial_install, triage_review_update, triage_review_update_from_baseline,
};
use omega_package_source::{ExternalSourceContext, LocalSourceLimits, SourceResolverStorage};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_root(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "omega-capability-conflict-{name}-{}-{stamp}",
        std::process::id()
    ))
}

fn hex_digest(digest: [u8; 32]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn write_package(root: &Path, main: &str) {
    std::fs::create_dir_all(root).expect("create test package");
    std::fs::write(
        root.join("build.omg"),
        r#"target windows_x86_64 { }

machine build(builder: &mut Build) {
    builder.package("conflict-probe");
}
"#,
    )
    .expect("write package declaration");
    std::fs::write(root.join("main.omg"), main).expect("write package source");
}

fn resolve_external_local_package_closure(
    live_root: impl AsRef<Path>,
    source_context: ExternalSourceContext,
    cache_base: impl AsRef<Path>,
    source_limits: LocalSourceLimits,
    closure_limits: PackageSourceClosureLimits,
) -> Result<ResolvedPackageSourceClosure, ResolveExternalLocalPackageClosureError> {
    let storage = SourceResolverStorage::for_hardened_base(cache_base).map_err(|error| {
        ResolveExternalLocalPackageClosureError::Root(ResolvePackageSourceError::Source(error))
    })?;
    resolve_external_local_package_closure_with_storage(
        live_root,
        source_context,
        omega_target::TargetProfile::CrossPlatformCli,
        &storage,
        source_limits,
        closure_limits,
    )
}

mod operational;
mod public_api;
mod transaction;
