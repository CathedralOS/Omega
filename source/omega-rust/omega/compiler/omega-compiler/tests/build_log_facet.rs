use omega_build_evaluation::BuildObservationClass;
use omega_compiler::{
    compile_to_checked_with_packages, compile_to_checked_with_packages_in_sponsored_build_session,
};
use omega_package_compilation::{PackageCompilationInputs, PackageSourceBinding};
use psi_checked_interpreter::{
    BuildEvaluationSponsor, BuildEvaluationSponsorLimits, FilesystemSponsor,
};
use psi_core::PackageKeyIdentity;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

const BUILD_LOG_LINE: &str = "build: compiler-owned log";
const BUILD_LOG_SUBPROCESS: &str = "OMEGA_BUILD_LOG_FACET_SUBPROCESS";
static NEXT_PROJECT: AtomicU64 = AtomicU64::new(0);

struct PackageProject(PathBuf);

impl PackageProject {
    fn new(label: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "omega-build-log-{label}-{}-{}",
            std::process::id(),
            NEXT_PROJECT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&root).expect("create temporary BuildLog package");
        Self(root)
    }

    fn write(&self, name: &str, source: &str) {
        fs::write(self.0.join(name), source).expect("write temporary BuildLog package source");
    }

    fn main(&self) -> PathBuf {
        self.0.join("main.omg")
    }

    fn package_inputs(&self, digest_byte: u8) -> PackageCompilationInputs {
        let package =
            PackageKeyIdentity::from_digest([digest_byte; 32]).expect("nonzero package identity");
        PackageCompilationInputs::new_package(
            package,
            vec![PackageSourceBinding::new(
                package,
                "build-log-package",
                self.0.clone(),
            )],
            Vec::new(),
        )
        .expect("single-package BuildLog compiler input")
    }
}

impl Drop for PackageProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn compile_package_build_log() {
    let project = PackageProject::new("execution");
    project.write("main.omg", "pub data Main { value: u8; }\n");
    project.write(
        "build.omg",
        r#"machine build(builder: &mut Build) {
    builder.application("build-log-package");
    builder.log.write_line("build: compiler-owned log");
}
"#,
    );

    let checked =
        compile_to_checked_with_packages(&project.main(), None, project.package_inputs(91))
            .unwrap_or_else(|diagnostics| {
                panic!(
                    "the exact compiler-owned BuildLog facet must execute in a package build: {}",
                    diagnostics
                        .iter()
                        .map(|diagnostic| diagnostic.message.as_str())
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            });

    let build = checked
        .selected_build_machine_symbol()
        .expect("package build retains its exact selected build machine");
    let reach = checked
        .facts
        .service_reaches
        .for_machine(build)
        .expect("selected build machine retains service-reach facts");
    assert!(
        checked
            .facts
            .service_reaches
            .rows
            .services(reach.inferred_transitive)
            .is_empty(),
        "BuildLog is a compiler facet, not std Console or another boundary service",
    );

    let observation = checked
        .build_observation_summary()
        .expect("BuildLog execution retains a build observation");
    assert_eq!(observation.ceiling(), BuildObservationClass::Hermetic);
    assert_eq!(observation.realized(), BuildObservationClass::Hermetic);
    assert_eq!(
        observation.build_log(),
        format!("{BUILD_LOG_LINE}\n").as_bytes()
    );
    let usage = checked
        .build_evaluation_usage()
        .expect("BuildLog execution retains deterministic usage");
    assert_eq!(usage.usage_schema_version, 7);
    assert!(usage.peak_live_cells > 0);
    assert!(usage.peak_live_text_bytes > 0);
    assert_eq!(
        usage.build_log_bytes,
        u64::try_from(BUILD_LOG_LINE.len() + 1).expect("short test log")
    );
    assert_eq!(usage.replay_build_log_bytes, 0);
    assert_eq!(usage.filesystem_operation_attempts, 0);
    assert_eq!(usage.replay_filesystem_operation_attempts, 0);
    assert!(observation.filesystem_operation_attempts().is_empty());
    let staged_output = observation
        .staged_output_tree()
        .expect("filesystem-inaccessible build has one canonical empty Output tree");
    assert_eq!(staged_output.entry_count(), 0);
    assert_eq!(staged_output.file_bytes(), 0);
}

#[test]
fn compiler_owned_build_log_executes_in_a_package_without_console_reach() {
    if std::env::var_os(BUILD_LOG_SUBPROCESS).is_some() {
        compile_package_build_log();
        return;
    }

    let output = Command::new(std::env::current_exe().expect("locate BuildLog test executable"))
        .args([
            "--exact",
            "compiler_owned_build_log_executes_in_a_package_without_console_reach",
            "--nocapture",
        ])
        .env(BUILD_LOG_SUBPROCESS, "1")
        .output()
        .expect("run isolated BuildLog compiler canary");
    let stdout = String::from_utf8(output.stdout).expect("BuildLog stdout is UTF-8");
    let stderr = String::from_utf8(output.stderr).expect("BuildLog stderr is UTF-8");
    assert!(
        output.status.success(),
        "isolated BuildLog compiler canary failed\nstdout:\n{stdout}\nstderr:\n{stderr}",
    );
    assert!(
        stdout.lines().any(|line| line == BUILD_LOG_LINE),
        "BuildLog::write_line did not emit the exact captured line\nstdout:\n{stdout}\nstderr:\n{stderr}",
    );
}

#[test]
fn sponsored_build_log_rejects_atomically_at_the_exact_closure_ceiling() {
    let project = PackageProject::new("ceiling");
    project.write("main.omg", "pub data Main { value: u8; }\n");
    project.write(
        "build.omg",
        r#"machine build(builder: &mut Build) {
    builder.application("build-log-ceiling");
    builder.log.write_line("build: compiler-owned log");
}
"#,
    );
    let build_root = project.0.join("build");
    fs::create_dir(&build_root).expect("create sponsored build root");
    let filesystem_sponsor = FilesystemSponsor::new(&build_root).expect("sponsor build root");
    let build_log_ceiling = u64::try_from(BUILD_LOG_LINE.len()).expect("short test log");
    let evaluation_sponsor = BuildEvaluationSponsor::new(
        BuildEvaluationSponsorLimits::new(
            1_000_000,
            build_log_ceiling,
            1024,
            64,
            1024,
            1024,
            1024,
            1024,
        )
        .expect("nonzero test ceilings"),
    );

    let diagnostics = compile_to_checked_with_packages_in_sponsored_build_session(
        &project.main(),
        &build_root,
        None,
        project.package_inputs(92),
        filesystem_sponsor,
        evaluation_sponsor.clone(),
    )
    .expect_err("newline must exceed the exact BuildLog ceiling");
    let rendered = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains(&format!(
            "aggregate BuildLog sponsor exhausted at {build_log_ceiling} bytes"
        )),
        "{rendered}"
    );
    assert_eq!(evaluation_sponsor.consumed_build_log_bytes(), 0);
}

#[test]
fn package_authored_build_log_lookalike_cannot_receive_the_compiler_facet() {
    let project = PackageProject::new("lookalike");
    project.write(
        "main.omg",
        r#"pub data BuildLog {
}

pub machine BuildLog::write_line(&mut self, text: &[u8]) {
}

machine accept_package_log(log: &mut BuildLog) {
    log.write_line("package lookalike");
}
"#,
    );
    project.write(
        "build.omg",
        r#"machine build(builder: &mut Build) {
    builder.application("build-log-lookalike");
    accept_package_log(&mut builder.log);
}
"#,
    );

    let diagnostics =
        compile_to_checked_with_packages(&project.main(), None, project.package_inputs(92))
            .expect_err("a package-authored BuildLog cannot receive the exact compiler activation");
    let rendered = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains("BuildLog")
            && (rendered.contains("accept_package_log") || rendered.contains("duplicate")),
        "the lookalike must reject at the exact nominal handoff, not merely because Build.log is absent:\n{rendered}",
    );
}
