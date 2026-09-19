//! Operations are callable in-process, including after a rejected request.

use compiler::CompileOptions;
use omega::compilation::{
    CompileProjectError, CompileProjectRequest, ProjectProduct, compile_project,
};
use omega::execution::{ExecutionOutcome, InterpreterComparison, RunRequest, run_project};
use omega::inspection::{InspectTerminalRequest, inspect_terminal};
use std::path::PathBuf;

struct Project(PathBuf);

impl Project {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "omega-application-api-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn write(&self, name: &str, source: &str) {
        std::fs::write(self.0.join(name), source).unwrap();
    }

    fn check_request(&self) -> CompileProjectRequest {
        let mut request = CompileProjectRequest::new(CompileOptions {
            root_path: self.0.join("main.omg"),
            build_dir: Some(self.0.join("build")),
            target_name: declared_target_when_host_is_unprofiled(),
        });
        request.product = ProjectProduct::Check;

        request
    }

    fn application(&self) {
        self.write(
            "build.omg",
            r#"
            machine build(builder: &mut Build) {
                builder.application("api-app");
                builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
                builder.roots.bind(linux_arm64::ProgramEntry, Main::main);
                builder.roots.bind(macos_arm64::ProgramEntry, Main::main);
                builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
            }
        "#,
        );
        self.write(
            "main.omg",
            "data Main {}\nmachine Main::main(&mut self) {}\n",
        );
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The targetless route resolves the compiler host's catalogued profile; a
/// host that owns none (macOS x86-64) pins an exact declared target for the
/// same coverage instead.
fn declared_target_when_host_is_unprofiled() -> Option<String> {
    target::TargetProfile::host_if_supported()
        .is_none()
        .then(|| "linux_x86_64".to_owned())
}

// Same provision as the CLI until recursive compiler paths have bounded stack use.
fn on_compiler_stack(test: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(256 * 1024 * 1024)
        .spawn(test)
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn compilation_returns_failure_then_success_without_terminating_the_caller() {
    on_compiler_stack(|| {
        let project = Project::new();
        assert!(compile_project(project.check_request()).is_err());
        project.write("main.omg", "machine main() {}\n");
        let outcome = compile_project(project.check_request()).unwrap();
        assert!(outcome.executable_path.is_none());
        assert!(outcome.timings.phases().is_empty());
        assert!(!project.0.join("build").exists());
        assert!(
            outcome
                .report
                .trust_admission_settlement()
                .is_exactly_admitted()
        );
    });
}

#[test]
fn unsettled_admissions_remain_structured_and_do_not_publish_or_rewrite_policy() {
    on_compiler_stack(|| {
        let project = Project::new();
        project.write("main.omg", "machine main() {}\n");
        let stale = format!("{}  accepted fact: stale\n", "1".repeat(64));
        project.write("omega.admissions", &stale);
        let Err(CompileProjectError::UnsettledAdmissions(settlement)) =
            compile_project(project.check_request())
        else {
            panic!("expected structured admission rejection");
        };
        assert!(!settlement.unused().is_empty());
        assert_eq!(
            std::fs::read_to_string(project.0.join("omega.admissions")).unwrap(),
            stale
        );
        assert!(!project.0.join("build/omega-program").exists());
        let mut request = project.check_request();
        request.accept_admissions = true;
        assert!(compile_project(request).is_ok());
        assert!(compile_project(project.check_request()).is_ok());
    });
}

#[test]
fn inspection_returns_verified_data_not_console_text() {
    on_compiler_stack(|| {
        let project = Project::new();
        project.write(
            "main.omg",
            r#"
            data Token { value: i32; }
            data Root {}
            machine Root::forward(token: Token) {
                transition { _ -> done(token) }
                state done(token: Token) {}
            }
        "#,
        );
        let mut request = InspectTerminalRequest {
            root_path: project.0.join("main.omg"),
            machine: "Root::missing".into(),
            target_name: declared_target_when_host_is_unprofiled(),
        };
        assert!(inspect_terminal(&request).is_err());
        request.machine = "Root::forward".into();
        let inspection = inspect_terminal(&request).unwrap();
        assert!(!inspection.module.machines.is_empty());
        assert!(matches!(
            inspection.fixed_fuel,
            omega::inspection::evidence::FixedFuel::Available(_)
        ));
    });
}

#[test]
fn run_returns_host_output_and_comparison_then_cross_target_without_execution() {
    // The first leg executes a host artifact, which is only possible when the
    // host owns a catalogued deployment profile.
    let Some(host) = target::TargetProfile::host_if_supported() else {
        eprintln!("skipping: this host admits no catalogued Omega deployment profile");
        return;
    };
    on_compiler_stack(move || {
        let project = Project::new();
        project.application();
        // The reusable run operation must not bypass ordinary project acceptance.
        assert!(
            run_project(RunRequest {
                root_path: project.0.join("main.omg"),
                target_name: None,
                compare_interpreter: false,
                keep_artifacts: false,
            })
            .is_err()
        );
        use package_manager::{
            PackageCommand, PackageCommandOptions, PackageCommandStatus, execute_package_command,
        };
        let accepted = execute_package_command(
            PackageCommand::Update {
                packages: Vec::new(),
                revision: None,
            },
            PackageCommandOptions {
                project_root: project.0.clone(),
                targets: vec![host, target::TargetProfile::LinuxX64],
                offline: true,
                build_inputs: None,
            },
            None,
        )
        .unwrap();
        assert_eq!(accepted.status, PackageCommandStatus::Published);
        let outcome = run_project(RunRequest {
            root_path: project.0.join("main.omg"),
            target_name: None,
            compare_interpreter: true,
            keep_artifacts: false,
        })
        .unwrap();
        let ExecutionOutcome::Host { output, comparison } = outcome.execution else {
            panic!("expected host execution");
        };
        assert_eq!(output.status.code(), Some(0));
        assert!(matches!(
            comparison,
            InterpreterComparison::Agrees { exit_code: 0 }
        ));
        assert!(!outcome.build_dir.exists());
        let outcome = run_project(RunRequest {
            root_path: project.0.join("main.omg"),
            target_name: Some("linux_x86_64".into()),
            compare_interpreter: true,
            keep_artifacts: true,
        })
        .unwrap();
        assert!(matches!(
            outcome.execution,
            ExecutionOutcome::TargetOnly { .. }
        ));
        assert!(outcome.build_dir.join("omega-program").is_file());
        std::fs::remove_dir_all(outcome.build_dir).unwrap();
    });
}
