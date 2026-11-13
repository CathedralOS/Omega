use std::path::Path;

use filesystem_authority::{SourceCaptureObligation as Obligation, SourceCaptureRequest};

use super::{
    LocalSourceLimits, capture_local_source_input, capture_selected_local_source_input, temp_root,
};

fn selected(paths: &[&str]) -> SourceCaptureRequest {
    SourceCaptureRequest::new(
        paths
            .iter()
            .map(|path| (path.as_bytes().to_vec(), Obligation::Required)),
    )
    .unwrap()
}

#[test]
fn whole_and_selected_capture_preserve_regular_file_root_rejection() {
    let root = temp_root("selected-source-file-root");
    std::fs::create_dir_all(&root).unwrap();
    let file = root.join("main.omg");
    std::fs::write(&file, "machine main() {}").unwrap();
    for result in [
        capture_local_source_input(&file, LocalSourceLimits::default()),
        capture_selected_local_source_input(
            &file,
            LocalSourceLimits::default(),
            &selected(&["main.omg"]),
        ),
    ] {
        assert!(matches!(
            result,
            Err(crate::SourceResolveError::NotDirectory { .. })
        ));
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn narrow_and_wide_captures_retain_their_own_namespace_and_real_origin() {
    let root = temp_root("selected-source-namespace");
    std::fs::create_dir_all(root.join("invalid")).unwrap();
    std::fs::write(root.join("main.omg"), "const BOUND: u64 = 7;").unwrap();
    std::fs::write(root.join("build.omg"), "machine build() {}").unwrap();
    std::fs::write(root.join("invalid/main.omg"), "const BOUND: u64 = 13;").unwrap();

    let narrow = capture_selected_local_source_input(
        &root,
        LocalSourceLimits::default(),
        &selected(&["main.omg", "build.omg"]),
    )
    .unwrap();
    assert_eq!(narrow.canonical_live_root(), root.canonicalize().unwrap());
    assert!(!narrow.tree().exists(Path::new("invalid/main.omg")));
    let mut view = source::SourceFileSystem::new();
    view.add_capture(
        narrow.canonical_live_root().to_path_buf(),
        narrow.tree().clone(),
    )
    .unwrap();
    assert!(view.read_to_string(&root.join("invalid/main.omg")).is_err());

    let wide = capture_local_source_input(&root, LocalSourceLimits::default()).unwrap();
    assert!(
        wide.tree()
            .read_to_string(Path::new("invalid/main.omg"))
            .unwrap()
            .contains("13")
    );
    assert_ne!(
        wide.normalized().content_identity,
        narrow.normalized().content_identity
    );
    assert_ne!(wide.selection_identity(), narrow.selection_identity());
    let narrow_again = capture_selected_local_source_input(
        &root,
        LocalSourceLimits::default(),
        &selected(&["main.omg", "build.omg"]),
    )
    .unwrap();
    assert_eq!(narrow_again, narrow);
    assert_eq!(wide.canonical_live_root(), narrow.canonical_live_root());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn equal_bytes_do_not_erase_distinct_capture_requests_from_records() {
    let root = temp_root("selected-source-request-commitment");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("main.omg"), "machine main() {}").unwrap();
    let narrow = capture_selected_local_source_input(
        &root,
        LocalSourceLimits::default(),
        &selected(&["main.omg"]),
    )
    .unwrap();
    let wide = capture_local_source_input(&root, LocalSourceLimits::default()).unwrap();
    assert_eq!(narrow.tree(), wide.tree());
    assert_eq!(narrow.normalized(), wide.normalized());
    assert_ne!(narrow.selection_identity(), wide.selection_identity());
    assert_ne!(
        compiler_storage::encode(&narrow, Default::default()).unwrap(),
        compiler_storage::encode(&wide, Default::default()).unwrap(),
    );
    let optional = SourceCaptureRequest::new([
        (b"main.omg".to_vec(), Obligation::Required),
        (b"absent.txt".to_vec(), Obligation::Optional),
    ])
    .unwrap();
    let with_absence =
        capture_selected_local_source_input(&root, LocalSourceLimits::default(), &optional)
            .unwrap();
    assert_eq!(narrow.tree(), with_absence.tree());
    assert_ne!(
        narrow.selection_identity(),
        with_absence.selection_identity()
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn selected_files_do_not_open_unselected_special_members_and_required_absence_fails() {
    let root = temp_root("selected");
    // macOS temporary roots can already exceed the Unix socket pathname bound.
    #[cfg(unix)]
    let root = Path::new("/tmp").join(root.file_name().unwrap());
    std::fs::create_dir_all(root.join("unselected")).unwrap();
    std::fs::write(root.join("main.omg"), "machine main() {}").unwrap();
    #[cfg(unix)]
    let _socket =
        std::os::unix::net::UnixListener::bind(root.join("unselected/source.sock")).unwrap();
    let narrow = capture_selected_local_source_input(
        &root,
        LocalSourceLimits::default(),
        &selected(&["main.omg"]),
    )
    .unwrap();
    assert!(!narrow.tree().exists(Path::new("unselected")));
    let error = capture_selected_local_source_input(
        &root,
        LocalSourceLimits::default(),
        &selected(&["absent.omg"]),
    )
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("required source capture entry `absent.omg` is absent")
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn whole_capture_keeps_native_filename_identity_with_a_literal_backslash() {
    let root = temp_root("selected-source-native-name");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join(r"literal\name.omg"), "native source bytes").unwrap();
    let captured = capture_local_source_input(&root, LocalSourceLimits::default()).unwrap();
    assert_eq!(
        captured
            .tree()
            .read_to_string(Path::new(r"literal\name.omg"))
            .unwrap()
            .as_ref(),
        "native source bytes"
    );
    assert!(!captured.tree().exists(Path::new("literal/name.omg")));
    std::fs::remove_dir_all(root).unwrap();
}
