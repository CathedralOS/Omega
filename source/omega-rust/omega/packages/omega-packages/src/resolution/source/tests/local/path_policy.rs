use super::*;

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
