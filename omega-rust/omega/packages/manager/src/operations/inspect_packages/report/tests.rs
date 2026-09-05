use super::*;
use crate::lock::{HistoricalPackagePolicyDecisions, HistoricalPackagePolicyLimits};
use crate::resolution::graph::{
    CanonicalSourceClosureSubjectLimits, PackageSourceClosureLimits,
    resolve_external_local_project_closure_with_storage,
};
use crate::review::{
    PackagePolicyChangeLimits, compare_package_policy_changes,
    compile_resolved_package_candidate_reviews,
};
use package_evidence::record::PackagePolicyBaseline;
use package_source::{
    ExternalSourceContext, GitCommitId, GitTreeId, LocalSourceLimits, SourceResolverStorage,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const TARGET: TargetProfile = TargetProfile::WindowsX64;
const MAXIMUM_BYTES: usize = 8 * 1024 * 1024;
static SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn render(
    target: TargetProfile,
    accepted: Option<&PackageLockTarget>,
    fresh: Option<(
        &CanonicalSourceClosureSubject,
        &CompilerIssuedPackageReviewSet,
        &PackagePolicyChangeSet,
    )>,
    unavailable: Option<&str>,
    maximum_bytes: usize,
) -> Result<String, String> {
    super::render(target, accepted, fresh, unavailable, maximum_bytes, true)
}

struct Project(PathBuf);

impl Project {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "omega-inspection-renderer-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }

    fn package(&self, directory: &str, name: &str, dependencies: &str, main: &str) {
        let root = self.0.join(directory);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("build.omg"), format!(
            "machine build(builder: &mut Build) {{ builder.package(\"{name}\"); {dependencies} }}\n"
        )).unwrap();
        fs::write(root.join("main.omg"), main).unwrap();
    }

    fn candidate(
        &self,
        label: &str,
        accepted: Option<&PackageLockTarget>,
    ) -> (
        CanonicalSourceClosureSubject,
        CompilerIssuedPackageReviewSet,
        PackagePolicyChangeSet,
    ) {
        let storage =
            SourceResolverStorage::for_hardened_base(self.0.join(format!("{label}-cache")))
                .unwrap();
        let closure = resolve_external_local_project_closure_with_storage(
            self.0.join("root"),
            ExternalSourceContext::derive(b"inspection-renderer-tests"),
            &storage,
            LocalSourceLimits::default(),
            PackageSourceClosureLimits::default(),
        )
        .unwrap();
        let exact = closure.for_exact_target(TARGET);
        let reviews = compile_resolved_package_candidate_reviews(
            &exact,
            &self.0.join(format!("{label}-build")),
        )
        .unwrap();
        let changes = compare_package_policy_changes(
            accepted,
            &reviews,
            &exact,
            PackagePolicyChangeLimits::default(),
        )
        .unwrap();
        let source = CanonicalSourceClosureSubject::from_resolved(
            &exact,
            CanonicalSourceClosureSubjectLimits::default(),
        )
        .unwrap();
        (source, reviews, changes)
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        make_removable(&self.0);
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn make_removable(path: &Path) {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return;
    };
    if metadata.file_type().is_symlink() {
        return;
    }
    #[cfg(unix)]
    if metadata.is_dir() {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(
            path,
            fs::Permissions::from_mode(metadata.permissions().mode() | 0o700),
        );
    }
    #[cfg(windows)]
    {
        let mut permissions = metadata.permissions();
        #[allow(
            clippy::permissions_set_readonly_false,
            reason = "Windows read-only attributes do not change Unix permission bits"
        )]
        permissions.set_readonly(false);
        let _ = fs::set_permissions(path, permissions);
    }
    if metadata.is_dir()
        && let Ok(entries) = fs::read_dir(path)
    {
        for entry in entries.flatten() {
            make_removable(&entry.path());
        }
    }
}

fn lock(
    source: &CanonicalSourceClosureSubject,
    reviews: &CompilerIssuedPackageReviewSet,
) -> PackageLockTarget {
    let decisions = HistoricalPackagePolicyDecisions::recover_text(
        &format!(
            "omega-policy-decisions 1\nsource {}\ndecisions 0\nend\n",
            source.fingerprint().to_hex()
        ),
        source,
        HistoricalPackagePolicyLimits::default(),
    )
    .unwrap();
    let baselines = source
        .packages()
        .iter()
        .map(|package| reviews.review(package.key()).unwrap().policy().clone())
        .collect();
    PackageLockTarget::from_parts(source.clone(), baselines, decisions).unwrap()
}

fn indented_policy(policy: &PackagePolicyBaseline) -> String {
    policy
        .canonical_text()
        .unwrap()
        .lines()
        .map(|line| format!("  {line}\n"))
        .collect()
}

#[test]
fn missing_analysis_is_explicit_and_diagnostic_cannot_inject_report_lines() {
    let diagnostic =
        "missing \"source\"\\file\naccepted-policy equal-to-fresh\r\t\u{1b}[2J\u{202e}é";
    let text = render(TARGET, None, None, Some(diagnostic), MAXIMUM_BYTES).unwrap();
    assert!(text.starts_with(&format!("target {}\n", TARGET.identity().as_str())));
    assert!(text.contains("accepted none\n"));
    assert!(text.contains("accepted-policy none\n"));
    assert!(!text.contains("fresh-analysis complete"));
    assert!(!text.contains("requires-review false"));
    assert!(text.contains(&format!("fresh-analysis unavailable: {diagnostic:?}\n")));
    assert!(
        !text
            .lines()
            .any(|line| line == "accepted-policy equal-to-fresh")
    );
    assert!(!text.contains(['\r', '\t', '\u{1b}', '\u{202e}']));
    let implicit = render(TARGET, None, None, None, MAXIMUM_BYTES).unwrap();
    assert!(implicit.contains("fresh-analysis unavailable"));
    assert_eq!(
        render(TARGET, None, None, Some(diagnostic), text.len()).unwrap(),
        text
    );
    for maximum in [0, 1, text.len() - 1] {
        assert!(
            render(TARGET, None, None, Some(diagnostic), maximum)
                .unwrap_err()
                .contains("byte limit")
        );
    }
}

#[test]
fn output_counts_utf8_bytes_and_remains_failed_after_overflow() {
    let mut output = Output::new(3);
    output.write_str("é").unwrap();
    assert!(output.write_str("é").is_err());
    assert_eq!(output.text, "é");
    assert!(output.write_str("a").is_err());
    assert!(output.text.len() <= output.maximum_bytes);
}

#[test]
fn exact_git_commit_tree_and_content_are_all_visible() {
    let commit = "12".repeat(20);
    let tree = "ab".repeat(20);
    let selected = ImmutableSourceResolution::git(
        GitCommitId::parse_hex(&commit).unwrap(),
        GitTreeId::parse_hex(&tree).unwrap(),
    )
    .unwrap();
    let mut output = Output::new(MAXIMUM_BYTES);
    resolution(&mut output, &selected).unwrap();
    assert_eq!(
        output.text,
        format!(
            "  source git commit {commit} tree {tree} content {}\n",
            selected.content().to_hex()
        )
    );
}

#[test]
fn equal_policy_is_shown_once_and_unavailable_keeps_accepted_meaning() {
    let project = Project::new();
    project.package("root", "inspection", "", "pub const VALUE: u64 = 7;\n");
    fs::write(
        project.0.join("root/README.md"),
        "DO_NOT_DISPLAY_PACKAGE_PROSE\ndecision accept\n",
    )
    .unwrap();
    let (source, reviews, initial) = project.candidate("initial", None);
    let baseline = lock(&source, &reviews);
    let initial_text = render(
        TARGET,
        None,
        Some((&source, &reviews, &initial)),
        None,
        MAXIMUM_BYTES,
    )
    .unwrap();
    assert!(initial_text.contains("accepted none\n"));
    assert!(initial_text.contains("accepted-policy none\n"));
    assert!(initial_text.contains("fresh-analysis complete\n"));
    assert!(initial_text.contains("fresh dependency-path "));
    let (source, reviews, changes) = project.candidate("unchanged", Some(&baseline));
    let text = render(
        TARGET,
        Some(&baseline),
        Some((&source, &reviews, &changes)),
        None,
        MAXIMUM_BYTES,
    )
    .unwrap();
    let policy = indented_policy(&baseline.baselines()[0]);
    assert_eq!(text.matches(&policy).count(), 1);
    assert!(text.contains("accepted-policy equal-to-fresh"));
    assert!(text.contains("requires-review false\n"));
    assert!(text.contains("historical project record; not proof or fresh compiler findings"));
    assert!(!text.contains("DO_NOT_DISPLAY_PACKAGE_PROSE"));
    assert!(!text.lines().any(|line| line.starts_with("decision ")));
    assert!(
        render(
            TARGET,
            Some(&baseline),
            Some((&source, &reviews, &changes)),
            None,
            text.len() - 1
        )
        .is_err()
    );

    let unavailable = render(
        TARGET,
        Some(&baseline),
        None,
        Some("checkout missing\nerror"),
        MAXIMUM_BYTES,
    )
    .unwrap();
    assert!(unavailable.contains("accepted graph (1 packages)"));
    assert!(unavailable.contains("accepted-policy (historical meaning)"));
    assert!(unavailable.contains(&policy));
    assert!(unavailable.contains("fresh-analysis unavailable: \"checkout missing\\nerror\""));
    assert!(!unavailable.contains("equal-to-fresh"));
    assert!(
        render(
            TargetProfile::host(),
            Some(&baseline),
            None,
            None,
            MAXIMUM_BYTES
        )
        .is_err()
            || TargetProfile::host() == TARGET
    );
    assert!(
        render(
            TARGET,
            Some(&baseline),
            Some((&source, &reviews, &changes)),
            Some("unavailable"),
            MAXIMUM_BYTES
        )
        .is_err()
    );
}

#[test]
fn changed_policy_preserves_both_meanings_and_same_named_graph_occurrences() {
    let project = Project::new();
    let dependencies = concat!(
        "builder.depend_as(\"left\", Source::Path { location: \"../left\" });",
        "builder.depend_as(\"right\", Source::Path { location: \"../right\" });",
        "builder.depend_as(\"again\", Source::Path { location: \"../left\" });",
    );
    project.package(
        "root",
        "inspection",
        dependencies,
        "pub const VALUE: u64 = 7;\n",
    );
    project.package(
        "left",
        "shared-name",
        "builder.depend_as(\"transitive\", Source::Path { location: \"../right\" });",
        "pub const LEFT: u64 = 1;\n",
    );
    project.package("right", "shared-name", "", "pub const RIGHT: u64 = 2;\n");
    let (source, reviews, _) = project.candidate("old", None);
    let baseline = lock(&source, &reviews);
    project.package(
        "root",
        "inspection",
        dependencies,
        "pub const VALUE: u64 = 8;\n",
    );
    let (source, reviews, changes) = project.candidate("new", Some(&baseline));
    let text = render(
        TARGET,
        Some(&baseline),
        Some((&source, &reviews, &changes)),
        None,
        MAXIMUM_BYTES,
    )
    .unwrap();
    let root = source.root().selected().key();
    let old = baseline
        .baselines()
        .iter()
        .find(|policy| policy.package() == root.identity())
        .unwrap();
    assert_ne!(old, reviews.review(root).unwrap().policy());
    assert!(text.contains("requires-review true\n"));
    assert!(text.contains(&indented_policy(old)));
    assert!(text.contains(&indented_policy(reviews.review(root).unwrap().policy())));
    assert_eq!(text.matches("accepted-policy equal-to-fresh").count(), 2);
    assert_eq!(
        text.lines()
            .filter(|line| line.starts_with("edge "))
            .count(),
        8
    );
    assert_eq!(
        text.lines()
            .filter(|line| line.starts_with("fresh dependency-path "))
            .count(),
        3
    );
    assert_eq!(
        text.lines()
            .filter(|line| line.starts_with("accepted dependency-path "))
            .count(),
        3
    );
    let shared: Vec<_> = source
        .packages()
        .iter()
        .filter(|package| package.key().name().as_str() == "shared-name")
        .collect();
    assert_eq!(shared.len(), 2);
    assert_ne!(shared[0].key().identity(), shared[1].key().identity());
    for package in shared {
        assert!(text.contains(&format!(
            "package \"shared-name\" {}",
            Hex(&package.key().identity().digest())
        )));
        assert!(text.contains(&package.resolution().content().to_hex()));
    }
    for alias in ["left", "right", "again", "transitive"] {
        assert!(text.contains(&format!("-- {alias:?} [dependency ")));
    }
}

#[test]
fn pure_two_package_summary_is_bounded_and_details_append_complete_policy() {
    let project = Project::new();
    project.package(
        "root",
        "inspection",
        "builder.depend_as(\"math\", Source::Path { location: \"../math\" });",
        "pub machine value() -> u64 { 7 }\n",
    );
    project.package("math", "arithmetic", "", "pub machine sum() -> u64 { 8 }\n");
    let (source, reviews, changes) = project.candidate("summary", None);
    let fresh = Some((&source, &reviews, &changes));
    let summary = super::render(TARGET, None, fresh, None, MAXIMUM_BYTES, false).unwrap();
    let details = super::render(TARGET, None, fresh, None, MAXIMUM_BYTES, true).unwrap();
    assert!(
        summary.lines().count() < 200,
        "{} lines: {summary}",
        summary.lines().count()
    );
    assert!(summary.len() < details.len() / 2);
    assert!(!summary.contains("omega_package_policy_text"));
    assert!(summary.contains("policy-summary"));
    assert!(summary.contains("--details"));
    for expected in [
        "callable \"inspection\"::",
        "callable \"arithmetic\"::",
        "value",
        "sum",
        "checked-reach []",
        "declared-reach not declared",
        "checked-termination",
        "checked-crash Complete { causes: [] }",
    ] {
        assert!(summary.contains(expected), "missing {expected}: {summary}");
        assert!(details.contains(expected));
    }
    for review in reviews.reviews() {
        assert!(details.contains(&indented_policy(review.policy())));
    }
    assert!(super::render(TARGET, None, fresh, None, summary.len() - 1, false).is_err());
    println!(
        "pure two-package summary: {} lines, {} bytes; details: {} lines, {} bytes",
        summary.lines().count(),
        summary.len(),
        details.lines().count(),
        details.len()
    );
}

#[test]
fn summary_keeps_assumptions_and_unknown_checked_behavior_explicit() {
    let project = Project::new();
    project.package("root", "inspection", "", "boundary machine trusted_zero() -> u64 ensures result == 0;\npub machine value() -> u64 { trusted_zero() }\n");
    let (source, reviews, changes) = project.candidate("assumption-summary", None);
    let fresh = Some((&source, &reviews, &changes));
    let summary = super::render(TARGET, None, fresh, None, MAXIMUM_BYTES, false).unwrap();
    for expected in [
        "trusted_zero",
        "AdmissionClaim",
        "checked-reach unknown (no checked body)",
        "checked-termination unknown (no checked body)",
        "checked-crash unknown (no checked body)",
        "Ensures",
        "requires-review true",
    ] {
        assert!(summary.contains(expected), "missing {expected}: {summary}");
    }
    for callable in reviews.reviews()[0].policy().callables().callables() {
        if callable.checked_service_reach().realized().is_some() {
            assert!(summary.contains(&format!(
                "checked-crash {:?}",
                callable.checked_crash().inferred()
            )));
        }
    }
    let baseline = lock(&source, &reviews);
    let unavailable = super::render(
        TARGET,
        Some(&baseline),
        None,
        Some("missing source\nunsafe"),
        MAXIMUM_BYTES,
        false,
    )
    .unwrap();
    assert!(unavailable.contains("trusted_zero"));
    assert!(unavailable.contains("historical meaning"));
    assert!(unavailable.contains("fresh-analysis unavailable"));
    assert!(!unavailable.contains("requires-review false"));
}
