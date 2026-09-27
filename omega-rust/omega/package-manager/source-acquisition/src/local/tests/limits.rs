use super::{LocalSourceLimits, SourceResolveError, resolve_local_source, temp_root};

#[test]
fn local_directory_collection_is_bounded_without_counting_reserved_exclusions() {
    let root = temp_root("bounded-directory-listing");
    std::fs::create_dir_all(root.join(".git")).expect("create excluded metadata");
    std::fs::create_dir_all(root.join("build")).expect("create excluded build output");
    std::fs::write(root.join("omega.lock"), "project state").expect("write excluded lock");
    std::fs::write(root.join("omega.admissions"), "compiler policy")
        .expect("write excluded compiler admissions");
    std::fs::write(root.join("first.omg"), "").expect("write first source");
    std::fs::write(root.join("second.omg"), "").expect("write second source");
    let limits = LocalSourceLimits {
        max_entries: 2,
        ..LocalSourceLimits::default()
    };

    let accepted = resolve_local_source(&root, limits)
        .expect("reserved exclusions must not consume source identity entries");
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
            max_entries: 1,
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
