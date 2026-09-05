use super::*;
use package_source::local::staging::stage_local_source_replacement_in_lane;
use sha2::{Digest, Sha256};
use std::process::Command;

const CHILD: &str = "OMEGA_OFFLINE_GRAPH_TEST_ROOT";
pub(super) const REPOSITORY: &str = "git@offline-fixture.invalid:repository.git";

pub(super) struct Fixture {
    pub(super) directory: PathBuf,
}

// Use the command fixtures' child-only SSH transport: real Git serves the
// temporary repository and no environment settings change in the test runner.
pub(super) fn run(test: &str, operation: impl FnOnce(&Fixture)) {
    if let Some(directory) = std::env::var_os(CHILD) {
        operation(&Fixture {
            directory: directory.into(),
        });
        return;
    }
    let directory = temp_root(test);
    std::fs::create_dir_all(&directory).unwrap();
    let fixture = Fixture {
        directory: std::fs::canonicalize(directory).unwrap(),
    };
    let quote = |path: PathBuf| format!("'{}'", path.to_str().unwrap().replace('\'', "'\\''"));
    std::fs::write(fixture.path("ssh-transport.sh"), format!(
        "#!/bin/sh\nprintf 'call\\n' >> {}\ncase \"$*\" in\n *git-upload-pack*) exec git upload-pack {} ;;\n *) exit 2 ;;\nesac\n",
        quote(fixture.path("transport.log")), quote(fixture.path("repository")),
    )).unwrap();
    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            &format!("resolution::graph::resolve::tests::offline::{test}"),
            "--nocapture",
        ])
        .env(CHILD, &fixture.directory)
        .env(
            "GIT_SSH_COMMAND",
            format!("sh {}", quote(fixture.path("ssh-transport.sh"))),
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
    // Require the exact child filter to execute a test, rather than pass empty.
    assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
    remove_fixture(&fixture.directory);
}

fn remove_fixture(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let metadata = std::fs::symlink_metadata(path).unwrap();
    if metadata.is_dir() {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
        for entry in std::fs::read_dir(path).unwrap() {
            remove_fixture(&entry.unwrap().path());
        }
        std::fs::remove_dir(path).unwrap();
    } else {
        std::fs::remove_file(path).unwrap();
    }
}

impl Fixture {
    pub(super) fn path(&self, path: &str) -> PathBuf {
        self.directory.join(path)
    }

    pub(super) fn storage(&self, name: &str) -> SourceResolverStorage {
        SourceResolverStorage::for_hardened_base(self.path(name)).unwrap()
    }

    pub(super) fn repository(&self) {
        write_package(&self.path("repository"), "dependency", None);
        run_test_git(&self.path("repository"), ["init", "--quiet"]);
        self.commit();
    }

    pub(super) fn commit(&self) {
        run_test_git(&self.path("repository"), ["add", "."]);
        run_test_git(
            &self.path("repository"),
            [
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
            ],
        );
    }

    pub(super) fn root(&self, dependencies: &str) {
        write_application(&self.path("root"), "consumer", None);
        std::fs::write(self.path("root/build.omg"), build(dependencies)).unwrap();
    }

    pub(super) fn resolve(
        &self,
        storage: &SourceResolverStorage,
        proposed: Option<&str>,
        options: GitResolutionOptions<'_>,
    ) -> Result<ResolvedPackageSourceClosure, ResolveExternalLocalPackageClosureError> {
        let context = ExternalSourceContext::derive(b"offline-graph");
        let root = self.path("root");
        if let Some(dependencies) = proposed {
            let original = std::fs::read(root.join("build.omg")).unwrap();
            let stage = stage_local_source_replacement_in_lane(
                &root,
                &SourceRelativePath::parse("build.omg").unwrap(),
                &Sha256::digest(original).into(),
                build(dependencies).as_bytes(),
                storage.external_local_sources(),
                LocalSourceLimits::default(),
            )
            .unwrap();
            resolve_staged_external_local_project_closure_with_options(
                &stage,
                context,
                storage,
                LocalSourceLimits::default(),
                PackageSourceClosureLimits::default(),
                options,
            )
        } else {
            resolve_external_local_project_closure_with_options(
                &root,
                context,
                storage,
                LocalSourceLimits::default(),
                PackageSourceClosureLimits::default(),
                options,
            )
        }
    }

    pub(super) fn transport_calls(&self) -> usize {
        match std::fs::read_to_string(self.path("transport.log")) {
            Ok(text) => text.lines().count(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
            Err(error) => panic!("read transport log: {error}"),
        }
    }
}

pub(super) fn build(dependencies: &str) -> String {
    format!(
        "machine build(builder: &mut Build) {{ builder.application(\"consumer\"); {dependencies} }}\n"
    )
}

pub(super) fn dependency(revision: &str) -> String {
    format!(
        "builder.depend(Source::Git {{ repository: \"{REPOSITORY}\", revision: \"{revision}\" }});"
    )
}

pub(super) fn subject(closure: &ResolvedPackageSourceClosure) -> CanonicalSourceClosureSubject {
    CanonicalSourceClosureSubject::from_resolved(
        &closure.for_exact_target(target::TargetProfile::CrossPlatformCli),
        CanonicalSourceClosureSubjectLimits::default(),
    )
    .unwrap()
}

pub(super) fn offline(pins: Option<GitDependencyPins<'_>>) -> GitResolutionOptions<'_> {
    GitResolutionOptions {
        pins,
        offline: true,
    }
}

pub(super) fn assert_selection_rejected(error: ResolveExternalLocalPackageClosureError) {
    assert!(
        error
            .to_string()
            .contains("offline resolution forbids new or refreshed Git selection"),
        "{error}"
    );
}
