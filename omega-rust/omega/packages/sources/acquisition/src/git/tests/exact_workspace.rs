//! Exact root objects select members without materializing unrelated payloads.

use super::*;
mod fixtures;
mod planner;
use fixtures::{Fixture, declaration_limits, limits};
use planner::Planner;

#[test]
fn exact_old_workspace_declarations_survive_branch_drift_and_offline_recovery() {
    let mut fixture = Fixture::new("exact-workspace-drift");
    fixture.advance();
    let mut planner = Planner::default();
    let first = fixture
        .resolve(GitExactRevisionAcquisition::AllowFetch, &mut planner)
        .unwrap();
    fixture.assert_original(&first);
    assert_eq!((planner.discoveries, planner.selections), (1, 1));
    assert_eq!(
        *first.evidence(),
        "selected from exact authenticated declarations"
    );
    fixture.disconnect();
    for mode in [
        GitExactRevisionAcquisition::Offline,
        GitExactRevisionAcquisition::AllowFetch,
    ] {
        let recovered = fixture.resolve(mode, &mut planner).unwrap();
        fixture.assert_original(&recovered);
        assert_eq!(
            recovered.source().content_identity(),
            first.source().content_identity()
        );
    }
    assert_eq!((planner.discoveries, planner.selections), (3, 3));
}

#[test]
fn exact_workspace_explicit_primary_keeps_root_and_member_tree_distinct() {
    let fixture = Fixture::new("exact-workspace-primary");
    let result = resolve_git_workspace_member_at_revision_in_lanes_with_primary_git(
        fixture.storage.git_sources().primary_git().unwrap(),
        &fixture.request,
        &fixture.commit,
        &fixture.root_tree,
        GitExactRevisionAcquisition::AllowFetch,
        fixture.storage.git_sources(),
        fixture.storage.workspace_members(),
        limits(),
        declaration_limits(),
        &mut Planner::default(),
    )
    .unwrap();
    fixture.assert_original(&result);
}

#[test]
fn absent_recorded_workspace_objects_fail_before_invoking_planner() {
    let fixture = Fixture::new("exact-workspace-offline-missing");
    let mut planner = Planner::default();
    let error = fixture
        .resolve(GitExactRevisionAcquisition::Offline, &mut planner)
        .unwrap_err();
    assert!(
        matches!(
            error,
            GitWorkspaceProjectionError::Source(
                SourceResolveError::GitExactRevisionUnavailable { .. }
            )
        ),
        "{error:?}"
    );
    assert_eq!((planner.discoveries, planner.selections), (0, 0));
}

#[test]
fn member_tree_cannot_replace_recorded_repository_root_before_planning() {
    let fixture = Fixture::new("exact-workspace-forged-root");
    let mut planner = Planner::default();
    let error = resolve_git_workspace_member_at_revision_in_lanes(
        &fixture.request,
        &fixture.commit,
        &fixture.member_tree,
        GitExactRevisionAcquisition::AllowFetch,
        fixture.storage.git_sources(),
        fixture.storage.workspace_members(),
        limits(),
        declaration_limits(),
        &mut planner,
    )
    .unwrap_err();
    assert!(
        matches!(
            error,
            GitWorkspaceProjectionError::Source(SourceResolveError::GitObjectInvalid { .. })
        ),
        "{error:?}"
    );
    assert_eq!((planner.discoveries, planner.selections), (0, 0));
}

#[test]
fn exact_workspace_rejects_selection_outside_authenticated_discovered_members() {
    let fixture = Fixture::new("exact-workspace-undiscovered");
    let mut planner = Planner {
        select_undiscovered: true,
        ..Planner::default()
    };
    let error = fixture
        .resolve(GitExactRevisionAcquisition::AllowFetch, &mut planner)
        .unwrap_err();
    assert_eq!((planner.discoveries, planner.selections), (1, 1));
    assert_eq!(
        error,
        GitWorkspaceProjectionError::Source(SourceResolveError::GitTreeInvalid {
            path: b"packages/other".to_vec(),
            message: "workspace planner selected a member absent from its discovered set".into(),
        })
    );
}

#[test]
fn warm_exact_workspace_reapplies_declaration_limits_before_planning() {
    let mut fixture = Fixture::new("exact-workspace-declaration-limit");
    fixture
        .resolve(
            GitExactRevisionAcquisition::AllowFetch,
            &mut Planner::default(),
        )
        .unwrap();
    fixture.disconnect();
    let mut planner = Planner::default();
    let error = resolve_git_workspace_member_at_revision_in_lanes(
        &fixture.request,
        &fixture.commit,
        &fixture.root_tree,
        GitExactRevisionAcquisition::Offline,
        fixture.storage.git_sources(),
        fixture.storage.workspace_members(),
        limits(),
        GitWorkspaceDeclarationLimits::new(8, 8, 8),
        &mut planner,
    )
    .unwrap_err();
    assert!(
        matches!(
            error,
            GitWorkspaceProjectionError::Source(SourceResolveError::TooManyBytes { limit: 8 })
        ),
        "{error:?}"
    );
    assert_eq!((planner.discoveries, planner.selections), (0, 0));
}
