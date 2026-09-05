use super::*;

#[test]
fn local_admissions_updates_leave_source_identity_and_snapshot_unchanged() {
    let root = temp_root("local-admissions-control-file");
    let cache = temp_root("local-admissions-control-cache");
    std::fs::create_dir_all(root.join("nested")).unwrap();
    std::fs::write(root.join("main.omg"), "source").unwrap();
    std::fs::write(root.join("nested/omega.admissions"), "nested source").unwrap();
    let limits = LocalSourceLimits {
        max_entries: 3,
        max_bytes: 19,
        ..Default::default()
    };
    let original = resolve_local_source(&root, limits).unwrap();
    let snapshot = resolve_local_source_snapshot_at_path(&root, &cache, limits).unwrap();
    for contents in [
        "first accepted project state",
        "changed accepted project state",
    ] {
        std::fs::write(root.join("omega.admissions"), contents).unwrap();
        let changed = resolve_local_source(&root, limits).unwrap();
        assert_eq!(original, changed);
        let reused = resolve_local_source_snapshot_at_path(&root, &cache, limits).unwrap();
        assert_eq!(snapshot, reused);
        assert!(!reused.snapshot_root().join("omega.admissions").exists());
        assert_eq!(
            std::fs::read(reused.snapshot_root().join("nested/omega.admissions")).unwrap(),
            b"nested source"
        );
    }
    std::fs::remove_file(root.join("omega.admissions")).unwrap();
    assert_eq!(original, resolve_local_source(&root, limits).unwrap());
    for name in ["OMEGA.ADMISSIONS", "Omega.Admissions", "oMeGa.aDmIsSiOnS"] {
        std::fs::write(root.join(name), "case variant of compiler policy").unwrap();
        assert_eq!(original, resolve_local_source(&root, limits).unwrap());
        let reused = resolve_local_source_snapshot_at_path(&root, &cache, limits).unwrap();
        assert_eq!(snapshot, reused);
        assert!(!reused.snapshot_root().join(name).exists());
        std::fs::remove_file(root.join(name)).unwrap();
    }
    std::fs::write(root.join("nested/omega.admissions"), "different").unwrap();
    assert_ne!(
        original.content_identity,
        resolve_local_source(&root, limits)
            .unwrap()
            .content_identity
    );
    make_tree_owner_writable(&cache);
    std::fs::remove_dir_all(root).unwrap();
    std::fs::remove_dir_all(cache).unwrap();
}

#[test]
fn exact_materialized_trees_keep_and_hash_their_admissions_files() {
    let root = temp_root("exact-admissions-source-file");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("main.omg"), "source").unwrap();
    std::fs::write(root.join("omega.admissions"), "first").unwrap();
    let first = resolve_materialized_source(&root, LocalSourceLimits::default()).unwrap();
    std::fs::write(root.join("omega.admissions"), "second").unwrap();
    let second = resolve_materialized_source(&root, LocalSourceLimits::default()).unwrap();
    assert_eq!(first.file_count, 2);
    assert_eq!(second.file_count, 2);
    assert_ne!(first.content_identity, second.content_identity);
    assert_eq!(
        resolve_local_source(&root, LocalSourceLimits::default())
            .unwrap()
            .file_count,
        1
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn local_symlinks_cannot_reintroduce_root_admissions_contents() {
    let root = temp_root("local-admissions-symlink");
    std::fs::create_dir_all(root.join("nested")).unwrap();
    let alias = root.join("nested/alias");
    for name in ["omega.admissions", "OMEGA.ADMISSIONS", "Omega.Admissions"] {
        std::fs::write(root.join(name), "compiler policy").unwrap();
        std::os::unix::fs::symlink(format!("../{name}"), &alias).unwrap();
        assert!(matches!(
            resolve_local_source(&root, LocalSourceLimits::default()),
            Err(SourceResolveError::SymlinkTargetsExcludedMetadata { .. })
        ));
        // Exact repository trees still capture the link and its policy target.
        assert_eq!(
            resolve_materialized_source(&root, LocalSourceLimits::default())
                .unwrap()
                .file_count,
            2
        );
        std::fs::remove_file(&alias).unwrap();
        std::fs::remove_file(root.join(name)).unwrap();
    }
    std::fs::write(root.join("nested/omega.admissions"), "ordinary source").unwrap();
    std::os::unix::fs::symlink("nested/omega.admissions", root.join("alias")).unwrap();
    assert_eq!(
        resolve_local_source(&root, LocalSourceLimits::default())
            .unwrap()
            .file_count,
        2
    );
    std::fs::remove_dir_all(root).unwrap();
}
