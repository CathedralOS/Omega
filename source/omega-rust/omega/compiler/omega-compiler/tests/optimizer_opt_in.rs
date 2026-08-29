use omega_compiler::{CompileOptions, compile_to_checked, compile_to_checked_with_packages};
use omega_optimization_core::Optimization;
use omega_optimization_pipeline::OptimizationReportRequest;
use omega_package_compilation::{
    PackageCompilationInputs, PackageDependencyBinding, PackageSourceBinding,
};
use std::fmt::Write as _;

fn compile_native_and_publish(
    options: CompileOptions,
) -> Result<omega_compiler::CompileReport, Vec<psi_diagnostics::Diagnostic>> {
    let build_dir = options.build_dir();
    let report = omega_compiler::compile(
        omega_compiler::CompileRequest::new(options)
            .with_requested_product(omega_compiler::RequestedCompileProduct::NativeArtifact),
    )?;
    report
        .publish_retained_native_artifact(&build_dir)
        .map_err(|error| vec![psi_diagnostics::Diagnostic::error(error)])
}

fn compile_check(
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
    std::fs::write(
        root.join("main.omg"),
        "data Main { value: u8; }\nmachine Main::main(&mut self) { }\n",
    )
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

fn exact_optimization_vocabulary_build(
    optimization: Optimization,
    filesystem_prelude: bool,
) -> String {
    let mut enable_call = String::new();
    writeln!(
        enable_call,
        "    builder.optimizations.enable(Optimization::{});",
        optimization.build_case_name()
    )
    .expect("writing an optimization enable call to a String cannot fail");
    let import = if filesystem_prelude {
        "use omega::language::std::filesystem_host;\n\n"
    } else {
        ""
    };
    let service_reach = if filesystem_prelude {
        "reaches FilesystemHost\n"
    } else {
        ""
    };
    format!(
        "{import}machine build(builder: &mut Build)\n{service_reach}{{\n    builder.application(\"optimizer-exact-vocabulary\");\n{enable_call}    builder.optimizations.emit_report();\n}}\n"
    )
}

#[test]
fn absent_and_role_only_builds_select_no_optimizations() {
    let absent = project("absent", None);
    let checked =
        compile_to_checked(&absent.join("main.omg"), None).expect("absent build remains valid");
    assert!(checked.optimization_selections().is_empty());
    assert_eq!(
        checked.optimization_report_request(),
        OptimizationReportRequest::Suppressed
    );

    let role_only = project(
        "role-only",
        Some(
            "machine build(builder: &mut Build) {\n    builder.application(\"optimizer-role-only\");\n}\n",
        ),
    );
    let checked = compile_to_checked(&role_only.join("main.omg"), None)
        .expect("role-only canonical build remains valid");
    assert!(checked.optimization_selections().is_empty());
    assert_eq!(
        checked.optimization_report_request(),
        OptimizationReportRequest::Suppressed
    );
}

#[test]
fn human_report_is_an_explicit_request_not_an_optimization_selection() {
    let root = project(
        "human-report",
        Some(
            r#"machine build(builder: &mut Build) {
    builder.application("optimizer-human-report");
    builder.optimizations.emit_report();
}
"#,
        ),
    );
    let checked = compile_to_checked(&root.join("main.omg"), None)
        .expect("an explicit report-only request should evaluate");
    assert!(checked.optimization_selections().is_empty());
    assert_eq!(
        checked.optimization_report_request(),
        OptimizationReportRequest::EmitHumanText
    );
}

#[test]
fn duplicate_human_report_requests_reject_during_build_evaluation() {
    let root = project(
        "duplicate-human-report",
        Some(
            r#"machine build(builder: &mut Build) {
    builder.application("optimizer-duplicate-human-report");
    builder.optimizations.emit_report();
    builder.optimizations.emit_report();
}
"#,
        ),
    );
    let diagnostics = compile_to_checked(&root.join("main.omg"), None)
        .expect_err("duplicate human report requests must reject");
    assert!(
        diagnostic_messages(&diagnostics)
            .contains("optimization human report is requested more than once")
    );
}

#[test]
fn every_exact_enable_call_maps_to_itself_through_both_build_preludes() {
    for optimization in Optimization::ALL {
        for (prelude_label, filesystem_prelude) in [("ordinary", false), ("filesystem", true)] {
            let build = exact_optimization_vocabulary_build(optimization, filesystem_prelude);
            let label = format!(
                "selected-{prelude_label}-{}",
                optimization.build_counter_field()
            );
            let root = project(&label, Some(&build));
            let checked = compile_to_checked(&root.join("main.omg"), None)
                .expect("the exact named selection should evaluate");
            assert_eq!(
                checked.optimization_selections().as_slice(),
                &[optimization]
            );
            assert_eq!(
                checked.optimization_report_request(),
                OptimizationReportRequest::EmitHumanText
            );
            assert_eq!(
                checked.optimization_selection_identity(),
                checked.optimization_selections().identity()
            );
        }
    }
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
            r#"target windows_x64 { }
machine build(builder: &mut Build) {
    builder.application("optimizer-fail-closed");
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
    builder.optimizations.enable(Optimization::SelectedIncomingU12ExactAddImmediate);
}
"#,
        ),
    );
    let build_dir = root.join("build");
    let diagnostics = compile_native_and_publish(CompileOptions {
        root_path: root.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("windows_x64".into()),
    })
    .expect_err("selected optimization must not fall through to legacy O0 lowering");
    assert_eq!(diagnostics.len(), 1);
    assert!(
        diagnostics[0]
            .message
            .contains("`SelectedIncomingU12ExactAddImmediate`"),
        "unexpected diagnostic: {}",
        diagnostics[0].message
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
fn exact_subtract_immediate_native_build_fails_closed_without_installing_output() {
    let root = project(
        "subtract-fail-closed",
        Some(
            r#"target windows_x64 { }
machine build(builder: &mut Build) {
    builder.application("optimizer-subtract-fail-closed");
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
    builder.optimizations.enable(Optimization::SelectedIncomingU12ExactSubtractImmediate);
}
"#,
        ),
    );
    let build_dir = root.join("build");
    let diagnostics = compile_native_and_publish(CompileOptions {
        root_path: root.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("windows_x64".into()),
    })
    .expect_err("selected optimization must not fall through to legacy O0 lowering");
    assert_eq!(diagnostics.len(), 1);
    assert!(
        diagnostics[0]
            .message
            .contains("`SelectedIncomingU12ExactSubtractImmediate`"),
        "unexpected diagnostic: {}",
        diagnostics[0].message
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
fn x86_rel8_relaxation_selection_round_trips_but_remains_default_off() {
    let absent = project("x86-rel8-default-off", None);
    let checked = compile_to_checked(&absent.join("main.omg"), None)
        .expect("an absent build must leave branch relaxation disabled");
    assert!(
        !checked
            .optimization_selections()
            .contains(Optimization::X86RelaxConditionalBranchesToRel8V1)
    );

    let selected = project(
        "x86-rel8-selected",
        Some(
            r#"target windows_x64 { }
machine build(builder: &mut Build) {
    builder.application("optimizer-x86-rel8-selected");
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
    builder.optimizations.enable(Optimization::X86RelaxConditionalBranchesToRel8V1);
}
"#,
        ),
    );
    let checked = compile_to_checked(&selected.join("main.omg"), Some("windows_x64"))
        .expect("the named function-relative-layout selection should evaluate");
    assert_eq!(
        checked.optimization_selections().as_slice(),
        &[Optimization::X86RelaxConditionalBranchesToRel8V1]
    );
    assert_eq!(
        checked.optimization_selection_identity(),
        checked.optimization_selections().identity()
    );

    let build_dir = selected.join("build");
    let diagnostics = compile_native_and_publish(CompileOptions {
        root_path: selected.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("windows_x64".into()),
    })
    .expect_err("the build-visible layout selection must remain execution-gated");
    assert_eq!(diagnostics.len(), 1);
    assert!(
        diagnostics[0]
            .message
            .contains("`X86RelaxConditionalBranchesToRel8V1`")
    );
    assert!(diagnostics[0].message.contains("no output was installed"));
    assert!(!build_dir.join("omega-program").exists());
    assert!(!build_dir.join("omega-program.exe").exists());
}

#[test]
fn aarch64_cbnz_fusion_selection_round_trips_but_remains_default_off() {
    let absent = project("aarch64-cbnz-default-off", None);
    let checked = compile_to_checked(&absent.join("main.omg"), None)
        .expect("an absent build must leave AArch64 CBNZ fusion disabled");
    assert!(
        !checked
            .optimization_selections()
            .contains(Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1)
    );

    let selected = project(
        "aarch64-cbnz-selected",
        Some(
            r#"target windows_x64 { }
machine build(builder: &mut Build) {
    builder.application("optimizer-aarch64-cbnz-selected");
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
    builder.optimizations.enable(Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1);
}
"#,
        ),
    );
    let checked = compile_to_checked(&selected.join("main.omg"), Some("windows_x64"))
        .expect("the named post-allocation machine selection should evaluate");
    assert_eq!(
        checked.optimization_selections().as_slice(),
        &[Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1]
    );
    assert_eq!(
        checked.optimization_selection_identity(),
        checked.optimization_selections().identity()
    );

    let build_dir = selected.join("build");
    let diagnostics = compile_native_and_publish(CompileOptions {
        root_path: selected.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("windows_x64".into()),
    })
    .expect_err("the build-visible machine selection must remain publication-gated");
    assert_eq!(diagnostics.len(), 1);
    assert!(
        diagnostics[0]
            .message
            .contains("`Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1`")
    );
    assert!(diagnostics[0].message.contains("no output was installed"));
    assert!(!build_dir.join("omega-program").exists());
    assert!(!build_dir.join("omega-program.exe").exists());
}

#[test]
fn aarch64_movn_materialization_selection_round_trips_but_remains_default_off() {
    let absent = project("aarch64-movn-default-off", None);
    let checked = compile_to_checked(&absent.join("main.omg"), None)
        .expect("an absent build must leave AArch64 MOVN materialization disabled");
    assert!(
        !checked
            .optimization_selections()
            .contains(Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1)
    );

    let selected = project(
        "aarch64-movn-selected",
        Some(
            r#"machine build(builder: &mut Build) {
    builder.application("optimizer-aarch64-movn-selected");
    builder.optimizations.enable(Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1);
}
"#,
        ),
    );
    let checked = compile_to_checked(&selected.join("main.omg"), None)
        .expect("the named MOVN materialization selection should evaluate");
    assert_eq!(
        checked.optimization_selections().as_slice(),
        &[Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1]
    );
    assert_eq!(
        checked.optimization_selection_identity(),
        checked.optimization_selections().identity()
    );
}

#[test]
fn x86_xor_zero_materialization_selection_round_trips_but_remains_default_off() {
    let absent = project("x86-xor-zero-default-off", None);
    let checked = compile_to_checked(&absent.join("main.omg"), None)
        .expect("an absent build must leave x86 XOR-zero materialization disabled");
    assert!(
        !checked
            .optimization_selections()
            .contains(Optimization::X86SelectXorZeroI64MaterializationV1)
    );

    let selected = project(
        "x86-xor-zero-selected",
        Some(
            r#"machine build(builder: &mut Build) {
    builder.application("optimizer-x86-xor-zero-selected");
    builder.optimizations.enable(Optimization::X86SelectXorZeroI64MaterializationV1);
}
"#,
        ),
    );
    let checked = compile_to_checked(&selected.join("main.omg"), None)
        .expect("the named x86 XOR-zero materialization selection should evaluate");
    assert_eq!(
        checked.optimization_selections().as_slice(),
        &[Optimization::X86SelectXorZeroI64MaterializationV1]
    );
    assert_eq!(
        checked.optimization_selection_identity(),
        checked.optimization_selections().identity()
    );
}

#[test]
fn shared_entry_fixed_view_copy_selection_round_trips_but_remains_default_off() {
    let absent = project("shared-entry-copy-default-off", None);
    let checked = compile_to_checked(&absent.join("main.omg"), None)
        .expect("an absent build must leave shared-entry copy insertion disabled");
    assert!(
        !checked
            .optimization_selections()
            .contains(Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1)
    );

    let selected = project(
        "shared-entry-copy-selected",
        Some(
            r#"target windows_x64 { }
machine build(builder: &mut Build) {
    builder.application("optimizer-shared-entry-copy-selected");
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
    builder.optimizations.enable(Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1);
}
"#,
        ),
    );
    let checked = compile_to_checked(&selected.join("main.omg"), Some("windows_x64"))
        .expect("the named allocation-recovery selection should evaluate");
    assert_eq!(
        checked.optimization_selections().as_slice(),
        &[Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1]
    );
    assert_eq!(
        checked.optimization_selection_identity(),
        checked.optimization_selections().identity()
    );

    let build_dir = selected.join("build");
    let diagnostics = compile_native_and_publish(CompileOptions {
        root_path: selected.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("windows_x64".into()),
    })
    .expect_err("the build-visible allocation recovery must remain publication-gated");
    assert_eq!(diagnostics.len(), 1);
    assert!(
        diagnostics[0]
            .message
            .contains("`SharedEntryFixedViewCopyAfterCompareBeforeBranchV1`")
    );
    assert!(diagnostics[0].message.contains("no output was installed"));
    assert!(!build_dir.join("omega-program").exists());
    assert!(!build_dir.join("omega-program.exe").exists());
}

#[test]
fn active_resident_multi_use_rematerialization_selection_round_trips_but_remains_default_off() {
    let absent = project("active-resident-rematerialization-default-off", None);
    let checked = compile_to_checked(&absent.join("main.omg"), None)
        .expect("an absent build must leave active-resident rematerialization disabled");
    assert!(
        !checked
            .optimization_selections()
            .contains(Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1)
    );

    let selected = project(
        "active-resident-rematerialization-selected",
        Some(
            r#"target windows_x64 { }
machine build(builder: &mut Build) {
    builder.application("optimizer-active-resident-rematerialization-selected");
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
    builder.optimizations.enable(Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1);
}
"#,
        ),
    );
    let checked = compile_to_checked(&selected.join("main.omg"), Some("windows_x64"))
        .expect("the named allocation-recovery selection should evaluate");
    assert_eq!(
        checked.optimization_selections().as_slice(),
        &[Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1]
    );
    assert_eq!(
        checked.optimization_selection_identity(),
        checked.optimization_selections().identity()
    );

    let build_dir = selected.join("build");
    let diagnostics = compile_native_and_publish(CompileOptions {
        root_path: selected.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("windows_x64".into()),
    })
    .expect_err("the build-visible rematerialization must remain publication-gated");
    assert_eq!(diagnostics.len(), 1);
    assert!(
        diagnostics[0]
            .message
            .contains("`ActiveResidentImmediateU64MultiUseRematerializationV1`")
    );
    assert!(
        diagnostics[0]
            .message
            .contains("complete verified optimizer pipeline"),
        "unexpected diagnostic: {}",
        diagnostics[0].message
    );
    assert!(diagnostics[0].message.contains("no output was installed"));
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
    compile_check(CompileOptions {
        root_path: root.join("main.omg"),
        build_dir: None,
        target_name: None,
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
    builder.optimizations.emit_report();
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
    assert_eq!(
        checked.optimization_report_request(),
        OptimizationReportRequest::Suppressed
    );
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
