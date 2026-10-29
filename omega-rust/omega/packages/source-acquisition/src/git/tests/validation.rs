use super::{
    LocalSourceLimits, SourceResolveError, parse_git_tree_entries, temp_root,
    validate_git_symlink_target,
};

#[test]
fn git_entry_paths_use_native_separators_without_changing_identity_bytes() {
    let repository = temp_root("git-native-entry-paths");
    let oid = "0123456789012345678901234567890123456789";
    let paths = [".agents", ".agents/skills", ".agents/skills/example"];
    let listing = paths
        .iter()
        .map(|path| format!("040000 tree {oid} -\t{path}\0"))
        .collect::<String>();
    let entries = parse_git_tree_entries(
        listing.as_bytes(),
        &repository,
        LocalSourceLimits::default(),
    )
    .unwrap();
    for (entry, spelling) in entries.iter().zip(paths) {
        let native: std::path::PathBuf = spelling.split('/').collect();
        assert_eq!(entry.relative_path.as_os_str(), native.as_os_str());
        assert_eq!(entry.relative_bytes, spelling.as_bytes());
    }
    let tree = source::CapturedSourceTree::new(
        entries
            .into_iter()
            .map(|entry| (entry.relative_path, source::CapturedSourceNode::Directory)),
    )
    .unwrap();
    assert!(tree.is_dir(std::path::Path::new(".agents/skills/example")));
    assert!(!repository.exists());
}

#[test]
fn git_tree_rejects_traversal_metadata_and_nonportable_paths_before_materialization() {
    let repository = temp_root("git-tree-path-validation");
    let oid = "0123456789012345678901234567890123456789";
    for path in [
        b"../escape.omg".as_slice(),
        b"nested/../../escape.omg".as_slice(),
        b"/absolute.omg".as_slice(),
        b"nested\\ambiguous.omg".as_slice(),
        b"nested/.git/config".as_slice(),
        b"C:/drive-prefixed.omg".as_slice(),
        b"nested/NUL.txt".as_slice(),
        b"aux.omg".as_slice(),
        b"name:stream.omg".as_slice(),
        b"trailing.".as_slice(),
        b"trailing ".as_slice(),
        b"question?.omg".as_slice(),
    ] {
        let mut listing = format!("100644 blob {oid} 1\t").into_bytes();
        listing.extend_from_slice(path);
        listing.push(0);
        let error = parse_git_tree_entries(&listing, &repository, LocalSourceLimits::default())
            .expect_err("unsafe Git path must reject");
        assert!(matches!(error, SourceResolveError::GitTreeInvalid { .. }));
    }
    assert!(
        !repository.exists(),
        "validation must not create a staging path"
    );
}

#[test]
fn git_symlink_targets_reject_windows_ambiguous_spellings() {
    for target in [
        b"C:/escape.omg".as_slice(),
        b"NUL".as_slice(),
        b"nested/COM1.log".as_slice(),
        b"name:stream".as_slice(),
        b"trailing.".as_slice(),
    ] {
        assert!(matches!(
            validate_git_symlink_target(b"link", target),
            Err(SourceResolveError::GitTreeInvalid { .. })
        ));
    }
}

#[test]
fn git_tree_entry_limit_counts_declared_directories() {
    let repository = temp_root("git-tree-directory-limit");
    let oid = "0123456789012345678901234567890123456789";
    let listing = format!("040000 tree {oid} -\tnested\0100644 blob {oid} 0\tnested/main.omg\0");

    let error = parse_git_tree_entries(
        listing.as_bytes(),
        &repository,
        LocalSourceLimits {
            max_entries: 1,
            ..LocalSourceLimits::default()
        },
    )
    .expect_err("directory and blob must consume separate identity entries");

    assert_eq!(error, SourceResolveError::TooManyFiles { limit: 1 });
    assert!(!repository.exists());
}
