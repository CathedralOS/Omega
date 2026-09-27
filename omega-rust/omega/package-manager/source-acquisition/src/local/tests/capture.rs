#[cfg(unix)]
use super::{
    CapabilityDirectory, LocalSourceLimits, OsStr, ambient_authority, open_canonical_source_root,
    open_captured_directory, read_capability_file_bounded, temp_root,
};
#[cfg(unix)]
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
