use super::{
    CacheCustodyKind, CacheEntryLock, SourceResolveError, temp_root,
    verify_cache_lock_path_identity, verify_cache_lock_path_identity_for_test,
};
#[cfg(unix)]
use super::{GitExecutor, cache_invalid, reconcile_git_cache_operation_result};
#[cfg(unix)]
use std::path::Path;
#[cfg(unix)]
use std::time::{Duration, Instant};

#[test]
fn git_cache_rejects_a_replaced_locked_path() {
    let cache = temp_root("git-lock-replaced");
    std::fs::create_dir_all(&cache).expect("create cache");
    let lock_path = cache.join("entry.lock");
    let displaced_path = cache.join("entry.lock.displaced");
    let file = CacheEntryLock::open_git(&lock_path).expect("open cache lock");
    file.lock().expect("lock cache entry");
    std::fs::rename(&lock_path, &displaced_path).expect("displace locked path");
    std::fs::write(&lock_path, []).expect("replace lock path");

    assert!(matches!(
        verify_cache_lock_path_identity_for_test(CacheCustodyKind::Git, &lock_path, &file),
        Err(SourceResolveError::GitCacheInvalid { .. })
    ));

    file.unlock().expect("unlock displaced cache entry");
    let _ = std::fs::remove_dir_all(&cache);
}

#[cfg(windows)]
fn create_test_symlink(target: &std::path::Path, link: &std::path::Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_file(target, link)
}

#[test]
#[cfg_attr(
    windows,
    ignore = "Windows prevents replacement while the retained parent handle is open"
)]
fn cache_lock_identity_rejects_a_replaced_parent_path() {
    for (name, kind) in [
        ("git", CacheCustodyKind::Git),
        ("local", CacheCustodyKind::LocalSnapshot),
    ] {
        let root = temp_root(&format!("cache-lock-parent-replaced-{name}"));
        let cache = root.join("cache");
        let retained = root.join("retained");
        std::fs::create_dir_all(&cache).expect("create cache lock parent");
        let lock_path = cache.join("entry.lock");
        let (file, parent, lock_name) = CacheEntryLock::open_retained(kind, &lock_path)
            .expect("open lock through retained parent");
        file.lock().expect("lock retained cache entry");

        std::fs::rename(&cache, &retained).expect("replace cache lock parent path");
        std::fs::create_dir(&cache).expect("create replacement cache lock parent");
        std::fs::write(cache.join("entry.lock"), []).expect("create replacement lock leaf");
        let error = verify_cache_lock_path_identity(kind, &lock_path, &parent, &lock_name, &file)
            .expect_err("replaced cache lock parent must reject");
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

        file.unlock().expect("unlock retained cache entry");
        let _ = std::fs::remove_dir_all(&root);
    }
}

#[cfg(unix)]
#[test]
fn local_cache_lock_wait_has_a_fail_closed_deadline() {
    let root = temp_root("local-lock-budget");
    std::fs::create_dir_all(&root).expect("create lock budget root");
    let lock_path = root.join("entry.lock");
    let held = CacheEntryLock::acquire_local(&lock_path, Duration::from_secs(1))
        .expect("hold local cache lock");
    let timeout = Duration::from_millis(30);
    let started = Instant::now();

    let result = CacheEntryLock::acquire_local(&lock_path, timeout);

    assert!(matches!(
        result,
        Err(SourceResolveError::LocalSnapshotLockTimedOut {
            ref path,
            timeout_millis: 30,
        }) if path == &lock_path
    ));
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "bounded local cache lock acquisition must not become an indefinite wait"
    );
    drop(held);
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn git_cache_lock_wait_obeys_the_whole_resolution_budget() {
    use std::os::unix::fs::PermissionsExt;

    let root = temp_root("git-lock-budget");
    std::fs::create_dir_all(&root).expect("create lock budget root");
    let lock_path = root.join("entry.lock");
    let held = CacheEntryLock::acquire(&lock_path).expect("hold cache lock");
    let fake_git = root.join("git");
    std::fs::write(&fake_git, b"#!/bin/sh\nexit 0\n").expect("write fake Git");
    std::fs::set_permissions(&fake_git, std::fs::Permissions::from_mode(0o700))
        .expect("make fake Git executable");
    let executor = GitExecutor::open_with_budget(&fake_git, 1, Duration::from_millis(30))
        .expect("capture time-bounded Git");

    assert!(matches!(
        CacheEntryLock::acquire_within(&lock_path, &executor),
        Err(SourceResolveError::GitResolutionTimedOut { .. })
    ));

    drop(held);
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn cache_namespace_and_invalidation_failures_outrank_operation_failure() {
    let operation = Err::<(), _>(SourceResolveError::Git {
        operation: "test".to_owned(),
        status: Some(1),
        stderr: "operation failed".to_owned(),
    });
    let namespace = Err(cache_invalid(
        Path::new("cache"),
        "namespace reconciliation failed",
    ));
    let error = reconcile_git_cache_operation_result(operation, namespace, None)
        .expect_err("namespace custody must outrank operation failure");
    assert!(matches!(
        error,
        SourceResolveError::GitCacheInvalid { message, .. }
            if message.contains("namespace reconciliation")
    ));

    let operation = Err::<(), _>(SourceResolveError::Git {
        operation: "test".to_owned(),
        status: Some(1),
        stderr: "operation failed".to_owned(),
    });
    let invalidation = Err(cache_invalid(
        Path::new("cache"),
        "invalidation synchronization failed",
    ));
    let error = reconcile_git_cache_operation_result(operation, Ok(()), Some(invalidation))
        .expect_err("invalidation custody must outrank operation failure");
    assert!(matches!(
        error,
        SourceResolveError::GitCacheInvalid { message, .. }
            if message.contains("invalidation synchronization")
    ));
}
