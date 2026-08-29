use super::*;
use crate::resolution::PackageSourceClosureLimitKind;
use crate::review::{
    PackageSourceReviewLimits, PackageTriageDisposition, PackageTriageReason,
    ReviewOnlyCapabilityConflictChange, ReviewOnlyCapabilityConflictLimits,
    assemble_update_source_review, compare_review_only_capabilities,
    compile_resolved_package_reviews, triage_review_update,
};
use omega_package_review::evidence::{
    PackageReviewCanonicalRowKind, PackageReviewCanonicalRowRisk,
    PackageReviewDangerousAuthorityClass, PackageReviewNominalOwner,
    PackageReviewSourceLocationRole,
};
use omega_package_source::GitTransportProfile;
use std::collections::BTreeSet;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../../tests/fixtures/packages")
}

fn temp_root(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time follows Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "omega-package-source-adapter-{name}-{}-{stamp}",
        std::process::id()
    ))
}

fn fixture_lineage() -> SourceLineage {
    SourceLineage::git("https://github.com/CathedralOS/package-fixtures.git")
        .expect("fixture lineage")
}

fn write_package(root: &Path, name: &str, dependency: Option<&str>) {
    std::fs::create_dir_all(root).expect("create package");
    let dependency = dependency
        .map(|location| {
            let location = location.replace('\\', "\\\\").replace('"', "\\\"");
            format!("    builder.depend(Source::Path {{ location: \"{location}\" }});\n")
        })
        .unwrap_or_default();
    std::fs::write(
            root.join("build.omg"),
            format!(
                "machine build(builder: &mut Build) {{\n    builder.package(\"{name}\");\n{dependency}}}\n"
            ),
        )
        .expect("write build file");
    std::fs::write(root.join("main.omg"), "machine root() {}\n").expect("write source");
}

fn write_application(root: &Path, name: &str, dependency: Option<&str>) {
    std::fs::create_dir_all(root).expect("create application");
    let dependency = dependency
        .map(|location| {
            let location = location.replace('\\', "\\\\").replace('"', "\\\"");
            format!("    builder.depend(Source::Path {{ location: \"{location}\" }});\n")
        })
        .unwrap_or_default();
    std::fs::write(
        root.join("build.omg"),
        format!(
            "machine build(builder: &mut Build) {{\n    builder.application(\"{name}\");\n{dependency}}}\n"
        ),
    )
    .expect("write application build file");
    std::fs::write(root.join("main.omg"), "machine root() {}\n").expect("write application source");
}

fn run_test_git<I, S>(directory: &Path, args: I)
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let output = Command::new("git")
        .current_dir(directory)
        .args(args)
        .output()
        .expect("spawn test Git");
    assert!(
        output.status.success(),
        "test Git command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn test_git_head(directory: &Path) -> String {
    let output = Command::new("git")
        .current_dir(directory)
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("read test Git HEAD");
    assert!(
        output.status.success(),
        "test Git rev-parse failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("Git object ID is UTF-8")
        .trim()
        .to_owned()
}
mod capability_review;
mod external_local;
mod git_cache;
mod git_requests;
mod limits;
mod target_profiles;
mod workspace;
