//! Build-purpose and product-purpose dependency edges stay independently
//! authorized through real acquisition, review, locks, and locked-source
//! recovery. Neither scope falls back to the other, imports never widen an
//! edge's authority, and a missing or wrong-purpose occurrence rejects
//! recovery instead of silently recapturing it.

use package_manager::declarations::{DependencyPurpose, PackageKey};
use package_manager::lock::{
    HistoricalPackagePolicyDecisions, HistoricalPackagePolicyLimits, PackageLock, PackageLockError,
    PackageLockRecoveryLimits, PackageLockTarget,
};
use package_manager::operations::{
    LockedSourceRecoveryOptions, RecoverLockedSourcesError, check_locked_sources,
    recover_locked_sources,
};
use package_manager::resolution::graph::GitResolutionOptions;
use package_manager::resolution::graph::{
    CanonicalSourceClosureSubject, CanonicalSourceClosureSubjectLimits, PackageRootSourceRequest,
    PackageSourceClosureLimits, ResolveLockedPackageClosureError, ResolvedPackageSourceClosure,
    resolve_external_local_project_closure, resolve_locked_local_project_closure,
};
use package_manager::resolution::package_compilation_inputs;
use package_manager::review::compile_resolved_package_reviews;
use package_source::git::resolution::GitExactRevisionAcquisition;
use package_source::{ExternalSourceContext, LocalSourceLimits, SourceResolverStorage};
use std::fs;
use std::path::{Path, PathBuf};
use target::TargetProfile;

#[path = "locked_source_recovery/support.rs"]
mod support;
use support::*;

const TARGET: TargetProfile = TargetProfile::WindowsX64;
const CONTEXT: &[u8] = b"dependency-purposes";

fn resolve(tree: &Tree, storage: &SourceResolverStorage) -> ResolvedPackageSourceClosure {
    resolve_external_local_project_closure(
        tree.path("sources/root"),
        ExternalSourceContext::derive(CONTEXT),
        storage,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
        GitResolutionOptions::default(),
    )
    .unwrap()
}

/// (purpose, alias, selected package name) for one requester's resolved edges.
fn edges(
    closure: &ResolvedPackageSourceClosure,
    requester: &str,
) -> Vec<(DependencyPurpose, String, String)> {
    closure
        .graph()
        .packages()
        .iter()
        .find(|package| package.source().key().name().as_str() == requester)
        .unwrap_or_else(|| panic!("{requester} is not in the resolved closure"))
        .dependencies()
        .iter()
        .map(|dependency| {
            (
                dependency.purpose(),
                dependency.alias().as_str().to_owned(),
                dependency.target().name().as_str().to_owned(),
            )
        })
        .collect()
}

fn package_key(closure: &ResolvedPackageSourceClosure, name: &str) -> PackageKey {
    closure
        .graph()
        .packages()
        .iter()
        .find(|package| package.source().key().name().as_str() == name)
        .unwrap_or_else(|| panic!("{name} is not in the resolved closure"))
        .source()
        .key()
        .clone()
}

/// (purpose, requester ordinal, alias, selected name) rows from the recorded
/// lock subject, sorted by purpose name for a stable comparison.
fn recorded_edges(subject: &CanonicalSourceClosureSubject) -> Vec<(String, usize, String, String)> {
    subject
        .dependency_requests()
        .iter()
        .map(|edge| {
            (
                edge.purpose().name().to_owned(),
                edge.dependency_index(),
                edge.alias().as_str().to_owned(),
                edge.selected().key().name().as_str().to_owned(),
            )
        })
        .collect()
}

/// One root package authorizes `../product` for product imports and `../host`
/// for its build context. The host tool chains to its own ordinary product
/// dependency. Both host packages are acquired and locked but can never be
/// imported by the product.
fn dual_purpose(tree: &Tree) {
    package(
        &tree.path("sources/root"),
        "purpose-root",
        concat!(
            " builder.depend_as(\"product_dep\", Source::Path { location: \"../product\" });\n",
            " builder.build_depend_as(\"host_tool\", Source::Path { location: \"../host\" });\n",
        ),
    );
    package(&tree.path("sources/product"), "product-dep", "");
    package(
        &tree.path("sources/host"),
        "host-tool",
        " builder.depend_as(\"host_lib\", Source::Path { location: \"../host-lib\" });\n",
    );
    package(&tree.path("sources/host-lib"), "host-lib", "");
}

#[test]
fn purpose_tagged_edges_survive_acquisition_review_lock_and_recovery() {
    let tree = Tree::new();
    dual_purpose(&tree);
    let storage = tree.storage("old-cache");
    let closure = resolve(&tree, &storage);

    // The resolved graph carries purpose on every edge. Scopes are distinct:
    // the build edge's ordinal is zero-based inside its own scope.
    assert_eq!(
        edges(&closure, "purpose-root"),
        [
            (DependencyPurpose::Product, "product_dep", "product-dep"),
            (DependencyPurpose::Build, "host_tool", "host-tool"),
        ]
        .map(|(purpose, alias, target)| (purpose, alias.to_owned(), target.to_owned()))
    );
    assert_eq!(
        edges(&closure, "host-tool"),
        [(DependencyPurpose::Product, "host_lib", "host-lib")].map(|(purpose, alias, target)| (
            purpose,
            alias.to_owned(),
            target.to_owned()
        ))
    );
    let root = closure
        .graph()
        .package(closure.graph().root())
        .expect("root node");
    let build_edge = root
        .dependencies()
        .iter()
        .find(|dependency| !dependency.purpose().is_product())
        .expect("root build edge");
    assert_eq!(build_edge.dependency_index(), 0);

    // Acquisition covers all four packages and the root's build entry may
    // import the host snapshots, so all four are compilation inputs. None of
    // that widens product authority: the host packages hold no product-scope
    // binding and never join the durable product closure.
    let root_key = closure.graph().root().clone();
    let product_key = package_key(&closure, "product-dep");
    let host_key = package_key(&closure, "host-tool");
    let host_lib_key = package_key(&closure, "host-lib");
    let inputs = package_compilation_inputs(&closure).unwrap();
    assert_eq!(inputs.packages().count(), 4);
    assert!(inputs.package_root(host_key.identity()).is_some());
    assert!(inputs.package_root(host_lib_key.identity()).is_some());
    assert_eq!(
        inputs.dependency_target(root_key.identity(), "product_dep"),
        Some(product_key.identity())
    );
    assert_eq!(
        inputs.dependency_target(root_key.identity(), "host_tool"),
        None
    );
    assert_eq!(
        inputs.dependency_target_for_purpose(
            root_key.identity(),
            DependencyPurpose::Build,
            "host_tool"
        ),
        Some(host_key.identity())
    );
    // The host tool's own product edge reaches host-lib inside this
    // compilation, but only product-scope imports may select it.
    assert_eq!(
        inputs.dependency_target_for_purpose(
            host_key.identity(),
            DependencyPurpose::Build,
            "host_lib"
        ),
        None,
        "a dependency's build edges never project into a consumer's inputs"
    );
    assert_eq!(
        inputs.dependency_target(host_key.identity(), "host_lib"),
        Some(host_lib_key.identity())
    );
    assert_eq!(inputs.dependency_closure().packages().len(), 2);

    let (lock, request) = capture_lock(&closure, &tree.path("build"));
    let subject = lock.target(TARGET).unwrap().source();
    assert_eq!(subject.packages().len(), 4);
    let mut recorded = recorded_edges(subject);
    recorded.sort();
    assert_eq!(
        recorded,
        [
            ("build", 0usize, "host_tool", "host-tool"),
            ("product", 0usize, "host_lib", "host-lib"),
            ("product", 0usize, "product_dep", "product-dep"),
        ]
        .map(|(purpose, ordinal, alias, selected)| (
            purpose.to_owned(),
            ordinal,
            alias.to_owned(),
            selected.to_owned()
        ))
    );

    // The canonical lock records purpose-tagged authored rows and edges; a
    // round trip through text is exact.
    let text = lock.canonical_text().unwrap();
    assert!(text.contains("authored-build 1"));
    assert!(text.contains("purpose \"build\""));
    assert!(text.contains("purpose \"product\""));
    assert_eq!(
        PackageLock::recover_text(&text, PackageLockRecoveryLimits::default()).unwrap(),
        lock
    );

    // Locked-source recovery into an empty cache reacquires every recorded
    // selection, including the build-only host packages, and the recovered
    // graph is byte-identical to the accepted record.
    let storage = tree.storage("new-cache");
    let fresh = recover_locked_sources(
        &lock,
        TARGET,
        &request,
        &storage,
        LockedSourceRecoveryOptions::default(),
    )
    .unwrap();
    let fresh_subject = CanonicalSourceClosureSubject::from_resolved(
        &fresh.for_exact_target(TARGET),
        CanonicalSourceClosureSubjectLimits::default(),
    )
    .unwrap();
    assert_eq!(&fresh_subject, subject);
    let fresh_cache = fs::canonicalize(tree.path("new-cache")).unwrap();
    assert_eq!(fresh.custodies().len(), 4);
    for custody in fresh.custodies() {
        assert!(custody.snapshot_root().starts_with(&fresh_cache));
    }
    let inputs = package_compilation_inputs(&fresh).unwrap();
    assert_eq!(inputs.packages().count(), 4);
    assert_eq!(
        inputs.dependency_target(root_key.identity(), "host_tool"),
        None
    );
    assert_eq!(
        inputs.dependency_target_for_purpose(
            root_key.identity(),
            DependencyPurpose::Build,
            "host_tool"
        ),
        Some(host_key.identity())
    );

    // Checking reacquires and reviews every recorded package; the build-only
    // host packages hold their own reviews and no policy silently changes.
    let checked = check_locked_sources(
        &lock,
        TARGET,
        &request,
        &storage,
        LockedSourceRecoveryOptions::default(),
        &tree.path("check-build"),
    )
    .unwrap();
    assert!(checked.changed_policies().is_empty());
    assert_eq!(checked.reviews().reviews().len(), 4);
    for source in checked.accepted().source().packages() {
        let review = checked.reviews().review(source.key()).unwrap();
        assert_eq!(review.resolution(), source.resolution());
    }
}

#[test]
fn one_alias_can_select_different_packages_in_the_two_purposes() {
    let tree = Tree::new();
    package(
        &tree.path("sources/root"),
        "alias-root",
        concat!(
            " builder.depend_as(\"shared\", Source::Path { location: \"../product\" });\n",
            " builder.build_depend_as(\"shared\", Source::Path { location: \"../host\" });\n",
        ),
    );
    package(&tree.path("sources/product"), "product-dep", "");
    package(&tree.path("sources/host"), "host-tool", "");
    let storage = tree.storage("cache");
    let closure = resolve(&tree, &storage);
    assert_eq!(
        edges(&closure, "alias-root"),
        [
            (DependencyPurpose::Product, "shared", "product-dep"),
            (DependencyPurpose::Build, "shared", "host-tool"),
        ]
        .map(|(purpose, alias, target)| (purpose, alias.to_owned(), target.to_owned()))
    );

    let (lock, request) = capture_lock(&closure, &tree.path("build"));
    let subject = lock.target(TARGET).unwrap().source();
    let shared = subject
        .dependency_requests()
        .iter()
        .filter(|edge| edge.alias().as_str() == "shared")
        .map(|edge| edge.purpose())
        .collect::<Vec<_>>();
    assert_eq!(
        shared,
        [DependencyPurpose::Product, DependencyPurpose::Build]
    );

    let storage = tree.storage("recovery-cache");
    let fresh = recover_locked_sources(
        &lock,
        TARGET,
        &request,
        &storage,
        LockedSourceRecoveryOptions::default(),
    )
    .unwrap();
    let fresh_subject = CanonicalSourceClosureSubject::from_resolved(
        &fresh.for_exact_target(TARGET),
        CanonicalSourceClosureSubjectLimits::default(),
    )
    .unwrap();
    assert_eq!(&fresh_subject, subject);

    // The one alias answers each scope independently: product imports select
    // the product package, the root's build entry selects the host package.
    let root_key = closure.graph().root().clone();
    let product_key = package_key(&closure, "product-dep");
    let host_key = package_key(&closure, "host-tool");
    let inputs = package_compilation_inputs(&closure).unwrap();
    assert_eq!(inputs.packages().count(), 3);
    assert_eq!(
        inputs.dependency_target(root_key.identity(), "shared"),
        Some(product_key.identity())
    );
    assert_eq!(
        inputs.dependency_target_for_purpose(
            root_key.identity(),
            DependencyPurpose::Build,
            "shared"
        ),
        Some(host_key.identity())
    );
}

#[test]
fn one_package_can_serve_product_and_build_purposes() {
    let Some(execution_profile) = TargetProfile::host_if_supported() else {
        eprintln!("skipping dual-purpose review: no supported build execution profile");
        return;
    };
    let tree = Tree::new();
    package(
        &tree.path("sources/root"),
        "dual-root",
        concat!(
            " builder.depend_as(\"std\", Source::Path { location: \"../std\" });\n",
            " builder.build_depend_as(\"std\", Source::Path { location: \"../std\" });\n",
        ),
    );
    package(&tree.path("sources/std"), "std", "");
    let storage = tree.storage("cache");
    let closure = resolve(&tree, &storage);

    // Two independently authorized occurrences select one shared custody.
    assert_eq!(
        edges(&closure, "dual-root"),
        [
            (DependencyPurpose::Product, "std", "std"),
            (DependencyPurpose::Build, "std", "std"),
        ]
        .map(|(purpose, alias, target)| (purpose, alias.to_owned(), target.to_owned()))
    );
    assert_eq!(closure.graph().packages().len(), 2);
    let (lock, request) = capture_lock(&closure, &tree.path("build"));
    let subject = lock.target(TARGET).unwrap().source();
    assert_eq!(subject.dependency_requests().len(), 2);
    assert!(
        subject
            .dependency_requests()
            .iter()
            .all(|edge| edge.selected().key().name().as_str() == "std")
    );

    let storage = tree.storage("recovery-cache");
    let fresh = recover_locked_sources(
        &lock,
        TARGET,
        &request,
        &storage,
        LockedSourceRecoveryOptions::default(),
    )
    .unwrap();
    let fresh_subject = CanonicalSourceClosureSubject::from_resolved(
        &fresh.for_exact_target(TARGET),
        CanonicalSourceClosureSubjectLimits::default(),
    )
    .unwrap();
    assert_eq!(&fresh_subject, subject);
    let inputs = package_compilation_inputs(&fresh).unwrap();
    assert_eq!(inputs.packages().count(), 2);
    let root_key = fresh.graph().root().clone();
    let std_key = package_key(&fresh, "std");
    let reviews = compile_resolved_package_reviews(
        &fresh.for_exact_target(TARGET),
        &tree.path("recovered-review"),
        package_manager::review::SemanticBindingReview::Explicit(&[]),
    )
    .unwrap();
    assert!(
        reviews.review(&std_key).is_none(),
        "package-only lookup must not choose between roles"
    );
    assert_eq!(reviews.reviews_for(&std_key).count(), 2);
    for purpose in DependencyPurpose::ALL {
        let review = reviews.review_occurrence(&std_key, purpose).unwrap();
        let context = review.checked_context();
        assert_eq!(context.purpose(), purpose);
        assert_eq!(context.build_execution_profile(), Some(execution_profile));
        assert_eq!(
            context.target(),
            match purpose {
                DependencyPurpose::Product => TARGET,
                DependencyPurpose::Build => execution_profile,
            }
        );
        assert!(
            lock.target(TARGET)
                .unwrap()
                .occurrences()
                .iter()
                .any(
                    |occurrence| occurrence.acceptance().package() == std_key.identity()
                        && occurrence.context() == context
                )
        );
    }
    assert_eq!(
        inputs.dependency_target(root_key.identity(), "std"),
        Some(std_key.identity())
    );
    assert_eq!(
        inputs.dependency_target_for_purpose(root_key.identity(), DependencyPurpose::Build, "std"),
        Some(std_key.identity())
    );
}

#[test]
fn dropping_or_repurposing_a_build_row_rejects_locked_recovery() {
    let tree = Tree::new();
    dual_purpose(&tree);
    let storage = tree.storage("old-cache");
    let closure = resolve(&tree, &storage);
    let (lock, request) = capture_lock(&closure, &tree.path("build"));
    let subject = lock.target(TARGET).unwrap().source().clone();
    let original_build = fs::read_to_string(tree.path("sources/root/build.omg")).unwrap();

    let locked_local = |storage: &SourceResolverStorage| {
        resolve_locked_local_project_closure(
            &subject,
            &request,
            GitExactRevisionAcquisition::Offline,
            storage,
            LocalSourceLimits::default(),
            PackageSourceClosureLimits::default(),
            CanonicalSourceClosureSubjectLimits::default(),
        )
    };

    // Removing the build edge changes the root's complete declared projection:
    // the recorded build selection cannot silently disappear.
    fs::write(
        tree.path("sources/root/build.omg"),
        "machine build(builder: &mut Build) {\n builder.package(\"purpose-root\");\n builder.depend_as(\"product_dep\", Source::Path { location: \"../product\" });\n}\n",
    )
    .unwrap();
    let storage = tree.storage("new-cache");
    assert!(matches!(
        locked_local(&storage),
        Err(ResolveLockedPackageClosureError::SourceMismatch {
            package,
            detail: "complete fresh dependency projection differs",
        }) if package.name().as_str() == "purpose-root"
    ));
    assert!(matches!(
        recover_locked_sources(
            &lock,
            TARGET,
            &request,
            &storage,
            LockedSourceRecoveryOptions::default(),
        ),
        Err(RecoverLockedSourcesError::Resolution(
            ResolveLockedPackageClosureError::SourceMismatch { .. }
        ))
    ));

    // The edited graph is a different graph, not a corruption of the old one:
    // captured under its own lock, the product-only closure recovers exactly —
    // the dropped build edge removed only its own authorized selections.
    let edited = resolve(&tree, &storage);
    assert_eq!(edited.graph().packages().len(), 2);
    let (edited_lock, edited_request) = capture_lock(&edited, &tree.path("edited-build"));
    let edited_fresh = recover_locked_sources(
        &edited_lock,
        TARGET,
        &edited_request,
        &tree.storage("edited-cache"),
        LockedSourceRecoveryOptions::default(),
    )
    .unwrap();
    assert_fresh_matches(&edited_lock, &edited_fresh);

    // Repurposing the same request into the build scope is equally rejected:
    // a product edge never broadens into build authority by relabeling.
    fs::write(
        tree.path("sources/root/build.omg"),
        "machine build(builder: &mut Build) {\n builder.package(\"purpose-root\");\n builder.build_depend_as(\"product_dep\", Source::Path { location: \"../product\" });\n builder.depend_as(\"host_tool\", Source::Path { location: \"../host\" });\n}\n",
    )
    .unwrap();
    assert!(matches!(
        locked_local(&storage),
        Err(ResolveLockedPackageClosureError::SourceMismatch {
            package,
            detail: "complete fresh dependency projection differs",
        }) if package.name().as_str() == "purpose-root"
    ));

    // Restoring the exact authored rows recovers the recorded graph: the
    // rejections above were purpose checks, not incidental content drift.
    fs::write(tree.path("sources/root/build.omg"), original_build).unwrap();
    let fresh = locked_local(&storage).unwrap();
    let fresh_subject = CanonicalSourceClosureSubject::from_resolved(
        &fresh.for_exact_target(TARGET),
        CanonicalSourceClosureSubjectLimits::default(),
    )
    .unwrap();
    assert_eq!(fresh_subject, subject);
}

#[test]
fn a_wrong_purpose_edge_in_the_lock_text_rejects() {
    let tree = Tree::new();
    dual_purpose(&tree);
    let storage = tree.storage("cache");
    let closure = resolve(&tree, &storage);
    let (lock, _) = capture_lock(&closure, &tree.path("build"));
    let text = lock.canonical_text().unwrap();

    // The lock frames its canonical source section as `source <bytes>`; edit
    // the section and keep the declared byte count canonical.
    let (head, tail) = text.split_once("\nsource ").unwrap();
    let (count, body) = tail.split_once('\n').unwrap();
    let count: usize = count.parse().unwrap();
    let (section, rest) = body.split_at(count);
    let edited = section.replacen("purpose \"build\"", "purpose \"product\"", 1);
    assert_ne!(edited, section);
    let tampered = format!("{head}\nsource {}\n{edited}{rest}", edited.len());
    assert!(matches!(
        PackageLock::recover_text(&tampered, PackageLockRecoveryLimits::default()),
        Err(PackageLockError::Source(_))
    ));
    assert_eq!(
        PackageLock::recover_text(&text, PackageLockRecoveryLimits::default()).unwrap(),
        lock
    );
}
