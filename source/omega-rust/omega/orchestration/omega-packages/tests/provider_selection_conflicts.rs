use omega_compiler::{
    PackageReviewCanonicalRowKind, PackageReviewCanonicalRowRisk, PackageReviewSourceLocationRole,
};
use omega_packages::{
    ExternalSourceContext, LocalSourceLimits, PackageSourceClosureLimits, PackageTriageDisposition,
    PackageTriageReason, ReviewOnlyCapabilityConflictChange, ReviewOnlyCapabilityConflictLimits,
    compare_review_only_capabilities, compile_resolved_package_reviews,
    resolve_external_local_package_closure, triage_review_update,
};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

struct TempTree(PathBuf);

impl TempTree {
    fn new() -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "omega-provider-selection-conflict-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create provider-selection test tree");
        Self(path)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn write_provider_package(root: &Path, provider: &str) {
    fs::create_dir_all(root).expect("create provider package");
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../../tests/fixtures/packages/provider-switchboard");
    let canonical_build = fs::read_to_string(fixture.join("build.omg"))
        .expect("read provider-switchboard build fixture");
    let selected_build = canonical_build.replace(
        "builder.select_provider<ClockHost, MonotonicClock>();",
        &format!("builder.select_provider<ClockHost, {provider}>();"),
    );
    fs::write(root.join("build.omg"), selected_build).expect("write selected provider");
    let mut main = fs::read_to_string(fixture.join("main.omg"))
        .expect("read provider-switchboard source fixture");
    main.push_str(
        r#"
data WallClock { }

machine WallClock::ticks() -> u64
satisfies ClockHost::ticks
{
    transition { _ -> (2) }
}
"#,
    );
    fs::write(root.join("main.omg"), main).expect("write provider realizations");
}

#[test]
fn provider_selection_update_becomes_an_exact_forced_review_conflict() {
    let tree = TempTree::new();
    let live = tree.path("live");
    let context = ExternalSourceContext::derive(b"provider-selection-conflict");
    write_provider_package(&live, "MonotonicClock");

    let baseline_sources = resolve_external_local_package_closure(
        &live,
        context.clone(),
        tree.path("baseline-cache"),
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve baseline provider custody");
    let baseline_reviews = compile_resolved_package_reviews(
        &baseline_sources,
        "windows_x64",
        &tree.path("compiler-workspace"),
    )
    .expect("compile baseline provider evidence");

    write_provider_package(&live, "WallClock");
    let candidate_sources = resolve_external_local_package_closure(
        &live,
        context,
        tree.path("candidate-cache"),
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve candidate provider custody");
    let candidate_reviews = compile_resolved_package_reviews(
        &candidate_sources,
        "windows_x64",
        &tree.path("compiler-workspace"),
    )
    .expect("compile candidate provider evidence");

    assert_eq!(
        baseline_sources.graph().root(),
        candidate_sources.graph().root()
    );
    let baseline = baseline_reviews
        .review(baseline_sources.graph().root())
        .expect("baseline root review");
    let candidate = candidate_reviews
        .review(candidate_sources.graph().root())
        .expect("candidate root review");
    let [baseline_provider] = baseline.projection().selected_providers() else {
        panic!("baseline must select exactly one provider")
    };
    let [candidate_provider] = candidate.projection().selected_providers() else {
        panic!("candidate must select exactly one provider")
    };
    assert_eq!(baseline_provider.provider_type(), "MonotonicClock");
    assert_eq!(candidate_provider.provider_type(), "WallClock");
    assert_eq!(baseline_provider.service_schema(), "ClockHost");
    assert_eq!(candidate_provider.service_schema(), "ClockHost");
    assert_eq!(
        baseline_provider.realizing_package(),
        Some(baseline.key().identity())
    );
    assert_eq!(
        candidate_provider.realizing_package(),
        Some(candidate.key().identity())
    );

    let conflicts = compare_review_only_capabilities(
        &baseline_reviews,
        &candidate_reviews,
        &candidate_sources,
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .expect("compare compiler-owned provider rows");
    let [package] = conflicts.packages() else {
        panic!("provider change must affect exactly one package")
    };
    let provider_conflicts = package
        .conflicts()
        .iter()
        .filter(|conflict| conflict.kind() == PackageReviewCanonicalRowKind::SelectedProviderSet)
        .collect::<Vec<_>>();
    let [provider_conflict] = provider_conflicts.as_slice() else {
        panic!("provider change must produce one selected-provider-set conflict")
    };
    assert_eq!(
        provider_conflict.change(),
        ReviewOnlyCapabilityConflictChange::Changed
    );
    assert_eq!(
        provider_conflict.risk(),
        PackageReviewCanonicalRowRisk::OpaqueBlocking
    );
    assert!(provider_conflict.is_blocking());
    assert!(provider_conflict.baseline_row().is_some());
    assert!(provider_conflict.candidate_row().is_some());
    assert_ne!(
        provider_conflict.baseline_row(),
        provider_conflict.candidate_row()
    );
    for source in [
        provider_conflict
            .baseline_source()
            .expect("baseline compiler source"),
        provider_conflict
            .candidate_source()
            .expect("candidate compiler source"),
    ] {
        let locations = source
            .authored_locations()
            .expect("provider row retains authored locations");
        assert!(locations.iter().any(|location| {
            location.role() == PackageReviewSourceLocationRole::ProviderSelection
                && location.relative_path() == "build.omg"
        }));
        assert!(locations.iter().any(|location| {
            location.role() == PackageReviewSourceLocationRole::ProviderRequirementDeclaration
                && location.relative_path() == "main.omg"
        }));
        assert!(locations.iter().any(|location| {
            location.role() == PackageReviewSourceLocationRole::ProviderRealization
                && location.relative_path() == "main.omg"
        }));
    }

    let triage = triage_review_update(&baseline_reviews, &candidate_reviews, &BTreeSet::new());
    let [decision] = triage.decisions() else {
        panic!("provider update must have one triage decision")
    };
    assert_eq!(
        decision.disposition(),
        PackageTriageDisposition::BlockedCapabilityChange
    );
    assert!(
        decision
            .reasons()
            .contains(&PackageTriageReason::CapabilityOrApiChanged)
    );
}
