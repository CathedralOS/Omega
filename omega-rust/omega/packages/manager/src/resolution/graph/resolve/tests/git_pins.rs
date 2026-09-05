//! Selective updates preserve accepted Git requests across real branch moves.

use super::super::cache::{GitAcquisitionCache, SourceCacheLane};
use super::{run_test_git, temp_root, test_git_head, write_application, write_package};
use crate::declarations::{PackageKey, PackageName, PackageSelection};
use crate::resolution::graph::{
    CanonicalSourceClosureSubject, CanonicalSourceClosureSubjectLimits, GitDependencyPins,
    GitDependencyPinsError, PackageSourceClosureLimits,
};
use crate::resolution::source::{
    GitPackageSourceRequest, ResolvePackageSourceError, ResolvedPackageSource,
};
use package_source::git::resolution::GitExactRevisionAcquisition;
use package_source::{
    GitSourceRequest, LocalSourceLimits, ResolvedGitSource, SourceResolveError,
    SourceResolverStorage,
};
use std::path::{Path, PathBuf};

mod requests;
mod support;
mod workspace;
use support::*;

#[test]
fn empty_selection_preserves_moving_branch_offline_for_packages_and_projects() {
    for application in [false, true] {
        let fixture = Fixture::package("warm-preserved", "original", application);
        let storage = fixture.storage("warm");
        let request = fixture.request();
        let accepted = fixture.subject(&request, &storage, application);
        let original = accepted.root().selected();
        fixture.advance("main.omg");
        assert_ne!(
            test_git_head(&fixture.repository),
            commit(original.resolution())
        );
        fixture.disconnect();

        let policy =
            GitDependencyPins::new(&accepted, &[], GitExactRevisionAcquisition::Offline).unwrap();
        let mut cache = GitAcquisitionCache::preserving(policy);
        let retained = resolve(&mut cache, &request, &storage, application).unwrap();
        assert_eq!(retained.key(), original.key());
        assert_eq!(retained.resolution(), original.resolution());
        assert_eq!(retained.source().requested_revision(), "main");
        assert_eq!(
            std::fs::read_to_string(retained.snapshot_root().join("main.omg")).unwrap(),
            "machine root() {}\n"
        );
        assert_eq!(cache.acquisition_count(), 1);
    }
}

#[test]
fn missing_preserved_pin_fails_offline_even_when_branch_is_available() {
    let fixture = Fixture::package("cold-offline", "original", false);
    let warm = fixture.storage("warm");
    let request = fixture.request();
    let accepted = fixture.subject(&request, &warm, false);
    fixture.advance("main.omg");
    let cold = fixture.storage("cold");
    let policy =
        GitDependencyPins::new(&accepted, &[], GitExactRevisionAcquisition::Offline).unwrap();
    let mut cache = GitAcquisitionCache::preserving(policy);

    let error = resolve(&mut cache, &request, &cold, false).unwrap_err();
    assert!(
        matches!(&error, ResolvePackageSourceError::Source(SourceResolveError::GitExactRevisionUnavailable { commit: missing, .. })
            if missing == &commit(accepted.root().selected().resolution())),
        "{error:?}"
    );
    assert_eq!(cache.acquisition_count(), 0);

    // The same source is reachable by normal acquisition: the failure above
    // represents the missing accepted revision in offline storage.
    let current = resolve(&mut GitAcquisitionCache::default(), &request, &cold, false).unwrap();
    assert_eq!(
        current.source().commit(),
        test_git_head(&fixture.repository)
    );
    assert_ne!(
        current.resolution(),
        accepted.root().selected().resolution()
    );
}

#[test]
fn cold_exact_fetch_preserves_accepted_commit_after_branch_moves() {
    let fixture = Fixture::package("cold-exact-fetch", "original", false);
    let warm = fixture.storage("warm");
    let request = fixture.request();
    let accepted = fixture.subject(&request, &warm, false);
    fixture.advance("main.omg");
    let cold = fixture.storage("cold");
    let policy =
        GitDependencyPins::new(&accepted, &[], GitExactRevisionAcquisition::AllowFetch).unwrap();
    let mut cache = GitAcquisitionCache::preserving(policy);

    let retained = resolve(&mut cache, &request, &cold, false).unwrap();
    assert_eq!(
        retained.resolution(),
        accepted.root().selected().resolution()
    );
    assert_ne!(
        retained.source().commit(),
        test_git_head(&fixture.repository)
    );
    fixture.disconnect();
    let offline =
        GitDependencyPins::new(&accepted, &[], GitExactRevisionAcquisition::Offline).unwrap();
    let retained_again = resolve(
        &mut GitAcquisitionCache::preserving(offline),
        &request,
        &cold,
        false,
    )
    .unwrap();
    assert_eq!(retained_again.resolution(), retained.resolution());
}

#[test]
fn selected_package_refreshes_branch_and_retains_one_operation_revision() {
    let fixture = Fixture::package("selected-refresh", "original", false);
    let storage = fixture.storage("warm");
    let request = fixture.request();
    let accepted = fixture.subject(&request, &storage, false);
    fixture.advance("main.omg");
    let selected = [accepted.root().selected().key().clone()];
    let policy =
        GitDependencyPins::new(&accepted, &selected, GitExactRevisionAcquisition::Offline).unwrap();
    let mut cache = GitAcquisitionCache::preserving(policy);

    let refreshed = resolve(&mut cache, &request, &storage, false).unwrap();
    assert_eq!(refreshed.key(), accepted.root().selected().key());
    assert_eq!(
        refreshed.source().commit(),
        test_git_head(&fixture.repository)
    );
    assert_ne!(
        refreshed.resolution(),
        accepted.root().selected().resolution()
    );
    fixture.advance("later.omg");
    let repeated = resolve(&mut cache, &request, &storage, false).unwrap();
    assert_eq!(repeated.resolution(), refreshed.resolution());
    assert_eq!(cache.acquisition_count(), 1);
}

#[test]
fn unknown_selected_key_rejects_even_when_its_package_name_matches() {
    let fixture = Fixture::package("known-source", "same-name", false);
    let other = Fixture::package("unknown-source", "same-name", false);
    let accepted = fixture.subject(&fixture.request(), &fixture.storage("warm"), false);
    let other_subject = other.subject(&other.request(), &other.storage("warm"), false);
    let unknown = other_subject.root().selected().key().clone();
    assert_eq!(unknown.name(), accepted.root().selected().key().name());
    assert_ne!(unknown, *accepted.root().selected().key());
    let selection = [unknown.clone()];

    let error = GitDependencyPins::new(&accepted, &selection, GitExactRevisionAcquisition::Offline)
        .expect_err("unknown source-qualified key must reject");
    assert_eq!(error, GitDependencyPinsError::UnknownPackage(unknown));
}

#[test]
fn duplicate_selected_key_rejects() {
    let fixture = Fixture::package("duplicate-selection", "original", false);
    let accepted = fixture.subject(&fixture.request(), &fixture.storage("warm"), false);
    let key = accepted.root().selected().key().clone();
    let selection = [key.clone(), key.clone()];

    let error = GitDependencyPins::new(
        &accepted,
        &selection,
        GitExactRevisionAcquisition::AllowFetch,
    )
    .expect_err("duplicate update selection must reject");
    assert_eq!(error, GitDependencyPinsError::DuplicatePackage(key));
}

#[test]
fn preserved_pin_rechecks_retained_source_content() {
    let fixture = Fixture::package("preserved-content", "original", false);
    let storage = fixture.storage("warm");
    let request = fixture.request();
    let accepted = fixture.subject(&request, &storage, false);
    let original = resolve(
        &mut GitAcquisitionCache::default(),
        &request,
        &storage,
        false,
    )
    .unwrap();
    let altered = original.snapshot_root().join("main.omg");
    make_owner_writable(&altered);
    std::fs::write(&altered, "machine changed() {}\n").unwrap();
    fixture.disconnect();
    let policy =
        GitDependencyPins::new(&accepted, &[], GitExactRevisionAcquisition::AllowFetch).unwrap();

    let error = resolve(
        &mut GitAcquisitionCache::preserving(policy),
        &request,
        &storage,
        false,
    )
    .unwrap_err();
    assert!(
        matches!(
            error,
            ResolvePackageSourceError::Source(SourceResolveError::GitCacheInvalid { .. })
        ),
        "{error:?}"
    );
}
