use super::{LocalSourceLimits, resolve_local_source, temp_root};
#[cfg(unix)]
use super::{PathBuf, SourceResolveError, SystemTime, UNIX_EPOCH};

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
