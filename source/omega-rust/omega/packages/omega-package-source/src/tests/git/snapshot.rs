use super::*;

fn authenticated_single_file_member_tree(repository: &Path) -> (String, Vec<GitTreeEntry>) {
    let bytes = b"machine Main::main() {}\n".to_vec();
    let blob = run_test_git_with_input(repository, ["rev-parse", "HEAD:main.omg"], b"");
    let tree = run_test_git_with_input(repository, ["rev-parse", "HEAD^{tree}"], b"");
    let size = u64::try_from(bytes.len()).expect("test source size");
    let end = bytes.len();
    (
        tree,
        vec![GitTreeEntry {
            relative_bytes: b"main.omg".to_vec(),
            relative_path: PathBuf::from("main.omg"),
            oid: blob,
            size,
            kind: GitTreeEntryKind::File {
                executable: false,
                bytes: GitBlobBytes {
                    batch: Arc::new(bytes),
                    start: 0,
                    end,
                },
            },
        }],
    )
}

#[test]
fn authenticated_member_snapshot_publishes_directly_in_workspace_member_lane() {
    let (repository, _) = create_git_source("git-member-snapshot");
    let (tree, entries) = authenticated_single_file_member_tree(&repository);
    let storage_base = temp_root("git-member-snapshot-storage");
    std::fs::create_dir_all(&storage_base).expect("create retained storage base");
    let storage =
        SourceResolverStorage::for_hardened_base(&storage_base).expect("retain resolver storage");
    let executor =
        test_system_git_executor(GitExecutionTransport::Https).expect("system Git executor");

    let (snapshot_root, local) = publish_git_member_snapshot(
        &executor,
        storage.workspace_members(),
        &tree,
        entries,
        LocalSourceLimits::default(),
    )
    .expect("publish authenticated member tree");

    let publication = storage
        .workspace_members()
        .path()
        .join(format!("tree-{tree}"));
    assert_eq!(snapshot_root, publication.join(GIT_SNAPSHOT_SOURCE));
    assert_eq!(
        std::fs::read(snapshot_root.join("main.omg")).expect("read member snapshot"),
        b"machine Main::main() {}\n"
    );
    assert_eq!(local.file_count, 1);
    assert!(
        storage
            .workspace_members()
            .path()
            .join(format!("tree-{tree}.lock"))
            .is_file()
    );
    assert!(
        !storage
            .workspace_members()
            .path()
            .join("snapshots")
            .exists()
    );
    assert!(publication.join(GIT_SNAPSHOT_METADATA).is_file());

    drop(storage);
    let _ = std::fs::remove_dir_all(&repository);
    make_tree_owner_writable(&storage_base);
    let _ = std::fs::remove_dir_all(&storage_base);
}

#[cfg(unix)]
#[test]
fn authenticated_member_snapshot_reuse_rejects_tampered_publication() {
    use std::os::unix::fs::PermissionsExt;

    let (repository, _) = create_git_source("git-member-snapshot-reuse");
    let (tree, entries) = authenticated_single_file_member_tree(&repository);
    let storage_base = temp_root("git-member-snapshot-reuse-storage");
    std::fs::create_dir_all(&storage_base).expect("create retained storage base");
    let storage =
        SourceResolverStorage::for_hardened_base(&storage_base).expect("retain resolver storage");
    let executor =
        test_system_git_executor(GitExecutionTransport::Https).expect("system Git executor");

    let (first_root, first_local) = publish_git_member_snapshot(
        &executor,
        storage.workspace_members(),
        &tree,
        entries.clone(),
        LocalSourceLimits::default(),
    )
    .expect("publish member snapshot");
    let (reused_root, reused_local) = publish_git_member_snapshot(
        &executor,
        storage.workspace_members(),
        &tree,
        entries.clone(),
        LocalSourceLimits::default(),
    )
    .expect("reuse verified member snapshot");
    assert_eq!(reused_root, first_root);
    assert_eq!(reused_local, first_local);

    let publication = first_root.parent().expect("publication root");
    make_tree_owner_writable(publication);
    let payload = first_root.join("main.omg");
    std::fs::set_permissions(&payload, std::fs::Permissions::from_mode(0o644))
        .expect("make member payload writable");
    std::fs::write(&payload, b"machine Tampered::main() {}\n").expect("tamper member payload");
    make_snapshot_read_only(publication).expect("restore snapshot modes");

    let error = publish_git_member_snapshot(
        &executor,
        storage.workspace_members(),
        &tree,
        entries,
        LocalSourceLimits::default(),
    )
    .expect_err("reuse must verify member snapshot content");
    assert!(matches!(error, SourceResolveError::GitCacheInvalid { .. }));

    drop(storage);
    let _ = std::fs::remove_dir_all(&repository);
    make_tree_owner_writable(&storage_base);
    let _ = std::fs::remove_dir_all(&storage_base);
}

#[test]
fn authenticated_member_snapshot_rejects_replaced_workspace_member_lane() {
    let (repository, _) = create_git_source("git-member-snapshot-custody");
    let (tree, entries) = authenticated_single_file_member_tree(&repository);
    let storage_base = temp_root("git-member-snapshot-custody-storage");
    std::fs::create_dir_all(&storage_base).expect("create retained storage base");
    let storage =
        SourceResolverStorage::for_hardened_base(&storage_base).expect("retain resolver storage");
    let lane_path = storage.workspace_members().path().to_path_buf();
    let retained_path = lane_path.with_extension("retained");
    std::fs::rename(&lane_path, &retained_path).expect("move retained lane");
    std::fs::create_dir(&lane_path).expect("replace lane pathname");
    let executor =
        test_system_git_executor(GitExecutionTransport::Https).expect("system Git executor");

    let error = publish_git_member_snapshot(
        &executor,
        storage.workspace_members(),
        &tree,
        entries,
        LocalSourceLimits::default(),
    )
    .expect_err("replaced workspace-member lane must reject");
    assert!(error.to_string().contains("no longer identifies"));
    assert_eq!(
        std::fs::read_dir(&lane_path)
            .expect("read replacement lane")
            .count(),
        0,
        "publication must not enter a replacement lane"
    );

    drop(storage);
    let _ = std::fs::remove_dir_all(&repository);
    let _ = std::fs::remove_dir_all(&lane_path);
    make_tree_owner_writable(&storage_base);
    let _ = std::fs::remove_dir_all(&storage_base);
}

#[cfg(unix)]
#[test]
fn git_snapshot_preserves_paths_executable_modes_and_symlink_spelling() {
    use std::os::unix::fs::PermissionsExt;

    let (repo, _) = create_git_source("git-snapshot-kinds");
    let script = repo.join("tools/generate");
    std::fs::create_dir_all(script.parent().expect("script parent")).expect("create tools");
    std::fs::write(&script, "#!/bin/sh\n").expect("write script");
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
        .expect("mark script executable");
    std::os::unix::fs::symlink("generate", repo.join("tools/current"))
        .expect("create source symlink");
    run_test_git(&repo, ["add", "tools/generate", "tools/current"]);
    run_test_git(&repo, ["commit", "--quiet", "-m", "add exact entry kinds"]);
    let cache = temp_root("git-snapshot-kinds-cache");
    let request = local_git_request(&repo, "HEAD");

    let resolved =
        resolve_git_source(&request, &cache, LocalSourceLimits::default()).expect("resolve kinds");
    let published_script = resolved.snapshot_root.join("tools/generate");
    let published_link = resolved.snapshot_root.join("tools/current");

    assert_eq!(
        std::fs::read(&published_script).expect("read script"),
        b"#!/bin/sh\n"
    );
    assert_ne!(
        std::fs::metadata(&published_script)
            .expect("script metadata")
            .permissions()
            .mode()
            & 0o111,
        0
    );
    assert_eq!(
        raw_os_bytes(
            std::fs::read_link(&published_link)
                .expect("read published symlink")
                .as_os_str()
        ),
        b"generate"
    );
    assert_eq!(resolved.local.file_count, 3);
    assert_eq!(
        std::fs::metadata(resolved.snapshot_root.join("tools"))
            .expect("nested directory metadata")
            .permissions()
            .mode()
            & 0o7777,
        u32::from(CANONICAL_DIRECTORY_MODE)
    );
    let verified = resolve_git_source(&request, &cache, LocalSourceLimits::default())
        .expect("verify nested snapshot reuse");
    assert_eq!(resolved.local, verified.local);

    let _ = std::fs::remove_dir_all(&repo);
    make_tree_owner_writable(&cache);
    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn git_snapshot_uses_blob_bytes_not_checkout_attribute_conversions() {
    let (repo, _) = create_git_source("git-snapshot-attributes");
    std::fs::write(repo.join(".gitattributes"), "*.omg eol=crlf\n")
        .expect("write checkout conversion attribute");
    run_test_git(&repo, ["add", ".gitattributes"]);
    run_test_git(
        &repo,
        ["commit", "--quiet", "-m", "add checkout conversion"],
    );
    let cache = temp_root("git-snapshot-attributes-cache");
    let request = local_git_request(&repo, "HEAD");

    let resolved = resolve_git_source(&request, &cache, LocalSourceLimits::default())
        .expect("materialize object bytes");

    assert_eq!(
        std::fs::read(resolved.snapshot_root.join("main.omg")).expect("read snapshot blob"),
        b"machine Main::main() {}\n"
    );
    let _ = std::fs::remove_dir_all(&repo);
    make_tree_owner_writable(&cache);
    let _ = std::fs::remove_dir_all(&cache);
}

#[cfg(unix)]
#[test]
fn git_snapshot_reuse_rejects_content_with_forged_matching_metadata() {
    use std::os::unix::fs::PermissionsExt;

    let (repo, _) = create_git_source("git-snapshot-reuse");
    let cache = temp_root("git-snapshot-reuse-cache");
    let request = local_git_request(&repo, "HEAD");
    let first =
        resolve_git_source(&request, &cache, LocalSourceLimits::default()).expect("first resolve");
    let second =
        resolve_git_source(&request, &cache, LocalSourceLimits::default()).expect("reuse snapshot");
    assert_eq!(first.snapshot_root, second.snapshot_root);
    assert_eq!(first.local, second.local);

    std::fs::set_permissions(&first.snapshot_root, std::fs::Permissions::from_mode(0o755))
        .expect("make source root writable for tamper simulation");
    let source = first.snapshot_root.join("main.omg");
    std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o644))
        .expect("make source writable for tamper simulation");
    std::fs::write(&source, "machine Tampered::main() {}\n").expect("tamper snapshot");
    let publication = first
        .snapshot_root
        .parent()
        .expect("snapshot source has a publication parent");
    let metadata_path = publication.join(GIT_SNAPSHOT_METADATA);
    std::fs::set_permissions(&metadata_path, std::fs::Permissions::from_mode(0o644))
        .expect("make snapshot metadata writable for tamper simulation");
    let forged = resolve_materialized_source(&first.snapshot_root, LocalSourceLimits::default())
        .expect("derive the public identity an attacker could recompute");
    std::fs::write(&metadata_path, git_snapshot_metadata(&first.tree, &forged))
        .expect("forge matching snapshot metadata");
    make_snapshot_read_only(publication).expect("restore canonical snapshot modes");

    let error = resolve_git_source(&request, &cache, LocalSourceLimits::default())
        .expect_err("tampered snapshot and matching forged metadata must reject");
    assert!(matches!(error, SourceResolveError::GitCacheInvalid { .. }));
    let entry = git_cache_entry_root(&cache, &request);
    assert!(!entry.join(GIT_CACHE_METADATA).exists());

    let _ = std::fs::remove_dir_all(&repo);
    make_tree_owner_writable(&cache);
    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn git_batch_failure_precedes_snapshot_staging() {
    let (repo, _) = create_git_source("git-snapshot-cleanup");
    let cache = temp_root("git-snapshot-cleanup-cache");
    let request = local_git_request(&repo, "HEAD");
    resolve_git_source(&request, &cache, LocalSourceLimits::default()).expect("prime cache");
    let entry_root = git_cache_entry_root(&cache, &request);
    let repository = entry_root.join(GIT_CACHE_REPOSITORY);
    let missing_oid = "0000000000000000000000000000000000000000";
    let executor =
        test_system_git_executor(GitExecutionTransport::Https).expect("system Git executor");
    let mut entries = vec![GitTreeEntry {
        relative_bytes: b"missing.omg".to_vec(),
        relative_path: PathBuf::from("missing.omg"),
        oid: missing_oid.to_owned(),
        size: 1,
        kind: GitTreeEntryKind::File {
            executable: false,
            bytes: GitBlobBytes::empty(),
        },
    }];
    let error = read_git_blobs_batch_from_path(
        &executor,
        &repository,
        &mut entries,
        LocalSourceLimits::default(),
    )
    .expect_err("missing object must fail before staged materialization");
    assert!(matches!(error, SourceResolveError::GitTreeInvalid { .. }));
    let snapshots = entry_root.join(GIT_CACHE_SNAPSHOTS);
    assert!(
        std::fs::read_dir(&snapshots)
            .expect("read snapshots")
            .all(|entry| !entry
                .expect("snapshot entry")
                .file_name()
                .to_string_lossy()
                .contains(".stage-")),
        "failed materialization must leave no staging directory"
    );
    assert!(
        !snapshots
            .join("tree-1111111111111111111111111111111111111111")
            .exists()
    );

    let _ = std::fs::remove_dir_all(&repo);
    make_tree_owner_writable(&cache);
    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn git_snapshot_excludes_untracked_source_worktree_state() {
    let (repo, _) = create_git_source("git-untracked-source");
    let cache = temp_root("git-untracked-cache");
    std::fs::write(repo.join("injected.omg"), "machine Injected::main() {}\n")
        .expect("write untracked source state");
    let request = local_git_request(&repo, "HEAD");
    let resolved =
        resolve_git_source(&request, &cache, LocalSourceLimits::default()).expect("prime cache");

    assert!(!resolved.snapshot_root.join("injected.omg").exists());
    assert_eq!(resolved.local.file_count, 1);
    let _ = std::fs::remove_dir_all(&repo);
    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn git_source_rejects_submodule_manifest() {
    let (repo, commit) = create_git_source("git-submodule");
    let cache = temp_root("git-submodule-cache");
    let request = local_git_request(&repo, "HEAD");
    let initial = resolve_git_source(&request, &cache, LocalSourceLimits::default())
        .expect("resolve initial source");
    let snapshot_source = initial.snapshot_root.join("main.omg");
    let initial_snapshot = std::fs::read(&snapshot_source).expect("read initial snapshot");

    std::fs::write(repo.join(".gitmodules"), "[submodule \"dep\"]\n").expect("write gitmodules");
    std::fs::write(repo.join("main.omg"), "machine Main::changed() {}\n").expect("change source");
    run_test_git(&repo, ["add", ".gitmodules"]);
    run_test_git(&repo, ["add", "main.omg"]);
    run_test_git(&repo, ["commit", "--quiet", "-m", "submodule manifest"]);

    let error = resolve_git_source(&request, &cache, LocalSourceLimits::default())
        .expect_err("submodule manifest should reject");

    assert!(matches!(
        error,
        SourceResolveError::GitSubmodulesUnsupported { .. }
    ));
    assert_eq!(
        std::fs::read(&snapshot_source).expect("read snapshot after rejection"),
        initial_snapshot,
        "the fetched submodule tree must be rejected before materialization"
    );
    assert!(!initial.snapshot_root.join("../source.identity").exists());
    assert!(!commit.is_empty());
    let _ = std::fs::remove_dir_all(&repo);
    let _ = std::fs::remove_dir_all(&cache);
}
