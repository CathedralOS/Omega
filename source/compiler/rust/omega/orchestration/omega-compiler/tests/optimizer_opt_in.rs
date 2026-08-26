use omega_compiler::{
    CompileOptions, Optimization, PackageCompilationInputs, PackageDependencyBinding,
    PackageSourceBinding, compile_to_checked, compile_to_checked_with_packages,
};

fn compile(
    options: CompileOptions,
) -> Result<omega_compiler::CompileReport, Vec<psi_diagnostics::Diagnostic>> {
    omega_compiler::compile(omega_compiler::CompileRequest::new(options))
}
use psi_core::PackageKeyIdentity;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static PROJECT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn project(label: &str, build: Option<&str>) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "omega-optimizer-opt-in-{label}-{}-{}",
        std::process::id(),
        PROJECT_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create optimizer opt-in project");
    std::fs::write(root.join("main.omg"), "data Main { value: u8; }\n")
        .expect("write optimizer opt-in main");
    if let Some(build) = build {
        std::fs::write(root.join("build.omg"), build).expect("write optimizer opt-in build");
    }
    root
}

fn diagnostic_messages(diagnostics: &[psi_diagnostics::Diagnostic]) -> String {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

fn package_identity(marker: u8) -> PackageKeyIdentity {
    PackageKeyIdentity::from_digest([marker; 32]).expect("nonzero package identity")
}

#[test]
fn absent_and_role_only_builds_select_no_optimizations() {
    let absent = project("absent", None);
    let checked =
        compile_to_checked(&absent.join("main.omg"), None).expect("absent build remains valid");
    assert!(checked.optimization_selections().is_empty());

    let role_only = project(
        "role-only",
        Some(
            "machine build(builder: &mut Build) {\n    builder.application(\"optimizer-role-only\");\n}\n",
        ),
    );
    let checked = compile_to_checked(&role_only.join("main.omg"), None)
        .expect("role-only canonical build remains valid");
    assert!(checked.optimization_selections().is_empty());
}

#[test]
fn enable_calls_project_the_exact_canonical_named_set() {
    let root = project(
        "selected",
        Some(
            r#"machine build(builder: &mut Build) {
    builder.application("optimizer-selected");
    builder.optimizations.enable(Optimization::ProofCheckElision);
    builder.optimizations.enable(Optimization::ControlFlowCleanup);
    builder.optimizations.enable(Optimization::CopyPropagation);
    builder.optimizations.enable(Optimization::SelectedIncomingU12ExactAddImmediate);
}
"#,
        ),
    );
    let checked = compile_to_checked(&root.join("main.omg"), None)
        .expect("explicit named selections should evaluate");
    assert_eq!(
        checked.optimization_selections().as_slice(),
        &[
            Optimization::ControlFlowCleanup,
            Optimization::CopyPropagation,
            Optimization::ProofCheckElision,
            Optimization::SelectedIncomingU12ExactAddImmediate,
        ]
    );
    assert_eq!(
        checked.optimization_selection_identity(),
        checked.optimization_selections().identity()
    );
}

#[test]
fn duplicate_enable_calls_reject_during_build_evaluation() {
    let root = project(
        "duplicate",
        Some(
            r#"machine build(builder: &mut Build) {
    builder.application("optimizer-duplicate");
    builder.optimizations.enable(Optimization::GlobalValueNumbering);
    builder.optimizations.enable(Optimization::GlobalValueNumbering);
}
"#,
        ),
    );
    let diagnostics = compile_to_checked(&root.join("main.omg"), None)
        .expect_err("duplicate optimization selections must reject");
    assert!(
        diagnostic_messages(&diagnostics)
            .contains("optimization `GlobalValueNumbering` is enabled more than once")
    );
}

#[test]
fn ordinary_authored_build_does_not_replace_selected_toolchain_build() {
    let root = project(
        "ordinary-build",
        Some(
            r#"machine build(builder: &mut Build) {
    builder.application("optimizer-ordinary-build");
}
"#,
        ),
    );
    std::fs::write(
        root.join("main.omg"),
        r#"data Build {
    freestanding: bool;
}
"#,
    )
    .expect("write ordinary legacy Build declaration");
    let checked = compile_to_checked(&root.join("main.omg"), None)
        .expect("ordinary authored Build must remain outside build-root vocabulary");
    assert!(checked.optimization_selections().is_empty());
}

#[test]
fn ordinary_authored_lookalike_selection_field_cannot_spoof_toolchain_build() {
    let root = project(
        "lookalike",
        Some(
            r#"machine build(builder: &mut Build) {
    builder.application("optimizer-lookalike");
}
"#,
        ),
    );
    std::fs::write(
        root.join("main.omg"),
        r#"data ProgramOptimizations {
}
data Build {
    freestanding: bool;
    optimizations: ProgramOptimizations;
}
"#,
    )
    .expect("write ordinary lookalike Build declaration");
    let checked = compile_to_checked(&root.join("main.omg"), None)
        .expect("ordinary lookalike vocabulary must not replace the toolchain Build");
    assert!(checked.optimization_selections().is_empty());
}

#[test]
fn selected_native_build_fails_closed_without_installing_output() {
    let root = project(
        "fail-closed",
        Some(
            r#"machine build(builder: &mut Build) {
    builder.application("optimizer-fail-closed");
    builder.optimizations.enable(Optimization::SelectedIncomingU12ExactAddImmediate);
}
"#,
        ),
    );
    let build_dir = root.join("build");
    let diagnostics = compile(CompileOptions {
        root_path: root.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect_err("selected optimization must not fall through to legacy O0 lowering");
    assert_eq!(diagnostics.len(), 1);
    assert!(
        diagnostics[0]
            .message
            .contains("`SelectedIncomingU12ExactAddImmediate`")
    );
    assert!(
        diagnostics[0]
            .message
            .contains("complete verified optimizer pipeline")
    );
    assert!(!build_dir.join("omega-program").exists());
    assert!(!build_dir.join("omega-program.exe").exists());
}

#[test]
fn selected_check_only_validates_without_entering_an_optimizer_backend() {
    let root = project(
        "selected-check-only",
        Some(
            r#"machine build(builder: &mut Build) {
    builder.application("optimizer-selected-check-only");
    builder.optimizations.enable(Optimization::ControlFlowCleanup);
}
"#,
        ),
    );
    compile(CompileOptions {
        root_path: root.join("main.omg"),
        build_dir: None,
        target_name: None,
        write_output: false,
    })
    .expect("check-only compilation validates selection without running optimization");
}

#[test]
fn dependency_build_selection_cannot_enable_root_package_optimization() {
    let root = project("dependency-selection", None);
    std::fs::write(
        root.join("main.omg"),
        "use dep::values;\ndata Main { value: u8; }\n",
    )
    .expect("write package-aware optimizer root");
    let dependency = root.with_file_name(format!(
        "{}-dependency",
        root.file_name()
            .and_then(|name| name.to_str())
            .expect("UTF-8 optimizer test root")
    ));
    std::fs::create_dir(&dependency).expect("create optimizer dependency");
    std::fs::write(dependency.join("values.omg"), "pub const VALUE: u8 = 1;\n")
        .expect("write optimizer dependency source");
    std::fs::write(
        dependency.join("build.omg"),
        r#"machine build(builder: &mut Build) {
    builder.optimizations.enable(Optimization::ProofCheckElision);
}
"#,
    )
    .expect("write dependency optimizer build");
    let root_identity = package_identity(1);
    let dependency_identity = package_identity(2);
    let inputs = PackageCompilationInputs::new(
        root_identity,
        vec![
            PackageSourceBinding::new(root_identity, "root", root.clone()),
            PackageSourceBinding::new(dependency_identity, "dependency", dependency),
        ],
        vec![PackageDependencyBinding::new(
            root_identity,
            "dep",
            dependency_identity,
        )],
    )
    .expect("optimizer package graph should validate");

    let checked = compile_to_checked_with_packages(&root.join("main.omg"), None, inputs)
        .expect("dependency build companion must not join root compilation");
    assert!(checked.optimization_selections().is_empty());
}

#[test]
fn package_aware_root_build_retains_its_exact_selection() {
    let root = project(
        "package-root-selection",
        Some(
            r#"machine build(builder: &mut Build) {
    builder.application("optimizer-package-root-selection");
    builder.optimizations.enable(Optimization::GlobalValueNumbering);
}
"#,
        ),
    );
    let root_identity = package_identity(3);
    let inputs = PackageCompilationInputs::new(
        root_identity,
        vec![PackageSourceBinding::new(
            root_identity,
            "root",
            root.clone(),
        )],
        Vec::new(),
    )
    .expect("root-only optimizer package graph should validate");

    let checked = compile_to_checked_with_packages(&root.join("main.omg"), None, inputs)
        .expect("root package build selection should check");
    assert_eq!(
        checked.optimization_selections().as_slice(),
        &[Optimization::GlobalValueNumbering]
    );
}
