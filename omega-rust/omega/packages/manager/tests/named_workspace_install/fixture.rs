use package_manager::lock::{PackageLock, PackageLockRecoveryLimits};
use package_manager::operations::{
    PackageCommand, PackageCommandError, PackageCommandKind, PackageCommandOptions,
    PackageCommandOutcome, execute_package_command_with_storage,
};
use package_source::SourceResolverStorage;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use target::TargetProfile;

pub(super) const REPOSITORY: &str = "git@named-fixture.invalid:workspace.git";
pub(super) const PURE: &str = "pub machine value() -> u64 { 7 }\n";
pub(super) const ASSUMPTION: &str = "boundary machine trusted_zero() -> u64 ensures result == 0;\n";
const CHILD: &str = "OMEGA_NAMED_INSTALL_TEST_ROOT";
static SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub(super) struct Fixture {
    directory: PathBuf,
    cleanup: bool,
}

// Child-only transport settings avoid mutating the test runner's environment
// or the invoking user's Git/SSH configuration. Git object acquisition,
// workspace projection and compiler checks are the production operations.
pub(super) fn run(test: &str, operation: impl FnOnce(&Fixture)) {
    if let Some(directory) = std::env::var_os(CHILD) {
        operation(&Fixture {
            directory: directory.into(),
            cleanup: false,
        });
        return;
    }
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "omega-named-install-{}-{stamp}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&directory).unwrap();
    let fixture = Fixture {
        directory: fs::canonicalize(directory).unwrap(),
        cleanup: true,
    };
    fixture.package("root", "consumer", "");
    fixture.package("repository/modules/selected", "exact-math", "");
    fixture.package("repository/modules/other", "other-library", "");
    fixture.write(
        "repository/build.omg",
        concat!(
            "machine build(builder: &mut Build) {\n",
            " builder.member(\"modules/selected\");\n",
            " builder.member(\"modules/other\");\n}\n",
        ),
    );
    fixture.git(&["init", "--quiet"]);
    // This is a test transport endpoint, not a replacement for source parsing
    // or semantic discovery. The server always serves this temporary repo.
    let script = format!(
        "#!/bin/sh\nprintf 'call\\n' >> {}\ncase \"$*\" in\n *git-upload-pack*) exec git upload-pack {} ;;\n *) exit 2 ;;\nesac\n",
        quote(fixture.path("transport-calls").to_str().unwrap()),
        quote(fixture.path("repository").to_str().unwrap())
    );
    fixture.write("transport-calls", "");
    fixture.write("ssh-transport.sh", &script);
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", &format!("cases::{test}"), "--nocapture"])
        .env(CHILD, &fixture.directory)
        .env(
            "GIT_SSH_COMMAND",
            format!(
                "sh {}",
                quote(fixture.path("ssh-transport.sh").to_str().unwrap())
            ),
        )
        .env("GIT_SSH_VARIANT", "simple")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{test}\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("1 passed; 0 failed"),
        "child must execute exactly cases::{test}: {}",
        String::from_utf8_lossy(&output.stdout)
    );
}

fn quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

impl Fixture {
    pub(super) fn path(&self, path: &str) -> PathBuf {
        self.directory.join(path)
    }
    pub(super) fn write(&self, path: &str, text: &str) {
        fs::write(self.path(path), text).unwrap();
    }
    pub(super) fn read(&self, path: &str) -> String {
        fs::read_to_string(self.path(path)).unwrap()
    }

    pub(super) fn package(&self, path: &str, name: &str, dependencies: &str) {
        fs::create_dir_all(self.path(path)).unwrap();
        self.write(&format!("{path}/build.omg"), &format!("machine build(builder: &mut Build) {{\n builder.package(\"{name}\");\n{dependencies}}}\n"));
        self.write(&format!("{path}/main.omg"), PURE);
    }

    pub(super) fn git(&self, arguments: &[&str]) -> String {
        let output = Command::new("git")
            .current_dir(self.path("repository"))
            .args(arguments)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "fixture Git: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    }

    pub(super) fn commit(&self) -> String {
        self.git(&["add", "."]);
        self.git(&[
            "-c",
            "user.name=Omega Tests",
            "-c",
            "user.email=omega@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--quiet",
            "-m",
            "fixture source",
        ]);
        self.git(&["rev-parse", "HEAD"])
    }

    pub(super) fn execute(
        &self,
        command: PackageCommand,
    ) -> Result<PackageCommandOutcome, PackageCommandError> {
        self.execute_with_offline(command, false)
    }

    pub(super) fn transport_calls(&self) -> usize {
        self.read("transport-calls").lines().count()
    }

    pub(super) fn execute_with_offline(
        &self,
        command: PackageCommand,
        offline: bool,
    ) -> Result<PackageCommandOutcome, PackageCommandError> {
        let calls = self.transport_calls();
        let targets = if matches!(
            command,
            PackageCommand::Resume { .. } | PackageCommand::DiscardReview
        ) {
            Vec::new()
        } else {
            vec![TargetProfile::WindowsX64]
        };
        let result = execute_package_command_with_storage(
            command,
            PackageCommandOptions {
                project_root: self.path("root"),
                targets,
                offline,
            },
            &SourceResolverStorage::for_hardened_base(self.path("cache")).unwrap(),
        );
        if offline {
            assert_eq!(
                self.transport_calls(),
                calls,
                "offline command used transport"
            );
        }
        result
    }

    pub(super) fn install(
        &self,
        name: Option<&str>,
        alias: Option<&str>,
    ) -> Result<PackageCommandOutcome, PackageCommandError> {
        self.execute(PackageCommand::Install {
            source: REPOSITORY.into(),
            revision: None,
            alias: alias.map(str::to_owned),
            package: name.map(str::to_owned),
        })
    }

    pub(super) fn resume(&self) -> Result<PackageCommandOutcome, PackageCommandError> {
        self.execute(PackageCommand::Resume {
            kind: PackageCommandKind::Install,
        })
    }

    #[allow(dead_code)]
    pub(super) fn accept_required(&self, outcome: &PackageCommandOutcome) {
        let mut decisions = 0;
        for path in &outcome.review_paths {
            let before = fs::read_to_string(path).unwrap();
            let after: String = before
                .split_inclusive('\n')
                .map(|line| {
                    if line.starts_with("decision ") {
                        decisions += 1;
                        format!("{} accept\n", line.strip_suffix(" pending\n").unwrap())
                    } else {
                        line.to_owned()
                    }
                })
                .collect();
            fs::write(path, after).unwrap();
        }
        assert!(decisions > 0, "fixture must require an explicit decision");
    }

    pub(super) fn pair(&self) -> (String, Option<String>) {
        let lock = match fs::read_to_string(self.path("root/omega.lock")) {
            Ok(text) => Some(text),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => panic!("read lock: {error}"),
        };
        (self.read("root/build.omg"), lock)
    }

    pub(super) fn lock(&self) -> PackageLock {
        let text = self.read("root/omega.lock");
        let lock = PackageLock::recover_text(&text, PackageLockRecoveryLimits::default()).unwrap();
        assert_eq!(lock.canonical_text().unwrap(), text);
        lock
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if self.cleanup {
            removable(&self.directory);
            fs::remove_dir_all(&self.directory).unwrap();
        }
    }
}

fn removable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let metadata = fs::symlink_metadata(path).unwrap();
    if metadata.is_dir() {
        fs::set_permissions(
            path,
            fs::Permissions::from_mode(metadata.permissions().mode() | 0o700),
        )
        .unwrap();
        for entry in fs::read_dir(path).unwrap() {
            removable(&entry.unwrap().path());
        }
    }
}
