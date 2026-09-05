use super::*;
use crate::local::staging::{StagedLocalSnapshot, stage_local_source_replacement_in_lane};
use crate::storage::{RetainedStorageLane, SourceResolverStorage};
use sha2::{Digest, Sha256};
use std::path::Path;

fn with_staging_source(name: &str, test: impl FnOnce(&Path, &RetainedStorageLane)) {
    let root = temp_root(name);
    let cache = temp_root(&format!("{name}-cache"));
    std::fs::create_dir_all(&root).expect("create source root");
    std::fs::write(root.join("main.omg"), b"before").expect("write original source");
    std::fs::write(root.join("other.omg"), b"kept").expect("write unrelated source");
    let storage = SourceResolverStorage::for_hardened_base(&cache).expect("retain storage");
    test(&root, storage.external_local_sources());
    drop(storage);
    make_tree_owner_writable(&cache);
    std::fs::remove_dir_all(&root).expect("remove live source");
    std::fs::remove_dir_all(&cache).expect("remove snapshot storage");
}

fn stage_main(root: &Path, lane: &RetainedStorageLane, replacement: &[u8]) -> StagedLocalSnapshot {
    stage_local_source_replacement_in_lane(
        root,
        &SourceRelativePath::parse("main.omg").expect("source path"),
        &Sha256::digest(b"before").into(),
        replacement,
        lane,
        LocalSourceLimits::default(),
    )
    .expect("stage replacement")
}

#[test]
fn staged_replacement_preserves_live_source_and_reuses_snapshot_after_edit() {
    with_staging_source("staging-snapshot-reuse", |root, lane| {
        std::fs::create_dir_all(root.join("nested/empty")).expect("create empty directory");
        std::fs::write(root.join("nested/omega.lock"), b"nested source")
            .expect("write captured nested lock");
        for excluded in [
            ".git/index",
            "build/output",
            "omega.lock",
            "omega.admissions",
        ] {
            let path = root.join(excluded);
            std::fs::create_dir_all(path.parent().unwrap()).expect("create excluded parent");
            std::fs::write(path, b"excluded").expect("write excluded content");
        }
        let limits = LocalSourceLimits::default();
        let original = resolve_local_source(root, limits).expect("resolve original source");
        let requested_root = root.join(".");
        let replacement = b"after\n\0proposed";
        let staged = stage_main(&requested_root, lane, replacement);

        assert_eq!(staged.requested_root(), requested_root);
        assert_eq!(staged.canonical_live_root(), original.root);
        assert_eq!(staged.original(), &original);
        assert_eq!(resolve_local_source(root, limits).unwrap(), original);
        assert_eq!(std::fs::read(root.join("main.omg")).unwrap(), b"before");
        staged
            .verify_live_source_unchanged()
            .expect("live source untouched");
        assert_ne!(staged.snapshot_root(), staged.canonical_live_root());
        assert_eq!(staged.normalized().root, staged.snapshot_root());
        assert_ne!(
            staged.normalized().content_identity,
            original.content_identity
        );
        assert_eq!(
            std::fs::read(staged.snapshot_root().join("main.omg")).unwrap(),
            replacement
        );
        assert_eq!(
            std::fs::read(staged.snapshot_root().join("other.omg")).unwrap(),
            b"kept"
        );
        assert_eq!(
            std::fs::read(staged.snapshot_root().join("nested/omega.lock")).unwrap(),
            b"nested source"
        );
        assert!(staged.snapshot_root().join("nested/empty").is_dir());
        for excluded in [".git", "build", "omega.lock", "omega.admissions"] {
            assert!(!staged.snapshot_root().join(excluded).exists());
        }
        assert_eq!(staged.normalized().file_count, original.file_count);
        assert_eq!(
            staged.normalized().byte_count,
            original.byte_count - 6 + replacement.len() as u64
        );
        let exact = resolve_materialized_source(staged.snapshot_root(), limits)
            .expect("resolve exact proposed snapshot");
        assert_eq!(&exact, staged.normalized());

        std::fs::write(root.join("main.omg"), replacement).expect("land proposed edit");
        let edited = resolve_local_source(root, limits).expect("resolve edited source");
        assert_eq!(
            edited.content_identity,
            staged.normalized().content_identity
        );
        assert_eq!(edited.byte_count, staged.normalized().byte_count);
        let reused = resolve_local_source_snapshot_in_lane(&requested_root, lane, limits)
            .expect("reuse proposed snapshot after edit");
        assert_eq!(reused.snapshot_root(), staged.snapshot_root());
        assert_eq!(reused.normalized(), staged.normalized());
        assert!(matches!(
            staged.verify_live_source_unchanged(),
            Err(SourceResolveError::LocalSourceChanged { .. })
        ));
    });
}

#[cfg(unix)]
#[test]
fn staged_replacement_preserves_executable_modes_and_unedited_symlinks() {
    use std::os::unix::fs::{PermissionsExt, symlink};

    with_staging_source("staging-modes", |root, lane| {
        std::fs::set_permissions(
            root.join("main.omg"),
            std::fs::Permissions::from_mode(0o751),
        )
        .expect("make replacement target executable");
        std::fs::set_permissions(
            root.join("other.omg"),
            std::fs::Permissions::from_mode(0o640),
        )
        .expect("set unrelated source mode");
        symlink("other.omg", root.join("linked.omg")).expect("create captured symlink");
        let staged = stage_main(root, lane, b"after");
        let mode = |path: &Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&root.join("main.omg")), 0o751);
        assert_eq!(mode(&root.join("other.omg")), 0o640);
        assert_eq!(mode(staged.snapshot_root()), 0o555);
        assert_eq!(mode(&staged.snapshot_root().join("main.omg")), 0o555);
        assert_eq!(mode(&staged.snapshot_root().join("other.omg")), 0o444);
        assert_eq!(
            std::fs::read_link(staged.snapshot_root().join("linked.omg")).unwrap(),
            Path::new("other.omg")
        );
        std::fs::write(root.join("main.omg"), b"after").expect("land executable edit");
        let edited = resolve_local_source(root, LocalSourceLimits::default()).unwrap();
        assert_eq!(
            edited.content_identity,
            staged.normalized().content_identity
        );
    });
}

#[test]
fn staged_replacement_rejects_stale_expected_digest() {
    with_staging_source("staging-stale-digest", |root, lane| {
        let error = stage_local_source_replacement_in_lane(
            root,
            &SourceRelativePath::parse("main.omg").unwrap(),
            &Sha256::digest(b"outdated").into(),
            b"after",
            lane,
            LocalSourceLimits::default(),
        )
        .expect_err("stale digest must reject");
        assert!(matches!(
            error,
            SourceResolveError::LocalSourceChanged { .. }
        ));
        assert_eq!(std::fs::read(root.join("main.omg")).unwrap(), b"before");
    });
}

fn assert_invalid_replacement(root: &Path, lane: &RetainedStorageLane, relative_path: &str) {
    let error = stage_local_source_replacement_in_lane(
        root,
        &SourceRelativePath::parse(relative_path).unwrap(),
        &Sha256::digest(b"before").into(),
        b"after",
        lane,
        LocalSourceLimits::default(),
    )
    .expect_err("only a captured regular file may be replaced");
    assert!(matches!(
        error,
        SourceResolveError::LocalSourceReplacementInvalid { path, message }
            if path.ends_with(relative_path) && !message.is_empty()
    ));
}

#[test]
fn staged_replacement_rejects_missing_directory_and_excluded_paths() {
    with_staging_source("staging-invalid-target", |root, lane| {
        std::fs::create_dir(root.join("directory")).expect("create directory target");
        let excluded = [
            ".git/index",
            "build/output",
            "omega.lock",
            "omega.admissions",
        ];
        for relative_path in excluded {
            let path = root.join(relative_path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, b"before").unwrap();
        }
        let original = resolve_local_source(root, LocalSourceLimits::default()).unwrap();
        for relative_path in [
            "missing.omg",
            "directory",
            ".git/index",
            "build/output",
            "omega.lock",
            "omega.admissions",
        ] {
            assert_invalid_replacement(root, lane, relative_path);
        }
        assert!(!root.join("missing.omg").exists());
        assert!(root.join("directory").is_dir());
        for relative_path in excluded {
            assert_eq!(std::fs::read(root.join(relative_path)).unwrap(), b"before");
        }
        assert_eq!(
            resolve_local_source(root, LocalSourceLimits::default()).unwrap(),
            original
        );
    });
}

#[cfg(unix)]
#[test]
fn staged_replacement_rejects_symlink_even_with_matching_target_digest() {
    with_staging_source("staging-symlink-target", |root, lane| {
        std::os::unix::fs::symlink("main.omg", root.join("linked.omg")).unwrap();
        assert_invalid_replacement(root, lane, "linked.omg");
        assert_eq!(
            std::fs::read_link(root.join("linked.omg")).unwrap(),
            Path::new("main.omg")
        );
        assert_eq!(std::fs::read(root.join("main.omg")).unwrap(), b"before");
    });
}

#[test]
fn staged_replacement_byte_budget_counts_proposed_tree() {
    with_staging_source("staging-byte-budget", |root, lane| {
        let limits = LocalSourceLimits {
            max_bytes: 11,
            ..LocalSourceLimits::default()
        };
        for replacement in [b"12345678".as_slice(), b"1234567", b"", b"short"] {
            let result = stage_local_source_replacement_in_lane(
                root,
                &SourceRelativePath::parse("main.omg").unwrap(),
                &Sha256::digest(b"before").into(),
                replacement,
                lane,
                limits,
            );
            if replacement.len() == 8 {
                assert_eq!(
                    result.expect_err("unchanged file also consumes bytes"),
                    SourceResolveError::TooManyBytes { limit: 11 }
                );
            } else {
                let staged = result.expect("replacement replaces original byte charge");
                assert_eq!(staged.normalized().byte_count, replacement.len() as u64 + 4);
                assert_eq!(
                    std::fs::read(staged.snapshot_root().join("main.omg")).unwrap(),
                    replacement
                );
                staged.verify_live_source_unchanged().unwrap();
            }
            assert_eq!(std::fs::read(root.join("main.omg")).unwrap(), b"before");
        }
    });
}

#[test]
fn staged_replacement_verification_detects_unrelated_source_drift() {
    with_staging_source("staging-unrelated-drift", |root, lane| {
        let staged = stage_main(root, lane, b"after");
        staged.verify_live_source_unchanged().unwrap();
        std::fs::write(root.join("other.omg"), b"drift").expect("change unrelated source");
        assert!(matches!(
            staged.verify_live_source_unchanged(),
            Err(SourceResolveError::LocalSourceChanged { .. })
        ));
        assert_eq!(std::fs::read(root.join("main.omg")).unwrap(), b"before");
        assert_eq!(
            std::fs::read(staged.snapshot_root().join("other.omg")).unwrap(),
            b"kept"
        );
    });
}

#[test]
fn no_op_staging_reuses_original_snapshot_and_ignores_root_control_changes() {
    with_staging_source("staging-no-op-lock", |root, lane| {
        let limits = LocalSourceLimits::default();
        let original = resolve_local_source_snapshot_in_lane(root, lane, limits).unwrap();
        let staged = stage_main(root, lane, b"before");
        assert_eq!(staged.snapshot_root(), original.snapshot_root());
        assert_eq!(staged.normalized(), original.normalized());
        assert_eq!(
            staged.original().content_identity,
            staged.normalized().content_identity
        );
        std::fs::write(root.join("main.omg"), b"before").expect("write identical source bytes");
        staged
            .verify_live_source_unchanged()
            .expect("no-op is not drift");
        for name in ["omega.lock", "omega.admissions"] {
            for contents in [b"first policy".as_slice(), b"changed policy"] {
                std::fs::write(root.join(name), contents).unwrap();
                staged
                    .verify_live_source_unchanged()
                    .expect("root control policy is not source drift");
                let reused = stage_main(root, lane, b"before");
                assert_eq!(reused.snapshot_root(), staged.snapshot_root());
                assert!(!reused.snapshot_root().join(name).exists());
            }
            std::fs::remove_file(root.join(name)).unwrap();
            staged
                .verify_live_source_unchanged()
                .expect("root control policy removal is not drift");
        }
    });
}
