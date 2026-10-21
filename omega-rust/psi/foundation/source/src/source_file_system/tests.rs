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

    // Repeated routes share one link observation per prefix; the answer must
    // stay the captured backing, never the host file behind the alias.
    for _ in 0..2 {
        let read = |path: &Path| view.read_to_string(path).unwrap().to_string();
        assert_eq!(read(&alias.join("main.omg")), "captured");
        assert_eq!(read(&alias.join("only_captured.omg")), "not on disk");
        assert_eq!(read(&project.join("main.omg")), "captured");
        assert_eq!(read(&directory.0.join("outside.omg")), "outside");
        assert!(!view.exists(&alias.join("missing.omg")));
    }

    // Clones share observations but compare by mounts alone.
    assert_eq!(view.clone(), view);
    assert_ne!(view, SourceFileSystem::new());
}
