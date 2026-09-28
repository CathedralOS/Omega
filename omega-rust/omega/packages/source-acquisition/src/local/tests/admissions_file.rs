#[cfg(unix)]
use super::SourceResolveError;
use super::{LocalSourceLimits, resolve_local_source, resolve_materialized_source, temp_root};

#[cfg(unix)]
#[test]
fn local_symlinks_cannot_reintroduce_root_admissions_contents() {
    let root = temp_root("local-admissions-symlink");
    std::fs::create_dir_all(root.join("nested")).unwrap();
    let alias = root.join("nested/alias");
    for name in ["omega.admissions", "OMEGA.ADMISSIONS", "Omega.Admissions"] {
        std::fs::write(root.join(name), "compiler policy").unwrap();
        std::os::unix::fs::symlink(format!("../{name}"), &alias).unwrap();
        assert!(matches!(
            resolve_local_source(&root, LocalSourceLimits::default()),
            Err(SourceResolveError::SymlinkTargetsExcludedMetadata { .. })
        ));
        // Exact repository trees still capture the link and its policy target.
        assert_eq!(
            resolve_materialized_source(&root, LocalSourceLimits::default())
                .unwrap()
                .file_count,
            2
        );
        std::fs::remove_file(&alias).unwrap();
        std::fs::remove_file(root.join(name)).unwrap();
    }
    std::fs::write(root.join("nested/omega.admissions"), "ordinary source").unwrap();
    std::os::unix::fs::symlink("nested/omega.admissions", root.join("alias")).unwrap();
    assert_eq!(
        resolve_local_source(&root, LocalSourceLimits::default())
            .unwrap()
            .file_count,
        2
    );
    std::fs::remove_dir_all(root).unwrap();
}
