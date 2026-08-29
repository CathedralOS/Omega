use super::*;

#[test]
fn package_fixtures_resolve_as_distinct_local_sources() {
    let fixtures_root = package_fixtures_root();
    let mut identities = BTreeSet::new();
    for package in PACKAGE_FIXTURES {
        PackageName::parse(*package).expect("fixture package names must be kebab-case");
        let root = fixtures_root.join(package);
        assert!(root.join("build.omg").is_file());
        assert!(root.join("main.omg").is_file());

        let resolved =
            resolve_local_source(&root, LocalSourceLimits::default()).expect("resolve fixture");
        assert!(resolved.file_count >= 3);
        assert!(identities.insert(resolved.content_identity));
    }
    assert_eq!(identities.len(), PACKAGE_FIXTURES.len());
}

#[test]
fn local_source_identity_is_order_independent_and_ignores_git_dir() {
    let root = temp_root("identity");
    std::fs::create_dir_all(root.join("src")).expect("create source tree");
    std::fs::create_dir_all(root.join(".git")).expect("create git dir");
    std::fs::write(root.join("src/lib.omg"), "machine Lib::id() {}\n").expect("write source");
    std::fs::write(root.join("README.md"), "package\n").expect("write readme");
    std::fs::write(root.join(".git/index"), "ignored").expect("write ignored git data");

    let first = resolve_local_source(&root, LocalSourceLimits::default()).expect("resolve");
    std::fs::write(root.join(".git/index"), "ignored but changed")
        .expect("change ignored git data");
    let second = resolve_local_source(&root, LocalSourceLimits::default()).expect("resolve");

    assert_eq!(first.file_count, 2);
    assert_eq!(first.content_identity, second.content_identity);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn local_package_identity_excludes_only_root_build_output() {
    let root = temp_root("root-build-output");
    std::fs::create_dir_all(root.join("build")).expect("create root build output");
    std::fs::create_dir_all(root.join("src/build")).expect("create nested source directory");
    std::fs::write(root.join("main.omg"), "machine Main::main() {}\n")
        .expect("write package source");
    std::fs::write(
        root.join("build/00_pipeline.html"),
        "first generated report",
    )
    .expect("write generated report");
    std::fs::write(
        root.join("src/build/rules.omg"),
        "machine Rules::apply() {}\n",
    )
    .expect("write nested source");

    let first =
        resolve_local_source(&root, LocalSourceLimits::default()).expect("resolve local package");
    std::fs::write(
        root.join("build/00_pipeline.html"),
        "changed generated report",
    )
    .expect("change generated report");
    let changed_output = resolve_local_source(&root, LocalSourceLimits::default())
        .expect("resolve package after output change");
    assert_eq!(first.file_count, 2);
    assert_eq!(first.content_identity, changed_output.content_identity);

    std::fs::write(
        root.join("src/build/rules.omg"),
        "machine Rules::replace() {}\n",
    )
    .expect("change nested source");
    let changed_source = resolve_local_source(&root, LocalSourceLimits::default())
        .expect("resolve package after source change");
    assert_ne!(
        changed_output.content_identity,
        changed_source.content_identity
    );

    let exact = resolve_materialized_source(&root, LocalSourceLimits::default())
        .expect("resolve exact materialized tree");
    assert_eq!(exact.file_count, 3);
    assert_ne!(changed_source.content_identity, exact.content_identity);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn local_source_identity_changes_when_source_bytes_change() {
    let root = temp_root("bytes");
    std::fs::create_dir_all(&root).expect("create source tree");
    std::fs::write(root.join("main.omg"), "machine Main::a() {}\n").expect("write source");
    let first = resolve_local_source(&root, LocalSourceLimits::default()).expect("resolve");

    std::fs::write(root.join("main.omg"), "machine Main::b() {}\n").expect("rewrite source");
    let second = resolve_local_source(&root, LocalSourceLimits::default()).expect("resolve");

    assert_ne!(first.content_identity, second.content_identity);

    let _ = std::fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[test]
fn local_capture_does_not_follow_replaced_regular_leaf() {
    let root = temp_root("nofollow-replaced-file");
    std::fs::create_dir_all(&root).expect("create source tree");
    std::fs::write(root.join("source.omg"), "classified bytes").expect("write classified source");
    std::fs::write(root.join("replacement.omg"), "replacement bytes")
        .expect("write replacement source");
    let canonical_root = root.canonicalize().expect("canonicalize source root");
    let directory = CapabilityDirectory::open_ambient_dir(&canonical_root, ambient_authority())
        .expect("open source root capability");
    assert!(
        directory
            .symlink_metadata("source.omg")
            .expect("classify source leaf")
            .is_file()
    );

    std::fs::remove_file(root.join("source.omg")).expect("remove classified source");
    std::os::unix::fs::symlink("replacement.omg", root.join("source.omg"))
        .expect("replace source with symlink");
    let _error = read_capability_file_bounded(
        &directory,
        OsStr::new("source.omg"),
        &canonical_root.join("source.omg"),
        LocalSourceLimits::default().max_bytes,
        LocalSourceLimits::default().max_bytes,
    )
    .expect_err("capture must not follow a replacement symlink");

    let _ = std::fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[test]
fn local_capture_does_not_follow_replaced_directory_leaf() {
    let root = temp_root("nofollow-replaced-directory");
    std::fs::create_dir_all(root.join("source")).expect("create classified directory");
    std::fs::create_dir_all(root.join("replacement")).expect("create replacement directory");
    let canonical_root = root.canonicalize().expect("canonicalize source root");
    let directory = CapabilityDirectory::open_ambient_dir(&canonical_root, ambient_authority())
        .expect("open source root capability");
    assert!(
        directory
            .symlink_metadata("source")
            .expect("classify source directory")
            .is_dir()
    );

    std::fs::remove_dir(root.join("source")).expect("remove classified directory");
    std::os::unix::fs::symlink("replacement", root.join("source"))
        .expect("replace directory with symlink");
    let _error = open_captured_directory(
        &directory,
        OsStr::new("source"),
        &canonical_root.join("source"),
    )
    .expect_err("capture must not follow a replacement directory symlink");

    let _ = std::fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[test]
fn local_capture_does_not_follow_replaced_root_leaf() {
    let root = temp_root("nofollow-replaced-root");
    let retained = root.with_extension("retained");
    let replacement = root.with_extension("replacement");
    std::fs::create_dir_all(&root).expect("create classified source root");
    std::fs::create_dir_all(&replacement).expect("create replacement source root");
    let canonical_root = root.canonicalize().expect("canonicalize source root");

    std::fs::rename(&root, &retained).expect("relocate classified source root");
    std::os::unix::fs::symlink(&replacement, &root).expect("replace source root with symlink");
    let _error = open_canonical_source_root(&canonical_root)
        .expect_err("root acquisition must not follow a replacement symlink");

    std::fs::remove_file(&root).expect("remove replacement root symlink");
    let _ = std::fs::remove_dir_all(&retained);
    let _ = std::fs::remove_dir_all(&replacement);
}

#[cfg(unix)]
#[test]
fn local_capture_remains_bound_to_open_root_after_path_replacement() {
    let root = temp_root("open-root-replacement");
    let retained = root.with_extension("retained");
    std::fs::create_dir_all(&root).expect("create source root");
    std::fs::write(root.join("main.omg"), "retained bytes").expect("write retained source");
    let canonical_root = root.canonicalize().expect("canonicalize source root");
    let directory = CapabilityDirectory::open_ambient_dir(&canonical_root, ambient_authority())
        .expect("open source root capability");

    std::fs::rename(&root, &retained).expect("relocate opened source root");
    std::fs::create_dir_all(&root).expect("create replacement root");
    std::fs::write(root.join("main.omg"), "replacement bytes").expect("write replacement source");

    let captured = capture_local_source_from_open_root(
        canonical_root,
        directory,
        LocalSourceLimits::default(),
        SourceTreePolicy::LocalPackage,
    )
    .expect("capture through retained root capability");
    let retained_identity = resolve_local_source(&retained, LocalSourceLimits::default())
        .expect("resolve retained source");
    let replacement_identity = resolve_local_source(&root, LocalSourceLimits::default())
        .expect("resolve replacement source");
    assert_eq!(
        captured.normalized.content_identity,
        retained_identity.content_identity
    );
    assert_ne!(
        captured.normalized.content_identity,
        replacement_identity.content_identity
    );

    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&retained);
}

#[test]
fn local_source_identity_includes_empty_directory_paths() {
    let root = temp_root("empty-directory-identity");
    std::fs::create_dir_all(&root).expect("create source tree");
    std::fs::write(root.join("main.omg"), "machine Main::main() {}\n").expect("write source");
    let without_empty =
        resolve_local_source(&root, LocalSourceLimits::default()).expect("resolve source");

    std::fs::create_dir(root.join("generated")).expect("create empty directory");
    let with_empty =
        resolve_local_source(&root, LocalSourceLimits::default()).expect("resolve source");
    assert_eq!(without_empty.file_count, with_empty.file_count);
    assert_ne!(without_empty.content_identity, with_empty.content_identity);

    std::fs::remove_dir(root.join("generated")).expect("remove empty directory");
    let removed =
        resolve_local_source(&root, LocalSourceLimits::default()).expect("resolve source");
    assert_eq!(without_empty.content_identity, removed.content_identity);

    let _ = std::fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[test]
fn local_source_identity_canonicalizes_live_directory_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let root = temp_root("directory-mode-identity");
    let directory = root.join("generated");
    std::fs::create_dir_all(&directory).expect("create source tree");
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o755))
        .expect("set writable directory mode");
    let writable =
        resolve_local_source(&root, LocalSourceLimits::default()).expect("resolve source");

    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o555))
        .expect("set read-only directory mode");
    let read_only =
        resolve_local_source(&root, LocalSourceLimits::default()).expect("resolve source");

    assert_eq!(writable.file_count, 0);
    assert_eq!(writable.content_identity, read_only.content_identity);

    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o755))
        .expect("restore directory mode");
    let _ = std::fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[test]
fn local_source_path_encoding_preserves_non_utf8_bytes() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let first = OsString::from_vec(b"source-\x80.omg".to_vec());
    let second = OsString::from_vec(b"source-\x81.omg".to_vec());

    assert_eq!(raw_os_bytes(&first), b"source-\x80.omg");
    assert_eq!(raw_os_bytes(&second), b"source-\x81.omg");
    assert_ne!(raw_os_bytes(&first), raw_os_bytes(&second));
}

#[cfg(all(unix, not(target_vendor = "apple")))]
#[test]
fn local_source_identity_distinguishes_non_utf8_paths() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let first_root = temp_root("non-utf8-first");
    let second_root = temp_root("non-utf8-second");
    std::fs::create_dir_all(&first_root).expect("create first source tree");
    std::fs::create_dir_all(&second_root).expect("create second source tree");
    let first_name = OsString::from_vec(b"source-\x80.omg".to_vec());
    let second_name = OsString::from_vec(b"source-\x81.omg".to_vec());
    std::fs::write(first_root.join(first_name), "same bytes").expect("write first source");
    std::fs::write(second_root.join(second_name), "same bytes").expect("write second source");

    let first =
        resolve_local_source(&first_root, LocalSourceLimits::default()).expect("resolve first");
    let second =
        resolve_local_source(&second_root, LocalSourceLimits::default()).expect("resolve second");

    assert_ne!(first.content_identity, second.content_identity);

    let _ = std::fs::remove_dir_all(&first_root);
    let _ = std::fs::remove_dir_all(&second_root);
}

#[cfg(unix)]
#[test]
fn local_source_rejects_symlinks_into_excluded_git_metadata() {
    let root = temp_root("symlink-git-metadata");
    std::fs::create_dir_all(root.join(".git")).expect("create ignored target directory");
    let target = root.join(".git/target.omg");
    let link = root.join("linked.omg");
    std::fs::write(&target, "first target bytes").expect("write target");
    std::os::unix::fs::symlink(".git/target.omg", &link).expect("create symlink");

    let error = resolve_local_source(&root, LocalSourceLimits::default())
        .expect_err("excluded metadata target must reject");
    assert!(matches!(
        error,
        SourceResolveError::SymlinkTargetsExcludedMetadata { .. }
    ));

    let _ = std::fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[test]
fn local_source_rejects_symlinks_into_excluded_root_build_output() {
    let root = temp_root("symlink-build-output");
    std::fs::create_dir_all(root.join("build")).expect("create excluded build output");
    std::fs::write(root.join("build/generated.omg"), "generated").expect("write generated output");
    std::os::unix::fs::symlink("build/generated.omg", root.join("linked.omg"))
        .expect("create source symlink");

    let error = resolve_local_source(&root, LocalSourceLimits::default())
        .expect_err("excluded build-output target must reject");
    assert!(matches!(
        error,
        SourceResolveError::SymlinkTargetsExcludedBuildOutput { .. }
    ));

    let _ = std::fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[test]
fn local_source_rejects_absolute_symlink_targets_inside_the_live_root() {
    let root = temp_root("absolute-symlink-target");
    std::fs::create_dir_all(&root).expect("create source tree");
    let target = root.join("target.omg");
    std::fs::write(&target, "target bytes").expect("write target");
    std::os::unix::fs::symlink(&target, root.join("linked.omg"))
        .expect("create absolute source symlink");

    let error = resolve_local_source(&root, LocalSourceLimits::default())
        .expect_err("absolute spelling cannot remain snapshot-rooted after publication");
    assert!(matches!(
        error,
        SourceResolveError::SymlinkEscapesRoot { .. }
    ));

    let _ = std::fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[test]
fn local_source_identity_hashes_internal_symlink_spelling_and_reachable_target() {
    let root = temp_root("symlink-identity");
    std::fs::create_dir_all(&root).expect("create source tree");
    let target = root.join("target.omg");
    let link = root.join("linked.omg");
    std::fs::write(&target, "first target bytes").expect("write target");
    std::os::unix::fs::symlink("target.omg", &link).expect("create symlink");

    let first = resolve_local_source(&root, LocalSourceLimits::default()).expect("resolve");
    std::fs::write(&target, "different target bytes").expect("rewrite target");
    let changed_target =
        resolve_local_source(&root, LocalSourceLimits::default()).expect("resolve target");
    assert_ne!(first.content_identity, changed_target.content_identity);

    std::fs::remove_file(&link).expect("remove symlink");
    std::os::unix::fs::symlink("./target.omg", &link).expect("recreate symlink");
    let changed_spelling =
        resolve_local_source(&root, LocalSourceLimits::default()).expect("resolve spelling change");
    assert_ne!(
        changed_target.content_identity,
        changed_spelling.content_identity
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[test]
fn local_source_identity_distinguishes_executable_mode() {
    use std::os::unix::fs::PermissionsExt;

    let root = temp_root("executable-mode");
    std::fs::create_dir_all(&root).expect("create source tree");
    let source = root.join("generate");
    std::fs::write(&source, "same bytes").expect("write source");
    std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o644))
        .expect("make source non-executable");
    let non_executable =
        resolve_local_source(&root, LocalSourceLimits::default()).expect("resolve mode");

    std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o755))
        .expect("make source executable");
    let executable =
        resolve_local_source(&root, LocalSourceLimits::default()).expect("resolve mode");

    assert_ne!(non_executable.content_identity, executable.content_identity);

    let _ = std::fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[test]
fn local_source_rejects_special_file_kind() {
    use std::os::unix::net::UnixListener;

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let root = PathBuf::from("/tmp").join(format!(
        "omega-source-socket-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("create source tree");
    let socket_path = root.join("source.sock");
    let listener = UnixListener::bind(&socket_path).expect("create Unix socket");
    let expected_path = root
        .canonicalize()
        .expect("canonicalize source tree")
        .join("source.sock");

    let error = resolve_local_source(&root, LocalSourceLimits::default())
        .expect_err("special file should reject");

    assert_eq!(
        error,
        SourceResolveError::UnsupportedFileType {
            path: expected_path
        }
    );

    drop(listener);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn local_source_limits_reject_too_many_files() {
    let root = temp_root("files");
    std::fs::create_dir_all(&root).expect("create source tree");
    std::fs::write(root.join("a.omg"), "").expect("write source");

    let error = resolve_local_source(
        &root,
        LocalSourceLimits {
            max_files: 0,
            ..LocalSourceLimits::default()
        },
    )
    .expect_err("file limit should reject");

    assert_eq!(error, SourceResolveError::TooManyFiles { limit: 0 });

    let _ = std::fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[test]
fn local_entry_limit_rejects_excess_before_classification_or_read() {
    let root = temp_root("entry-limit-before-read");
    std::fs::create_dir_all(&root).expect("create source tree");
    std::fs::write(root.join("a.omg"), "accepted entry").expect("write accepted source");
    std::os::unix::fs::symlink("/outside-source-root", root.join("b.omg"))
        .expect("create excess escaping link");

    let error = resolve_local_source(
        &root,
        LocalSourceLimits {
            max_files: 1,
            ..LocalSourceLimits::default()
        },
    )
    .expect_err("entry limit must reject before classifying the excess leaf");
    assert_eq!(error, SourceResolveError::TooManyFiles { limit: 1 });

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn local_directory_collection_is_bounded_without_counting_reserved_exclusions() {
    let root = temp_root("bounded-directory-listing");
    std::fs::create_dir_all(root.join(".git")).expect("create excluded metadata");
    std::fs::create_dir_all(root.join("build")).expect("create excluded build output");
    std::fs::write(root.join("first.omg"), "").expect("write first source");
    std::fs::write(root.join("second.omg"), "").expect("write second source");
    let limits = LocalSourceLimits {
        max_files: 2,
        ..LocalSourceLimits::default()
    };

    let accepted = resolve_local_source(&root, limits)
        .expect("the two reserved exclusions must not consume source identity entries");
    assert_eq!(accepted.file_count, 2);

    std::fs::write(root.join("third.omg"), "").expect("write excess source");
    assert_eq!(
        resolve_local_source(&root, limits)
            .expect_err("directory collection must stop at its bounded allowance"),
        SourceResolveError::TooManyFiles { limit: 2 }
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn local_source_limits_count_directories_and_report_identity_entries() {
    let root = temp_root("directory-entry-limit");
    std::fs::create_dir_all(root.join("nested")).expect("create source tree");
    std::fs::write(root.join("nested/main.omg"), "").expect("write source");

    let error = resolve_local_source(
        &root,
        LocalSourceLimits {
            max_files: 1,
            ..LocalSourceLimits::default()
        },
    )
    .expect_err("directory and file must consume separate identity entries");

    assert_eq!(error, SourceResolveError::TooManyFiles { limit: 1 });
    assert_eq!(
        error.to_string(),
        "source root exceeds identity entry limit of 1"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn local_source_limits_reject_file_before_reading_past_byte_limit() {
    let root = temp_root("bytes-limit");
    std::fs::create_dir_all(&root).expect("create source tree");
    std::fs::write(root.join("source.omg"), "four").expect("write source");

    let error = resolve_local_source(
        &root,
        LocalSourceLimits {
            max_bytes: 3,
            ..LocalSourceLimits::default()
        },
    )
    .expect_err("byte limit should reject");

    assert_eq!(error, SourceResolveError::TooManyBytes { limit: 3 });

    let _ = std::fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[test]
fn local_source_rejects_symlink_escape() {
    let root = temp_root("symlink");
    let outside = temp_root("outside");
    std::fs::create_dir_all(&root).expect("create source tree");
    std::fs::create_dir_all(&outside).expect("create outside tree");
    std::fs::write(outside.join("secret.omg"), "secret").expect("write outside source");
    std::os::unix::fs::symlink(outside.join("secret.omg"), root.join("secret.omg"))
        .expect("create escaping symlink");

    let error =
        resolve_local_source(&root, LocalSourceLimits::default()).expect_err("escape rejects");

    assert!(matches!(
        error,
        SourceResolveError::SymlinkEscapesRoot { .. }
    ));

    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&outside);
}

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

    let resolved = resolve_local_source_snapshot(&root, &cache, LocalSourceLimits::default())
        .expect("snapshot local source");

    assert_eq!(resolved.requested_root, root);
    assert_eq!(resolved.canonical_live_root, live.root);
    assert_ne!(resolved.snapshot_root, resolved.canonical_live_root);
    assert_eq!(resolved.normalized.root, resolved.snapshot_root);
    assert!(resolved.snapshot_root.join("generated/empty").is_dir());
    assert!(!resolved.snapshot_root.join(".git").exists());
    assert!(!resolved.snapshot_root.join("build").exists());
    assert_eq!(resolved.normalized.file_count, 1);
    assert_eq!(resolved.normalized.byte_count, live.byte_count);
    assert_eq!(resolved.normalized.content_identity, live.content_identity);
    assert!(
        resolved
            .snapshot_root
            .parent()
            .expect("publication root")
            .join(LOCAL_SNAPSHOT_METADATA)
            .is_file()
    );
    assert!(
        !resolved
            .snapshot_root
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

#[test]
fn byte_identical_local_sources_retain_distinct_custody_roots() {
    let first_root = temp_root("local-snapshot-first-lineage");
    let second_root = temp_root("local-snapshot-second-lineage");
    let cache = temp_root("local-snapshot-lineage-cache");
    for root in [&first_root, &second_root] {
        std::fs::create_dir_all(root).expect("create source root");
        std::fs::write(
            root.join("main.omg"),
            "pub machine identity() -> u64 { 1 }\n",
        )
        .expect("write identical source");
    }

    let first = resolve_local_source_snapshot(&first_root, &cache, LocalSourceLimits::default())
        .expect("publish first lineage snapshot");
    let second = resolve_local_source_snapshot(&second_root, &cache, LocalSourceLimits::default())
        .expect("publish second lineage snapshot");

    assert_eq!(
        first.normalized.content_identity, second.normalized.content_identity,
        "content identity must remain independent of source lineage"
    );
    assert_ne!(first.canonical_live_root, second.canonical_live_root);
    assert_ne!(
        first.snapshot_root, second.snapshot_root,
        "distinct lineages need distinct physical custody roots for compiler attribution"
    );
    assert_eq!(
        resolve_local_source_snapshot(&first_root, &cache, LocalSourceLimits::default())
            .expect("reuse first lineage snapshot"),
        first
    );
    assert_eq!(
        resolve_local_source_snapshot(&second_root, &cache, LocalSourceLimits::default())
            .expect("reuse second lineage snapshot"),
        second
    );

    make_tree_owner_writable(&cache);
    let _ = std::fs::remove_dir_all(&first_root);
    let _ = std::fs::remove_dir_all(&second_root);
    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn local_snapshot_rejects_cache_inside_source_before_creating_it() {
    let root = temp_root("local-snapshot-overlap");
    let cache = root.join("target/omega-cache");
    std::fs::create_dir_all(&root).expect("create source");
    std::fs::write(root.join("main.omg"), "machine Main::main() {}\n").expect("write source");

    let error = resolve_local_source_snapshot(&root, &cache, LocalSourceLimits::default())
        .expect_err("overlapping cache must reject");
    assert!(matches!(
        error,
        SourceResolveError::LocalSnapshotCacheOverlapsSource { .. }
    ));
    assert!(!cache.exists());
    assert!(!root.join("target").exists());

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn local_snapshot_rejects_live_source_beneath_snapshot_collection() {
    let cache = temp_root("local-snapshot-containing-cache");
    let root = cache.join(LOCAL_CACHE_SNAPSHOTS).join("imported/source");
    std::fs::create_dir_all(&root).expect("create nested source");
    std::fs::write(root.join("main.omg"), "machine Main::main() {}\n").expect("write source");

    let error = resolve_local_source_snapshot(&root, &cache, LocalSourceLimits::default())
        .expect_err("resolver-owned collection source must reject");
    assert!(matches!(
        error,
        SourceResolveError::LocalSnapshotCacheOverlapsSource { .. }
    ));

    let _ = std::fs::remove_dir_all(&cache);
}

#[cfg(unix)]
#[test]
fn local_snapshot_canonicalizes_permissions_and_preserves_symlink_spelling() {
    use std::os::unix::fs::PermissionsExt;

    let root = temp_root("local-snapshot-modes-symlink");
    let cache = temp_root("local-snapshot-modes-symlink-cache");
    std::fs::create_dir_all(root.join("tools")).expect("create tools");
    std::fs::write(root.join("main.omg"), "machine Main::main() {}\n").expect("write source");
    std::fs::write(root.join("tools/generate"), "generator\n").expect("write executable");
    std::fs::set_permissions(root.join("tools"), std::fs::Permissions::from_mode(0o700))
        .expect("set live directory mode");
    std::fs::set_permissions(
        root.join("tools/generate"),
        std::fs::Permissions::from_mode(0o711),
    )
    .expect("set executable mode");
    std::os::unix::fs::symlink("generate", root.join("tools/current"))
        .expect("create relative symlink");

    let resolved = resolve_local_source_snapshot(&root, &cache, LocalSourceLimits::default())
        .expect("snapshot local source");
    let mode = |path: &Path| {
        std::fs::symlink_metadata(path)
            .expect("snapshot metadata")
            .permissions()
            .mode()
            & 0o777
    };
    assert_eq!(mode(&resolved.snapshot_root), 0o555);
    assert_eq!(mode(&resolved.snapshot_root.join("tools")), 0o555);
    assert_eq!(mode(&resolved.snapshot_root.join("main.omg")), 0o444);
    assert_eq!(mode(&resolved.snapshot_root.join("tools/generate")), 0o555);
    assert_eq!(
        std::fs::read_link(resolved.snapshot_root.join("tools/current"))
            .expect("read snapshot symlink"),
        PathBuf::from("generate")
    );
    assert_eq!(
        std::fs::read(resolved.snapshot_root.join("tools/current"))
            .expect("follow snapshot symlink"),
        b"generator\n"
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

    let first = resolve_local_source_snapshot(&root, &cache, LocalSourceLimits::default())
        .expect("publish snapshot");
    let second = resolve_local_source_snapshot(&root, &cache, LocalSourceLimits::default())
        .expect("reuse snapshot");
    assert_eq!(first, second);

    std::fs::set_permissions(&first.snapshot_root, std::fs::Permissions::from_mode(0o755))
        .expect("make snapshot root writable");
    let source = first.snapshot_root.join("main.omg");
    std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o644))
        .expect("make snapshot file writable");
    std::fs::write(&source, "machine Tampered::main() {}\n").expect("tamper snapshot");

    let error = resolve_local_source_snapshot(&root, &cache, LocalSourceLimits::default())
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
