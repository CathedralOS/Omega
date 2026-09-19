//! `alpha_bootstrap` target-profile recognition through the ordinary
//! target-package route: an inactive Alpha root binding admits without
//! demanding Alpha realization, selecting the profile reports a target
//! not-implemented diagnostic instead of a checked result, and unknown
//! profile or slot names still reject.

use compiler::{CompileOptions, CompileRequest, RequestedCompileProduct};

fn write_project(label: &str, build: &str, main: &str) -> std::path::PathBuf {
    let project = std::env::temp_dir().join(format!(
        "omega-alpha-profile-{label}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(&project).expect("create project");
    std::fs::write(project.join("build.omg"), build).expect("write build declaration");
    std::fs::write(project.join("main.omg"), main).expect("write source");
    project
}

const MAIN: &str = "data Main {}\nmachine Main::main(&mut self) {}\n";

const BUILD_BIND_ALPHA: &str = r#"machine build(builder: &mut Build) {
    builder.application("alpha-profile");
    builder.roots.bind(local_unchecked::ProgramEntry, Main::main);
    builder.roots.bind(alpha_bootstrap::ProgramEntry, Main::main);
}
"#;

fn check(project: &std::path::Path, target: &str) -> Result<compiler::CompileReport, Vec<diagnostics::Diagnostic>> {
    compiler::compile(
        CompileRequest::new(CompileOptions {
            root_path: project.join("main.omg"),
            build_dir: Some(project.join("build")),
            target_name: Some(target.to_owned()),
        })
        .with_requested_product(RequestedCompileProduct::Check),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
}

fn messages(diagnostics: &[diagnostics::Diagnostic]) -> String {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn inactive_alpha_root_binding_demands_no_alpha_backend() {
    let project = write_project("inactive", BUILD_BIND_ALPHA, MAIN);

    check(&project, "local_unchecked")
        .unwrap_or_else(|diagnostics| {
            panic!(
                "an inactive alpha_bootstrap binding must not demand Alpha realization: {diagnostics:#?}"
            )
        });

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn selected_alpha_profile_reports_not_implemented() {
    let project = write_project("selected", BUILD_BIND_ALPHA, MAIN);

    let diagnostics = check(&project, "alpha_bootstrap")
        .expect_err("selecting alpha_bootstrap must not yield a checked result");
    let text = messages(&diagnostics);
    assert!(
        text.contains("native realization for target profile `alpha_bootstrap` is not implemented"),
        "the selected unimplemented target reports not implemented: {text}"
    );

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn unknown_root_slot_owner_still_rejects() {
    let project = write_project(
        "unknown",
        r#"machine build(builder: &mut Build) {
    builder.application("alpha-profile");
    builder.roots.bind(local_unchecked::ProgramEntry, Main::main);
    builder.roots.bind(alpha_bootstraps::ProgramEntry, Main::main);
}
"#,
        MAIN,
    );

    let diagnostics = check(&project, "local_unchecked")
        .expect_err("a misspelled profile must not silently become inactive");
    let text = messages(&diagnostics);
    assert!(
        text.contains("unknown target profile `alpha_bootstraps`"),
        "unknown root-slot owners reject by name: {text}"
    );

    let _ = std::fs::remove_dir_all(project);
}
