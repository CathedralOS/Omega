use super::{
    CacheCustodyKind, CacheEntryLock, OpenOptions, SourceResolveError, temp_root,
    verify_cache_lock_path_identity, verify_cache_lock_path_identity_for_test,
};
#[cfg(unix)]
use super::{GitExecutor, cache_invalid, reconcile_git_cache_operation_result};
#[cfg(unix)]
use std::path::Path;
#[cfg(unix)]
use std::time::{Duration, Instant};

#[test]
fn git_cache_serializes_access_without_unlinking_its_lock() {
    use std::sync::mpsc;
    use std::time::Duration;

    let cache = temp_root("git-lock");
    std::fs::create_dir_all(&cache).expect("create cache");
    let lock_path = cache.join("entry.lock");
    let first = CacheEntryLock::acquire(&lock_path).expect("acquire first lock");
    let thread_lock_path = lock_path.clone();
    let (sender, receiver) = mpsc::channel();
    let waiter = std::thread::spawn(move || {
        let second = CacheEntryLock::acquire(&thread_lock_path).expect("acquire serialized lock");
        sender.send(()).expect("report lock acquisition");
        drop(second);
    });

    assert!(receiver.recv_timeout(Duration::from_millis(100)).is_err());
    drop(first);
    receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("waiter should acquire released lock");
    waiter.join().expect("join lock waiter");
    assert!(lock_path.is_file());

    let _ = std::fs::remove_dir_all(&cache);
}

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

#[test]
fn local_cache_rejects_a_replaced_locked_path() {
    let cache = temp_root("local-lock-replaced");
    std::fs::create_dir_all(&cache).expect("create cache");
    let lock_path = cache.join("entry.lock");
    let displaced_path = cache.join("entry.lock.displaced");
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .expect("open local cache lock");
    file.lock().expect("lock local cache entry");
    std::fs::rename(&lock_path, &displaced_path).expect("displace locked path");
    std::fs::write(&lock_path, []).expect("replace lock path");

    assert!(matches!(
        verify_cache_lock_path_identity_for_test(
            CacheCustodyKind::LocalSnapshot,
            &lock_path,
            &file,
        ),
        Err(SourceResolveError::LocalSnapshotInvalid { .. })
    ));

    file.unlock().expect("unlock displaced cache entry");
    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn cache_lock_open_does_not_follow_a_preexisting_symlink() {
    let root = temp_root("cache-lock-symlink");
    std::fs::create_dir_all(&root).expect("create cache lock root");
    let target = root.join("target");
    std::fs::write(&target, b"untouched").expect("create symlink target");

    for (name, kind) in [
        ("git.lock", CacheCustodyKind::Git),
        ("local.lock", CacheCustodyKind::LocalSnapshot),
    ] {
        let lock_path = root.join(name);
        #[cfg(unix)]
        std::os::unix::fs::symlink(&target, &lock_path).expect("create cache lock symlink");
        #[cfg(windows)]
        std::os::windows::fs::symlink_file(&target, &lock_path).expect("create cache lock symlink");
        let error = CacheEntryLock::open_retained(kind, &lock_path)
            .expect_err("cache lock open must not follow a symlink");
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
        std::fs::remove_file(&lock_path).expect("remove cache lock symlink");
    }
    assert_eq!(
        std::fs::read(&target).expect("read symlink target"),
        b"untouched"
    );
    let _ = std::fs::remove_dir_all(&root);
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
    let held = CacheEntryLock::acquire_local_with_timeout(&lock_path, Duration::from_secs(1))
        .expect("hold local cache lock");
    let timeout = Duration::from_millis(30);
    let started = Instant::now();

    let result = CacheEntryLock::acquire_local_with_timeout(&lock_path, timeout);

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
fn local_cache_lock_acquires_after_the_competing_handle_releases() {
    let root = temp_root("local-lock-release");
    std::fs::create_dir_all(&root).expect("create lock release root");
    let lock_path = root.join("entry.lock");
    let held = CacheEntryLock::acquire_local_with_timeout(&lock_path, Duration::from_secs(1))
        .expect("hold local cache lock");
    drop(held);

    CacheEntryLock::acquire_local_with_timeout(&lock_path, Duration::from_secs(1))
        .expect("released local cache lock must become available");

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
        CacheEntryLock::acquire_with_budget(&lock_path, &executor),
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

#[cfg(unix)]
#[test]
fn failed_operation_still_reconciles_the_retained_lock_parent() {
    let cache = temp_root("git-failed-operation-parent-reconciliation");
    let retained = cache.with_extension("retained");
    std::fs::create_dir_all(&cache).expect("create cache parent");
    let cache = cache.canonicalize().expect("canonicalize cache parent");
    let lock_path = cache.join("entry.lock");
    let lock = CacheEntryLock::acquire(&lock_path).expect("acquire retained cache lock");

    std::fs::rename(&cache, &retained).expect("replace retained cache parent path");
    std::fs::create_dir(&cache).expect("create replacement cache parent");
    let operation = Err::<(), _>(SourceResolveError::Git {
        operation: "test".to_owned(),
        status: Some(1),
        stderr: "native operation failed".to_owned(),
    });
    let error = reconcile_git_cache_operation_result(operation, lock.verify_path_identity(), None)
        .expect_err("post-operation namespace reconciliation must still run");

    assert!(matches!(
        error,
        SourceResolveError::GitCacheInvalid { path, message }
            if path == cache && message.contains("retained directory")
    ));
    drop(lock);
    let _ = std::fs::remove_dir_all(&cache);
    let _ = std::fs::remove_dir_all(&retained);
}
