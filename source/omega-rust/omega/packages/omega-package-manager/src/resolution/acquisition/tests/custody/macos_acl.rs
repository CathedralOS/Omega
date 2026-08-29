use super::super::{
    CacheCustodyKind, LocalSourceLimits, OpenOptions, SourceResolveError, change_macos_acl,
    create_git_source, git_cache_entry_root, local_git_request, open_absolute_directory_nofollow,
    resolve_git_source, resolve_local_source_snapshot, temp_root, verify_cache_custody,
    verify_cache_custody_from_open_root, verify_cache_lock_path_identity_for_test,
    verify_macos_open_cache_directory_acl_custody,
};

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
    let payload = resolved.snapshot_root().join("main.omg");
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
