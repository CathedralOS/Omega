//! Named command selection reaches ordinary Git workspace discovery and review.

#[cfg(unix)]
#[path = "named_workspace_install/cases.rs"]
mod cases;
#[cfg(unix)]
use crate::named_workspace_fixture as fixture;

#[cfg(not(unix))]
#[test]
#[ignore = "named Git command integration uses a Unix test-only SSH transport; portable CLI parsing and declaration tests run separately"]
fn named_git_transport_fixture_requires_unix_shell() {}

// Local filesystem sources exercise the same command paths without the Git
// transport, so these cases run on hosts without the test-only SSH transport.
pub(crate) mod local {
    use package_manager::lock::{PackageLock, PackageLockRecoveryLimits};
    use package_manager::{
        PackageCommand, PackageCommandError, PackageCommandOptions, PackageCommandOutcome,
        PackageCommandStatus, execute_package_command,
    };
    use package_source::{PrimaryGitChoices, SourceResolverStorage};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    use target::TargetProfile;

    const PURE: &str = "pub machine value() -> u64 { 7 }\n";
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);

    pub(crate) struct Fixture {
        directory: PathBuf,
    }

    impl Fixture {
        pub(crate) fn new(topic: &str) -> Self {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let directory = std::env::temp_dir().join(format!(
                "omega-local-command-{topic}-{}-{stamp}-{}",
                std::process::id(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&directory).unwrap();
            let fixture = Self {
                directory: fs::canonicalize(directory).unwrap(),
            };
            fixture.package("root", "consumer", "");
            fixture
        }

        pub(crate) fn path(&self, path: &str) -> PathBuf {
            self.directory.join(path)
        }
        pub(crate) fn write(&self, path: &str, text: &str) {
            fs::write(self.path(path), text).unwrap();
        }
        pub(crate) fn read(&self, path: &str) -> String {
            fs::read_to_string(self.path(path)).unwrap()
        }

        pub(crate) fn package(&self, path: &str, name: &str, dependencies: &str) {
            fs::create_dir_all(self.path(path)).unwrap();
            self.write(
                &format!("{path}/build.omg"),
                &format!(
                    "machine build(builder: &mut Build) {{\n builder.package(\"{name}\");\n{dependencies}}}\n"
                ),
            );
            self.write(&format!("{path}/main.omg"), PURE);
        }

        pub(crate) fn execute(
            &self,
            command: PackageCommand,
        ) -> Result<PackageCommandOutcome, PackageCommandError> {
            self.execute_with_offline(command, false)
        }

        pub(crate) fn execute_with_offline(
            &self,
            command: PackageCommand,
            offline: bool,
        ) -> Result<PackageCommandOutcome, PackageCommandError> {
            execute_package_command(
                command,
                PackageCommandOptions {
                    project_root: self.path("root"),
                    targets: vec![TargetProfile::WindowsX64],
                    offline,
                    build_inputs: None,
                },
                Some(
                    &SourceResolverStorage::for_hardened_base(
                        self.path("cache"),
                        PrimaryGitChoices::default(),
                    )
                    .unwrap(),
                ),
            )
        }

        pub(crate) fn pair(&self) -> (String, Option<String>) {
            let lock = match fs::read_to_string(self.path("root/omega.lock")) {
                Ok(text) => Some(text),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                Err(error) => panic!("read lock: {error}"),
            };
            (self.read("root/build.omg"), lock)
        }

        pub(crate) fn lock(&self) -> PackageLock {
            let text = self.read("root/omega.lock");
            let lock =
                PackageLock::recover_text(&text, PackageLockRecoveryLimits::default()).unwrap();
            assert_eq!(lock.canonical_text().unwrap(), text);
            lock
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            removable(&self.directory);
            fs::remove_dir_all(&self.directory).unwrap();
        }
    }

    // Hardened cache contents arrive read-only; restore owner write before
    // removal so cleanup works on hosts that enforce the bit.
    #[cfg_attr(windows, allow(clippy::permissions_set_readonly_false))]
    fn removable(path: &Path) {
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
                removable(&entry.path());
            }
        }
    }

    #[test]
    fn named_selection_rejects_local_sources() {
        let fixture = Fixture::new("named-local-selection");
        fixture.package("local/modules/selected", "exact-math", "");
        let before = fixture.pair();
        let error = fixture
            .execute(PackageCommand::Install {
                source: "../local".into(),
                revision: None,
                alias: None,
                package: Some("exact-math".into()),
            })
            .unwrap_err()
            .to_string();
        assert!(error.contains("only valid for a Git source"), "{error}");
        assert_eq!(fixture.pair(), before);
    }

    #[test]
    fn local_install_with_alias_publishes_under_the_alias() {
        let fixture = Fixture::new("named-local-alias");
        fixture.package("local", "local-library", "");
        let installed = fixture
            .execute(PackageCommand::Install {
                source: "../local".into(),
                revision: None,
                alias: Some("math".into()),
                package: None,
            })
            .unwrap();
        assert_eq!(
            installed.status,
            PackageCommandStatus::Published,
            "{}",
            installed.report
        );
        let (build, _) = fixture.pair();
        assert!(build.contains("depend_as(\"math\""), "{build}");
        assert!(
            build.contains("Source::Path { location: \"../local\" }"),
            "{build}"
        );
        let lock = fixture.lock();
        let edge = &lock.targets()[0].source().dependency_requests()[0];
        assert_eq!(edge.alias().as_str(), "math");
    }
}
