use super::*;

#[test]
fn git_cache_identity_is_full_policy_versioned_and_injectively_framed() {
    let first = git_cache_identity("a\0b", "c", GitExecutionTransport::Https);
    let second = git_cache_identity("a", "b\0c", GitExecutionTransport::Https);

    assert_eq!(first.len(), 64);
    assert!(first.chars().all(|character| character.is_ascii_hexdigit()));
    assert_ne!(first, second);
    assert_ne!(
        first,
        git_cache_identity("a\0b", "C", GitExecutionTransport::Https)
    );
    assert_ne!(
        first,
        git_cache_identity("a\0b", "c", GitExecutionTransport::Ssh)
    );
}

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
        std::os::unix::fs::symlink(&target, &lock_path).expect("create cache lock symlink");
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
        CacheEntryLock::acquire_with_git_budget(&lock_path, &executor),
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

#[test]
fn provisional_git_cache_directory_is_cleaned_if_retention_fails() {
    let cache = temp_root("git-provisional-stage-cleanup");
    std::fs::create_dir_all(&cache).expect("create provisional cache parent");
    let cache = cache.canonicalize().expect("canonicalize cache parent");
    let parent = open_absolute_directory_nofollow(&cache).expect("retain cache parent");
    create_private_cache_directory(&parent, "provisional")
        .expect("create provisional cache directory");
    {
        let _provisional = ProvisionalCacheDirectory::new(&parent, OsStr::new("provisional"));
        // Returning from a failed retention path drops this guard while it
        // still owns the just-created parent-relative name.
    }
    assert!(!cache.join("provisional").exists());
    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn git_cache_rejects_resolver_metadata_substitution() {
    let (repo, _) = create_git_source("git-metadata-source");
    let (substitute, _) = create_git_source("git-metadata-substitute");
    let cache = temp_root("git-metadata-cache");
    let substitute_url = substitute.display().to_string();
    let request = local_git_request(&repo, "HEAD");
    resolve_git_source(&request, &cache, LocalSourceLimits::default()).expect("prime cache");
    let entry = git_cache_entry_root(&cache, &request);
    std::fs::write(
        entry.join(GIT_CACHE_METADATA),
        git_cache_metadata(&substitute_url, "HEAD", GitExecutionTransport::File),
    )
    .expect("substitute metadata");

    let error = resolve_git_source(&request, &cache, LocalSourceLimits::default())
        .expect_err("substituted metadata must reject");

    assert!(matches!(error, SourceResolveError::GitCacheInvalid { .. }));
    assert!(!entry.join(GIT_CACHE_METADATA).exists());
    let _ = std::fs::remove_dir_all(&repo);
    let _ = std::fs::remove_dir_all(&substitute);
    let _ = std::fs::remove_dir_all(&cache);
}

#[cfg(unix)]
#[test]
fn git_cache_invalidation_does_not_follow_a_substituted_entry_symlink() {
    let cache = temp_root("git-invalidation-symlink");
    let target = cache.join("target");
    let entry = cache.join("git-substituted");
    std::fs::create_dir_all(&target).expect("create invalidation target");
    let target_metadata = target.join(GIT_CACHE_METADATA);
    std::fs::write(&target_metadata, b"must remain").expect("write target metadata");
    std::os::unix::fs::symlink(&target, &entry).expect("substitute Git cache entry");

    let error = invalidate_git_cache_entry_from_retained_parent(&entry)
        .expect_err("invalidation must reject a substituted entry symlink");

    assert!(matches!(error, SourceResolveError::GitCacheInvalid { .. }));
    assert_eq!(
        std::fs::read(&target_metadata).expect("read retained target metadata"),
        b"must remain"
    );
    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn git_cache_rejects_transport_profile_substitution() {
    let (repo, _) = create_git_source("git-transport-metadata-source");
    let cache = temp_root("git-transport-metadata-cache");
    let request = local_git_request(&repo, "HEAD");
    resolve_git_source(&request, &cache, LocalSourceLimits::default()).expect("prime cache");
    let entry = git_cache_entry_root(&cache, &request);
    std::fs::write(
        entry.join(GIT_CACHE_METADATA),
        git_cache_metadata(
            request.locator_identity(),
            request.requested_revision(),
            GitExecutionTransport::Https,
        ),
    )
    .expect("substitute transport profile metadata");

    let error = resolve_git_source(&request, &cache, LocalSourceLimits::default())
        .expect_err("substituted transport profile must reject");

    assert!(matches!(error, SourceResolveError::GitCacheInvalid { .. }));
    assert!(!entry.join(GIT_CACHE_METADATA).exists());
    let _ = std::fs::remove_dir_all(&repo);
    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn git_cache_rejects_repository_config_substitution_without_asking_git() {
    let (repo, _) = create_git_source("git-origin-source");
    let cache = temp_root("git-origin-cache");
    let request = local_git_request(&repo, "HEAD");
    resolve_git_source(&request, &cache, LocalSourceLimits::default()).expect("prime cache");
    let repository = git_cache_entry_root(&cache, &request).join(GIT_CACHE_REPOSITORY);
    let config = repository.join("config");
    assert_eq!(std::fs::read(&config).unwrap(), GIT_CONFIG_SHA1);
    let mut substituted = GIT_CONFIG_SHA1.to_vec();
    substituted.extend_from_slice(b"[remote \"origin\"]\n\turl = /substitute\n");
    std::fs::write(&config, substituted).expect("substitute repository config");
    let entry = git_cache_entry_root(&cache, &request);

    let error = resolve_git_source(&request, &cache, LocalSourceLimits::default())
        .expect_err("any noncanonical repository configuration must reject");

    assert!(matches!(error, SourceResolveError::GitCacheInvalid { .. }));
    assert!(!entry.join(GIT_CACHE_METADATA).exists());
    let _ = std::fs::remove_dir_all(&repo);
    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn verified_git_repository_rejects_replaced_repository_path() {
    let (repo, _) = create_git_source("git-retained-repository-source");
    let cache = temp_root("git-retained-repository-cache");
    let request = local_git_request(&repo, "HEAD");
    resolve_git_source(&request, &cache, LocalSourceLimits::default()).expect("prime cache");
    let verified = open_verified_git_repository(&cache, &request);
    let repository = verified.path().to_path_buf();
    let displaced = repository.with_file_name("repository.displaced");
    std::fs::rename(&repository, &displaced).expect("displace retained repository");
    std::fs::create_dir_all(repository.join("objects")).expect("create replacement repository");

    let error = verified
        .verify_identity()
        .expect_err("repository replacement must reject");
    assert!(matches!(error, SourceResolveError::GitCacheInvalid { .. }));

    let _ = std::fs::remove_dir_all(&repo);
    make_tree_owner_writable(&cache);
    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn verified_git_repository_rejects_replaced_objects_path() {
    let (repo, _) = create_git_source("git-retained-objects-source");
    let cache = temp_root("git-retained-objects-cache");
    let request = local_git_request(&repo, "HEAD");
    resolve_git_source(&request, &cache, LocalSourceLimits::default()).expect("prime cache");
    let verified = open_verified_git_repository(&cache, &request);
    let objects = verified.path().join("objects");
    let displaced = verified.path().join("objects.displaced");
    std::fs::rename(&objects, &displaced).expect("displace retained object store");
    std::fs::create_dir(&objects).expect("create replacement object store");

    let error = verified
        .verify_identity()
        .expect_err("object-store replacement must reject");
    assert!(matches!(error, SourceResolveError::GitCacheInvalid { .. }));

    let _ = std::fs::remove_dir_all(&repo);
    make_tree_owner_writable(&cache);
    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn git_cache_forbidden_record_probe_rejects_non_not_found_errors() {
    let (repo, _) = create_git_source("git-forbidden-probe-source");
    let cache = temp_root("git-forbidden-probe-cache");
    let request = local_git_request(&repo, "HEAD");
    resolve_git_source(&request, &cache, LocalSourceLimits::default()).expect("prime cache");
    let entry = git_cache_entry_root(&cache, &request);
    let info = entry.join(GIT_CACHE_REPOSITORY).join("objects/info");
    std::fs::remove_dir(&info).expect("remove empty Git info directory");
    std::fs::write(&info, b"not a directory").expect("replace info with a regular file");

    let error = resolve_git_source(&request, &cache, LocalSourceLimits::default())
        .expect_err("NotADirectory must not prove a forbidden record absent");
    assert!(matches!(error, SourceResolveError::GitCacheInvalid { .. }));

    let _ = std::fs::remove_dir_all(&repo);
    make_tree_owner_writable(&cache);
    let _ = std::fs::remove_dir_all(&cache);
}

#[cfg(unix)]
#[test]
fn git_cache_rejects_symlinks_in_owned_repository_namespaces() {
    for relative in ["config", "FETCH_HEAD", "HEAD"] {
        let (repo, _) = create_git_source(&format!("git-symlink-{relative}-source"));
        let cache = temp_root(&format!("git-symlink-{relative}-cache"));
        let request = local_git_request(&repo, "HEAD");
        resolve_git_source(&request, &cache, LocalSourceLimits::default()).expect("prime cache");
        let entry = git_cache_entry_root(&cache, &request);
        let repository = entry.join(GIT_CACHE_REPOSITORY);
        let path = repository.join(relative);
        let displaced = repository.join(format!("{relative}.displaced"));
        std::fs::rename(&path, &displaced).expect("displace repository file");
        std::os::unix::fs::symlink(&displaced, &path).expect("install repository symlink");

        let error = resolve_git_source(&request, &cache, LocalSourceLimits::default())
            .expect_err("repository symlink must reject");
        assert!(matches!(error, SourceResolveError::GitCacheInvalid { .. }));

        let _ = std::fs::remove_dir_all(&repo);
        make_tree_owner_writable(&cache);
        let _ = std::fs::remove_dir_all(&cache);
    }

    let (repo, _) = create_git_source("git-symlink-object-source");
    let cache = temp_root("git-symlink-object-cache");
    let request = local_git_request(&repo, "HEAD");
    resolve_git_source(&request, &cache, LocalSourceLimits::default()).expect("prime cache");
    let repository = git_cache_entry_root(&cache, &request).join(GIT_CACHE_REPOSITORY);
    let object = first_regular_descendant(&repository.join("objects"));
    let displaced = object.with_extension("displaced");
    std::fs::rename(&object, &displaced).expect("displace object payload");
    std::os::unix::fs::symlink(&displaced, &object).expect("install object symlink");

    let error = resolve_git_source(&request, &cache, LocalSourceLimits::default())
        .expect_err("object-store symlink must reject");
    assert!(matches!(error, SourceResolveError::GitCacheInvalid { .. }));

    let _ = std::fs::remove_dir_all(&repo);
    make_tree_owner_writable(&cache);
    let _ = std::fs::remove_dir_all(&cache);
}

#[cfg(unix)]
#[test]
fn git_cache_rejects_multiply_linked_regular_files() {
    let (repo, _) = create_git_source("git-hardlink-source");
    let cache = temp_root("git-hardlink-cache");
    let request = local_git_request(&repo, "HEAD");
    resolve_git_source(&request, &cache, LocalSourceLimits::default()).expect("prime cache");
    let entry = git_cache_entry_root(&cache, &request);
    let config = entry.join(GIT_CACHE_REPOSITORY).join("config");
    std::fs::hard_link(&config, cache.join("config-alias"))
        .expect("add external hard link to repository file");

    let error = resolve_git_source(&request, &cache, LocalSourceLimits::default())
        .expect_err("multiply-linked repository file must reject");
    assert!(matches!(error, SourceResolveError::GitCacheInvalid { .. }));

    let _ = std::fs::remove_dir_all(&repo);
    make_tree_owner_writable(&cache);
    let _ = std::fs::remove_dir_all(&cache);
}

#[cfg(unix)]
#[test]
fn git_cache_rejects_group_or_other_writable_custody() {
    use std::os::unix::fs::PermissionsExt;

    let (repo, _) = create_git_source("git-custody-source");
    let cache = temp_root("git-custody-cache");
    let request = local_git_request(&repo, "HEAD");
    resolve_git_source(&request, &cache, LocalSourceLimits::default()).expect("prime cache");
    std::fs::set_permissions(&cache, std::fs::Permissions::from_mode(0o777))
        .expect("make cache externally writable");

    let error = resolve_git_source(&request, &cache, LocalSourceLimits::default())
        .expect_err("externally writable cache custody must reject");
    assert!(matches!(error, SourceResolveError::GitCacheInvalid { .. }));

    std::fs::set_permissions(&cache, std::fs::Permissions::from_mode(0o700)).unwrap();
    let _ = std::fs::remove_dir_all(&repo);
    let _ = std::fs::remove_dir_all(&cache);
}

#[cfg(unix)]
#[test]
fn cache_custody_rejects_replaceable_nonsticky_ancestry() {
    use std::os::unix::fs::PermissionsExt;

    let parent = temp_root("replaceable-cache-parent");
    let cache = parent.join("cache");
    std::fs::create_dir_all(&cache).expect("create nested cache");
    std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o777))
        .expect("make parent replaceable");
    std::fs::set_permissions(&cache, std::fs::Permissions::from_mode(0o700))
        .expect("keep cache itself private");

    assert!(matches!(
        verify_git_cache_root_custody(&cache),
        Err(SourceResolveError::GitCacheInvalid { .. })
    ));

    std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o700)).unwrap();
    let _ = std::fs::remove_dir_all(&parent);
}

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

#[test]
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

    std::fs::rename(&publication, &retained).expect("replace publication root path");
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
    )
    .expect("publish through retained parent handle");

    assert!(retained.join("published").is_dir());
    assert!(!cache.join("published").exists());
    let _ = std::fs::remove_dir_all(&root);
}

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

#[cfg(target_os = "macos")]
#[test]
fn cache_custody_acl_observation_remains_bound_to_open_root() {
    let cache = temp_root("cache-open-root-acl-replacement");
    let retained = cache.with_extension("retained");
    std::fs::create_dir_all(&cache).expect("create cache root");
    let canonical_cache = cache.canonicalize().expect("canonicalize cache root");
    let directory =
        open_absolute_directory_nofollow(&canonical_cache).expect("open cache root capability");

    std::fs::rename(&cache, &retained).expect("relocate opened cache root");
    std::fs::create_dir_all(&cache).expect("create replacement cache root");
    change_macos_acl(&cache, &["+a", "everyone allow write"]);

    verify_cache_custody_from_open_root(&canonical_cache, directory, CacheCustodyKind::Git, 0)
        .expect("ACL observation must remain on the retained cache root");

    change_macos_acl(&cache, &["-N"]);
    let _ = std::fs::remove_dir_all(&cache);
    let _ = std::fs::remove_dir_all(&retained);
}

#[cfg(target_os = "macos")]
#[test]
fn cache_ancestry_acl_open_rejects_classified_directory_replacement() {
    let cache = temp_root("cache-ancestry-acl-replacement");
    let retained = cache.with_extension("retained");
    std::fs::create_dir_all(&cache).expect("create classified cache directory");
    let cache = cache.canonicalize().expect("canonicalize cache directory");
    let classified =
        std::fs::symlink_metadata(&cache).expect("classify cache directory before replacement");

    std::fs::rename(&cache, &retained).expect("relocate classified cache directory");
    std::fs::create_dir(&cache).expect("create replacement cache directory");
    change_macos_acl(&cache, &["+a", "everyone allow write"]);

    let error =
        verify_macos_open_cache_directory_acl_custody(CacheCustodyKind::Git, &cache, &classified)
            .expect_err("different directory identity must reject before its ACL can contribute");
    assert!(matches!(
        error,
        SourceResolveError::GitCacheInvalid { message, .. }
            if message.contains("changed between classification")
    ));

    change_macos_acl(&cache, &["-N"]);
    let _ = std::fs::remove_dir_all(&cache);
    let _ = std::fs::remove_dir_all(&retained);
}

#[cfg(target_os = "macos")]
#[test]
fn cache_custody_rejects_extended_acl_allow_entries_on_root_and_nodes() {
    use std::os::unix::fs::PermissionsExt;

    let cache = temp_root("cache-acl-custody");
    std::fs::create_dir_all(&cache).expect("create cache");
    let cache = cache.canonicalize().expect("canonicalize cache");
    std::fs::set_permissions(&cache, std::fs::Permissions::from_mode(0o700))
        .expect("make cache private");
    let payload = cache.join("payload");
    std::fs::write(&payload, b"custody").expect("write cache payload");
    std::fs::set_permissions(&payload, std::fs::Permissions::from_mode(0o600))
        .expect("make cache payload private");

    change_macos_acl(&cache, &["+a", "everyone allow write"]);
    let root_error = verify_cache_custody(&cache, CacheCustodyKind::Git, 1024)
        .expect_err("extended ACL allow on cache root must reject");
    assert!(matches!(
        &root_error,
        SourceResolveError::GitCacheInvalid { path, message }
            if path == &cache && message.contains("extended ACL allow")
    ));
    change_macos_acl(&cache, &["-N"]);

    change_macos_acl(&payload, &["+a", "everyone allow write"]);
    let node_error = verify_cache_custody(&cache, CacheCustodyKind::Git, 1024)
        .expect_err("extended ACL allow on cache node must reject");
    assert!(
        matches!(
            &node_error,
            SourceResolveError::GitCacheInvalid { path, message }
                if path == &payload && message.contains("extended ACL allow")
        ),
        "unexpected cache node ACL error: {node_error:?}"
    );
    change_macos_acl(&payload, &["-N"]);
    change_macos_acl(&payload, &["+a", "everyone deny write"]);
    verify_cache_custody(&cache, CacheCustodyKind::Git, 1024)
        .expect("deny-only ACL does not broaden cache custody");

    change_macos_acl(&payload, &["-N"]);
    let _ = std::fs::remove_dir_all(&cache);
}

#[cfg(target_os = "macos")]
#[test]
fn cache_locks_reject_extended_acl_allow_entries() {
    let root = temp_root("cache-lock-acl-custody");
    std::fs::create_dir_all(&root).expect("create cache lock root");

    for (name, kind) in [
        ("git.lock", CacheCustodyKind::Git),
        ("local.lock", CacheCustodyKind::LocalSnapshot),
    ] {
        let path = root.join(name);
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .expect("open cache lock");
        change_macos_acl(&path, &["+a", "everyone allow write"]);
        let error = verify_cache_lock_path_identity_for_test(kind, &path, &file)
            .expect_err("extended ACL allow on cache lock must reject");
        assert!(
            matches!(
                (&kind, &error),
                (CacheCustodyKind::Git, SourceResolveError::GitCacheInvalid { message, .. })
                    | (
                        CacheCustodyKind::LocalSnapshot,
                        SourceResolveError::LocalSnapshotInvalid { message, .. }
                    ) if message.contains("extended ACL allow")
            ),
            "unexpected cache lock ACL error: {error:?}"
        );
        change_macos_acl(&path, &["-N"]);
    }

    let _ = std::fs::remove_dir_all(&root);
}

#[cfg(target_os = "macos")]
#[test]
fn git_cache_reuse_rejects_extended_acl_allow_entry() {
    let (repository, _) = create_git_source("git-cache-acl-source");
    let cache = temp_root("git-cache-acl-cache");
    let request = local_git_request(&repository, "HEAD");
    resolve_git_source(&request, &cache, LocalSourceLimits::default()).expect("prime cache");
    let cache = cache.canonicalize().expect("canonicalize Git cache");
    let entry = git_cache_entry_root(&cache, &request);
    change_macos_acl(&entry, &["+a", "everyone allow write"]);

    let error = resolve_git_source(&request, &cache, LocalSourceLimits::default())
        .expect_err("extended ACL allow on Git cache must reject reuse");
    assert!(
        matches!(
            &error,
            SourceResolveError::GitCacheInvalid { path, message }
                if path == &entry && message.contains("extended ACL allow")
        ),
        "unexpected Git cache ACL error: {error:?}"
    );

    let _ = std::fs::remove_dir_all(&repository);
    let _ = std::fs::remove_dir_all(&cache);
}

#[cfg(target_os = "macos")]
#[test]
fn local_snapshot_reuse_rejects_extended_acl_allow_entry() {
    let source = temp_root("local-cache-acl-source");
    let cache = temp_root("local-cache-acl-cache");
    std::fs::create_dir_all(&source).expect("create source");
    std::fs::write(source.join("main.omg"), b"machine main() { }").expect("write source");
    let resolved = resolve_local_source_snapshot(&source, &cache, LocalSourceLimits::default())
        .expect("prime local snapshot cache");
    let payload = resolved.snapshot_root.join("main.omg");
    change_macos_acl(&payload, &["+a", "everyone allow write"]);

    let error = resolve_local_source_snapshot(&source, &cache, LocalSourceLimits::default())
        .expect_err("extended ACL allow on local snapshot must reject reuse");
    assert!(matches!(
        &error,
        SourceResolveError::LocalSnapshotInvalid { path, message }
            if path == &payload && message.contains("extended ACL allow")
    ));

    change_macos_acl(&payload, &["-N"]);
    let _ = std::fs::remove_dir_all(&source);
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

#[cfg(unix)]
#[test]
fn local_snapshot_cache_rejects_group_or_other_writable_custody() {
    use std::os::unix::fs::PermissionsExt;

    let source = temp_root("local-custody-source");
    let cache = temp_root("local-custody-cache");
    std::fs::create_dir_all(&source).expect("create source");
    std::fs::write(source.join("main.omg"), b"machine main() { }").expect("write source");
    resolve_local_source_snapshot(&source, &cache, LocalSourceLimits::default())
        .expect("prime local snapshot cache");
    std::fs::set_permissions(&cache, std::fs::Permissions::from_mode(0o777))
        .expect("make cache externally writable");

    let error = resolve_local_source_snapshot(&source, &cache, LocalSourceLimits::default())
        .expect_err("externally writable local cache custody must reject");
    assert!(matches!(
        error,
        SourceResolveError::LocalSnapshotInvalid { .. }
    ));

    std::fs::set_permissions(&cache, std::fs::Permissions::from_mode(0o700)).unwrap();
    let _ = std::fs::remove_dir_all(&source);
    let _ = std::fs::remove_dir_all(&cache);
}
