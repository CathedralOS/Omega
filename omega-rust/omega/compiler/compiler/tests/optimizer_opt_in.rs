use compiler::CheckedCompileRequest;
use compiler::{
    CompileOptions, CompileRequest, OptimizationRollback, RequestedCompileProduct,
    compile_to_checked,
};
use optimization_core::Optimization;
use optimization_core::OptimizationReportRequest;
use package_compilation::{
    PackageCompilationInputs, PackageDependencyBinding, PackageSourceBinding,
};
use std::fmt::Write as _;

#[path = "support/console_acceptance.rs"]
mod console_acceptance;
#[path = "support/macos_entry_acceptance.rs"]
mod macos_entry_acceptance;

fn compile_native_and_publish(
    options: CompileOptions,
) -> Result<compiler::CompileReport, Vec<diagnostics::Diagnostic>> {
    let build_dir = options.build_dir();
    let report = compiler::compile(
        compiler::CompileRequest::new(options)
            .with_requested_product(compiler::RequestedCompileProduct::NativeArtifact),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)?;
    report
        .publish_retained_native_artifact(&build_dir)
        .map_err(|error| vec![diagnostics::Diagnostic::error(error)])
}

fn compile_check(
    options: CompileOptions,
) -> Result<compiler::CompileReport, Vec<diagnostics::Diagnostic>> {
    compiler::compile(compiler::CompileRequest::new(options))
        .and_then(compiler::CompileOutcomes::into_single_report)
}
use semantic_vocabulary::PackageKeyIdentity;
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

fn diagnostic_messages(diagnostics: &[diagnostics::Diagnostic]) -> String {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

fn package_identity(marker: u8) -> PackageKeyIdentity {
    PackageKeyIdentity::from_digest([marker; 32]).expect("nonzero package identity")
}

fn exact_optimization_vocabulary_build(optimization: Optimization) -> String {
    let mut enable_call = String::new();
    writeln!(
        enable_call,
        "    builder.optimizations.enable(Optimization::{});",
        optimization.build_case_name()
    )
    .expect("writing an optimization enable call to a String cannot fail");
    // Checked-tree phase members require an authored product root; every other
    // phase member evaluates on the unrooted default product.
    let root_binding = (optimization.execution_phase()
        == optimization_core::OptimizationExecutionPhase::CheckedTrees)
        .then_some("    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);\n")
        .unwrap_or("");
    format!(
        "machine build(builder: &mut Build) {{\n    builder.application(\"optimizer-exact-vocabulary\");\n{root_binding}{enable_call}    builder.optimizations.emit_report();\n}}\n"
    )
}

#[test]
fn absent_and_role_only_builds_select_no_optimizations() {
    let absent = project("absent", None);
    let checked = compile_to_checked(CheckedCompileRequest::new(&absent.join("main.omg"), None))
        .expect("absent build remains valid");
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
    let checked = compile_to_checked(CheckedCompileRequest::new(
        &role_only.join("main.omg"),
        None,
    ))
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
    let checked = compile_to_checked(CheckedCompileRequest::new(&root.join("main.omg"), None))
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
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&root.join("main.omg"), None))
        .expect_err("duplicate human report requests must reject");
    assert!(
        diagnostic_messages(&diagnostics)
            .contains("optimization human report is requested more than once")
    );
}

#[test]
fn every_exact_enable_and_rollback_maps_to_itself_through_the_build_prelude() {
    for optimization in Optimization::ALL {
        let build = exact_optimization_vocabulary_build(optimization);
        let label = format!("selected-{}", optimization.build_counter_field());
        let root = project(&label, Some(&build));
        let checked = compile_to_checked(CheckedCompileRequest::new(&root.join("main.omg"), None))
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

        let rollback = OptimizationRollback::from_exact_names([optimization.build_case_name()])
            .expect("the exact build name is also the exact rollback name");
        let receipt = rollback
            .reconcile(checked.optimization_selections())
            .expect("one exact rollback request leaves custody");
        assert_eq!(receipt.build_selected().as_slice(), &[optimization]);
        assert_eq!(receipt.requested_disabled().as_slice(), &[optimization]);
        assert_eq!(receipt.actually_disabled().as_slice(), &[optimization]);
        assert!(receipt.effective().is_empty());
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
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&root.join("main.omg"), None))
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
    let checked = compile_to_checked(CheckedCompileRequest::new(&root.join("main.omg"), None))
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
    let checked = compile_to_checked(CheckedCompileRequest::new(&root.join("main.omg"), None))
        .expect("ordinary lookalike vocabulary must not replace the toolchain Build");
    assert!(checked.optimization_selections().is_empty());
}

#[test]
fn return_only_identity_build_preserves_complete_native_evidence() {
    let standard_library = native_evidence_standard_library();
    let macos_entry = macos_entry_acceptance::candidate_macos_entry_binding(
        &standard_library,
        package_identity(2),
    )
    .expect("check and explicitly accept the real target entry contract");
    for target in [
        "windows_x86_64",
        "linux_x86_64",
        "linux_arm64",
        "macos_arm64",
    ] {
        let report = compile_source_native_evidence(
            target,
            false,
            "data Main {} machine Main::main() {}\n",
            &standard_library,
            &macos_entry,
        );
        validate_source_native_evidence(&report);
        let physical = report
            .retained_native_artifact()
            .unwrap()
            .physical_evidence()
            .expect("complete empty physical evidence");
        assert!(physical.projection().operator_occurrences().is_empty());
        assert!(physical.projection().boundary_occurrences().is_empty());
        assert!(physical.children().is_empty());
        publish_source_native_evidence(report, target);
    }
}

#[test]
fn scalar_returning_source_calls_preserve_native_evidence_on_every_target() {
    let standard_library = native_evidence_standard_library();
    let macos_entry = macos_entry_acceptance::candidate_macos_entry_binding(
        &standard_library,
        package_identity(2),
    )
    .expect("check and explicitly accept the real target entry contract");
    for target in [
        "windows_x86_64",
        "linux_x86_64",
        "linux_arm64",
        "macos_arm64",
    ] {
        for copy_propagation in [false, true] {
            let report = compile_source_native_evidence(
                target,
                copy_propagation,
                r#"
machine pick(left: u64, right: u64) -> u64
requires true
ensures result == right
{ transition { _ -> right } }
data Main {}
machine Main::main() {
    let first: u64 = pick(7u64, 9u64);
    let second: u64 = pick(first, first);
    let third: u64 = pick(first, second);
}
"#,
                &standard_library,
                &macos_entry,
            );
            validate_source_native_evidence(&report);
            let artifact = report.retained_native_artifact().unwrap();
            assert!(
                image_emission::derive_stack_demand(artifact.object(), artifact.object().entry())
                    .unwrap()
                    .ceiling_bytes()
                    > 0
            );
            publish_source_native_evidence(report, target);
        }
    }
}

fn native_evidence_standard_library() -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|path| path.join("source/library/std").is_dir())
        .expect("compiler checkout owns the authored standard package")
        .join("source/library/std")
}

fn compile_source_native_evidence(
    target: &str,
    copy_propagation: bool,
    source: &str,
    standard_library: &std::path::Path,
    macos_entry: &package_compilation::AcceptedSemanticBinding,
) -> compiler::CompileReport {
    let dependency = if target == "macos_arm64" {
        format!(
            "builder.depend(Source::Path {{ location: \"{}\" }});",
            standard_library.to_string_lossy().replace('\\', "/")
        )
    } else {
        String::new()
    };
    let optimization = if copy_propagation {
        "builder.optimizations.enable(Optimization::CopyPropagation);"
    } else {
        ""
    };
    let root = project(
        "source-native-evidence",
        Some(&format!(
            "machine build(builder: &mut Build) {{\n\
             builder.application(\"source-native-evidence\");\n\
             {dependency}\n\
             builder.roots.bind({target}::ProgramEntry, Main::main);\n\
             {optimization}\n}}\n"
        )),
    );
    std::fs::write(root.join("main.omg"), source)
        .expect("write source-owned native evidence fixture");
    let mut request = CompileRequest::new(CompileOptions {
        root_path: root.join("main.omg"),
        build_dir: Some(root.join("build")),
        target_name: Some(target.into()),
    })
    .with_requested_product(RequestedCompileProduct::NativeArtifact);
    if target == "macos_arm64" {
        let application = package_identity(1);
        let standard = package_identity(2);
        let inputs = PackageCompilationInputs::new(
            application,
            package_compilation::BuildDeclarationKind::Application,
            vec![
                PackageSourceBinding::new(application, "source-native-evidence", root.clone()),
                PackageSourceBinding::new(
                    standard,
                    "omega-language-std",
                    standard_library.to_path_buf(),
                ),
            ],
            vec![PackageDependencyBinding::new(
                application,
                "omega_language_std",
                standard,
            )],
        )
        .expect("exact application and standard dependency graph")
        .with_accepted_semantic_bindings(vec![macos_entry.clone()])
        .expect("explicitly accepted checked target entry candidate");
        request = request.with_package_inputs(inputs);
    }
    compiler::compile(request)
        .and_then(compiler::CompileOutcomes::into_single_report)
        .unwrap_or_else(|diagnostics| {
            panic!("{target}, CopyPropagation={copy_propagation}: {diagnostics:?}")
        })
}

fn publish_source_native_evidence(report: compiler::CompileReport, target: &str) {
    // Publication replaces retained artifact custody with a receipt. Replay and
    // installation-record assertions therefore run before consuming the report.
    let build_dir = report.root_path().parent().unwrap().join("build");
    let report = report
        .publish_retained_native_artifact(&build_dir)
        .expect("publish the independently validated source-owned native artifact");
    let executable = report
        .checked_native_executable_path()
        .expect("published executable receipt");
    assert!(executable.is_file());
    if target == "macos_arm64" {
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            let output = std::process::Command::new(executable)
                .output()
                .expect("execute published macOS source fixture");
            assert_eq!(output.status.code(), Some(0), "{output:?}");
        }
        #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
        eprintln!(
            "SKIP macOS process execution: requires a macOS arm64 host; image replay still checked"
        );
    }
}

fn validate_source_native_evidence(report: &compiler::CompileReport) {
    let artifact = report
        .retained_native_artifact()
        .expect("retained native artifact");
    artifact
        .validate()
        .expect("identity artifact independently replays");
    assert!(matches!(
        artifact.physical_evidence_scope(),
        native_realization::NativePhysicalEvidenceScope::ValidatedOptimizedProjection(_)
    ));
    assert!(artifact.physical_evidence().is_some());
    let record = image_emission::build_installation_record(
        artifact.image(),
        semantic_vocabulary::ProfileDecisionId::new(1).unwrap(),
    )
    .unwrap();
    let encoded = image_emission::encode_installation_record(&record).unwrap();
    let decoded = image_emission::decode_installation_record(&encoded).unwrap();
    image_emission::validate_installation_record(&decoded, artifact.image()).unwrap();
    assert_eq!(
        image_emission::derive_installation_stack_demand(
            &decoded,
            artifact.image(),
            artifact.object().entry(),
        )
        .unwrap(),
        image_emission::derive_stack_demand(artifact.object(), artifact.object().entry()).unwrap(),
    );
}

#[test]
fn return_only_selected_lowering_build_rejoins_native_artifact_production() {
    let root = project(
        "fail-closed",
        Some(
            r#"machine build(builder: &mut Build) {
    builder.application("optimizer-fail-closed");
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
    builder.optimizations.enable(Optimization::SelectedIncomingU12ExactAddImmediate);
}
"#,
        ),
    );
    let build_dir = root.join("build");
    let report = compiler::compile(
        CompileRequest::new(CompileOptions {
            root_path: root.join("main.omg"),
            build_dir: Some(build_dir.clone()),
            target_name: Some("windows_x86_64".into()),
        })
        .with_requested_product(RequestedCompileProduct::NativeArtifact),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect("the exact return-only selected-lowering cohort should reach native custody");
    let artifact = report
        .retained_native_artifact()
        .expect("selected-lowering compilation retains its native artifact");
    artifact
        .validate()
        .expect("selected-lowering native artifact should replay");
    assert!(matches!(
        artifact.physical_evidence_scope(),
        native_realization::NativePhysicalEvidenceScope::ValidatedOptimizedProjection(_)
    ));
    let physical = artifact
        .physical_evidence()
        .expect("empty D32 coverage is still exact physical evidence");
    assert!(physical.projection().operator_occurrences().is_empty());
    assert!(physical.projection().boundary_occurrences().is_empty());
    assert!(physical.children().is_empty());
    assert!(!build_dir.join("omega-program").exists());
    assert!(!build_dir.join("omega-program.exe").exists());
}

#[test]
fn partial_rollback_routes_the_remaining_psi_selection_to_preterminal() {
    let root = project(
        "partial-rollback-fail-closed",
        Some(
            r#"machine build(builder: &mut Build) {
    builder.application("optimizer-partial-rollback-fail-closed");
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
    builder.optimizations.enable(Optimization::ControlFlowCleanup);
    builder.optimizations.enable(Optimization::CopyPropagation);
}
"#,
        ),
    );
    let build_dir = root.join("build");
    let report = compiler::compile(
        CompileRequest::new(CompileOptions {
            root_path: root.join("main.omg"),
            build_dir: Some(build_dir.clone()),
            target_name: Some("windows_x86_64".into()),
        })
        .with_requested_product(RequestedCompileProduct::NativeArtifact)
        .with_optimization_rollback(
            OptimizationRollback::new([Optimization::CopyPropagation])
                .expect("the partial rollback request must be unique"),
        ),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect("the remaining Psi selection executes at its pre-Terminal owner");
    let receipt = report
        .optimization_rollback_receipt()
        .expect("a partial rollback reports the receipt of what actually ran");
    assert_eq!(
        receipt.actually_disabled().as_slice(),
        &[Optimization::CopyPropagation]
    );
    assert_eq!(
        receipt.effective().as_slice(),
        &[Optimization::ControlFlowCleanup]
    );
    let artifact = report
        .retained_native_artifact()
        .expect("the surviving selection still reaches native custody");
    let executed = artifact.psi_artifact().optimization().selections();
    assert_eq!(executed.as_slice().len(), 1);
    assert!(
        executed.contains(
            Optimization::ControlFlowCleanup
                .optimization()
                .expect("ControlFlowCleanup is a Psi selection")
        )
    );
    assert!(
        !executed.contains(
            Optimization::CopyPropagation
                .optimization()
                .expect("CopyPropagation is a Psi selection")
        )
    );
    artifact
        .validate()
        .expect("the retained native artifact replays");
    assert!(!build_dir.join("omega-program.exe").exists());
}

#[test]
fn return_only_exact_subtract_rejoins_native_artifact_production() {
    let root = project(
        "subtract-fail-closed",
        Some(
            r#"machine build(builder: &mut Build) {
    builder.application("optimizer-subtract-fail-closed");
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
    builder.optimizations.enable(Optimization::SelectedIncomingU12ExactSubtractImmediate);
}
"#,
        ),
    );
    let build_dir = root.join("build");
    let report = compiler::compile(
        CompileRequest::new(CompileOptions {
            root_path: root.join("main.omg"),
            build_dir: Some(build_dir.clone()),
            target_name: Some("windows_x86_64".into()),
        })
        .with_requested_product(RequestedCompileProduct::NativeArtifact),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect("the exact return-only subtract selection should reach native custody");
    let artifact = report
        .retained_native_artifact()
        .expect("selected-lowering compilation retains its native artifact");
    artifact
        .validate()
        .expect("selected-lowering native artifact should replay");
    assert!(matches!(
        artifact.physical_evidence_scope(),
        native_realization::NativePhysicalEvidenceScope::ValidatedOptimizedProjection(_)
    ));
    assert!(
        artifact
            .physical_evidence()
            .expect("empty D32 coverage remains exact")
            .children()
            .is_empty()
    );
    assert!(!build_dir.join("omega-program").exists());
    assert!(!build_dir.join("omega-program.exe").exists());
}

#[test]
fn return_only_compare_immediate_rejoins_native_artifact_production() {
    let root = project(
        "compare-rejoin",
        Some(
            r#"machine build(builder: &mut Build) {
    builder.application("optimizer-compare-rejoin");
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
    builder.optimizations.enable(Optimization::SelectedIncomingU12CompareImmediate);
}
"#,
        ),
    );
    let build_dir = root.join("build");
    let report = compiler::compile(
        CompileRequest::new(CompileOptions {
            root_path: root.join("main.omg"),
            build_dir: Some(build_dir.clone()),
            target_name: Some("windows_x86_64".into()),
        })
        .with_requested_product(RequestedCompileProduct::NativeArtifact),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect("the exact return-only compare selection should reach native custody");
    let artifact = report
        .retained_native_artifact()
        .expect("selected-lowering compilation retains its native artifact");
    artifact
        .validate()
        .expect("selected-lowering native artifact should replay");
    assert!(matches!(
        artifact.physical_evidence_scope(),
        native_realization::NativePhysicalEvidenceScope::ValidatedOptimizedProjection(_)
    ));
    assert!(
        artifact
            .physical_evidence()
            .expect("empty D32 coverage remains exact")
            .children()
            .is_empty()
    );
    assert!(!build_dir.join("omega-program").exists());
    assert!(!build_dir.join("omega-program.exe").exists());
}

#[test]
fn selected_lowering_boundary_occurrence_replays_one_exact_physical_child() {
    let root = std::env::temp_dir().join(format!(
        "omega-optimizer-opt-in-selected-lowering-physical-child-{}-{}",
        std::process::id(),
        PROJECT_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create selected-lowering physical-child project");
    std::fs::write(
        root.join("main.omg"),
        r#"data CheckedMath {}

boundary operator CheckedMath::select_left(left: u64, right: u64) -> u64;

data CheckedMathProvider {}

machine CheckedMathProvider::select_left_impl(left: u64, right: u64) -> u64
satisfies CheckedMath::select_left
{
    transition { _ -> left }
}

data Main {}

machine Main::main(&mut self) {
    let result: u64 = CheckedMath::select_left(7u64, 9u64);
}
"#,
    )
    .expect("write selected-lowering physical-child main");
    std::fs::write(
        root.join("build.omg"),
        r#"machine build(builder: &mut Build) {
    builder.application("optimizer-selected-lowering-physical-child");
    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
    builder.optimizations.enable(Optimization::SelectedIncomingU12CompareImmediate);
}
"#,
    )
    .expect("write selected-lowering physical-child build");
    let root_identity = package_identity(41);
    let inputs = PackageCompilationInputs::new_package(
        root_identity,
        vec![PackageSourceBinding::new(
            root_identity,
            "root",
            root.clone(),
        )],
        Vec::new(),
    )
    .expect("physical-child package graph should validate");
    let report = compiler::compile(
        CompileRequest::new(CompileOptions {
            root_path: root.join("main.omg"),
            build_dir: Some(root.join("build")),
            target_name: Some("linux_x86_64".into()),
        })
        .with_requested_product(RequestedCompileProduct::NativeArtifact)
        .with_package_inputs(inputs),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect("one selected-lowering operation must carry a boundary occurrence to native custody");
    let artifact = report
        .retained_native_artifact()
        .expect("selected-lowering compilation retains its native artifact");
    artifact
        .validate()
        .expect("selected-lowering native artifact should replay independently");
    assert!(matches!(
        artifact.physical_evidence_scope(),
        native_realization::NativePhysicalEvidenceScope::ValidatedOptimizedProjection(_)
    ));
    let physical = artifact
        .physical_evidence()
        .expect("the surviving boundary occurrence retains nonempty physical evidence");
    let [occurrence] = physical.projection().operator_occurrences() else {
        panic!("the checked boundary operator must survive as exactly one operator occurrence")
    };
    assert!(physical.projection().boundary_occurrences().is_empty());
    let [child] = physical.children() else {
        panic!("the surviving occurrence must bind exactly one physical child")
    };
    assert_eq!(
        child.occurrence(),
        native_realization::NativePhysicalOccurrence::Operator(occurrence.identity())
    );
    assert_eq!(child.projection(), physical.projection().identity());
    assert!(matches!(
        child.parent(),
        native_realization::PhysicalChildParent::OperatorApplicationCoverage(_)
    ));
    assert!(child.machine_span().byte_count() > 0);
    assert!(child.object_span().byte_count() > 0);
    assert_eq!(
        child.relocation(),
        native_realization::PhysicalRelocationDisposition::ResolvedInternalCall
    );

    let parts = report
        .into_retained_native_artifact()
        .expect("owned native artifact")
        .into_parts();
    let mut missing = replay_native_artifact_parts(&parts);
    let evidence = missing
        .physical_evidence
        .take()
        .expect("replay physical evidence")
        .into_parts();
    missing.physical_evidence = Some(
        native_realization::NativePhysicalEvidence::from_replayed_parts(
            native_realization::NativePhysicalEvidenceParts {
                projection: evidence.projection,
                children: Vec::new(),
                identity: evidence.identity,
            },
        ),
    );
    assert!(
        native_realization::NativeArtifact::from_replayed_parts(missing).is_err(),
        "a missing physical child must not replay"
    );

    let mut duplicate = replay_native_artifact_parts(&parts);
    let evidence = duplicate
        .physical_evidence
        .take()
        .expect("replay physical evidence")
        .into_parts();
    let [only_child] = evidence.children.as_slice() else {
        panic!("one physical child before duplication")
    };
    duplicate.physical_evidence = Some(
        native_realization::NativePhysicalEvidence::from_replayed_parts(
            native_realization::NativePhysicalEvidenceParts {
                projection: evidence.projection,
                children: vec![only_child.clone(), only_child.clone()],
                identity: evidence.identity,
            },
        ),
    );
    assert!(
        native_realization::NativeArtifact::from_replayed_parts(duplicate).is_err(),
        "a duplicate physical child must not replay"
    );

    let assert_mutated_child_rejected =
        |mutate: &dyn Fn(&mut native_realization::NativePhysicalChildParts)| {
            let mut replay = replay_native_artifact_parts(&parts);
            let evidence = replay
                .physical_evidence
                .take()
                .expect("replay physical evidence")
                .into_parts();
            let [child] = evidence.children.as_slice() else {
                panic!("one physical child before mutation")
            };
            let mut child = child.clone().into_parts();
            mutate(&mut child);
            replay.physical_evidence = Some(
                native_realization::NativePhysicalEvidence::from_replayed_parts(
                    native_realization::NativePhysicalEvidenceParts {
                        projection: evidence.projection,
                        children: vec![
                            native_realization::NativePhysicalChild::from_replayed_parts(child),
                        ],
                        identity: evidence.identity,
                    },
                ),
            );
            assert!(
                native_realization::NativeArtifact::from_replayed_parts(replay).is_err(),
                "a mutated physical child must not replay"
            );
        };
    assert_mutated_child_rejected(&|child| {
        assert!(matches!(
            child.parent,
            native_realization::PhysicalChildParent::OperatorApplicationCoverage(_)
        ));
        child.occurrence = native_realization::NativePhysicalOccurrence::Boundary(
            optimization_core::OptimizedBoundaryOccurrenceIdentity::from_bytes(
                child.occurrence.identity(),
            ),
        );
    });
    assert_mutated_child_rejected(&|child| {
        child.machine_span = native_realization::NativeByteSpan::from_replayed_parts(
            child.machine_span.offset(),
            child.machine_span.byte_count() + 1,
        );
    });
    assert_mutated_child_rejected(&|child| {
        child.projection =
            optimization_core::NativeOptimizationProjectionIdentity::from_bytes([0x5A; 32]);
    });
}

#[test]
fn selected_lowering_replays_one_physical_child_per_surviving_occurrence_role() {
    // One program retains both surviving boundary-occurrence roles under an
    // admitted selected-lowering optimization: the closed operator application
    // settles through its provider call interval, and the hosted console exit
    // settles through a BoundaryTraitSettlement. Replay must derive exactly one
    // physical child per surviving occurrence with the matching parent role.
    let root = std::env::temp_dir().join(format!(
        "omega-optimizer-selected-lowering-boundary-settlement-{}-{}",
        std::process::id(),
        PROJECT_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create selected-lowering package root");
    let standard_library = native_evidence_standard_library();
    std::fs::write(
        root.join("build.omg"),
        format!(
            "machine build(builder: &mut Build) {{\n\
             \x20   builder.application(\"optimizer-selected-lowering-boundary-settlement\");\n\
             \x20   builder.depend(Source::Path {{ location: \"{}\" }});\n\
             \x20   builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);\n\
             \x20   builder.optimizations.enable(Optimization::SelectedIncomingU12CompareImmediate);\n\
             }}\n",
            standard_library.display()
        ),
    )
    .expect("write selected-lowering package build");
    std::fs::write(
        root.join("main.omg"),
        "use omega_language_std::console;\n\
         \n\
         data CheckedMath {}\n\
         \n\
         boundary operator CheckedMath::select_left(left: u64, right: u64) -> u64;\n\
         \n\
         data CheckedMathProvider {}\n\
         \n\
         machine CheckedMathProvider::select_left_impl(left: u64, right: u64) -> u64\n\
         satisfies CheckedMath::select_left\n\
         {\n\
             transition { _ -> left }\n\
         }\n\
         \n\
         data Main { console: Console; }\n\
         \n\
         machine Main::main(&mut self)\n\
         reaches\n\
             Console\n\
         {\n\
             let result: u64 = CheckedMath::select_left(7u64, 9u64);\n\
             self.console.exit_process(70);\n\
         }\n",
    )
    .expect("write selected-lowering boundary-settlement program");

    let application = package_identity(1);
    let standard = package_identity(2);
    let inputs = PackageCompilationInputs::new(
        application,
        package_compilation::BuildDeclarationKind::Application,
        vec![
            PackageSourceBinding::new(
                application,
                "optimizer-selected-lowering-boundary-settlement",
                root.clone(),
            ),
            PackageSourceBinding::new(standard, "omega-language-std", standard_library.clone()),
        ],
        vec![PackageDependencyBinding::new(
            application,
            "omega_language_std",
            standard,
        )],
    )
    .expect("selected-lowering package inputs should validate");
    let preliminary = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs.clone()),
        ..CheckedCompileRequest::new(&root.join("main.omg"), Some("linux_x86_64"))
    })
    .expect("boundary-settlement program checks");
    let console_binding =
        console_acceptance::candidate_console_exit_binding(&preliminary, standard, false, false)
            .expect("hosted console exit acceptance should derive from the checked program");
    let inputs = inputs
        .with_accepted_semantic_bindings(vec![console_binding])
        .expect("console exit acceptance binds to the std package");
    let permission_policy = native_realization::terminal_authority_permission_policy_with_rows(
        inputs
            .accepted_semantic_bindings()
            .flat_map(|binding| binding.terminal_authority_permissions().iter().cloned())
            .collect(),
    )
    .expect("console exit permission policy should validate");

    let report = compiler::compile(
        CompileRequest::new(CompileOptions {
            root_path: root.join("main.omg"),
            build_dir: Some(root.join("build")),
            target_name: Some("linux_x86_64".into()),
        })
        .with_requested_product(RequestedCompileProduct::NativeArtifact)
        .with_package_inputs(inputs)
        .with_terminal_authority_permission_policy(permission_policy),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect("selected-lowering boundary-settlement program emits an artifact");
    let artifact = report
        .retained_native_artifact()
        .expect("selected-lowering compilation retains its native artifact");
    artifact
        .validate()
        .expect("selected-lowering native artifact should replay independently");
    assert!(matches!(
        artifact.physical_evidence_scope(),
        native_realization::NativePhysicalEvidenceScope::ValidatedOptimizedProjection(_)
    ));
    let physical = artifact
        .physical_evidence()
        .expect("the surviving occurrences retain nonempty physical evidence");
    let [operator_occurrence] = physical.projection().operator_occurrences() else {
        panic!("the checked boundary operator must survive as exactly one operator occurrence")
    };
    let [boundary_occurrence] = physical.projection().boundary_occurrences() else {
        panic!("the hosted console exit must survive as exactly one boundary occurrence")
    };
    let [operator_child, boundary_child] = physical.children() else {
        panic!("each surviving occurrence must bind exactly one physical child")
    };
    assert_eq!(
        operator_child.occurrence(),
        native_realization::NativePhysicalOccurrence::Operator(operator_occurrence.identity())
    );
    assert_eq!(
        operator_child.projection(),
        physical.projection().identity()
    );
    assert!(matches!(
        operator_child.parent(),
        native_realization::PhysicalChildParent::OperatorApplicationCoverage(_)
    ));
    assert!(operator_child.machine_span().byte_count() > 0);
    assert!(operator_child.object_span().byte_count() > 0);
    assert_eq!(
        operator_child.relocation(),
        native_realization::PhysicalRelocationDisposition::ResolvedInternalCall
    );
    assert_eq!(
        boundary_child.occurrence(),
        native_realization::NativePhysicalOccurrence::Boundary(boundary_occurrence.identity())
    );
    assert_eq!(
        boundary_child.projection(),
        physical.projection().identity()
    );
    let native_realization::PhysicalChildParent::BoundaryTraitSettlement(settlement) =
        boundary_child.parent()
    else {
        panic!("the boundary occurrence must retain its boundary-settlement parent")
    };
    assert_eq!(settlement.occurrence(), boundary_occurrence);
    assert_eq!(
        settlement.execution(),
        target_operations::BoundaryExecutionBinding::CompilerBuiltin(
            target_operations::CompilerBuiltinExecution::HostedExitProcessI32
        )
    );
    assert!(matches!(
        settlement.role(),
        native_realization::BoundaryTraitSettlementRole::CompilerBuiltin {
            execution: target_operations::CompilerBuiltinExecution::HostedExitProcessI32,
            ..
        } | native_realization::BoundaryTraitSettlementRole::CompilerBuiltinRuntimeScalar {
            execution: target_operations::CompilerBuiltinExecution::HostedExitProcessI32,
            ..
        }
    ));
    assert!(boundary_child.machine_span().byte_count() > 0);
    assert!(boundary_child.object_span().byte_count() > 0);
    assert_eq!(
        boundary_child.relocation(),
        native_realization::PhysicalRelocationDisposition::DirectInstructionBytes
    );

    // Independent replay derives the same two-child custody from the published
    // parts alone; each mutation class must fail closed.
    let parts = report
        .into_retained_native_artifact()
        .expect("owned native artifact")
        .into_parts();
    let assert_mutated_children_rejected =
        |mutate: &dyn Fn(&mut Vec<native_realization::NativePhysicalChild>)| {
            let mut replay = replay_native_artifact_parts(&parts);
            let evidence = replay
                .physical_evidence
                .take()
                .expect("replay physical evidence")
                .into_parts();
            let mut children = evidence.children;
            mutate(&mut children);
            replay.physical_evidence = Some(
                native_realization::NativePhysicalEvidence::from_replayed_parts(
                    native_realization::NativePhysicalEvidenceParts {
                        projection: evidence.projection,
                        children,
                        identity: evidence.identity,
                    },
                ),
            );
            assert!(
                native_realization::NativeArtifact::from_replayed_parts(replay).is_err(),
                "mutated physical-child custody must fail independent replay"
            );
        };
    assert_mutated_children_rejected(&|children| {
        children.retain(|child| {
            matches!(
                child.parent(),
                native_realization::PhysicalChildParent::OperatorApplicationCoverage(_)
            )
        });
    });
    assert_mutated_children_rejected(&|children| {
        children.retain(|child| {
            matches!(
                child.parent(),
                native_realization::PhysicalChildParent::BoundaryTraitSettlement(_)
            )
        });
    });
    assert_mutated_children_rejected(&|children| {
        children.push(children[0].clone());
    });
    assert_mutated_children_rejected(&|children| {
        let settlement = children[1].parent().clone();
        let mut swapped = children[0].clone().into_parts();
        swapped.parent = settlement;
        children[0] = native_realization::NativePhysicalChild::from_replayed_parts(swapped);
    });
}

#[test]
fn selected_lowering_exact_add_occurrence_replays_one_exact_physical_child() {
    // A second selected-lowering family carries the same physical-child
    // contract: the package-bound boundary operator program compiles under
    // `SelectedIncomingU12ExactAddImmediate`, and the provider body's bounded
    // `value + 5` gives the catalog's `EXACT_ADD_IMMEDIATE_U12` pair rule a
    // real source-reachable candidate. The surviving operator occurrence still
    // binds exactly one OperatorApplicationCoverage child through allocation,
    // layout, and native emission; independent replay rejects every mutation
    // class — missing, duplicate, stale, substituted, padded, and
    // role-swapped children.
    let root = std::env::temp_dir().join(format!(
        "omega-optimizer-opt-in-exact-add-physical-child-{}-{}",
        std::process::id(),
        PROJECT_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create exact-add physical-child project");
    std::fs::write(
        root.join("main.omg"),
        r#"data CheckedMath {}

boundary operator CheckedMath::add_five(value: u64 [0..=100]) -> u64 [0..=105];

data CheckedMathProvider {}

machine CheckedMathProvider::add_five_impl(value: u64 [0..=100]) -> u64 [0..=105]
satisfies CheckedMath::add_five
{
    transition { _ -> (value + 5) }
}

data Main {}

machine Main::main(&mut self) {
    let picked: u64 = CheckedMath::add_five(7u64);
}
"#,
    )
    .expect("write exact-add physical-child main");
    std::fs::write(
        root.join("build.omg"),
        r#"machine build(builder: &mut Build) {
    builder.application("optimizer-exact-add-physical-child");
    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
    builder.optimizations.enable(Optimization::SelectedIncomingU12ExactAddImmediate);
}
"#,
    )
    .expect("write exact-add physical-child build");
    let root_identity = package_identity(44);
    let inputs = PackageCompilationInputs::new_package(
        root_identity,
        vec![PackageSourceBinding::new(
            root_identity,
            "root",
            root.clone(),
        )],
        Vec::new(),
    )
    .expect("exact-add physical-child package graph should validate");
    let report = compiler::compile(
        CompileRequest::new(CompileOptions {
            root_path: root.join("main.omg"),
            build_dir: Some(root.join("build")),
            target_name: Some("linux_x86_64".into()),
        })
        .with_requested_product(RequestedCompileProduct::NativeArtifact)
        .with_package_inputs(inputs),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect("the exact-add selection must carry a boundary occurrence to native custody");
    let artifact = report
        .retained_native_artifact()
        .expect("exact-add compilation retains its native artifact");
    artifact
        .validate()
        .expect("exact-add native artifact should replay independently");
    assert!(matches!(
        artifact.physical_evidence_scope(),
        native_realization::NativePhysicalEvidenceScope::ValidatedOptimizedProjection(_)
    ));
    let physical = artifact
        .physical_evidence()
        .expect("the surviving boundary occurrence retains nonempty physical evidence");
    let [occurrence] = physical.projection().operator_occurrences() else {
        panic!("the checked boundary operator must survive as exactly one operator occurrence")
    };
    assert!(physical.projection().boundary_occurrences().is_empty());
    let [child] = physical.children() else {
        panic!("the surviving occurrence must bind exactly one physical child")
    };
    assert_eq!(
        child.occurrence(),
        native_realization::NativePhysicalOccurrence::Operator(occurrence.identity())
    );
    assert_eq!(child.projection(), physical.projection().identity());
    assert!(matches!(
        child.parent(),
        native_realization::PhysicalChildParent::OperatorApplicationCoverage(_)
    ));
    assert!(child.machine_span().byte_count() > 0);
    assert!(child.object_span().byte_count() > 0);
    assert_eq!(
        child.relocation(),
        native_realization::PhysicalRelocationDisposition::ResolvedInternalCall
    );

    // Independent replay from the published parts alone: every mutation
    // class must fail closed.
    let parts = report
        .into_retained_native_artifact()
        .expect("owned native artifact")
        .into_parts();

    let mut missing = replay_native_artifact_parts(&parts);
    let evidence = missing
        .physical_evidence
        .take()
        .expect("replay physical evidence")
        .into_parts();
    missing.physical_evidence = Some(
        native_realization::NativePhysicalEvidence::from_replayed_parts(
            native_realization::NativePhysicalEvidenceParts {
                projection: evidence.projection,
                children: Vec::new(),
                identity: evidence.identity,
            },
        ),
    );
    assert!(
        native_realization::NativeArtifact::from_replayed_parts(missing).is_err(),
        "a missing physical child must not replay"
    );

    let mut duplicate = replay_native_artifact_parts(&parts);
    let evidence = duplicate
        .physical_evidence
        .take()
        .expect("replay physical evidence")
        .into_parts();
    let [only_child] = evidence.children.as_slice() else {
        panic!("one physical child before duplication")
    };
    duplicate.physical_evidence = Some(
        native_realization::NativePhysicalEvidence::from_replayed_parts(
            native_realization::NativePhysicalEvidenceParts {
                projection: evidence.projection,
                children: vec![only_child.clone(), only_child.clone()],
                identity: evidence.identity,
            },
        ),
    );
    assert!(
        native_realization::NativeArtifact::from_replayed_parts(duplicate).is_err(),
        "a duplicate physical child must not replay"
    );

    let assert_mutated_child_rejected =
        |mutate: &dyn Fn(&mut native_realization::NativePhysicalChildParts)| {
            let mut replay = replay_native_artifact_parts(&parts);
            let evidence = replay
                .physical_evidence
                .take()
                .expect("replay physical evidence")
                .into_parts();
            let [child] = evidence.children.as_slice() else {
                panic!("one physical child before mutation")
            };
            let mut child = child.clone().into_parts();
            mutate(&mut child);
            replay.physical_evidence = Some(
                native_realization::NativePhysicalEvidence::from_replayed_parts(
                    native_realization::NativePhysicalEvidenceParts {
                        projection: evidence.projection,
                        children: vec![
                            native_realization::NativePhysicalChild::from_replayed_parts(child),
                        ],
                        identity: evidence.identity,
                    },
                ),
            );
            assert!(
                native_realization::NativeArtifact::from_replayed_parts(replay).is_err(),
                "a mutated physical child must not replay"
            );
        };
    // Role-swapped: the operator occurrence cannot be re-presented as a
    // boundary occurrence.
    assert_mutated_child_rejected(&|child| {
        assert!(matches!(
            child.parent,
            native_realization::PhysicalChildParent::OperatorApplicationCoverage(_)
        ));
        child.occurrence = native_realization::NativePhysicalOccurrence::Boundary(
            optimization_core::OptimizedBoundaryOccurrenceIdentity::from_bytes(
                child.occurrence.identity(),
            ),
        );
    });
    // Padded: the machine span must name exactly the emitted call interval.
    assert_mutated_child_rejected(&|child| {
        child.machine_span = native_realization::NativeByteSpan::from_replayed_parts(
            child.machine_span.offset(),
            child.machine_span.byte_count() + 1,
        );
    });
    // Substituted: the child must bind the validated projection identity.
    assert_mutated_child_rejected(&|child| {
        child.projection =
            optimization_core::NativeOptimizationProjectionIdentity::from_bytes([0x5A; 32]);
    });
    // Stale: an occurrence identity no surviving projection names cannot carry
    // a child.
    assert_mutated_child_rejected(&|child| {
        child.occurrence = native_realization::NativePhysicalOccurrence::Operator(
            optimization_core::OptimizedOperatorOccurrenceIdentity::from_bytes([0xA7; 32]),
        );
    });
}

#[test]
fn verified_eliminated_occurrence_needs_no_physical_child() {
    // The child-exemption half of the physical-child contract: a D29 covered
    // operation that an independently validated optimization proves
    // unreachable needs no physical child. Two private machines each apply
    // the boundary operator, and the entry machine reaches one of them only
    // through a constant-false transition arm, so checked D29 coverage names
    // both Terminal operations and the ordinary build binds each surviving
    // occurrence to its own child. ControlFlowCleanup's constant-conditional
    // rule folds the dead arm, the unreachable-private-machine rule then
    // proves the dead callee unreachable — pruned custody plus
    // ProvenUnreachableAt provenance for its call node in the validated
    // ledger — and the validated optimized projection keeps only the
    // surviving occurrence.
    let root = std::env::temp_dir().join(format!(
        "omega-optimizer-opt-in-eliminated-occurrence-{}-{}",
        std::process::id(),
        PROJECT_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create eliminated-occurrence project");
    std::fs::write(
        root.join("main.omg"),
        r#"data CheckedMath {}

boundary operator CheckedMath::select_left(left: u64, right: u64) -> u64;

data CheckedMathProvider {}

machine CheckedMathProvider::select_left_impl(left: u64, right: u64) -> u64
satisfies CheckedMath::select_left
{
    transition { _ -> left }
}

machine dead_pick() {
    let dead: u64 = CheckedMath::select_left(1u64, 2u64);
}

machine live_pick() {
    let kept: u64 = CheckedMath::select_left(7u64, 9u64);
}

data Main {}

machine Main::main() {
    transition false {
        true -> run_dead()
        false -> run_live()
    }

    state run_dead() {
        dead_pick();
        transition { _ -> run_live() }
    }

    state run_live() {
        live_pick();
        transition { _ -> done() }
    }

    state done() { }
}
"#,
    )
    .expect("write eliminated-occurrence main");
    std::fs::write(
        root.join("build.omg"),
        r#"machine build(builder: &mut Build) {
    builder.application("optimizer-eliminated-occurrence");
    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
    builder.optimizations.enable(Optimization::SelectedIncomingU12CompareImmediate);
}
"#,
    )
    .expect("write eliminated-occurrence build");
    let root_identity = package_identity(43);
    let inputs = PackageCompilationInputs::new_package(
        root_identity,
        vec![PackageSourceBinding::new(
            root_identity,
            "root",
            root.clone(),
        )],
        Vec::new(),
    )
    .expect("eliminated-occurrence package graph should validate");
    let report = compiler::compile(
        CompileRequest::new(CompileOptions {
            root_path: root.join("main.omg"),
            build_dir: Some(root.join("build")),
            target_name: Some("linux_x86_64".into()),
        })
        .with_requested_product(RequestedCompileProduct::NativeArtifact)
        .with_package_inputs(inputs),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect("the constant-dead boundary application must reach native custody");
    let artifact = report
        .retained_native_artifact()
        .expect("the ordinary build retains its native artifact");
    artifact
        .validate()
        .expect("the ordinary native artifact replays independently");
    assert!(matches!(
        artifact.physical_evidence_scope(),
        native_realization::NativePhysicalEvidenceScope::ValidatedOptimizedProjection(_)
    ));
    let physical = artifact
        .physical_evidence()
        .expect("both covered applications retain nonempty physical evidence");
    // Both covered applications survive the ordinary build: two operator
    // occurrences, each bound to exactly one physical child.
    let [first_covered, second_covered] = physical.projection().operator_occurrences() else {
        panic!("the ordinary build must keep both covered applications")
    };
    assert!(physical.projection().boundary_occurrences().is_empty());
    assert_eq!(physical.children().len(), 2);
    let covered_operations = [first_covered.operation(), second_covered.operation()];

    // Independently replay the published artifact sections into the canonical
    // optimizer and rerun the eliminative Psi schedule. `NativeRealizationRequest`
    // structurally cannot carry a Psi-phase selection, so this test drives the
    // identical `optimize_verified_abstract_input` admission that the
    // production optimization stage uses.
    let input = terminal_psi_to_abstract_operations::lower_artifact_for_optimization(
        terminal_psi_to_abstract_operations::ArtifactSections {
            semantic_bytes: artifact.semantic_bytes(),
            proof_bytes: artifact.proof_bytes(),
            obligation_ledger_bytes: None,
        },
        &proof_admission::AdmissionProfile::default(),
    )
    .and_then(|admitted| admitted.try_into_optimization_input())
    .expect("the published artifact replays into verified optimizer input");
    let selections =
        optimization_core::OptimizationSelections::new([Optimization::ControlFlowCleanup])
            .expect("one exact optimization selection");
    let optimized = native_realization::optimize_verified_abstract_input(
        input.clone(),
        native_realization::compiler_baseline_request_v1(&selections),
    )
    .expect("the validating run accepts the proven elimination");

    // Both applications remain D29-covered in the published Terminal module;
    // the optimized projection keeps exactly one of them.
    let coverage = artifact
        .boundary_application_coverage()
        .expect("the boundary applications retain D29 coverage");
    assert_eq!(coverage.references().len(), 2);
    let eliminated_scope =
        native_realization::NativePhysicalEvidenceScope::from_validated_optimization(
            optimized.plan(),
            optimized.validation().psi(),
            optimized.validation().identity(),
            optimized.validation().final_unit(),
            coverage,
        )
        .expect("the validated eliminated plan still derives its physical scope");
    let native_realization::NativePhysicalEvidenceScope::ValidatedOptimizedProjection(
        eliminated_projection,
    ) = &eliminated_scope
    else {
        panic!("the eliminated run derives a validated optimized projection")
    };
    let [survivor] = eliminated_projection.projection().operator_occurrences() else {
        panic!("exactly one operator occurrence survives the verified elimination")
    };
    assert!(
        eliminated_projection
            .projection()
            .boundary_occurrences()
            .is_empty()
    );
    let eliminated_operation = covered_operations
        .into_iter()
        .find(|operation| *operation != survivor.operation())
        .expect("the projection names one covered survivor and one covered elimination");
    assert!(
        coverage
            .references()
            .iter()
            .any(|reference| reference.terminal_operation() == eliminated_operation),
        "the eliminated operation retains its checked D29 coverage reference"
    );

    // The elimination is independently verified, not merely absent from the
    // final plan: the validated transformation ledger carries a
    // ProvenUnreachableAt rewrite row for the exact input node that owned the
    // covered call.
    let module = terminal_codec::decode_module(artifact.semantic_bytes())
        .expect("replay Terminal semantics");
    let eliminated_site = module
        .machines
        .iter()
        .flat_map(|machine| machine.blocks.iter().map(move |block| (machine.id, block)))
        .flat_map(|(machine, block)| {
            block
                .operations
                .iter()
                .enumerate()
                .map(move |(node, operation)| (machine, block.id, node, operation.id))
        })
        .find(|(.., operation)| *operation == eliminated_operation)
        .map(|(machine, block, node, _)| {
            (
                machine,
                block,
                u32::try_from(node).expect("operation node index fits u32"),
            )
        })
        .expect("the eliminated covered operation exists in the source Terminal module");
    // The covered call lived in a private machine the validated run proved
    // unreachable: the replay unit retains its pruned custody while the final
    // plan drops the whole function.
    let [pruned] = optimized.unit().pruned_machines.as_slice() else {
        panic!("ControlFlowCleanup must prove exactly one private machine unreachable")
    };
    let dead_machine = pruned.machine;
    assert_eq!(
        dead_machine, eliminated_site.0,
        "the eliminated covered operation lives in the proven-unreachable machine"
    );
    assert!(
        optimized
            .verified_input()
            .plan()
            .functions
            .iter()
            .any(|function| function.machine == dead_machine),
        "the eliminated machine is present in the verified input plan"
    );
    assert!(
        !optimized
            .plan()
            .functions
            .iter()
            .any(|function| function.machine == dead_machine),
        "the eliminated machine is absent from the validated final plan"
    );
    assert!(
        optimized
            .transformation_ledger()
            .records()
            .iter()
            .any(|record| {
                record.provenance.iter().any(|rewrite| {
                    !rewrite.disposition.is_realized()
                        && rewrite.input.node().is_some_and(|location| {
                            (location.machine, location.block, location.node) == eliminated_site
                        })
                })
            }),
        "the validated ledger proves the covered call site unreachable"
    );

    // The same coverage over the identity plan still requires the child: the
    // exemption attaches to the verified elimination, not to the coverage row.
    let identity = native_realization::optimize_verified_abstract_input(
        input,
        native_realization::compiler_baseline_request_v1(
            &optimization_core::OptimizationSelections::default(),
        ),
    )
    .expect("the identity run revalidates the unchanged plan");
    let retained_scope =
        native_realization::NativePhysicalEvidenceScope::from_validated_optimization(
            identity.plan(),
            identity.validation().psi(),
            identity.validation().identity(),
            identity.validation().final_unit(),
            coverage,
        )
        .expect("the identity run still derives its physical scope");
    let native_realization::NativePhysicalEvidenceScope::ValidatedOptimizedProjection(
        retained_projection,
    ) = &retained_scope
    else {
        panic!("the identity run derives a validated optimized projection")
    };
    assert_eq!(
        retained_projection
            .projection()
            .operator_occurrences()
            .len(),
        2,
        "a covered operation that survives still requires its physical child"
    );

    // A replayed child bound to the occurrence identity the eliminated
    // operation would have carried had it survived the eliminating run is
    // stale: no validated survivor set answers for it. The canonical identity
    // encoding is the same terminal + validation + final-unit authority walk
    // the projection derivation applies.
    let (eliminated_machine, eliminated_ordinal) = [first_covered, second_covered]
        .into_iter()
        .find(|occurrence| occurrence.operation() == eliminated_operation)
        .map(|occurrence| (occurrence.machine(), occurrence.operation_ordinal()))
        .expect("the ordinary projection names the eliminated covered operation");
    let parts = report
        .into_retained_native_artifact()
        .expect("owned native artifact")
        .into_parts();
    let mut stale = replay_native_artifact_parts(&parts);
    let evidence = stale
        .physical_evidence
        .take()
        .expect("replay physical evidence")
        .into_parts();
    let terminal = optimized.validation().psi();
    let mut canonical = Vec::new();
    canonical.extend_from_slice(&terminal.vocabulary_marker.get().to_le_bytes());
    canonical.extend_from_slice(terminal.program_fingerprint.as_bytes());
    canonical.extend_from_slice(&optimized.validation().identity().bytes());
    canonical.extend_from_slice(&optimized.validation().final_unit().bytes());
    canonical.extend_from_slice(&eliminated_machine.get().to_le_bytes());
    canonical.extend_from_slice(&eliminated_operation.get().to_le_bytes());
    canonical.extend_from_slice(
        &u64::try_from(eliminated_ordinal)
            .expect("occurrence ordinal")
            .to_le_bytes(),
    );
    let mut stale_child = evidence.children[0].clone().into_parts();
    stale_child.projection = eliminated_projection.projection().identity();
    stale_child.occurrence = native_realization::NativePhysicalOccurrence::Operator(
        optimization_core::OptimizedOperatorOccurrenceIdentity::from_canonical_bytes(&canonical),
    );
    stale.physical_evidence_scope = eliminated_scope.clone();
    stale.physical_evidence = Some(
        native_realization::NativePhysicalEvidence::from_replayed_parts(
            native_realization::NativePhysicalEvidenceParts {
                projection: eliminated_projection.projection().clone(),
                children: vec![
                    native_realization::NativePhysicalChild::from_replayed_parts(stale_child),
                ],
                identity: evidence.identity,
            },
        ),
    );
    assert!(
        native_realization::NativeArtifact::from_replayed_parts(stale).is_err(),
        "a physical child bound to a verified-eliminated occurrence must not replay"
    );

    // An unverified omission is equally rejected: under the published plan's
    // own scope both covered occurrences still demand their children, so
    // dropping one cannot replay.
    let mut missing = replay_native_artifact_parts(&parts);
    let mut retained = missing
        .physical_evidence
        .take()
        .expect("replay physical evidence")
        .into_parts();
    retained.children.pop();
    missing.physical_evidence = Some(
        native_realization::NativePhysicalEvidence::from_replayed_parts(
            native_realization::NativePhysicalEvidenceParts {
                projection: retained.projection,
                children: retained.children,
                identity: retained.identity,
            },
        ),
    );
    assert!(
        native_realization::NativeArtifact::from_replayed_parts(missing).is_err(),
        "omitting a required physical child must not replay"
    );
}

fn replay_native_artifact_parts(
    parts: &native_realization::NativeArtifactParts,
) -> native_realization::NativeArtifactParts {
    let module = terminal_codec::decode_module(parts.psi_artifact.semantic_bytes())
        .expect("replay Terminal semantics");
    let proof = terminal_codec::decode_proof_bundle(parts.psi_artifact.proof_bytes())
        .expect("replay Terminal proof");
    let debug = parts
        .psi_artifact
        .debug_bytes()
        .map(|bytes| terminal_codec::decode_debug_map(&module, bytes).expect("debug map"));
    native_realization::NativeArtifactParts {
        target: parts.target,
        psi_artifact: terminal_codec::CanonicalTerminalArtifact::from_parts(
            &module,
            &proof,
            parts.psi_artifact.optimization(),
            debug.as_ref(),
        )
        .expect("reconstruct canonical Terminal artifact"),
        object: parts.object.clone(),
        image: parts.image.clone(),
        selected_provider_closure_report_identity: parts.selected_provider_closure_report_identity,
        selected_provider_closure_digest: parts.selected_provider_closure_digest,
        selected_provider_plans: parts.selected_provider_plans.clone(),
        provider_executions: parts.provider_executions.clone(),
        terminal_authority_policy_identity: parts.terminal_authority_policy_identity,
        terminal_authority_permission_policy_identity: parts
            .terminal_authority_permission_policy_identity,
        terminal_authority_closure_review: parts.terminal_authority_closure_review.clone(),
        boundary_application_coverage: parts.boundary_application_coverage.clone(),
        physical_evidence_scope: parts.physical_evidence_scope.clone(),
        physical_evidence: parts.physical_evidence.clone(),
    }
}

#[test]
fn x86_rel8_relaxation_selection_round_trips_but_remains_default_off() {
    let absent = project("x86-rel8-default-off", None);
    let checked = compile_to_checked(CheckedCompileRequest::new(&absent.join("main.omg"), None))
        .expect("an absent build must leave branch relaxation disabled");
    assert!(
        !checked
            .optimization_selections()
            .contains(Optimization::X86RelaxConditionalBranchesToRel8V1)
    );

    let selected = project(
        "x86-rel8-selected",
        Some(
            r#"machine build(builder: &mut Build) {
    builder.application("optimizer-x86-rel8-selected");
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
    builder.optimizations.enable(Optimization::X86RelaxConditionalBranchesToRel8V1);
}
"#,
        ),
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(
        &selected.join("main.omg"),
        Some("windows_x86_64"),
    ))
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
    let report = compile_native_and_publish(CompileOptions {
        root_path: selected.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("windows_x86_64".into()),
    })
    .expect("the selected layout rule executes through native publication on x86");
    let executable = report
        .checked_native_executable_path()
        .expect("publication returns the executable receipt");
    assert_eq!(executable, build_dir.join("omega-program.exe").as_path());
    assert!(executable.is_file());
    assert!(!build_dir.join("omega-program").exists());
}

#[test]
fn aarch64_cbnz_fusion_selection_round_trips_but_remains_default_off() {
    let absent = project("aarch64-cbnz-default-off", None);
    let checked = compile_to_checked(CheckedCompileRequest::new(&absent.join("main.omg"), None))
        .expect("an absent build must leave AArch64 CBNZ fusion disabled");
    assert!(
        !checked
            .optimization_selections()
            .contains(Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1)
    );

    let selected = project(
        "aarch64-cbnz-selected",
        Some(
            r#"machine build(builder: &mut Build) {
    builder.application("optimizer-aarch64-cbnz-selected");
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
    builder.optimizations.enable(Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1);
}
"#,
        ),
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(
        &selected.join("main.omg"),
        Some("windows_x86_64"),
    ))
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
        target_name: Some("windows_x86_64".into()),
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
    let checked = compile_to_checked(CheckedCompileRequest::new(&absent.join("main.omg"), None))
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
    let checked = compile_to_checked(CheckedCompileRequest::new(&selected.join("main.omg"), None))
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
    let checked = compile_to_checked(CheckedCompileRequest::new(&absent.join("main.omg"), None))
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
    let checked = compile_to_checked(CheckedCompileRequest::new(&selected.join("main.omg"), None))
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
fn x86_mov_r32_imm32_materialization_selection_round_trips_but_remains_default_off() {
    let absent = project("x86-mov-r32-imm32-default-off", None);
    let checked = compile_to_checked(CheckedCompileRequest::new(&absent.join("main.omg"), None))
        .expect("an absent build must leave x86 MOV-r32-imm32 materialization disabled");
    assert!(
        !checked
            .optimization_selections()
            .contains(Optimization::X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1)
    );

    let selected = project(
        "x86-mov-r32-imm32-selected",
        Some(
            r#"machine build(builder: &mut Build) {
    builder.application("optimizer-x86-mov-r32-imm32-selected");
    builder.optimizations.enable(Optimization::X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1);
}
"#,
        ),
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(&selected.join("main.omg"), None))
        .expect("the named x86 MOV-r32-imm32 materialization selection should evaluate");
    assert_eq!(
        checked.optimization_selections().as_slice(),
        &[Optimization::X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1]
    );
    assert_eq!(
        checked.optimization_selection_identity(),
        checked.optimization_selections().identity()
    );
}

#[test]
fn x86_mov_r64_imm32_materialization_selection_round_trips_but_remains_default_off() {
    let absent = project("x86-mov-r64-imm32-default-off", None);
    let checked = compile_to_checked(CheckedCompileRequest::new(&absent.join("main.omg"), None))
        .expect("an absent build must leave x86 MOV-r64-imm32 materialization disabled");
    assert!(
        !checked
            .optimization_selections()
            .contains(Optimization::X86SelectMovR64Imm32SignExtendedI64MaterializationV1)
    );

    let selected = project(
        "x86-mov-r64-imm32-selected",
        Some(
            r#"machine build(builder: &mut Build) {
    builder.application("optimizer-x86-mov-r64-imm32-selected");
    builder.optimizations.enable(Optimization::X86SelectMovR64Imm32SignExtendedI64MaterializationV1);
}
"#,
        ),
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(&selected.join("main.omg"), None))
        .expect("the named x86 MOV-r64-imm32 materialization selection should evaluate");
    assert_eq!(
        checked.optimization_selections().as_slice(),
        &[Optimization::X86SelectMovR64Imm32SignExtendedI64MaterializationV1]
    );
    assert_eq!(
        checked.optimization_selection_identity(),
        checked.optimization_selections().identity()
    );
}

#[test]
fn shared_entry_fixed_view_copy_selection_round_trips_but_remains_default_off() {
    let absent = project("shared-entry-copy-default-off", None);
    let checked = compile_to_checked(CheckedCompileRequest::new(&absent.join("main.omg"), None))
        .expect("an absent build must leave shared-entry copy insertion disabled");
    assert!(
        !checked
            .optimization_selections()
            .contains(Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1)
    );

    let selected = project(
        "shared-entry-copy-selected",
        Some(
            r#"machine build(builder: &mut Build) {
    builder.application("optimizer-shared-entry-copy-selected");
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
    builder.optimizations.enable(Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1);
}
"#,
        ),
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(
        &selected.join("main.omg"),
        Some("windows_x86_64"),
    ))
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
    let report = compile_native_and_publish(CompileOptions {
        root_path: selected.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("windows_x86_64".into()),
    })
    .expect("the selected allocation-recovery rule executes through native publication");
    let executable = report
        .checked_native_executable_path()
        .expect("publication returns the executable receipt");
    assert_eq!(executable, build_dir.join("omega-program.exe").as_path());
    assert!(executable.is_file());
    assert!(!build_dir.join("omega-program").exists());
}

#[test]
fn active_resident_multi_use_rematerialization_selection_round_trips_but_remains_default_off() {
    let absent = project("active-resident-rematerialization-default-off", None);
    let checked = compile_to_checked(CheckedCompileRequest::new(&absent.join("main.omg"), None))
        .expect("an absent build must leave active-resident rematerialization disabled");
    assert!(
        !checked
            .optimization_selections()
            .contains(Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1)
    );

    let selected = project(
        "active-resident-rematerialization-selected",
        Some(
            r#"machine build(builder: &mut Build) {
    builder.application("optimizer-active-resident-rematerialization-selected");
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
    builder.optimizations.enable(Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1);
}
"#,
        ),
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(
        &selected.join("main.omg"),
        Some("windows_x86_64"),
    ))
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
        target_name: Some("windows_x86_64".into()),
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
fn terminal_product_routes_selected_psi_pass_to_preterminal_stage() {
    let root = project(
        "selected-terminal-product",
        Some(
            r#"machine build(builder: &mut Build) {
    builder.application("optimizer-selected-terminal-product");
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
    builder.optimizations.enable(Optimization::ControlFlowCleanup);
}
"#,
        ),
    );
    let report = compiler::compile(
        CompileRequest::new(CompileOptions {
            root_path: root.join("main.omg"),
            build_dir: None,
            target_name: Some("windows_x86_64".into()),
        })
        .with_requested_product(RequestedCompileProduct::TerminalArtifact),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect("a selected Psi pass executes at its pre-Terminal owner before publication");
    let artifact = report
        .artifact()
        .expect("the Terminal product retains its canonical artifact");
    let executed = artifact.optimization().selections();
    assert_eq!(executed.as_slice().len(), 1);
    assert!(
        executed.contains(
            Optimization::ControlFlowCleanup
                .optimization()
                .expect("ControlFlowCleanup is a Psi selection")
        )
    );
    assert_eq!(
        artifact.manifest().optimization(),
        artifact.optimization().identity(),
        "the published manifest binds the execution that produced it"
    );
    artifact
        .validate()
        .expect("the optimized Terminal artifact replays");
}

#[test]
fn terminal_product_executes_and_can_disable_the_selected_psi_pass() {
    let root = project(
        "terminal-dead-scalars",
        Some(
            r#"
machine build(builder: &mut Build) {
    builder.application("terminal-dead-scalars");
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
    builder.optimizations.enable(Optimization::DeadPureScalarElimination);
}
"#,
        ),
    );
    let request = || {
        CompileRequest::new(CompileOptions {
            root_path: root.join("main.omg"),
            build_dir: None,
            target_name: Some("windows_x86_64".into()),
        })
        .with_requested_product(RequestedCompileProduct::TerminalArtifact)
    };
    let identity = compiler::compile(request().with_optimization_rollback(
        OptimizationRollback::new([Optimization::DeadPureScalarElimination]).unwrap(),
    ))
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect("rollback executes the identity stage");
    let selected = compiler::compile(request())
        .and_then(compiler::CompileOutcomes::into_single_report)
        .expect("selected pre-Terminal pass executes");
    assert_eq!(
        identity.artifact().unwrap().semantic_bytes(),
        selected.artifact().unwrap().semantic_bytes(),
        "an empty Unit body is already minimal"
    );
    assert_ne!(
        identity.artifact().unwrap().optimization().selection(),
        selected.artifact().unwrap().optimization().selection()
    );
    let receipt = identity.optimization_rollback_receipt().unwrap();
    assert!(receipt.effective().is_empty());
    assert!(
        receipt
            .actually_disabled()
            .contains(Optimization::DeadPureScalarElimination)
    );
    assert!(identity.has_consistent_executable_publication_custody());
    assert!(selected.optimization_rollback_receipt().is_none());
    selected.artifact().unwrap().validate().unwrap();
    assert!(
        selected
            .with_terminal_optimization_rollback(Some(receipt.clone()))
            .is_err(),
        "a rollback receipt cannot substitute for the selection that actually ran"
    );
}

#[test]
fn terminal_product_eliminates_unused_block_parameters_and_edge_arguments() {
    let root = project(
        "terminal-dead-block-parameters",
        Some(
            r#"
machine build(builder: &mut Build) {
    builder.application("terminal-dead-block-parameters");
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
    builder.optimizations.enable(Optimization::DeadPureScalarElimination);
}
"#,
        ),
    );
    std::fs::write(
        root.join("main.omg"),
        "data Main { value: i32; }\n\
         machine Main::compute(a: i32, b: i32) -> i32 {\n\
             let unused: bool = a < b;\n\
             let dead_const: i32 = 7;\n\
             transition {\n\
                 a == b -> (1)\n\
                 _ -> (2)\n\
             }\n\
         }\n\
         machine Main::main(&mut self) {\n\
             self.value = Main::compute(1, 2);\n\
         }\n",
    )
    .expect("write dead block parameter source");
    let request = || {
        CompileRequest::new(CompileOptions {
            root_path: root.join("main.omg"),
            build_dir: None,
            target_name: Some("windows_x86_64".into()),
        })
        .with_requested_product(RequestedCompileProduct::TerminalArtifact)
    };
    let identity = compiler::compile(request().with_optimization_rollback(
        OptimizationRollback::new([Optimization::DeadPureScalarElimination]).unwrap(),
    ))
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect("rollback executes the identity stage");
    let selected = compiler::compile(request())
        .and_then(compiler::CompileOutcomes::into_single_report)
        .expect("selected pre-Terminal pass executes");
    selected.artifact().unwrap().validate().unwrap();
    let identity_module =
        terminal_codec::decode_module(identity.artifact().unwrap().semantic_bytes())
            .expect("identity artifact decodes");
    let selected_module =
        terminal_codec::decode_module(selected.artifact().unwrap().semantic_bytes())
            .expect("selected artifact decodes");
    let compute_id = |module: &terminal_psi::TerminalModule| {
        module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .find_map(|operation| match &operation.kind {
                terminal_psi::OperationKind::Call { callee, .. } => Some(*callee),
                _ => None,
            })
            .expect("main calls compute")
    };
    fn machine(
        module: &terminal_psi::TerminalModule,
        id: semantic_vocabulary::MachineId,
    ) -> &terminal_psi::TerminalMachine {
        module
            .machines
            .iter()
            .find(|machine| machine.id == id)
            .expect("machine exists")
    }
    let old_compute = machine(&identity_module, compute_id(&identity_module));
    let new_compute = machine(&selected_module, compute_id(&selected_module));
    for operation in new_compute
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
    {
        assert!(
            !matches!(
                operation.kind,
                terminal_psi::OperationKind::IntegerLessThan { .. }
            ),
            "the dead comparison is eliminated before publication"
        );
        assert!(
            !matches!(
                operation.kind,
                terminal_psi::OperationKind::IntegerConstant {
                    value: semantic_vocabulary::IntegerValue::Signed(7)
                }
            ),
            "the dead constant is eliminated before publication"
        );
    }
    let scalar_wiring = |machine: &terminal_psi::TerminalMachine| -> (usize, usize) {
        let parameters = machine
            .blocks
            .iter()
            .map(|block| block.parameters.len())
            .sum();
        let arguments = machine
            .blocks
            .iter()
            .map(|block| match &block.terminator {
                terminal_psi::Terminator::Jump { arguments, .. } => arguments.len(),
                terminal_psi::Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => when_true.arguments.len() + when_false.arguments.len(),
                _ => 0,
            })
            .sum();
        (parameters, arguments)
    };
    let (old_parameters, old_arguments) = scalar_wiring(old_compute);
    let (new_parameters, new_arguments) = scalar_wiring(new_compute);
    assert!(
        new_parameters < old_parameters,
        "selected compute drops dead block parameters: {old_parameters} -> {new_parameters}"
    );
    assert!(
        new_arguments < old_arguments,
        "selected compute drops matching edge arguments: {old_arguments} -> {new_arguments}"
    );
    let operation_kinds = |machine: &terminal_psi::TerminalMachine| {
        machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .map(|operation| operation.kind.clone())
            .collect::<Vec<_>>()
    };
    for old_main in &identity_module.machines {
        if old_main.id == old_compute.id {
            continue;
        }
        let new_main = machine(&selected_module, old_main.id);
        assert_eq!(operation_kinds(new_main), operation_kinds(old_main));
    }
}

#[test]
fn terminal_product_propagates_copies_through_block_parameters() {
    let root = project(
        "terminal-copy-propagation",
        Some(
            r#"
machine build(builder: &mut Build) {
    builder.application("terminal-copy-propagation");
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
    builder.optimizations.enable(Optimization::CopyPropagation);
}
"#,
        ),
    );
    std::fs::write(
        root.join("main.omg"),
        "data Main { value: i32; }\n\
         machine Main::compute(a: i32, b: i32) -> i32 {\n\
             let unused: bool = a < b;\n\
             let dead_const: i32 = 7;\n\
             transition {\n\
                 a == b -> (1)\n\
                 _ -> (2)\n\
             }\n\
         }\n\
         machine Main::main(&mut self) {\n\
             self.value = Main::compute(1, 2);\n\
         }\n",
    )
    .expect("write forwarded-copy source");
    let request = || {
        CompileRequest::new(CompileOptions {
            root_path: root.join("main.omg"),
            build_dir: None,
            target_name: Some("windows_x86_64".into()),
        })
        .with_requested_product(RequestedCompileProduct::TerminalArtifact)
    };
    let identity = compiler::compile(request().with_optimization_rollback(
        OptimizationRollback::new([Optimization::CopyPropagation]).unwrap(),
    ))
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect("rollback executes the identity stage");
    let selected = compiler::compile(request())
        .and_then(compiler::CompileOutcomes::into_single_report)
        .expect("selected pre-Terminal pass executes");
    selected.artifact().unwrap().validate().unwrap();
    let identity_module =
        terminal_codec::decode_module(identity.artifact().unwrap().semantic_bytes())
            .expect("identity artifact decodes");
    let selected_module =
        terminal_codec::decode_module(selected.artifact().unwrap().semantic_bytes())
            .expect("selected artifact decodes");
    let compute_id = |module: &terminal_psi::TerminalModule| {
        module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .find_map(|operation| match &operation.kind {
                terminal_psi::OperationKind::Call { callee, .. } => Some(*callee),
                _ => None,
            })
            .expect("main calls compute")
    };
    fn machine(
        module: &terminal_psi::TerminalModule,
        id: semantic_vocabulary::MachineId,
    ) -> &terminal_psi::TerminalMachine {
        module
            .machines
            .iter()
            .find(|machine| machine.id == id)
            .expect("compute machine exists")
    }
    let old_compute = machine(&identity_module, compute_id(&identity_module));
    let new_compute = machine(&selected_module, compute_id(&selected_module));
    let scalar_wiring = |machine: &terminal_psi::TerminalMachine| -> (usize, usize) {
        let parameters = machine
            .blocks
            .iter()
            .map(|block| block.parameters.len())
            .sum();
        let arguments = machine
            .blocks
            .iter()
            .map(|block| match &block.terminator {
                terminal_psi::Terminator::Jump { arguments, .. } => arguments.len(),
                terminal_psi::Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => when_true.arguments.len() + when_false.arguments.len(),
                _ => 0,
            })
            .sum();
        (parameters, arguments)
    };
    let (old_parameters, old_arguments) = scalar_wiring(old_compute);
    let (new_parameters, new_arguments) = scalar_wiring(new_compute);
    assert!(
        new_parameters < old_parameters,
        "selected compute collapses forwarded copies: {old_parameters} -> {new_parameters}"
    );
    assert!(
        new_arguments < old_arguments,
        "selected compute drops matching edge arguments: {old_arguments} -> {new_arguments}"
    );
    let dispatch = new_compute
        .blocks
        .iter()
        .find(|block| {
            matches!(
                block.terminator,
                terminal_psi::Terminator::Conditional { .. }
            )
        })
        .expect("the equality dispatch survives");
    let equal = dispatch
        .operations
        .iter()
        .find_map(|operation| match &operation.kind {
            terminal_psi::OperationKind::IntegerEqual { left, right } => Some((*left, *right)),
            _ => None,
        })
        .expect("the dispatch condition survives");
    assert_eq!(
        equal,
        (new_compute.parameters[0].id, new_compute.parameters[1].id),
        "the equality reads the machine parameters directly"
    );
    assert_ne!(
        identity.artifact().unwrap().semantic_bytes(),
        selected.artifact().unwrap().semantic_bytes(),
        "copy propagation changes the published semantic bytes"
    );
}

#[test]
fn terminal_product_retains_the_exact_pending_physical_selection() {
    let root = project(
        "selected-terminal-physical-proposal",
        Some(
            r#"machine build(builder: &mut Build) {
    builder.application("optimizer-selected-terminal-physical-proposal");
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
    builder.optimizations.enable(Optimization::SelectedIncomingU12ExactAddImmediate);
}
"#,
        ),
    );
    let report = compiler::compile(
        CompileRequest::new(CompileOptions {
            root_path: root.join("main.omg"),
            build_dir: None,
            target_name: Some("windows_x86_64".into()),
        })
        .with_requested_product(RequestedCompileProduct::TerminalArtifact),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect("a physical selection remains pending after Terminal publication");
    let proposal = report
        .terminal_native_realization_proposal()
        .expect("target-constrained Terminal product retains its native proposal");
    let expected = optimization_core::PostTerminalOptimizationSelections::new(
        optimization_core::OptimizationSelections::new([
            Optimization::SelectedIncomingU12ExactAddImmediate,
        ])
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        proposal.post_terminal_optimizations().selections(),
        &expected
    );
    assert_eq!(
        proposal.post_terminal_optimizations().complete_selection(),
        expected.identity()
    );
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
    let inputs = PackageCompilationInputs::new_package(
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

    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
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
    let inputs = PackageCompilationInputs::new_package(
        root_identity,
        vec![PackageSourceBinding::new(
            root_identity,
            "root",
            root.clone(),
        )],
        Vec::new(),
    )
    .expect("root-only optimizer package graph should validate");

    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect("root package build selection should check");
    assert_eq!(
        checked.optimization_selections().as_slice(),
        &[Optimization::GlobalValueNumbering]
    );
}

fn checked_tree_pruning_project(label: &str, pruning: bool, roots: bool) -> PathBuf {
    let root = project(
        label,
        Some(&format!(
            "machine build(builder: &mut Build) {{\n\
             builder.application(\"{label}\");\n\
             {binding}\
             {optimization}}}\n",
            binding = roots
                .then_some("    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);\n")
                .unwrap_or(""),
            optimization = pruning
                .then_some(
                    "    builder.optimizations.enable(Optimization::CheckedTreeProductPruning);\n",
                )
                .unwrap_or(""),
        )),
    );
    std::fs::write(
        root.join("main.omg"),
        "data Main { value: u8; }\nmachine Main::main(&mut self) { }\ndata Dead { value: u8; }\nmachine Dead::unused(&mut self) { }\n",
    )
    .expect("write checked-tree product pruning source");
    root
}

#[test]
fn checked_tree_product_pruning_retains_the_selected_product_root() {
    let root = checked_tree_pruning_project("checked-tree-pruning", true, true);

    let checked = compile_to_checked(CheckedCompileRequest::new(
        &root.join("main.omg"),
        Some("linux_x86_64"),
    ))
    .expect("the selected checked-tree product compiles");

    let machine_names = checked
        .typed
        .machines()
        .iter()
        .map(|machine| machine.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(machine_names, ["Main::main"]);

    let entry = checked
        .selected_program_entry()
        .expect("the selected target retains its program entry")
        .source_signature()
        .machine_symbol();
    let selection = checked
        .checked_tree_product_selection()
        .expect("checked-tree product pruning retains its selection evidence");
    assert_eq!(selection.roots().machines(), &[entry]);
    assert_eq!(selection.retained_machines(), &[entry]);
    let pruned_names = selection
        .pruned_machines()
        .iter()
        .map(|symbol| checked.typed.symbols.display_path(*symbol, "::"))
        .collect::<Vec<_>>();
    for pruned in ["Dead::unused", "build"] {
        assert!(
            pruned_names.iter().any(|name| name.as_str() == pruned),
            "pruned: {pruned_names:?}"
        );
    }
    assert_ne!(selection.identity().as_bytes(), [0; 32]);
    assert!(
        checked
            .optimization_selections()
            .contains(Optimization::CheckedTreeProductPruning)
    );
}

#[test]
fn absent_checked_tree_pruning_selection_is_the_identity_boundary() {
    let root = checked_tree_pruning_project("checked-tree-identity", false, true);

    let checked = compile_to_checked(CheckedCompileRequest::new(
        &root.join("main.omg"),
        Some("linux_x86_64"),
    ))
    .expect("an unselected checked-tree phase is the identity boundary");

    let machine_names = checked
        .typed
        .machines()
        .iter()
        .map(|machine| machine.name.as_str())
        .collect::<Vec<_>>();
    for name in ["Main::main", "Dead::unused", "build"] {
        assert!(machine_names.contains(&name), "retained: {machine_names:?}");
    }
    assert!(checked.checked_tree_product_selection().is_none());
}

#[test]
fn checked_tree_product_pruning_without_a_bound_product_root_rejects() {
    let root = checked_tree_pruning_project("checked-tree-no-roots", true, false);

    let diagnostics = compile_to_checked(CheckedCompileRequest::new(
        &root.join("main.omg"),
        Some("linux_x86_64"),
    ))
    .expect_err("product pruning selected without a bound product root rejects");
    assert!(
        diagnostic_messages(&diagnostics).contains("requires at least one bound product root"),
        "unexpected diagnostics: {}",
        diagnostic_messages(&diagnostics)
    );
}

#[test]
fn checked_tree_product_pruning_cannot_hide_an_invalid_authored_declaration() {
    let root = checked_tree_pruning_project("checked-tree-invalid", true, true);
    std::fs::write(
        root.join("main.omg"),
        "data Main { value: u8; }\nmachine Main::main(&mut self) { }\ndata Dead { value: u8; }\nmachine Dead::unused(&mut self) { self.missing(); }\n",
    )
    .expect("write unreachable invalid authored declaration");

    let diagnostics = compile_to_checked(CheckedCompileRequest::new(
        &root.join("main.omg"),
        Some("linux_x86_64"),
    ))
    .expect_err("checking precedes pruning, so the unreachable invalid machine rejects");
    let messages = diagnostic_messages(&diagnostics);
    assert!(
        messages.contains("missing"),
        "unexpected diagnostics: {messages}"
    );
    assert!(
        !messages.contains("product root"),
        "the rejection must come from checking, not pruning: {messages}"
    );
}

#[test]
fn checked_tree_product_pruning_rollback_restores_the_full_product() {
    let root = checked_tree_pruning_project("checked-tree-rollback", true, true);

    let checked = compile_to_checked(CheckedCompileRequest {
        optimization_rollback: OptimizationRollback::new([Optimization::CheckedTreeProductPruning])
            .expect("the rollback selection is unique"),
        ..CheckedCompileRequest::new(&root.join("main.omg"), Some("linux_x86_64"))
    })
    .expect("the rolled-back checked-tree phase is the identity boundary");

    let machine_names = checked
        .typed
        .machines()
        .iter()
        .map(|machine| machine.name.as_str())
        .collect::<Vec<_>>();
    for name in ["Main::main", "Dead::unused", "build"] {
        assert!(machine_names.contains(&name), "retained: {machine_names:?}");
    }
    assert!(checked.checked_tree_product_selection().is_none());
    // The authored selection remains the retained identity even though the
    // effective selection executed as identity.
    assert!(
        checked
            .optimization_selections()
            .contains(Optimization::CheckedTreeProductPruning)
    );
}
