//! Executed Git acquisition through the real application facade and two engines.
#[path = "../tests/support/fixture_package_inputs.rs"]
mod fixture_package_inputs;
#[path = "../tests/support/joined_product_execution.rs"]
mod joined_product_execution;

use std::path::PathBuf;

fn main() {
    if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("--build-profile")) {
        println!("{}", compiler::compiler_build_profile());
        return;
    }
    if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("--worker")) {
        execute_fixture();
        return;
    }
    #[cfg(unix)]
    fixture_process();
    #[cfg(not(unix))]
    panic!("the local Git transport adapter currently requires a Unix host");
}

#[cfg(unix)]
fn fixture_process() {
    use std::os::unix::fs::PermissionsExt;
    let temporary =
        platform_custody::temporary_directory::TemporaryDirectory::create("executed-git", false)
            .unwrap();
    let directory = temporary.path().canonicalize().unwrap();
    let repository = fixture_package_inputs::repo_root();
    let remote = directory.join("remote");
    std::fs::create_dir(&remote).unwrap();
    let library = repository.join("tests/omega/pass/build/executed_dependency_source_library");
    for name in ["build.omg", "main.omg"] {
        std::fs::copy(library.join(name), remote.join(name)).unwrap();
    }
    let search = std::env::var_os("PATH").unwrap();
    let real_git = std::env::split_paths(&search)
        .map(|directory| directory.join("git"))
        .find(|candidate| candidate.is_file())
        .unwrap()
        .canonicalize()
        .unwrap();
    let git = |arguments: &[&str]| {
        let result = std::process::Command::new(&real_git)
            .current_dir(&remote)
            .args(arguments)
            .output()
            .unwrap();
        assert!(result.status.success(), "{result:?}");
    };
    git(&["init", "--quiet", "-b", "executed-build-fixture-v1"]);
    git(&["add", "build.omg", "main.omg"]);
    git(&[
        "-c",
        "user.name=Omega SDK",
        "-c",
        "user.email=sdk@example.invalid",
        "commit",
        "--quiet",
        "-m",
        "authored source fixture",
    ]);
    let tools = directory.join("operator-tools");
    std::fs::create_dir(&tools).unwrap();
    std::fs::copy(
        repository.join("omega-rust/omega/compilation/driver/tests/support/git_source_proxy.py"),
        tools.join("git"),
    )
    .unwrap();
    std::fs::set_permissions(tools.join("git"), std::fs::Permissions::from_mode(0o700)).unwrap();
    let search =
        std::env::join_paths(std::iter::once(tools).chain(std::env::split_paths(&search))).unwrap();
    let observed = std::process::Command::new(std::env::current_exe().unwrap())
        .arg("--worker")
        .env("PATH", search)
        .env("OMEGA_FIXTURE_GIT_DIRECTORY", &directory)
        .env("OMEGA_FIXTURE_GIT_REPOSITORY", &remote)
        .env("OMEGA_FIXTURE_REAL_GIT", &real_git)
        .env("OMEGA_FIXTURE_GIT_FETCH_LOG", directory.join("fetch.log"))
        .output()
        .unwrap();
    print!("{}", String::from_utf8_lossy(&observed.stdout));
    eprint!("{}", String::from_utf8_lossy(&observed.stderr));
    assert!(
        observed.status.success(),
        "Git source journey failed: {:?}",
        observed.status
    );
}

fn execute_fixture() {
    let directory = PathBuf::from(std::env::var_os("OMEGA_FIXTURE_GIT_DIRECTORY").unwrap());
    let fixture = fixture_package_inputs::repo_root()
        .join("tests/omega/pass/build/executed_git_source_join_exit");
    let profile = target::TargetProfile::host_if_supported().unwrap();
    let project = directory.join("project");
    std::fs::create_dir(&project).unwrap();
    for name in ["build.omg", "main.omg"] {
        std::fs::copy(fixture.join(name), project.join(name)).unwrap();
    }
    let log = directory.join("fetch.log");
    foreign_policy_control(&directory, &project, profile, &log);
    let discover = |session: &mut package_manager::PackageBuildSession, offline| {
        omega_build::resolve_project_sources(
            session,
            omega_build::ProjectSourceRequest {
                preparation: package_manager::operations::begin_local_project_preparation(
                    &project.join("main.omg"),
                    &[profile],
                    offline,
                )
                .unwrap()
                .unwrap(),
                execution_profile: profile,
                snapshot: None,
            },
        )
    };
    for private_call in [false, true] {
        std::fs::copy(fixture.join("build.omg"), project.join("build.omg")).unwrap();
        if private_call {
            std::fs::copy(
                fixture_package_inputs::repo_root()
                    .join("tests/omega/fail/build/executed_dependency_private_call/main.omg"),
                project.join("main.omg"),
            )
            .unwrap();
        }
        let mut session =
            package_manager::PackageBuildSession::create(&directory.join(if private_call {
                "private"
            } else {
                "valid"
            }))
            .unwrap();
        let before = artifacts::compile_timings::counters::build_execution_count();
        let mut resolved = discover(&mut session, false)
            .unwrap_or_else(|error| panic!("Git source discovery: {error:?}"));
        assert_eq!(
            resolved.builds.len(),
            2,
            "two aliases reuse one dependency activation"
        );
        let fetches_at_return = std::fs::read(&log).unwrap();
        if !private_call {
            let remote = PathBuf::from(std::env::var_os("OMEGA_FIXTURE_GIT_REPOSITORY").unwrap());
            std::fs::copy(
                fixture.join("controls/changed_library.omg"),
                remote.join("main.omg"),
            )
            .unwrap();
            let observed =
                std::process::Command::new(std::env::var_os("OMEGA_FIXTURE_REAL_GIT").unwrap())
                    .current_dir(&remote)
                    .args([
                        "-c",
                        "user.name=Omega SDK",
                        "-c",
                        "user.email=sdk@example.invalid",
                        "commit",
                        "--quiet",
                        "-am",
                        "move selector after discovery",
                    ])
                    .output()
                    .unwrap();
            assert!(observed.status.success(), "{observed:?}");
        }
        for name in ["main.omg", "build.omg"] {
            std::fs::remove_file(project.join(name)).unwrap();
        }
        let child = resolved.builds.pop().unwrap();
        let inputs = child.package_inputs().clone();
        let child = child.complete(session.compiler(), profile, inputs).unwrap();
        let root = resolved.builds.pop().unwrap();
        let inputs = root
            .package_inputs()
            .clone()
            .with_complete_dependency_generated_sources(vec![
                child.generated_source_bundle().clone(),
            ])
            .unwrap();
        let root = root.complete(session.compiler(), profile, inputs).unwrap();
        let result = joined_product_execution::execute_joined_product(
            session.compiler(),
            root,
            &directory.join(if private_call {
                "invalid-output"
            } else {
                "output"
            }),
        );
        if private_call {
            let diagnostics = result
                .expect_err("Git acquisition does not widen private machine access")
                .into_diagnostics();
            assert!(
                diagnostics.iter().any(|diagnostic| diagnostic
                    .message
                    .contains("selects private machine `private_answer::entry`")),
                "{diagnostics:?}"
            );
        } else {
            result.unwrap_or_else(|error| panic!("Git Product: {:?}", error.into_diagnostics()));
        }
        assert_eq!(
            artifacts::compile_timings::counters::build_execution_count() - before,
            2,
            "no Build replay across source join and native/Terminal production"
        );
        assert_eq!(
            std::fs::read(&log).unwrap(),
            fetches_at_return,
            "Product completion cannot resolve the moved Git selector again"
        );
        session.close().unwrap();
    }
    for name in ["main.omg", "build.omg"] {
        std::fs::copy(fixture.join(name), project.join(name)).unwrap();
    }
    let before = std::fs::read(&log).unwrap();
    let mut session =
        package_manager::PackageBuildSession::create(&directory.join("offline")).unwrap();
    let error = discover(&mut session, true)
        .err()
        .expect("unaccepted offline selector refuses even when source is cached");
    assert!(
        format!("{error:?}").contains("unchanged recorded pin is required"),
        "{error:?}"
    );
    assert_eq!(
        std::fs::read(&log).unwrap(),
        before,
        "offline refusal must precede transport"
    );
    session.close().unwrap();
    assert!(!before.is_empty());
    println!(
        "executed_git_source_native_terminal_exit0_shared_activation_offline_refusal_and_private_call_control"
    );
}

fn foreign_policy_control(
    directory: &std::path::Path,
    project: &std::path::Path,
    profile: target::TargetProfile,
    fetch_log: &std::path::Path,
) {
    let preparation = package_manager::operations::begin_local_project_preparation(
        &project.join("main.omg"),
        &[profile],
        false,
    )
    .unwrap()
    .unwrap();
    let acquired = preparation.acquire_root().unwrap();
    let mut session =
        package_manager::PackageBuildSession::create(&directory.join("foreign-policy")).unwrap();
    let request = session
        .source_build_request(
            acquired.source_capture(),
            &preparation.entry_path(),
            profile,
        )
        .unwrap();
    let mut invocation = session.compiler().begin_package_build(request).unwrap();
    let mut host = invocation.execution().unwrap().host().unwrap();
    let mut activation = host.start().unwrap();
    while !activation.advance(1024).unwrap().returned {}
    let returned = activation.complete().unwrap();
    let resolution = acquired
        .declare_build(&returned)
        .unwrap()
        .resolve()
        .unwrap();
    let package_manager::resolution::source::ExecutedSourceResolution::Request(pending) =
        resolution
    else {
        panic!("authored dependency request must remain outstanding");
    };
    let foreign = directory.join("foreign-project");
    std::fs::create_dir(&foreign).unwrap();
    for name in ["main.omg", "build.omg"] {
        std::fs::copy(project.join(name), foreign.join(name)).unwrap();
    }
    let foreign = package_manager::operations::begin_local_project_preparation(
        &foreign.join("main.omg"),
        &[profile],
        false,
    )
    .unwrap()
    .unwrap();
    let error = foreign
        .source_acquisitions()
        .unwrap()
        .acquire(pending)
        .err()
        .expect("a traversal cannot borrow another project's source policy");
    assert!(
        error.contains("source acquisition policy belongs to another project request"),
        "{error}"
    );
    assert!(
        !fetch_log.exists(),
        "foreign policy must refuse before Git acquisition"
    );
    session.close().unwrap();
    println!("foreign_acquisition_policy_refused_before_dependency_transport");
}
