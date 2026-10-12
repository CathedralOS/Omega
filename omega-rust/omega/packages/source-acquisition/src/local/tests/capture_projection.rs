//! Inert captured namespace policy projections; no compiler or host reads.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use source::{CapturedSourceNode, CapturedSourceTree};

use crate::LocalSourceCapturePolicy;
use crate::local::operations::project_local_source_scope;

#[test]
fn consuming_scope_omits_root_reservations_and_preserves_nested_members() {
    let bytes: Arc<[u8]> = Arc::from(&b"original"[..]);
    let file = || CapturedSourceNode::File {
        bytes: Arc::clone(&bytes),
        executable: false,
    };
    let tree = CapturedSourceTree::new([
        (PathBuf::from("main.omg"), file()),
        (PathBuf::from("OMEGA.LOCK"), file()),
        (PathBuf::from("build"), CapturedSourceNode::Directory),
        (PathBuf::from("build/output"), file()),
        (PathBuf::from("nested"), CapturedSourceNode::Directory),
        (PathBuf::from("nested/omega.lock"), file()),
        (PathBuf::from("nested/build"), CapturedSourceNode::Directory),
        (PathBuf::from("nested/build/member"), file()),
        (PathBuf::from("nested/.git"), CapturedSourceNode::Directory),
        (PathBuf::from("nested/.git/index"), file()),
        (PathBuf::from("generated"), CapturedSourceNode::Directory),
        (
            PathBuf::from("generated/output"),
            CapturedSourceNode::Directory,
        ),
        (PathBuf::from("generated/output/member"), file()),
    ])
    .unwrap();
    let outputs = [PathBuf::from("generated/output")];
    let projected =
        project_local_source_scope(&tree, LocalSourceCapturePolicy::new(&outputs).unwrap())
            .unwrap();
    assert_eq!(
        projected
            .entries()
            .map(|(path, _)| path.to_path_buf())
            .collect::<Vec<_>>(),
        vec![
            PathBuf::from("main.omg"),
            PathBuf::from("nested"),
            PathBuf::from("nested/build"),
            PathBuf::from("nested/build/member"),
            PathBuf::from("nested/omega.lock"),
        ]
    );
    let (
        _,
        CapturedSourceNode::File {
            bytes: retained, ..
        },
    ) = projected
        .entries()
        .find(|(path, _)| *path == Path::new("main.omg"))
        .unwrap()
    else {
        panic!("missing file")
    };
    assert!(Arc::ptr_eq(&bytes, retained));
}

#[test]
fn consuming_scope_refuses_links_to_omitted_root_members() {
    let tree = CapturedSourceTree::new([
        (
            PathBuf::from("omega.lock"),
            CapturedSourceNode::File {
                bytes: Arc::from(&b"control"[..]),
                executable: false,
            },
        ),
        (
            PathBuf::from("link"),
            CapturedSourceNode::Symlink {
                target: PathBuf::from("omega.lock"),
            },
        ),
    ])
    .unwrap();
    assert_eq!(
        project_local_source_scope(&tree, LocalSourceCapturePolicy::default())
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::PermissionDenied
    );
}
