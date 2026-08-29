use super::super::{
    CACHE_CUSTODY_ENTRY_LIMIT, CACHE_CUSTODY_FIXED_BYTE_ALLOWANCE, CacheCustodyKind,
    GIT_CACHE_CUSTODY_ABSOLUTE_BYTE_LIMIT, LOCAL_CACHE_CUSTODY_ABSOLUTE_BYTE_LIMIT,
    LocalSourceLimits, SourceResolveError, cache_custody_has_capacity,
    git_cache_custody_byte_limit, local_cache_custody_byte_limit, read_bounded_cache_record,
    temp_root, verify_cache_custody,
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

#[test]
fn cache_custody_walk_returns_exact_accepted_measurements() {
    let cache = temp_root("cache-accepted-measurements");
    std::fs::create_dir_all(cache.join("nested")).expect("create cache tree");
    std::fs::write(cache.join("nested/payload"), b"12345").expect("write cache payload");

    let measurement = verify_cache_custody(&cache, CacheCustodyKind::Git, 5)
        .expect("measure accepted cache custody");

    assert_eq!(measurement.entry_count, 3, "root, directory, and file");
    assert_eq!(measurement.logical_bytes, 5);
    assert_eq!(measurement.maximum_depth, 1);

    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn bounded_cache_record_read_rejects_content_above_its_exact_limit() {
    let cache = temp_root("bounded-cache-record");
    std::fs::create_dir_all(&cache).expect("create cache record root");
    std::fs::write(cache.join("record"), b"12345").expect("write oversized cache record");

    let error = read_bounded_cache_record(CacheCustodyKind::Git, &cache, Path::new("record"), 4)
        .expect_err("oversized cache record must reject before unbounded allocation");

    assert!(matches!(error, SourceResolveError::GitCacheInvalid { .. }));
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

#[test]
fn cache_custody_entry_capacity_accepts_the_exact_ceiling_only() {
    assert!(cache_custody_has_capacity(CACHE_CUSTODY_ENTRY_LIMIT - 1, 0));
    assert!(!cache_custody_has_capacity(CACHE_CUSTODY_ENTRY_LIMIT, 0));
    assert!(!cache_custody_has_capacity(usize::MAX, 1));
}

#[test]
fn cache_custody_wide_tree_does_not_retain_one_handle_per_sibling() {
    let cache = temp_root("cache-wide-directory");
    std::fs::create_dir_all(&cache).expect("create cache root");
    for index in 0..1_024 {
        std::fs::create_dir(cache.join(format!("directory-{index:04}")))
            .expect("create sibling cache directory");
    }
    let cache = cache.canonicalize().expect("canonicalize cache root");

    verify_cache_custody(&cache, CacheCustodyKind::Git, 0)
        .expect("wide custody walk must retain paths rather than sibling handles");

    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn cache_custody_byte_ceilings_are_source_scaled_and_absolutely_capped() {
    let small = LocalSourceLimits {
        max_bytes: 1024,
        ..LocalSourceLimits::default()
    };
    assert_eq!(
        git_cache_custody_byte_limit(small),
        CACHE_CUSTODY_FIXED_BYTE_ALLOWANCE + 3 * 1024
    );
    assert_eq!(
        local_cache_custody_byte_limit(small),
        CACHE_CUSTODY_FIXED_BYTE_ALLOWANCE + 1024
    );

    let unbounded_input = LocalSourceLimits {
        max_bytes: u64::MAX,
        ..LocalSourceLimits::default()
    };
    assert_eq!(
        git_cache_custody_byte_limit(unbounded_input),
        GIT_CACHE_CUSTODY_ABSOLUTE_BYTE_LIMIT
    );
    assert_eq!(
        local_cache_custody_byte_limit(unbounded_input),
        LOCAL_CACHE_CUSTODY_ABSOLUTE_BYTE_LIMIT
    );
}
