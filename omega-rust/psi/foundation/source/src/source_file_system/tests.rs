use crate::{CapturedSourceNode, CapturedSourceTree, SourceFileSystem};
use std::path::{Path, PathBuf};
use std::sync::Arc;

struct TemporaryDirectory(PathBuf);

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn temporary_directory() -> TemporaryDirectory {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path =
        std::env::temp_dir().join(format!("omega-source-view-{}-{nonce}", std::process::id()));
    std::fs::create_dir(&path).unwrap();
    // Capture roots must be canonical, and temporary directories are often
    // reached through a host link (macOS `/var` -> `/private/var`).
    TemporaryDirectory(std::fs::canonicalize(&path).unwrap())
}

fn captured_file(text: &str) -> CapturedSourceNode {
    CapturedSourceNode::File {
        bytes: Arc::from(text.as_bytes()),
        executable: false,
    }
}

#[test]
fn joining_captures_keeps_shared_snapshots_and_refuses_conflicts_atomically() {
    let directory = temporary_directory();
    let source = |text| {
        Arc::new(
            CapturedSourceTree::new([(PathBuf::from("main.omg"), captured_file(text))]).unwrap(),
        )
    };
    let first = directory.0.join("first");
    let second = directory.0.join("second");
    let mut destination = SourceFileSystem::new();
    destination
        .add_capture(first.clone(), source("first snapshot"))
        .unwrap();
    let mut supplied = SourceFileSystem::new();
    supplied
        .add_capture(first.clone(), source("first snapshot"))
        .unwrap();
    supplied
        .add_capture(second.clone(), source("second snapshot"))
        .unwrap();
    destination.join_captures(&supplied).unwrap();
    assert_eq!(
        destination
            .read_to_string(&second.join("main.omg"))
            .unwrap()
            .as_ref(),
        "second snapshot"
    );
    let saved = destination.clone();
    let mut conflict = SourceFileSystem::new();
    conflict
        .add_capture(directory.0.join("additional"), source("must not leak"))
        .unwrap();
    conflict
        .add_capture(second, source("changed snapshot"))
        .unwrap();
    assert!(destination.join_captures(&conflict).is_err());
    assert_eq!(destination, saved);
    assert!(!destination.exists(&first.join("not captured.omg")));
}

#[cfg(unix)]
#[test]
fn host_links_into_a_capture_read_captured_bytes_on_every_repeated_route() {
    let directory = temporary_directory();
    let project = directory.0.join("project");
    std::fs::create_dir(&project).unwrap();
    std::fs::write(project.join("main.omg"), "on disk").unwrap();
    std::fs::write(directory.0.join("outside.omg"), "outside").unwrap();
    let alias = directory.0.join("alias");
    std::os::unix::fs::symlink(&project, &alias).unwrap();

    let tree = CapturedSourceTree::new([
        (PathBuf::from("main.omg"), captured_file("captured")),
        (
            PathBuf::from("only_captured.omg"),
            captured_file("not on disk"),
        ),
    ])
    .unwrap();
    let mut view = SourceFileSystem::new();
    view.add_capture(project.clone(), Arc::new(tree)).unwrap();

    // Repeated routes must stay on the captured backing, never the host file
    // behind the alias.
    for _ in 0..2 {
        let read = |path: &Path| view.read_to_string(path).unwrap().to_string();
        assert_eq!(read(&alias.join("main.omg")), "captured");
        assert_eq!(read(&alias.join("only_captured.omg")), "not on disk");
        assert_eq!(read(&project.join("main.omg")), "captured");
        assert_eq!(read(&directory.0.join("outside.omg")), "outside");
        assert!(!view.exists(&alias.join("missing.omg")));
    }

    // Clones preserve the mounted source view.
    assert_eq!(view.clone(), view);
    assert_ne!(view, SourceFileSystem::new());
}

#[cfg(unix)]
fn changing_host_alias_enters_capture(start_with_link: bool) {
    let directory = temporary_directory();
    let project = directory.0.join("project");
    let outside = directory.0.join("outside");
    let alias = directory.0.join("alias");
    std::fs::create_dir(&project).unwrap();
    std::fs::create_dir(&outside).unwrap();
    std::fs::write(project.join("main.omg"), "live capture disk").unwrap();
    std::fs::write(project.join("missing.omg"), "disk only").unwrap();
    std::fs::write(outside.join("main.omg"), "outside").unwrap();
    std::fs::write(outside.join("missing.omg"), "outside missing").unwrap();
    if start_with_link {
        std::os::unix::fs::symlink(&outside, &alias).unwrap();
    } else {
        std::fs::create_dir(&alias).unwrap();
        std::fs::write(alias.join("main.omg"), "outside").unwrap();
        std::fs::write(alias.join("missing.omg"), "outside missing").unwrap();
    }
    let tree = CapturedSourceTree::new([
        (
            PathBuf::from("main.omg"),
            captured_file("captured immutable"),
        ),
        (
            PathBuf::from("captured_directory"),
            CapturedSourceNode::Directory,
        ),
    ])
    .unwrap();
    let mut view = SourceFileSystem::new();
    view.add_capture(project.clone(), Arc::new(tree)).unwrap();
    let clone = view.clone();
    assert_eq!(
        view.read_to_string(&alias.join("main.omg"))
            .unwrap()
            .as_ref(),
        "outside"
    );
    assert!(view.exists(&alias.join("missing.omg")));
    if start_with_link {
        std::fs::remove_file(&alias).unwrap();
    } else {
        std::fs::remove_dir_all(&alias).unwrap();
    }
    std::os::unix::fs::symlink(&project, &alias).unwrap();

    for view in [&view, &clone] {
        assert_eq!(
            view.read_to_string(&alias.join("main.omg"))
                .unwrap()
                .as_ref(),
            "captured immutable"
        );
        assert_eq!(
            view.canonicalize(&alias.join("main.omg")).unwrap(),
            project.join("main.omg")
        );
        assert!(view.is_file(&alias.join("main.omg")));
        assert!(view.is_dir(&alias.join("captured_directory")));
        assert!(!view.exists(&alias.join("missing.omg")));
        assert!(!view.is_file(&alias.join("missing.omg")));
        assert_eq!(
            view.read_to_string(&alias.join("missing.omg"))
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::NotFound
        );
        assert_eq!(
            view.canonicalize(&alias.join("missing.omg"))
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::NotFound
        );
    }
}

#[cfg(unix)]
#[test]
fn replaced_host_directory_cannot_bypass_a_capture() {
    changing_host_alias_enters_capture(false);
}

#[cfg(unix)]
#[test]
fn retargeted_host_link_cannot_bypass_a_capture() {
    changing_host_alias_enters_capture(true);
}
