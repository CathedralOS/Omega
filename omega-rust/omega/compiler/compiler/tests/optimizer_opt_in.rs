//! Fixtures shared by the optimizer opt-in tests: native compilation,
//! package identities and replayed artifact parts.

#[path = "optimizer_opt_in/build_selection_reports.rs"]
mod build_selection_reports;
#[path = "support/console_acceptance.rs"]
mod console_acceptance;
#[path = "support/linux_entry_acceptance.rs"]
mod linux_entry_acceptance;
#[path = "support/macos_entry_acceptance.rs"]
mod macos_entry_acceptance;
#[path = "optimizer_opt_in/product_pruning.rs"]
mod product_pruning;
#[path = "optimizer_opt_in/selected_lowering_replays.rs"]
mod selected_lowering_replays;

use compiler::{CompileOptions, CompileRequest, RequestedCompileProduct};
use optimization_core::Optimization;
use package_compilation::{
    PackageCompilationInputs, PackageDependencyBinding, PackageSourceBinding,
};
use std::fmt::Write as _;

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
    let root_binding = if optimization.execution_phase()
        == optimization_core::OptimizationExecutionPhase::CheckedTrees
    {
        "    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);\n"
    } else {
        ""
    };
    format!(
        "machine build(builder: &mut Build) {{\n    builder.application(\"optimizer-exact-vocabulary\");\n{root_binding}{enable_call}    builder.optimizations.emit_report();\n}}\n"
    )
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

fn checked_tree_pruning_project(label: &str, pruning: bool, roots: bool) -> PathBuf {
    let root = project(
        label,
        Some(&format!(
            "machine build(builder: &mut Build) {{\n\
             builder.application(\"{label}\");\n\
             {binding}\
             {optimization}}}\n",
            binding = if roots {
                "    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);\n"
            } else {
                ""
            },
            optimization = if pruning {
                "    builder.optimizations.enable(Optimization::CheckedTreeProductPruning);\n"
            } else {
                ""
            },
        )),
    );
    std::fs::write(
        root.join("main.omg"),
        "data Main { value: u8; }\nmachine Main::main(&mut self) { }\ndata Dead { value: u8; }\nmachine Dead::unused(&mut self) { }\n",
    )
    .expect("write checked-tree product pruning source");
    root
}
