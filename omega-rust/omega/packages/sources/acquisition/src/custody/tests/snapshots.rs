use super::{
    CacheCustodyKind, GIT_CACHE_SNAPSHOTS, LocalSourceLimits, PendingMaterializedSnapshot,
    SourceResolveError, create_git_source, local_git_request, make_tree_owner_writable,
    open_verified_git_repository, resolve_git_source, temp_root,
    write_snapshot_file_from_open_root,
};
#[cfg(unix)]
use super::{
    make_open_snapshot_read_only, make_open_snapshot_root_publishable,
    make_open_snapshot_root_read_only, make_snapshot_read_only, open_absolute_directory_nofollow,
    same_capability_file_identity, verify_open_snapshot_tree_modes,
};
use std::path::Path;

#[test]
#[cfg_attr(
    windows,
    ignore = "Windows prevents replacement while the retained snapshot-parent handle is open"
)]
fn materialized_snapshot_writes_and_cleanup_remain_bound_to_the_open_stage() {
    let root = temp_root("retained-materialized-stage");
    let snapshots = root.join("snapshots");
    let retained_parent = root.join("retained-snapshots");
    std::fs::create_dir_all(&snapshots).expect("create snapshot parent");
    let pending = PendingMaterializedSnapshot::create(
        CacheCustodyKind::LocalSnapshot,
        &snapshots,
        ".source-test.stage",
    )
    .expect("create retained materialization stage");
    let stage_name = pending.stage_name.clone();

    std::fs::rename(&snapshots, &retained_parent).expect("replace snapshot parent path");
    std::fs::create_dir(&snapshots).expect("create replacement snapshot parent");
    write_snapshot_file_from_open_root(
        CacheCustodyKind::LocalSnapshot,
        pending.directory().expect("retain stage directory"),
        Path::new("payload"),
        &pending.root,
        b"retained",
        false,
    )
    .expect("write through retained stage");

    assert_eq!(
        std::fs::read(retained_parent.join(&stage_name).join("payload"))
            .expect("read retained stage payload"),
        b"retained"
    );
    assert!(!snapshots.join(&stage_name).exists());
    drop(pending);
    assert!(!retained_parent.join(&stage_name).exists());
    assert!(snapshots.is_dir());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
#[cfg_attr(
    windows,
    ignore = "Windows prevents replacement while the retained cache-entry handle is open"
)]
fn git_snapshot_bootstrap_and_staging_remain_bound_to_the_retained_entry() {
    let (repo, _) = create_git_source("retained-snapshot-bootstrap-source");
    let cache = temp_root("retained-snapshot-bootstrap-cache");
    let request = local_git_request(&repo, "HEAD");
    resolve_git_source(&request, &cache, LocalSourceLimits::default()).expect("prime cache");
    let verified = open_verified_git_repository(&cache, &request);
    let snapshots_path = verified.entry_root.join(GIT_CACHE_SNAPSHOTS);
    make_tree_owner_writable(&snapshots_path);
    std::fs::remove_dir_all(&snapshots_path).expect("remove primed snapshot collection");

    let snapshots = verified
        .open_or_create_snapshots()
        .expect("bootstrap snapshots through retained entry");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        assert_eq!(
            std::fs::metadata(&snapshots_path)
                .expect("read snapshot collection mode")
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
    }

    let displaced_entry = verified.entry_root.with_file_name("entry.displaced");
    std::fs::rename(&verified.entry_root, &displaced_entry).expect("displace retained cache entry");
    std::fs::create_dir(&verified.entry_root).expect("create replacement cache entry");
    let pending = PendingMaterializedSnapshot::create_from_open_parent(
        CacheCustodyKind::Git,
        &snapshots.path,
        &snapshots.directory,
        ".tree-retained.stage",
    )
    .expect("stage through retained snapshots collection");
    let stage_name = pending.stage_name.clone();
    assert!(
        displaced_entry
            .join(GIT_CACHE_SNAPSHOTS)
            .join(&stage_name)
            .is_dir()
    );
    assert!(
        !verified
            .entry_root
            .join(GIT_CACHE_SNAPSHOTS)
            .join(&stage_name)
            .exists()
    );
    drop(pending);

    let _ = std::fs::remove_dir_all(&repo);
    make_tree_owner_writable(&cache);
    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
#[cfg_attr(
    windows,
    ignore = "Windows prevents replacement while the retained snapshot-stage handle is open"
)]
fn materialized_snapshot_publication_rejects_a_replaced_stage_name() {
    let root = temp_root("replaced-materialized-stage");
    let snapshots = root.join("snapshots");
    std::fs::create_dir_all(&snapshots).expect("create snapshot parent");
    let mut pending =
        PendingMaterializedSnapshot::create(CacheCustodyKind::Git, &snapshots, ".tree-test.stage")
            .expect("create retained materialization stage");
    let displaced = snapshots.join("displaced-stage");
    std::fs::rename(&pending.root, &displaced).expect("displace retained stage name");
    std::fs::create_dir(&pending.root).expect("create replacement stage directory");
    let replacement = pending.root.clone();
    let publication = snapshots.join("tree-test");

    let error = pending
        .publish(&snapshots, &publication)
        .expect_err("replacement stage name must not publish");

    assert!(matches!(error, SourceResolveError::GitCacheInvalid { .. }));
    assert!(pending.root.is_dir());
    assert!(!publication.exists());
    drop(pending);
    assert!(!displaced.exists());
    assert!(replacement.is_dir());
    let _ = std::fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[test]
fn sealed_materialized_snapshot_publishes_with_retained_identity_and_canonical_modes() {
    use std::os::unix::fs::PermissionsExt;

    let root = temp_root("sealed-materialized-publication");
    let snapshots = root.join("snapshots");
    std::fs::create_dir_all(&snapshots).expect("create snapshot parent");
    let mut pending = PendingMaterializedSnapshot::create(
        CacheCustodyKind::LocalSnapshot,
        &snapshots,
        ".source-sealed.stage",
    )
    .expect("create retained materialization stage");
    write_snapshot_file_from_open_root(
        CacheCustodyKind::LocalSnapshot,
        pending.directory().expect("retain stage directory"),
        Path::new("nested/payload"),
        &pending.root,
        b"retained",
        false,
    )
    .expect("write staged payload");
    make_open_snapshot_read_only(
        CacheCustodyKind::LocalSnapshot,
        pending.directory().expect("retain stage directory"),
        &pending.root,
    )
    .expect("seal staged snapshot");

    let stage = pending.root.clone();
    let retained = pending
        .directory()
        .expect("retain sealed stage")
        .dir_metadata()
        .expect("inspect sealed stage");
    assert_eq!(
        std::fs::symlink_metadata(&stage)
            .expect("inspect staged root mode")
            .permissions()
            .mode()
            & 0o777,
        0o555
    );

    let publication = snapshots.join("source-published");
    pending
        .publish(&snapshots, &publication)
        .expect("publish a sealed snapshot");

    assert!(!stage.exists());
    let published = pending
        .parent
        .symlink_metadata("source-published")
        .expect("inspect publication through retained parent");
    assert!(same_capability_file_identity(&retained, &published));
    assert_eq!(
        std::fs::symlink_metadata(&publication)
            .expect("inspect published root mode")
            .permissions()
            .mode()
            & 0o777,
        0o555
    );
    assert_eq!(
        std::fs::symlink_metadata(publication.join("nested"))
            .expect("inspect nested directory")
            .permissions()
            .mode()
            & 0o777,
        0o555
    );
    assert_eq!(
        std::fs::symlink_metadata(publication.join("nested/payload"))
            .expect("inspect published payload")
            .permissions()
            .mode()
            & 0o777,
        0o444
    );
    verify_open_snapshot_tree_modes(
        CacheCustodyKind::LocalSnapshot,
        pending.directory().expect("retain published directory"),
        &publication,
    )
    .expect("verify the complete published tree");

    make_tree_owner_writable(&root);
    let _ = std::fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[test]
fn materialized_snapshot_write_rejects_a_nested_directory_symlink_substitution() {
    let root = temp_root("materialized-stage-nested-symlink");
    let stage = root.join("stage");
    let target = root.join("target");
    std::fs::create_dir_all(stage.join("nested")).expect("create stage directory");
    std::fs::create_dir(&target).expect("create substitution target");
    let stage_directory = open_absolute_directory_nofollow(
        &stage.canonicalize().expect("canonicalize stage directory"),
    )
    .expect("open stage directory");
    std::fs::remove_dir(stage.join("nested")).expect("remove nested stage directory");
    std::os::unix::fs::symlink(&target, stage.join("nested"))
        .expect("substitute nested directory symlink");

    let error = write_snapshot_file_from_open_root(
        CacheCustodyKind::LocalSnapshot,
        &stage_directory,
        Path::new("nested/payload"),
        &stage,
        b"must not escape",
        false,
    )
    .expect_err("nested symlink substitution must reject");

    assert!(matches!(
        error,
        SourceResolveError::LocalSnapshotInvalid { .. }
    ));
    assert!(!target.join("payload").exists());
    let _ = std::fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[test]
fn published_snapshot_mode_verification_remains_bound_to_its_open_root() {
    use std::os::unix::fs::PermissionsExt;

    let parent = temp_root("published-snapshot-open-root");
    let publication = parent.join("publication");
    let retained = parent.join("retained");
    std::fs::create_dir_all(publication.join("nested")).expect("create published snapshot tree");
    std::fs::write(publication.join("nested/payload"), b"retained")
        .expect("write published snapshot payload");
    make_snapshot_read_only(&publication).expect("finalize published snapshot modes");
    let canonical_publication = publication
        .canonicalize()
        .expect("canonicalize published snapshot");
    let directory = open_absolute_directory_nofollow(&canonical_publication)
        .expect("open published snapshot root");

    make_open_snapshot_root_publishable(&directory, &canonical_publication)
        .expect("make retained root publishable");
    std::fs::rename(&publication, &retained).expect("replace publication root path");
    make_open_snapshot_root_read_only(&directory, &canonical_publication)
        .expect("restore retained root mode");
    std::fs::create_dir(&publication).expect("create replacement publication root");
    std::fs::set_permissions(&publication, std::fs::Permissions::from_mode(0o777))
        .expect("make replacement publication writable");

    verify_open_snapshot_tree_modes(
        CacheCustodyKind::LocalSnapshot,
        &directory,
        &canonical_publication,
    )
    .expect("verification must remain on the retained publication");
    assert_eq!(
        std::fs::read(retained.join("nested/payload")).expect("read retained payload"),
        b"retained"
    );
    std::fs::set_permissions(&publication, std::fs::Permissions::from_mode(0o700)).unwrap();
    make_tree_owner_writable(&retained);
    let _ = std::fs::remove_dir_all(&parent);
}
