use omega_package_review::{
    PackageReviewCanonicalRowKind, PackageReviewCanonicalRowRisk, PackageReviewCanonicalRowSource,
    PackageReviewSourceLocationRole,
};
use omega_packages::{
    ExternalSourceContext, LocalSourceLimits, PackageSourceClosureLimits, PackageTriageDisposition,
    PackageTriageReason, ReviewOnlyBaselineCapsule, ReviewOnlyBaselineLimits,
    ReviewOnlyCapabilityConflictChange, ReviewOnlyCapabilityConflictError,
    ReviewOnlyCapabilityConflictLimits, ReviewOnlyRootPolicyDirectory,
    ReviewOnlyRootPolicyDisposition, ReviewOnlyRootPolicyFileError, ReviewOnlyRootPolicyName,
    ReviewOnlyRootPolicyNameError, ReviewOnlyRootPolicyRecordError,
    ReviewOnlyRootPolicyRecordLimits, ReviewOnlyRootPolicyResolutionError,
    compare_review_only_capabilities, compare_review_only_capabilities_from_baseline,
    compile_resolved_package_reviews, recover_review_only_root_policy_resolution,
    resolve_external_local_package_closure, resolve_review_only_root_policy_decisions,
    triage_review_update, triage_review_update_from_baseline,
};
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
        r#"target windows_x64 { }

machine build(builder: &mut Build) {
    builder.package("conflict-probe");
}
"#,
    )
    .expect("write package declaration");
    std::fs::write(root.join("main.omg"), main).expect("write package source");
}

mod operational;
mod public_api;
mod transaction;
