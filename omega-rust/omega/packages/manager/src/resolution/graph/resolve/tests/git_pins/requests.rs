use super::*;
use crate::declarations::dependencies::read::DependencySourceRequest;
use crate::resolution::graph::reconcile::resolve_package_source_closure;
use crate::resolution::graph::{PackageRootSourceRequest, ResolvedPackageSourceClosure};

#[test]
fn changed_revision_or_authored_locator_resolves_normally() {
    let fixture = Fixture::package("changed-pin-request", "original", false);
    let storage = fixture.storage("warm");
    let accepted = fixture.subject(&fixture.request(), &storage, false);
    fixture.advance("main.omg");
    let policy =
        GitDependencyPins::new(&accepted, &[], GitExactRevisionAcquisition::Offline).unwrap();
    let mut cache = GitAcquisitionCache::preserving(policy);
    let alternate_locator = fixture.locator.trim_end_matches(".git");
    for request in [
        fixture.request_at(&fixture.locator, "HEAD", PackageSelection::Root),
        fixture.request_at(alternate_locator, "main", PackageSelection::Root),
    ] {
        assert_eq!(
            request.acquisition().lineage(),
            fixture.request().acquisition().lineage()
        );
        let current = resolve(&mut cache, &request, &storage, false).unwrap();
        assert_eq!(
            current.source().commit(),
            test_git_head(&fixture.repository)
        );
        assert_eq!(current.key(), accepted.root().selected().key());
        assert_ne!(
            current.resolution(),
            accepted.root().selected().resolution()
        );
    }
    let retained = resolve(&mut cache, &fixture.request(), &storage, false).unwrap();
    assert_eq!(
        retained.resolution(),
        accepted.root().selected().resolution()
    );
    assert_eq!(cache.acquisition_count(), 3);
}

#[test]
fn new_repository_request_resolves_normally_under_preservation() {
    let fixture = Fixture::package("accepted-request", "original", false);
    let new_source = Fixture::package("new-request", "new-package", false);
    let storage = fixture.storage("warm");
    let accepted = fixture.subject(&fixture.request(), &storage, false);
    let policy =
        GitDependencyPins::new(&accepted, &[], GitExactRevisionAcquisition::Offline).unwrap();

    let current = resolve(
        &mut GitAcquisitionCache::preserving(policy),
        &new_source.request(),
        &storage,
        false,
    )
    .unwrap();
    assert_eq!(
        current.source().commit(),
        test_git_head(&new_source.repository)
    );
    assert_eq!(current.key().name().as_str(), "new-package");
    assert_ne!(
        current.key().source_lineage(),
        accepted.root().selected().key().source_lineage()
    );
}

// The adapter changes only transport for local fixtures. Every custody and
// dependency projection comes from the actual package source resolver.
fn resolve_dependencies(
    root: &Fixture,
    dependencies: &[&Fixture],
    storage: &SourceResolverStorage,
    cache: &mut GitAcquisitionCache<'_>,
) -> ResolvedPackageSourceClosure {
    let request = root.request();
    let source = resolve(
        &mut GitAcquisitionCache::default(),
        &request,
        storage,
        false,
    )
    .unwrap();
    resolve_package_source_closure(
        PackageRootSourceRequest::Git(request),
        source.into_custody(),
        |_, request| {
            let DependencySourceRequest::Git {
                repository,
                revision,
                selection,
                ..
            } = request
            else {
                panic!("fixture declares only Git dependency requests")
            };
            let fixture = dependencies
                .iter()
                .find(|fixture| fixture.locator == *repository)
                .expect("every authored locator has a local fixture");
            let local = fixture.request_at(repository, revision, selection.clone());
            resolve(cache, &local, storage, false).map(ResolvedPackageSource::into_custody)
        },
    )
    .unwrap()
}

fn write_dependencies(root: &Fixture, dependencies: &[(&str, &Fixture)]) {
    let declarations = dependencies.iter().map(|(alias, fixture)| {
        format!(
            "    builder.depend_as(\"{alias}\", Source::Git {{ repository: \"{}\", revision: \"main\", selection: PackageSelection::Root {{}} }});\n",
            fixture.locator,
        )
    }).collect::<String>();
    std::fs::write(
        root.repository.join("build.omg"),
        format!("machine build(builder: &mut Build) {{\n    builder.package(\"consumer\");\n{declarations}}}\n"),
    )
    .unwrap();
    root.commit();
}

fn subject(closure: &ResolvedPackageSourceClosure) -> CanonicalSourceClosureSubject {
    CanonicalSourceClosureSubject::from_resolved(
        &closure.for_exact_target(target::TargetProfile::CrossPlatformCli),
        CanonicalSourceClosureSubjectLimits::default(),
    )
    .unwrap()
}

#[test]
fn selected_repository_refresh_leaves_same_named_other_repository_pinned() {
    let root = Fixture::package("selective-consumer", "consumer", false);
    let first = Fixture::package("selected-repository", "same-name", false);
    let second = Fixture::package("unrelated-repository", "same-name", false);
    write_dependencies(&root, &[("first", &first), ("second", &second)]);
    let storage = root.storage("warm");
    let closure = resolve_dependencies(
        &root,
        &[&first, &second],
        &storage,
        &mut GitAcquisitionCache::default(),
    );
    let accepted = subject(&closure);
    let first_edge = accepted
        .dependency_requests()
        .iter()
        .find(|edge| edge.alias().as_str() == "first")
        .unwrap();
    let second_edge = accepted
        .dependency_requests()
        .iter()
        .find(|edge| edge.alias().as_str() == "second")
        .unwrap();
    assert_eq!(
        first_edge.selected().key().name(),
        second_edge.selected().key().name()
    );
    assert_ne!(first_edge.selected().key(), second_edge.selected().key());
    first.advance("main.omg");
    second.advance("main.omg");
    second.disconnect();
    let selection = [first_edge.selected().key().clone()];
    let policy =
        GitDependencyPins::new(&accepted, &selection, GitExactRevisionAcquisition::Offline)
            .unwrap();
    let candidate = resolve_dependencies(
        &root,
        &[&first, &second],
        &storage,
        &mut GitAcquisitionCache::preserving(policy),
    );
    let candidate = subject(&candidate);
    let refreshed = candidate
        .packages()
        .iter()
        .find(|source| source.key() == first_edge.selected().key())
        .unwrap();
    let retained = candidate
        .packages()
        .iter()
        .find(|source| source.key() == second_edge.selected().key())
        .unwrap();
    assert_eq!(
        commit(refreshed.resolution()),
        test_git_head(&first.repository)
    );
    assert_ne!(refreshed.resolution(), first_edge.selected().resolution());
    assert_eq!(retained.resolution(), second_edge.selected().resolution());
}

#[test]
fn alias_change_preserves_the_exact_authored_dependency_request() {
    let root = Fixture::package("renamed-alias-consumer", "consumer", false);
    let dependency = Fixture::package("renamed-alias-source", "dependency", false);
    write_dependencies(&root, &[("before", &dependency)]);
    let storage = root.storage("warm");
    let accepted = subject(&resolve_dependencies(
        &root,
        &[&dependency],
        &storage,
        &mut GitAcquisitionCache::default(),
    ));
    dependency.advance("main.omg");
    write_dependencies(&root, &[("after", &dependency)]);
    dependency.disconnect();
    let policy =
        GitDependencyPins::new(&accepted, &[], GitExactRevisionAcquisition::Offline).unwrap();

    let candidate = subject(&resolve_dependencies(
        &root,
        &[&dependency],
        &storage,
        &mut GitAcquisitionCache::preserving(policy),
    ));
    let before = &accepted.dependency_requests()[0];
    let after = &candidate.dependency_requests()[0];
    assert_eq!(before.alias().as_str(), "before");
    assert_eq!(after.alias().as_str(), "after");
    assert_eq!(after.selected(), before.selected());
}
