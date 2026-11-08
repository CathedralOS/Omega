use super::{
    CacheCustodyKind, SourceResolveError, read_bounded_cache_record, temp_root,
    verify_cache_custody,
};
use std::path::Path;

#[test]
fn cache_custody_rejects_logical_resident_byte_overflow() {
    let cache = temp_root("cache-byte-ceiling");
    std::fs::create_dir_all(&cache).expect("create cache");
    std::fs::write(cache.join("oversized"), b"12345").expect("write cache payload");

    assert!(matches!(
        verify_cache_custody(&cache, CacheCustodyKind::Git, 4),
        Err(SourceResolveError::GitCacheInvalid { .. })
    ));

    let _ = std::fs::remove_dir_all(&cache);
}

#[cfg(unix)]
#[test]
fn bounded_cache_record_read_does_not_follow_a_symlink_leaf() {
    let cache = temp_root("bounded-cache-record-symlink");
    std::fs::create_dir_all(&cache).expect("create cache record root");
    let target = cache.join("target");
    std::fs::write(&target, b"outside").expect("write cache record target");
    std::os::unix::fs::symlink(&target, cache.join("record")).expect("create cache record symlink");

    let error = read_bounded_cache_record(
        CacheCustodyKind::LocalSnapshot,
        &cache,
        Path::new("record"),
        64,
    )
    .expect_err("cache record read must not follow a symlink leaf");

    assert!(matches!(
        error,
        SourceResolveError::LocalSnapshotInvalid { .. }
    ));
    assert_eq!(std::fs::read(&target).expect("read target"), b"outside");
    let _ = std::fs::remove_dir_all(&cache);
}
