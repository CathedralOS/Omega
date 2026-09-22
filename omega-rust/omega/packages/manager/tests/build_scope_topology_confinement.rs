//! Input-confinement control for the `tests/fixtures/packages/
//! build-scope-topology` project: the topology helper reads its admitted
//! inputs through the caller's `builder.source` facet, so it must never
//! reach dependency-adjacent artifacts — its own package files — nor
//! absolute/host spellings outside the root package's source tree.
//!
//! Each case splices one escape attempt into the workspace copy of
//! `topology/compose.omg` (the committed fixture stays green — its
//! production path is covered by the topology crate's composition_build
//! acceptance). A build that still composed would prove the helper bypassed
//! input confinement; the expected outcome is evaluation refusal with the
//! canonical-spelling diagnostic before any byte is read.

#[path = "support/accepted_policy.rs"]
mod accepted_policy_fixture;

use accepted_policy_fixture::accepted_policy;
use package_manager::admission::{
    AcceptedNativeInput, AcceptedNativeRealizationRequest, accept_ordinary_closure_evidence,
    realize_accepted_native_report,
};
use package_manager::resolution::graph::{
    PackageSourceClosureLimits, resolve_workspace_project_closure,
};
use package_manager::review::{
    CanonicalPackageReconstructionQuestionLimits, ReviewOnlyCapabilityConflictLimits,
    SemanticBindingReview, compile_resolved_package_candidate_for_production,
};
use package_source::{
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
            "omega-build-scope-confinement-{}-{}",
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
        .join("tests/fixtures/packages/build-scope-topology")
        .canonicalize()
        .expect("committed build-scope-topology fixture exists")
}

/// The admitted request read the escape attempts replace.
const REQUEST_READ: &str = r#"let req_path: BuildPath = source.resolve("inputs/request.bin");"#;

/// Copies the committed fixture, optionally splicing one replacement line
/// for `inputs/request.bin`'s resolve into the topology helper's compose
/// source — the point where a bypass would actually read outside the
/// admitted tree.
fn copy_fixture(workspace: &Path, compose_replacement: Option<&str>) {
    let fixture = fixture_root();
    for entry in [
        "root/build.omg",
        "root/main.omg",
        "topology/build.omg",
        "topology/policies.omg",
        "topology/compose.omg",
        "topology/digest.omg",
    ] {
        let data = fs::read(fixture.join(entry)).unwrap();
        let target = workspace.join(entry);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(target, data).unwrap();
    }
    if let Some(replacement) = compose_replacement {
        let compose = workspace.join("topology/compose.omg");
        let data = fs::read_to_string(&compose).unwrap();
        assert!(
            data.contains(REQUEST_READ),
            "fixture's admitted request read must be present to splice"
        );
        fs::write(&compose, data.replace(REQUEST_READ, replacement)).unwrap();
    }
    for name in ["request.bin", "components.bin", "bindings.bin"] {
        let data = fs::read(fixture.join("root/inputs").join(name)).unwrap();
        let target = workspace.join("root/inputs").join(name);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(target, data).unwrap();
    }
}

/// Compiles the fixture workspace's root project for production; `Err`
/// carries the build-time evaluation diagnostic.
fn compose(workspace: &Path) -> Result<PathBuf, String> {
    let storage = SourceResolverStorage::for_hardened_base(
        workspace.join("cache"),
        PrimaryGitChoices::default(),
    )
    .unwrap();
    let closure = resolve_workspace_project_closure(
        &SourceLineage::git("https://example.com/topology-build-fixture.git").unwrap(),
        SourceRelativePath::parse("root").unwrap(),
        workspace,
        &storage,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("fixture closure resolves");

    let target = closure.for_exact_target(target::TargetProfile::LinuxX64);
    let candidate = compile_resolved_package_candidate_for_production(
        &target,
        &workspace.join("review"),
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
    let report = realize_accepted_native_report(
        AcceptedNativeInput::Reviewed {
            candidate: Box::new(candidate),
            optimization_rollback: &compiler::OptimizationRollback::default(),
        },
        AcceptedNativeRealizationRequest {
            evidence: &evidence,
            profile: &proof_admission::AdmissionProfile::default(),
            terminal_authority_policy: native_realization::current_terminal_authority_policy(),
            receiving_terminal_authority_permission_policy: None,
            imports: &[],
        },
    )
    .map_err(|diagnostics| format!("{diagnostics:?}"))?;
    let publication = workspace.join("published");
    let report = report.publish_completed_build_outputs(&publication)?;
    let outputs = report
        .build_outputs()
        .ok_or_else(|| "no retained build outputs".to_owned())?;
    let directory = outputs
        .published_directory()
        .ok_or_else(|| "no published directory".to_owned())?;
    Ok(directory.to_path_buf())
}

/// Positive control: the untouched fixture still composes and publishes the
/// plan, so the escape refusals below are attributable to the probe and not
/// ambient breakage.
#[test]
fn admitted_inputs_compose_and_publish() {
    let fixture = Fixture::new();
    copy_fixture(&fixture.0, None);
    let directory = compose(&fixture.0).expect("untouched fixture composes");
    assert!(
        directory.join("files/payments.plan").is_file(),
        "payments.plan was published"
    );
}

/// A `..` spelling reaching for the helper's own dependency-adjacent build
/// file is refused at evaluation with the canonical-components diagnostic —
/// the escape is never opened, so no artifact can publish.
#[test]
fn helper_cannot_resolve_a_dependency_adjacent_artifact() {
    let fixture = Fixture::new();
    copy_fixture(
        &fixture.0,
        Some(r#"let req_path: BuildPath = source.resolve("../topology/build.omg");"#),
    );
    let error = compose(&fixture.0).expect_err("escape read must refuse");
    assert!(error.contains("canonical relative components"), "{error}");
}

/// An absolute spelling is refused with the host-spelling diagnostic; the
/// admitted input inventory is never reached.
#[test]
fn helper_cannot_resolve_an_absolute_path() {
    let fixture = Fixture::new();
    copy_fixture(
        &fixture.0,
        Some(r#"let req_path: BuildPath = source.resolve("/etc/passwd");"#),
    );
    let error = compose(&fixture.0).expect_err("absolute path must refuse");
    assert!(
        error.contains("absolute or host-specific spelling")
            || error.contains("canonical relative components"),
        "{error}"
    );
}
