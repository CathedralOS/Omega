use super::*;

fn prepare(project: &Project) -> Result<Option<PreparedLocalProject>, PrepareLocalProjectError> {
    prepare_with_options_and_storage(
        &project.root().join("main.omg"),
        LocalProjectPreparationOptions {
            target: TargetProfile::host(),
            offline: true,
        },
        |_| Ok(project.storage()),
    )
}

#[test]
fn offline_local_preparation_allows_root_edits_and_unlocked_local_dependencies() {
    let project = Project::new("application");
    assert!(prepare(&project).unwrap().is_some());
    assert!(!project.root().join("omega.lock").exists());
    project.lock();
    let before = fs::read(project.root().join("omega.lock")).unwrap();
    let edited = "pub machine value() -> u64 { 23 }\n";
    fs::write(project.root().join("main.omg"), edited).unwrap();
    let prepared = prepare(&project).unwrap().unwrap();
    assert_eq!(fs::read_to_string(prepared.entry_path).unwrap(), edited);
    assert_eq!(fs::read(project.root().join("omega.lock")).unwrap(), before);
}

#[cfg(unix)]
#[test]
fn offline_git_preparation_keeps_exact_pins_and_never_calls_transport() {
    use std::process::Command;
    const CHILD: &str = "OMEGA_OFFLINE_PREPARATION_ROOT";
    let Some(directory) = std::env::var_os(CHILD) else {
        let project = Project::new("package");
        let repository = project.0.join("dependency");
        let script = project.0.join("transport.sh");
        let log = project.0.join("transport.log");
        let quote = |path: &Path| format!("'{}'", path.to_str().unwrap().replace('\'', "'\\''"));
        fs::write(
            &script,
            format!(
                "#!/bin/sh\nprintf 'call\\n' >> {}\nexec git upload-pack {}\n",
                quote(&log),
                quote(&repository)
            ),
        )
        .unwrap();
        let output = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "operations::prepare_project::tests::locked::offline::offline_git_preparation_keeps_exact_pins_and_never_calls_transport", "--nocapture"])
            .env(CHILD, &project.0)
            .env("GIT_SSH_COMMAND", format!("sh {}", quote(&script)))
            .env("GIT_SSH_VARIANT", "simple")
            .output().unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed; 0 failed"));
        return;
    };
    // The parent owns cleanup. Child-only environment preserves other tests'
    // transport settings and uses production Git object and source checking.
    let project = std::mem::ManuallyDrop::new(Project(PathBuf::from(directory)));
    let repository = project.0.join("dependency");
    let git = |arguments: &[&str]| {
        let output = Command::new("git")
            .current_dir(&repository)
            .args(arguments)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    git(&["init", "--quiet"]);
    git(&["add", "."]);
    let commit = || {
        git(&[
            "-c",
            "user.name=Omega Tests",
            "-c",
            "user.email=omega@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--quiet",
            "-m",
            "fixture",
        ])
    };
    commit();
    let locator = "git@offline-preparation.invalid:dependency.git";
    fs::write(project.root().join("build.omg"), format!("machine build(builder: &mut Build) {{ builder.package(\"root\"); builder.depend(Source::Git {{ repository: \"{locator}\", revision: \"HEAD\" }}); }}\n")).unwrap();
    let initial = project.prepare().unwrap().unwrap();
    let lock = project.lock_closure(&initial.source_closure);
    drop(initial);
    let before = fs::read(project.root().join("omega.lock")).unwrap();
    let build = fs::read(project.root().join("build.omg")).unwrap();
    let edited = "pub machine value() -> u64 { 31 }\n";
    fs::write(project.root().join("main.omg"), edited).unwrap();
    fs::write(
        repository.join("main.omg"),
        "pub machine value() -> u64 { 99 }\n",
    )
    .unwrap();
    git(&["add", "."]);
    commit();
    let request = package_source::GitSourceRequest::new(locator, Some("HEAD".into())).unwrap();
    let advanced = crate::resolution::source::resolve_git_package_source_with_storage(
        &request,
        &project.storage(),
        LocalSourceLimits::default(),
    )
    .unwrap();
    let expected = lock.targets()[0]
        .source()
        .packages()
        .iter()
        .find(|package| package.key().name().as_str() == "dependency")
        .unwrap();
    assert_ne!(advanced.resolution(), expected.resolution());
    drop(advanced);
    let calls = fs::read(project.0.join("transport.log")).unwrap();
    assert!(
        !calls.is_empty(),
        "online setup must exercise the test transport"
    );
    let prepared = prepare(&project).unwrap().unwrap();
    assert_eq!(fs::read_to_string(&prepared.entry_path).unwrap(), edited);
    assert_eq!(
        prepared
            .source_closure
            .custody(expected.key())
            .unwrap()
            .resolution(),
        expected.resolution()
    );
    drop(prepared);
    assert_eq!(fs::read(project.0.join("transport.log")).unwrap(), calls);
    fs::rename(project.0.join("cache"), project.0.join("warm-cache")).unwrap();
    let error = prepare(&project)
        .err()
        .expect("cold offline pins must fail");
    assert!(
        error
            .to_string()
            .contains("unavailable in the retained cache"),
        "{error}"
    );
    assert_eq!(fs::read(project.0.join("transport.log")).unwrap(), calls);
    assert_eq!(fs::read(project.root().join("omega.lock")).unwrap(), before);
    assert_eq!(fs::read(project.root().join("build.omg")).unwrap(), build);
}

#[cfg(not(unix))]
#[test]
#[ignore = "offline Git transport counter requires a Unix shell; offline local preparation runs on every host"]
fn offline_git_preparation_keeps_exact_pins_and_never_calls_transport() {}
