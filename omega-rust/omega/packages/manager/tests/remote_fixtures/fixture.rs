use super::{local_package_root, temp_root, workspace_root};
use package_source::{LocalSourceLimits, resolve_local_source};
use std::fs;
use std::path::{Path, PathBuf};

pub(super) struct Fixture(PathBuf);

impl Fixture {
    pub(super) fn new() -> Self {
        let root = temp_root("sources");
        fs::create_dir(&root).unwrap();
        Self(root)
    }

    pub(super) fn path(&self, relative: &str) -> PathBuf {
        self.0.join(relative)
    }

    pub(super) fn expected_package(&self, package: &str) -> PathBuf {
        let local_root = local_package_root(package);
        let remote_build = workspace_root()
            .join("tests/fixtures/package-remotes")
            .join(package)
            .join("build.omg");
        if !remote_build.exists() {
            return local_root;
        }
        assert!(matches!(
            package,
            "file-journal" | "process-exit" | "remote-journal" | "graph-workbench"
        ));
        let local = resolve_local_source(&local_root, LocalSourceLimits::default()).unwrap();
        assert_eq!(
            local.file_count, 3,
            "update the complete remote expectation when adding fixture files"
        );
        let expected = self.path("expected").join(package);
        fs::create_dir_all(&expected).unwrap();
        for name in ["README.md", "main.omg"] {
            fs::copy(local_root.join(name), expected.join(name)).unwrap();
        }
        fs::copy(remote_build, expected.join("build.omg")).unwrap();
        expected
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        make_removable(&self.0);
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!(
                "could not clean remote fixture {}: {error}",
                self.0.display()
            );
        }
    }
}

#[cfg_attr(windows, allow(clippy::permissions_set_readonly_false))]
fn make_removable(path: &Path) {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return;
    };
    if metadata.file_type().is_symlink() {
        return;
    }
    #[cfg(unix)]
    if metadata.is_dir() {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(
            path,
            fs::Permissions::from_mode(metadata.permissions().mode() | 0o700),
        );
    }
    #[cfg(windows)]
    {
        let mut permissions = metadata.permissions();
        permissions.set_readonly(false);
        let _ = fs::set_permissions(path, permissions);
    }
    if metadata.is_dir()
        && let Ok(entries) = fs::read_dir(path)
    {
        for entry in entries.flatten() {
            make_removable(&entry.path());
        }
    }
}
