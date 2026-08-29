use super::super::{
    CacheCustodyKind, LocalSourceLimits, SourceResolveError, SourceResolverStorage,
    external_local_storage_lane, open_absolute_directory_nofollow, open_cache_custody_directory,
    resolve_local_source_snapshot_with_storage, temp_root, verify_cache_custody_from_open_root,
};
use std::path::Path;

#[cfg(unix)]
#[test]
fn git_cache_custody_does_not_follow_replaced_directory_leaf() {
    assert_cache_custody_does_not_follow_replaced_directory_leaf(CacheCustodyKind::Git);
}

#[cfg(unix)]
#[test]
fn local_cache_custody_does_not_follow_replaced_directory_leaf() {
    assert_cache_custody_does_not_follow_replaced_directory_leaf(CacheCustodyKind::LocalSnapshot);
}

#[cfg(unix)]
fn assert_cache_custody_does_not_follow_replaced_directory_leaf(kind: CacheCustodyKind) {
    let cache = temp_root("cache-nofollow-replaced-directory");
    std::fs::create_dir_all(cache.join("classified")).expect("create classified directory");
    std::fs::create_dir_all(cache.join("replacement")).expect("create replacement directory");
    let canonical_cache = cache.canonicalize().expect("canonicalize cache root");
    let directory =
        open_absolute_directory_nofollow(&canonical_cache).expect("open cache root capability");
    let classified = directory
        .symlink_metadata("classified")
        .expect("classify cache directory");
    assert!(classified.is_dir());

    std::fs::remove_dir(cache.join("classified")).expect("remove classified directory");
    std::os::unix::fs::symlink("replacement", cache.join("classified"))
        .expect("replace cache directory with symlink");
    let error = open_cache_custody_directory(
        &directory,
        Path::new("classified"),
        &canonical_cache.join("classified"),
        &classified,
        kind,
    )
    .expect_err("cache custody must not follow a replacement directory symlink");
    assert!(matches!(
        (kind, error),
        (
            CacheCustodyKind::Git,
            SourceResolveError::GitCacheInvalid { .. }
        ) | (
            CacheCustodyKind::LocalSnapshot,
            SourceResolveError::LocalSnapshotInvalid { .. }
        )
    ));

    let _ = std::fs::remove_dir_all(&cache);
}

#[cfg(unix)]
#[test]
fn cache_custody_rejects_replaced_concrete_directory_identity() {
    let cache = temp_root("cache-replaced-concrete-directory");
    std::fs::create_dir_all(cache.join("classified")).expect("create classified directory");
    let canonical_cache = cache.canonicalize().expect("canonicalize cache root");
    let directory =
        open_absolute_directory_nofollow(&canonical_cache).expect("open cache root capability");
    let classified = directory
        .symlink_metadata("classified")
        .expect("classify cache directory");

    std::fs::rename(cache.join("classified"), cache.join("retained"))
        .expect("retain classified directory identity");
    std::fs::create_dir(cache.join("classified")).expect("replace with concrete directory");
    let error = open_cache_custody_directory(
        &directory,
        Path::new("classified"),
        &canonical_cache.join("classified"),
        &classified,
        CacheCustodyKind::Git,
    )
    .expect_err("cache custody must reject a different concrete directory identity");
    assert!(matches!(
        error,
        SourceResolveError::GitCacheInvalid { message, .. }
            if message.contains("changed between classification")
    ));

    let _ = std::fs::remove_dir_all(&cache);
}

#[cfg(unix)]
#[test]
fn cache_custody_walk_remains_bound_to_open_root_after_path_replacement() {
    let cache = temp_root("cache-open-root-replacement");
    let retained = cache.with_extension("retained");
    std::fs::create_dir_all(&cache).expect("create cache root");
    let canonical_cache = cache.canonicalize().expect("canonicalize cache root");
    let directory =
        open_absolute_directory_nofollow(&canonical_cache).expect("open cache root capability");

    std::fs::rename(&cache, &retained).expect("relocate opened cache root");
    std::fs::create_dir_all(&cache).expect("create replacement cache root");
    std::fs::write(
        cache.join("replacement"),
        b"payload exceeding retained ceiling",
    )
    .expect("write replacement payload");

    verify_cache_custody_from_open_root(&canonical_cache, directory, CacheCustodyKind::Git, 3)
        .expect("custody walk must remain bound to the opened cache root");

    let _ = std::fs::remove_dir_all(&cache);
    let _ = std::fs::remove_dir_all(&retained);
}

#[cfg(unix)]
#[test]
fn local_snapshot_cache_rejects_group_or_other_writable_custody() {
    use std::os::unix::fs::PermissionsExt;

    let source = temp_root("local-custody-source");
    let cache = temp_root("local-custody-cache");
    std::fs::create_dir_all(&source).expect("create source");
    std::fs::write(source.join("main.omg"), b"machine main() { }").expect("write source");
    let storage = SourceResolverStorage::for_hardened_base(&cache)
        .expect("create retained local snapshot storage");
    resolve_local_source_snapshot_with_storage(&source, &storage, LocalSourceLimits::default())
        .expect("prime local snapshot cache");
    let local_lane = external_local_storage_lane(&cache);
    std::fs::set_permissions(&local_lane, std::fs::Permissions::from_mode(0o777))
        .expect("make cache externally writable");

    let error =
        resolve_local_source_snapshot_with_storage(&source, &storage, LocalSourceLimits::default())
            .expect_err("externally writable local cache custody must reject");
    assert!(matches!(
        error,
        SourceResolveError::LocalSnapshotInvalid { .. }
    ));

    std::fs::set_permissions(&local_lane, std::fs::Permissions::from_mode(0o700)).unwrap();
    drop(storage);
    let _ = std::fs::remove_dir_all(&source);
    let _ = std::fs::remove_dir_all(&cache);
}
