use super::{
    LOCAL_CACHE_SNAPSHOTS, LOCAL_SNAPSHOT_METADATA, LocalSourceLimits, SourceContentDigest,
    SourceResolveError, SourceTreePolicy, VerifiedPackageSourceEntryKind, capture_local_source,
    capture_verified_package_source_snapshot, local_snapshot_custody_identity,
    make_snapshot_read_only, make_tree_owner_writable, publish_local_snapshot,
    resolve_local_source, resolve_local_source_snapshot_from_hardened_base,
    resolve_materialized_source, temp_root,
};
#[test]
fn local_snapshot_preserves_empty_directories_and_uses_published_identity() {
    let root = temp_root("local-snapshot-empty-directory");
    let cache = temp_root("local-snapshot-empty-directory-cache");
    std::fs::create_dir_all(root.join("generated/empty")).expect("create empty directory");
    std::fs::create_dir_all(root.join(".git")).expect("create excluded metadata");
    std::fs::create_dir_all(root.join("build")).expect("create excluded build output");
    std::fs::write(root.join("main.omg"), "machine Main::main() {}\n").expect("write source");
    std::fs::write(root.join(".git/index"), "excluded").expect("write Git metadata");
    std::fs::write(root.join("build/omega-program"), "excluded").expect("write build output");
    let live = resolve_local_source(&root, LocalSourceLimits::default()).expect("resolve live");

    let resolved = resolve_local_source_snapshot_from_hardened_base(
        &root,
        &cache,
        LocalSourceLimits::default(),
    )
    .expect("snapshot local source");

    assert_eq!(resolved.requested_root(), root);
    assert_eq!(resolved.canonical_live_root(), live.root);
    assert_ne!(resolved.snapshot_root(), resolved.canonical_live_root());
    assert_eq!(resolved.normalized().root, resolved.snapshot_root());
    assert!(resolved.snapshot_root().join("generated/empty").is_dir());
    assert!(!resolved.snapshot_root().join(".git").exists());
    assert!(!resolved.snapshot_root().join("build").exists());
    assert_eq!(resolved.normalized().file_count, 1);
    assert_eq!(resolved.normalized().byte_count, live.byte_count);
    assert_eq!(
        resolved.normalized().content_identity,
        live.content_identity
    );
    assert!(
        resolved
            .snapshot_root()
            .parent()
            .expect("publication root")
            .join(LOCAL_SNAPSHOT_METADATA)
            .is_file()
    );
    assert!(
        !resolved
            .snapshot_root()
            .join(LOCAL_SNAPSHOT_METADATA)
            .exists()
    );

    make_tree_owner_writable(&cache);
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn local_snapshot_detects_live_mutation_and_removes_staging_tree() {
    let root = temp_root("local-snapshot-mutation");
    let cache = temp_root("local-snapshot-mutation-cache");
    std::fs::create_dir_all(&root).expect("create source");
    std::fs::write(root.join("main.omg"), "machine Before::main() {}\n")
        .expect("write initial source");
    let captured = capture_local_source(
        &root,
        LocalSourceLimits::default(),
        SourceTreePolicy::LocalPackage,
    )
    .expect("capture source");
    let captured_custody_identity = local_snapshot_custody_identity(
        &captured.normalized.root,
        &captured.normalized.content_identity,
    );
    std::fs::write(root.join("main.omg"), "machine After::main() {}\n")
        .expect("mutate live source");

    let error =
        publish_local_snapshot(root.clone(), captured, &cache, LocalSourceLimits::default())
            .expect_err("concurrent mutation must reject");
    assert!(matches!(
        error,
        SourceResolveError::LocalSourceChanged { .. }
    ));
    let snapshots = cache.join(LOCAL_CACHE_SNAPSHOTS);
    assert!(
        !snapshots
            .join(format!("source-{captured_custody_identity}"))
            .exists()
    );
    assert!(
        std::fs::read_dir(&snapshots)
            .expect("read snapshot collection")
            .all(|entry| !entry
                .expect("snapshot collection entry")
                .file_name()
                .to_string_lossy()
                .contains(".stage-"))
    );

    make_tree_owner_writable(&cache);
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&cache);
}

#[cfg(unix)]
#[test]
fn local_snapshot_reuse_rehashes_and_rejects_tampering() {
    use std::os::unix::fs::PermissionsExt;

    let root = temp_root("local-snapshot-reuse");
    let cache = temp_root("local-snapshot-reuse-cache");
    std::fs::create_dir_all(&root).expect("create source");
    std::fs::write(root.join("main.omg"), "machine Main::main() {}\n").expect("write source");

    let first = resolve_local_source_snapshot_from_hardened_base(
        &root,
        &cache,
        LocalSourceLimits::default(),
    )
    .expect("publish snapshot");
    let second = resolve_local_source_snapshot_from_hardened_base(
        &root,
        &cache,
        LocalSourceLimits::default(),
    )
    .expect("reuse snapshot");
    assert_eq!(first, second);

    std::fs::set_permissions(
        first.snapshot_root(),
        std::fs::Permissions::from_mode(0o755),
    )
    .expect("make snapshot root writable");
    let source = first.snapshot_root().join("main.omg");
    std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o644))
        .expect("make snapshot file writable");
    std::fs::write(&source, "machine Tampered::main() {}\n").expect("tamper snapshot");

    let error = resolve_local_source_snapshot_from_hardened_base(
        &root,
        &cache,
        LocalSourceLimits::default(),
    )
    .expect_err("tampered snapshot must reject");
    assert!(matches!(
        error,
        SourceResolveError::LocalSnapshotInvalid { .. }
    ));

    make_tree_owner_writable(&cache);
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn verified_published_capture_keeps_root_build_directory() {
    let root = temp_root("verified-exact-build-directory");
    std::fs::create_dir_all(root.join("build/nested")).expect("create exact source tree");
    std::fs::write(root.join("main.omg"), b"machine main() {}\n").expect("write source");
    std::fs::write(
        root.join("build/nested/generated.omg"),
        b"const VALUE: u8 = 1;\n",
    )
    .expect("write exact root build entry");
    let normalized = resolve_materialized_source(&root, LocalSourceLimits::default())
        .expect("derive exact materialized identity");
    let expected = SourceContentDigest::derive(normalized.content_identity.as_bytes());
    make_snapshot_read_only(&root).expect("make exact source tree read-only");

    let captured =
        capture_verified_package_source_snapshot(&root, &expected, LocalSourceLimits::default())
            .expect("capture exact published source");

    assert!(captured.iter().any(|entry| {
        entry.relative_path == b"build/nested/generated.omg"
            && matches!(entry.kind, VerifiedPackageSourceEntryKind::File { .. })
    }));
    make_tree_owner_writable(&root);
    let _ = std::fs::remove_dir_all(&root);
}
