use package_manager::lock::{PackageLock, PackageLockRecoveryLimits};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub(super) struct Fixture(PathBuf);

impl Fixture {
    pub(super) fn new() -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "omega-package-cli-{}-{stamp}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        let fixture = Self(fs::canonicalize(path).unwrap());
        fs::create_dir(fixture.path("root")).unwrap();
        fs::create_dir(fixture.path("dependency")).unwrap();
        fixture.write(
            "root/build.omg",
            "machine build(builder: &mut Build) {\n    builder.package(\"cli-project\");\n}\n",
        );
        fixture.write("root/main.omg", "machine main() {}\n");
        fixture.write("dependency/build.omg", "machine build(builder: &mut Build) {\n    builder.package(\"arithmetic-kernels\");\n}\n");
        fixture.write("dependency/main.omg", "pub machine value() -> u64 { 7 }\n");
        fixture
    }

    pub(super) fn with_assumption() -> Self {
        let fixture = Self::new();
        fixture.write(
            "dependency/main.omg",
            "boundary machine trusted_zero() -> u64 ensures result == 0;\n",
        );
        fixture
    }

    pub(super) fn path(&self, relative: &str) -> PathBuf {
        self.0.join(relative)
    }
    pub(super) fn write(&self, relative: &str, contents: &str) {
        fs::write(self.path(relative), contents).unwrap();
    }
    pub(super) fn read(&self, relative: &str) -> String {
        fs::read_to_string(self.path(relative)).unwrap()
    }

    pub(super) fn omega(&self, arguments: &[&str]) -> Output {
        let executable = fs::canonicalize(env!("CARGO_BIN_EXE_omega"))
            .expect("resolve omega before changing child directory");
        // Use the resolver's supported per-user cache selection. Cleanup owns
        // only this fixture; shared immutable source snapshots are not ours.
        Command::new(executable)
            .current_dir(self.path("root"))
            .args(arguments)
            .output()
            .expect("run omega")
    }

    pub(super) fn accepted_files(&self) -> (Option<Vec<u8>>, Option<Vec<u8>>) {
        (
            read_optional(&self.path("root/build.omg")),
            read_optional(&self.path("root/omega.lock")),
        )
    }

    pub(super) fn lock(&self) -> PackageLock {
        PackageLock::recover_text(
            &self.read("root/omega.lock"),
            PackageLockRecoveryLimits::default(),
        )
        .expect("CLI published a structurally valid lock")
    }

    pub(super) fn assert_published(&self, before: &(Option<Vec<u8>>, Option<Vec<u8>>)) {
        let after = self.accepted_files();
        assert_ne!(after.0, before.0, "install did not edit build.omg");
        assert!(
            after.1.as_ref().is_some_and(|bytes| !bytes.is_empty()),
            "install did not publish omega.lock"
        );
        let lock = self.lock();
        assert!(!lock.targets().is_empty());
        for target in lock.targets() {
            assert!(
                target
                    .source()
                    .packages()
                    .iter()
                    .any(|package| package.key().name().as_str() == "arithmetic-kernels"),
                "installed package missing from lock"
            );
        }
    }

    pub(super) fn review_paths(&self, output: &Output) -> Vec<PathBuf> {
        let stdout = String::from_utf8(output.stdout.clone()).unwrap();
        let paths: Vec<_> = stdout
            .lines()
            .filter_map(|line| line.strip_prefix("review: "))
            .map(PathBuf::from)
            .collect();
        assert!(
            !paths.is_empty(),
            "review required without printed paths: {stdout}"
        );
        for path in &paths {
            assert!(
                path.starts_with(self.path("root/build/package-manager")),
                "review path escaped project: {}",
                path.display()
            );
            assert!(
                path.is_file(),
                "printed review path does not exist: {}",
                path.display()
            );
        }
        paths
    }
}

fn read_optional(path: &Path) -> Option<Vec<u8>> {
    match fs::read(path) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => panic!("cannot read {}: {error}", path.display()),
    }
}

pub(super) fn assert_status(output: &Output, expected: i32) {
    assert_eq!(
        output.status.code(),
        Some(expected),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

impl Drop for Fixture {
    fn drop(&mut self) {
        make_directories_removable(&self.0);
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!(
                "cannot clean package CLI fixture {}: {error}",
                self.0.display()
            );
        }
    }
}

#[cfg_attr(windows, allow(clippy::permissions_set_readonly_false))]
fn make_directories_removable(path: &Path) {
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
            make_directories_removable(&entry.path());
        }
    }
}
