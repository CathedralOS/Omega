//! A machine's fixed operator token is part of its public signature: two
//! revisions of a package that differ only in that binding must review as a
//! decision-requiring callable change against the retained lock, while a
//! revision that re-reviews the same tokenless declaration must report no
//! policy rows at all (no churn on existing locks).

use package_evidence::record::PackagePolicyRowKind;
use package_manager::lock::{
    HistoricalPackagePolicyDecisions, HistoricalPackagePolicyLimits, PackageLockTarget,
};
use package_manager::resolution::graph::{
    CanonicalSourceClosureSubject, CanonicalSourceClosureSubjectLimits, GitResolutionOptions,
    PackageSourceClosureLimits, ResolvedPackageSourceClosure,
    resolve_external_local_project_closure,
};
use package_manager::review::{
    CompilerIssuedPackageReviewSet, PackagePolicyChangeKind, PackagePolicyChangeLimits,
    SemanticBindingReview, compare_package_policy_changes, compile_resolved_package_reviews,
};
use package_source::PrimaryGitChoices;
use package_source::{ExternalSourceContext, LocalSourceLimits, SourceResolverStorage};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use target::TargetProfile;

const TARGET: TargetProfile = TargetProfile::WindowsX64;
static SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct Tree {
    root: PathBuf,
}

impl Tree {
    fn new() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "omega-token-binding-revision-{}-{nanos}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).expect("create temporary fixture root");
        Self { root }
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    fn storage(&self, name: &str) -> SourceResolverStorage {
        SourceResolverStorage::for_hardened_base(self.path(name), PrimaryGitChoices::default())
            .expect("create source resolver storage")
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

const TOKENLESS_SOURCE: &str = r#"pub data Wrapped { value: u8; }

pub machine Wrapped::add(left: Wrapped, right: Wrapped) -> u64 {
    (left.value as u64) + (right.value as u64)
}

pub boundary machine Wrapped::trusted_product(left: Wrapped, right: Wrapped) -> u64
ensures result == 0;
"#;

const BUILD: &str = r#"machine build(builder: &mut Build) {
    builder.package("token-fixture");
}
"#;

fn bound_source() -> String {
    TOKENLESS_SOURCE
        .replace("pub machine Wrapped::add", "pub machine + Wrapped::add")
        .replace(
            "pub boundary machine Wrapped::trusted_product",
            "pub boundary machine * Wrapped::trusted_product",
        )
}

fn write_revision(tree: &Tree, source: &str) {
    let package = tree.path("sources/root");
    fs::create_dir_all(&package).expect("create package directory");
    fs::write(package.join("main.omg"), source).expect("write package source");
    fs::write(package.join("build.omg"), BUILD).expect("write package build");
}

fn review(
    tree: &Tree,
    label: &str,
) -> (ResolvedPackageSourceClosure, CompilerIssuedPackageReviewSet) {
    let storage = tree.storage(&format!("{label}-cache"));
    let closure = resolve_external_local_project_closure(
        tree.path("sources/root"),
        ExternalSourceContext::derive(b"token-binding-revision"),
        &storage,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
        GitResolutionOptions::default(),
    )
    .expect("resolve the package closure");
    let reviews = compile_resolved_package_reviews(
        &closure.for_exact_target(TARGET),
        &tree.path(&format!("{label}-build")),
        SemanticBindingReview::Discover,
    )
    .expect("the revision should compile for review");
    (closure, reviews)
}

fn accepted_lock(
    closure: &ResolvedPackageSourceClosure,
    reviews: &CompilerIssuedPackageReviewSet,
) -> PackageLockTarget {
    let source = CanonicalSourceClosureSubject::from_resolved(
        &closure.for_exact_target(TARGET),
        CanonicalSourceClosureSubjectLimits::default(),
    )
    .expect("canonical source subject");
    let decisions = HistoricalPackagePolicyDecisions::recover_text(
        &format!(
            "omega-policy-decisions 1\nsource {}\ndecisions 0\nend\n",
            source.fingerprint().to_hex()
        ),
        &source,
        HistoricalPackagePolicyLimits::default(),
    )
    .expect("empty decision history");
    let mut policies = Vec::new();
    for package in source.packages() {
        for purpose in package_manager::declarations::DependencyPurpose::ALL {
            if let Some(review) = reviews.review_occurrence(package.key(), purpose) {
                policies.push((review.checked_context(), review.policy()));
            }
        }
    }
    PackageLockTarget::from_policies(source, &policies, decisions).expect("accepted lock target")
}

fn callable_texts(reviews: &CompilerIssuedPackageReviewSet, name: &str) -> Vec<String> {
    let [review] = reviews.reviews() else {
        panic!("one reviewed package");
    };
    review
        .policy()
        .callables()
        .callables()
        .iter()
        .filter(|callable| callable.identity().path().contains(name))
        .map(|callable| callable.identity().path().to_owned())
        .collect()
}

#[test]
fn a_changed_token_binding_is_a_breaking_revision_of_the_reviewed_callables() {
    let tree = Tree::new();
    write_revision(&tree, TOKENLESS_SOURCE);
    let (baseline_closure, baseline_reviews) = review(&tree, "baseline");
    let accepted = accepted_lock(&baseline_closure, &baseline_reviews);

    // The same tokenless declarations reviewed again: identical review bytes
    // and no policy rows against the retained lock.
    write_revision(&tree, TOKENLESS_SOURCE);
    let (same_closure, same_reviews) = review(&tree, "same");
    assert_eq!(
        baseline_reviews.reviews()[0]
            .policy()
            .canonical_text()
            .unwrap(),
        same_reviews.reviews()[0].policy().canonical_text().unwrap(),
        "a tokenless declaration must review byte-identically"
    );
    let unchanged = compare_package_policy_changes(
        Some(&accepted),
        &same_reviews,
        &same_closure.for_exact_target(TARGET),
        PackagePolicyChangeLimits::default(),
    )
    .expect("compare the unchanged revision");
    assert!(
        unchanged
            .packages()
            .iter()
            .all(|package| package.rows().is_empty() && !package.source_changed()),
        "no churn against the retained lock: {unchanged:#?}"
    );

    // Only the token bindings differ.
    write_revision(&tree, &bound_source());
    let (bound_closure, bound_reviews) = review(&tree, "bound");
    assert_ne!(
        baseline_reviews.reviews()[0]
            .policy()
            .canonical_text()
            .unwrap(),
        bound_reviews.reviews()[0]
            .policy()
            .canonical_text()
            .unwrap(),
    );
    // The ordinary public machine is fresh-audit material: its reviewed
    // callable identity carries the token and no longer matches the baseline.
    let baseline_adds = callable_texts(&baseline_reviews, "Wrapped::add");
    let bound_adds = callable_texts(&bound_reviews, "Wrapped::add");
    let ([baseline_add], [bound_add]) = (baseline_adds.as_slice(), bound_adds.as_slice()) else {
        panic!("one `Wrapped::add` callable per revision");
    };
    assert_ne!(baseline_add, bound_add);
    assert!(bound_add.contains("token-bound") && bound_add.contains("1:+"));
    assert!(!baseline_add.contains("token-bound"));

    // The admission claim is an explicit acceptance row: its token change
    // retires the accepted row and adds one that needs a decision.
    let changes = compare_package_policy_changes(
        Some(&accepted),
        &bound_reviews,
        &bound_closure.for_exact_target(TARGET),
        PackagePolicyChangeLimits::default(),
    )
    .expect("compare the token-bound revision");
    let [package] = changes.packages() else {
        panic!("one reviewed package: {changes:#?}");
    };
    assert!(package.source_changed());
    let callable_rows = package
        .rows()
        .iter()
        .filter(|row| row.kind() == PackagePolicyRowKind::Callable)
        .collect::<Vec<_>>();
    let kinds = callable_rows
        .iter()
        .map(|row| row.change())
        .collect::<Vec<_>>();
    assert!(
        kinds.contains(&PackagePolicyChangeKind::Removed)
            && kinds.contains(&PackagePolicyChangeKind::Added),
        "the tokenless claim row is retired and the token-bound one added: {kinds:?}"
    );
    assert!(
        callable_rows.iter().all(|row| row.requires_decision()),
        "a token binding change is a breaking revision that needs a decision"
    );
    assert!(package.requires_decision());
    assert!(
        callable_rows
            .iter()
            .filter_map(|row| row.candidate())
            .any(|row| row.canonical_text().contains("token-bound")
                && row.canonical_text().contains("1:*")),
        "the added row names the token binding"
    );
    assert!(
        callable_rows
            .iter()
            .filter_map(|row| row.baseline())
            .all(|row| !row.canonical_text().contains("token-bound")),
        "the retired tokenless row carried no token coordinate"
    );
}
