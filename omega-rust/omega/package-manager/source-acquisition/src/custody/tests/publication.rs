use super::{
    CacheCustodyKind, PendingCacheEntry, SourceResolveError, open_absolute_directory_nofollow,
    publish_cache_directory, temp_root,
};
use std::ffi::OsStr;

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

#[test]
#[cfg_attr(
    windows,
    ignore = "Windows prevents replacement while the retained stage handle is open"
)]
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
