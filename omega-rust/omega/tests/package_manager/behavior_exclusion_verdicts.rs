//! The verdict half of `tests/omega/packages/behavior-exclusions`: a build
//! that excludes a behavior must refuse a composition that still reaches it,
//! and must publish one that does not.
//!
//! These projects lost their harness when the per-feature `compiler --test`
//! targets were deleted, so nothing compared their verdicts. Compiling is not
//! a verdict: all six projects check clean, and the distinction only appears
//! at realization. The pair below is the smallest one that shows it --
//! `sink-app` and `no-op-app` differ in whether the selected provider reaches
//! `Sink::emit`, and in nothing else.

use crate::accepted_policy_fixture::accepted_policy;
use omega::package_manager::admission::{
    AcceptedNativeInput, AcceptedNativeRealizationRequest, accept_ordinary_closure_evidence,
    realize_accepted_native_report,
};
use omega::package_manager::resolution::graph::{
    PackageSourceClosureLimits, resolve_workspace_project_closure,
};
use omega::package_manager::review::{
    CanonicalPackageReconstructionQuestionLimits, ReviewOnlyCapabilityConflictLimits,
    SemanticBindingReview, compile_resolved_package_candidate_for_production,
};
use omega::package_source::{
    LocalSourceLimits, PrimaryGitChoices, SourceLineage, SourceRelativePath, SourceResolverStorage,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "omega-behavior-exclusion-verdicts-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        Self(root)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../..")
        .join("tests/omega/packages/behavior-exclusions")
        .canonicalize()
        .expect("committed behavior-exclusions fixture exists")
}

/// The apps depend on sibling kits by relative path, so the whole committed
/// tree is copied rather than one project.
fn copy_tree(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let target = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// Realizes one app of the copied tree for `linux_x86_64`, the target its own
/// `build.omg` binds. `Err` carries the realization diagnostics.
///
/// The exclusion verdict is decided during realization, so this stops there
/// rather than publishing: publication asks further questions of its own and
/// would let an unrelated failure masquerade as a verdict.
fn realize(workspace: &Path, app: &str) -> Result<(), String> {
    let storage = SourceResolverStorage::for_hardened_base(
        workspace.join("cache"),
        PrimaryGitChoices::default(),
    )
    .unwrap();
    let closure = resolve_workspace_project_closure(
        &SourceLineage::git("https://example.com/behavior-exclusions-fixture.git").unwrap(),
        SourceRelativePath::parse(app).unwrap(),
        workspace,
        &storage,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("fixture closure resolves");

    let target = closure.for_exact_target(target::TargetProfile::LinuxX64);
    let candidate = compile_resolved_package_candidate_for_production(
        &target,
        &workspace.join("review").join(app),
        SemanticBindingReview::Explicit(&[]),
        None,
    )
    .map_err(|error| format!("{error:?}"))?;

    let policy = accepted_policy(&target, candidate.reviews());
    let evidence = accept_ordinary_closure_evidence(
        &target,
        candidate.reviews(),
        CanonicalPackageReconstructionQuestionLimits::default(),
        ReviewOnlyCapabilityConflictLimits::default(),
        Some(&policy),
    )
    .unwrap();
    realize_accepted_native_report(
        AcceptedNativeInput::Reviewed {
            candidate: Box::new(candidate),
            optimization_rollback: &omega::compiler::OptimizationRollback::default(),
        },
        AcceptedNativeRealizationRequest {
            evidence: &evidence,
            profile: &proof_admission::AdmissionProfile::default(),
            terminal_authority_policy: omega::compiler::native::current_terminal_authority_policy(),
            receiving_terminal_authority_permission_policy: None,
            imports: &[],
        },
    )
    .map_err(|diagnostics| format!("{diagnostics:?}"))?;
    Ok(())
}

/// A selected provider that never reaches the excluded service realizes.
/// This is the positive control: without it, the refusal below could come
/// from ambient breakage in the fixture tree rather than from the exclusion.
#[test]
fn a_composition_that_avoids_the_excluded_service_realizes() {
    let fixture = Fixture::new();
    copy_tree(&fixture_root(), &fixture.0);
    realize(&fixture.0, "no-op-app").expect("the no-op composition satisfies its exclusion");
}

/// The same build declaration over a provider that does reach `Sink::emit`
/// refuses, and names the service, the machine and the reaching call rather
/// than reporting an unsatisfied count alone.
#[test]
fn a_composition_reaching_the_excluded_service_refuses() {
    let fixture = Fixture::new();
    copy_tree(&fixture_root(), &fixture.0);
    let error =
        realize(&fixture.0, "sink-app").expect_err("a reachable excluded service must not publish");
    for expected in [
        "behavior exclusion violated",
        "service `Sink` is reachable",
        "Main::main",
    ] {
        assert!(
            error.contains(expected),
            "refusal should name {expected}: {error}"
        );
    }
}
