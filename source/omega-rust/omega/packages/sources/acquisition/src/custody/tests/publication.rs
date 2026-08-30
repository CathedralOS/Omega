use super::{
    CacheCustodyKind, PendingCacheEntry, SourceResolveError, open_absolute_directory_nofollow,
    publish_cache_directory, temp_root,
};
#[cfg(unix)]
use super::{
    GIT_CACHE_METADATA, LocalSourceLimits, create_git_source, git_cache_entry_root,
    invalidate_git_cache_entry_from_open_parent, local_git_request,
    publish_cache_directory_from_open_parent, resolve_git_source,
};
use std::ffi::OsStr;

#[test]
fn cache_publication_renames_a_direct_child_through_its_parent_capability() {
    let cache = temp_root("capability-publication");
    std::fs::create_dir_all(&cache).expect("create publication parent");
    let canonical_cache = cache
        .canonicalize()
        .expect("canonicalize publication parent");
    let staged = canonical_cache.join("staged");
    let publication = canonical_cache.join("published");
    std::fs::create_dir_all(&staged).expect("create publication stage");
    std::fs::write(staged.join("payload"), b"retained").expect("write staged payload");

    publish_cache_directory(
        CacheCustodyKind::Git,
        &canonical_cache,
        &staged,
        &publication,
    )
    .expect("publish through retained cache parent");

    assert!(!staged.exists());
    assert_eq!(
        std::fs::read(publication.join("payload")).expect("read published payload"),
        b"retained"
    );
    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn cache_publication_rejects_a_preexisting_destination() {
    let cache = temp_root("capability-publication-existing");
    std::fs::create_dir_all(&cache).expect("create publication parent");
    let canonical_cache = cache
        .canonicalize()
        .expect("canonicalize publication parent");
    let staged = canonical_cache.join("staged");
    let publication = canonical_cache.join("published");
    std::fs::create_dir_all(&staged).expect("create publication stage");
    std::fs::create_dir(&publication).expect("create existing publication");

    let error = publish_cache_directory(
        CacheCustodyKind::LocalSnapshot,
        &canonical_cache,
        &staged,
        &publication,
    )
    .expect_err("publication must not replace an existing cache child");

    assert!(matches!(
        error,
        SourceResolveError::LocalSnapshotInvalid { .. }
    ));
    assert!(staged.is_dir());
    assert!(publication.is_dir());
    let _ = std::fs::remove_dir_all(&cache);
}

#[cfg(unix)]
#[test]
fn git_cache_stage_and_metadata_use_explicit_private_modes() {
    use std::os::unix::fs::PermissionsExt;

    let (repository, _) = create_git_source("git-private-cache-modes");
    let cache = temp_root("git-private-cache-modes-cache");
    let request = local_git_request(&repository, "HEAD");
    resolve_git_source(&request, &cache, LocalSourceLimits::default())
        .expect("materialize private Git cache entry");
    let entry = git_cache_entry_root(&cache, &request);

    assert_eq!(
        std::fs::symlink_metadata(&entry)
            .expect("inspect published cache entry")
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    assert_eq!(
        std::fs::symlink_metadata(entry.join(GIT_CACHE_METADATA))
            .expect("inspect resolver metadata")
            .permissions()
            .mode()
            & 0o777,
        0o600
    );

    let _ = std::fs::remove_dir_all(&repository);
    let _ = std::fs::remove_dir_all(&cache);
}

#[cfg(unix)]
#[test]
fn pending_git_cache_cleanup_does_not_remove_a_replacement_stage_name() {
    let cache = temp_root("git-retained-stage-cleanup");
    let retained_stage = cache.join("retained-stage");
    std::fs::create_dir_all(&cache).expect("create Git cache parent");
    let cache = cache.canonicalize().expect("canonicalize Git cache parent");
    let parent = open_absolute_directory_nofollow(&cache).expect("retain Git cache parent");
    let pending = PendingCacheEntry::create(&cache, &parent, "cleanup")
        .expect("create retained Git cache stage");
    let stage = pending.root.clone();

    std::fs::rename(&stage, &retained_stage).expect("relocate retained Git stage");
    std::fs::create_dir(&stage).expect("create replacement Git stage");
    std::fs::write(stage.join("sentinel"), b"replacement").expect("write replacement sentinel");
    drop(pending);

    assert_eq!(
        std::fs::read(stage.join("sentinel")).expect("read replacement sentinel"),
        b"replacement"
    );
    let _ = std::fs::remove_dir_all(&cache);
    let _ = std::fs::remove_dir_all(&retained_stage);
}

#[test]
fn pending_git_cache_publication_rejects_a_replaced_stage_name() {
    let cache = temp_root("git-retained-stage-publication");
    let retained_stage = cache.join("retained-stage");
    std::fs::create_dir_all(&cache).expect("create Git cache parent");
    let cache = cache.canonicalize().expect("canonicalize Git cache parent");
    let parent = open_absolute_directory_nofollow(&cache).expect("retain Git cache parent");
    let mut pending = PendingCacheEntry::create(&cache, &parent, "publication")
        .expect("create retained Git cache stage");
    let stage = pending.root.clone();
    let publication = cache.join("published");

    std::fs::rename(&stage, &retained_stage).expect("relocate retained Git stage");
    std::fs::create_dir(&stage).expect("create replacement Git stage");
    let error = pending
        .publish(&cache, &publication, OsStr::new("published"))
        .expect_err("publication must reject a replaced Git stage name");

    assert!(matches!(error, SourceResolveError::GitCacheInvalid { .. }));
    assert!(!publication.exists());
    drop(pending);
    assert!(stage.is_dir(), "cleanup must not remove the replacement");
    let _ = std::fs::remove_dir_all(&cache);
    let _ = std::fs::remove_dir_all(&retained_stage);
}

#[cfg(unix)]
#[test]
fn retained_git_cache_parent_owns_staging_and_invalidation_after_path_replacement() {
    let cache = temp_root("git-retained-parent-namespace");
    let retained_cache = cache.with_extension("retained");
    std::fs::create_dir_all(cache.join("entry")).expect("create retained cache entry");
    std::fs::write(cache.join("entry").join(GIT_CACHE_METADATA), b"retained")
        .expect("write retained metadata");
    let cache = cache.canonicalize().expect("canonicalize Git cache parent");
    let parent = open_absolute_directory_nofollow(&cache).expect("retain Git cache parent");

    std::fs::rename(&cache, &retained_cache).expect("replace Git cache parent path");
    std::fs::create_dir_all(cache.join("entry")).expect("create replacement cache entry");
    std::fs::write(cache.join("entry").join(GIT_CACHE_METADATA), b"replacement")
        .expect("write replacement metadata");

    let pending = PendingCacheEntry::create(&cache, &parent, "parent")
        .expect("create stage beneath retained cache parent");
    let retained_stage_name = pending.stage_name.clone();
    assert!(retained_cache.join(&retained_stage_name).is_dir());
    assert!(!cache.join(&retained_stage_name).exists());
    drop(pending);

    invalidate_git_cache_entry_from_open_parent(
        &cache,
        &parent,
        OsStr::new("entry"),
        &cache.join("entry"),
    )
    .expect("invalidate through retained Git cache parent");
    assert!(
        !retained_cache
            .join("entry")
            .join(GIT_CACHE_METADATA)
            .exists()
    );
    assert_eq!(
        std::fs::read(cache.join("entry").join(GIT_CACHE_METADATA))
            .expect("read replacement metadata"),
        b"replacement"
    );

    let _ = std::fs::remove_dir_all(&cache);
    let _ = std::fs::remove_dir_all(&retained_cache);
}

#[cfg(unix)]
#[test]
fn open_cache_parent_publication_is_not_redirected_by_path_replacement() {
    let root = temp_root("capability-publication-parent-replacement");
    let cache = root.join("cache");
    let retained = root.join("retained");
    std::fs::create_dir_all(cache.join("staged")).expect("create publication stage");
    let canonical_cache = cache.canonicalize().expect("canonicalize cache parent");
    let directory = open_absolute_directory_nofollow(&canonical_cache).expect("open cache parent");

    std::fs::rename(&cache, &retained).expect("replace opened cache parent path");
    std::fs::create_dir(&cache).expect("create replacement cache parent");
    publish_cache_directory_from_open_parent(
        CacheCustodyKind::Git,
        &canonical_cache,
        &directory,
        OsStr::new("staged"),
        OsStr::new("published"),
        None,
        || {},
    )
    .expect("publish through retained parent handle");

    assert!(retained.join("published").is_dir());
    assert!(!cache.join("published").exists());
    let _ = std::fs::remove_dir_all(&root);
}
