//! Tests for complete Build evaluation.

use crate::{BuildConfig, BuildEvaluationRequest, evaluate_build};

struct PackageFixture {
    root: std::path::PathBuf,
    source_id: source::SourceId,
    sources: source::SourceMap,
    typed: typed_trees::TypedTrees,
    packages: package_compilation::PackageCompilationInputs,
    target_machine_declarations:
        provider_planning::target_machines::SelectedTargetMachineDeclarations,
}

impl PackageFixture {
    fn new(text: &str) -> Self {
        let directory = std::env::current_dir()
            .expect("test directory")
            .canonicalize()
            .expect("canonical test directory");
        let root = directory.join("activation_fixture.omg");
        let package = semantic_vocabulary::PackageKeyIdentity::from_digest([7; 32])
            .expect("package identity");
        let mut sources = source::SourceMap::default();
        let source_id = sources
            .add_with_metadata(
                root.clone(),
                text.to_owned(),
                directory.clone(),
                Some(package),
                source::SourceOrigin::User,
            )
            .source_id;
        let tokens = source_files_to_tokens::Lexer::new(text)
            .tokenize()
            .expect("tokens");
        let syntax =
            tokens_to_syntax_trees::parse_syntax_trees_with_id(source_id, &tokens).expect("syntax");
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
                syntax: &syntax,
                sources: Some(std::sync::Arc::new(sources.clone())),
                top_level_bindings: Vec::new(),
            },
        )
        .expect("resolution");
        let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
            .expect("typing");
        let packages = package_compilation::PackageCompilationInputs::new_package(
            package,
            vec![package_compilation::PackageSourceBinding::new(
                package, "fixture", directory,
            )],
            Vec::new(),
        )
        .expect("package inputs");
        Self {
            root,
            source_id,
            sources,
            typed,
            packages,
            target_machine_declarations: Default::default(),
        }
    }

    fn request(&self, has_companion: bool) -> BuildEvaluationRequest<'_> {
        BuildEvaluationRequest {
            root_path: &self.root,
            typed: &self.typed,
            target_machine_declarations: &self.target_machine_declarations,
            build_source_id: has_companion.then_some(self.source_id),
            sources: &self.sources,
            package_inputs: Some(&self.packages),
            generated_source_custody: &[],
            execution_profile: None,
            build_directory: None,
            filesystem_sponsor: None,
            evaluation_sponsor: None,
            snapshot: None,
            restricted_build_grants: None,
            artifact_only: false,
        }
    }
}

#[test]
fn no_build_evaluation_preserves_default_configuration_and_evidence() {
    let typed = typed_trees::TypedTrees::default();
    let sources = source::SourceMap::default();
    let evaluated = evaluate_build(
        BuildEvaluationRequest {
            root_path: std::path::Path::new("main.omg"),
            typed: &typed,
            target_machine_declarations: &Default::default(),
            build_source_id: None,
            sources: &sources,
            package_inputs: None,
            generated_source_custody: &[],
            execution_profile: None,
            build_directory: None,
            filesystem_sponsor: None,
            evaluation_sponsor: None,
            snapshot: None,
            restricted_build_grants: None,
            artifact_only: false,
        },
        &mut artifacts::compile_timings::CompileTimings::default(),
    )
    .expect("the empty program has a complete no-build result");
    assert!(evaluated.restricted_build_requests.is_empty());
    assert_eq!(evaluated.base_source_consumption_commitment, None);
    let executed = evaluated.configuration;
    assert_eq!(executed.config, BuildConfig::default());
    assert_eq!(
        executed.optimization_report_request,
        optimization_core::OptimizationReportRequest::Suppressed
    );
    assert_eq!(executed.evaluation_usage, None);
    assert_eq!(executed.observation_summary, None);
    assert_eq!(executed.selected_build_machine_symbol, None);
    assert!(executed.generated_sources.is_empty());
}

#[test]
fn absent_build_does_not_capture_an_execution_snapshot() {
    let typed = typed_trees::TypedTrees::default();
    let sources = source::SourceMap::default();
    let snapshot = package_compilation::BuildSnapshotRequest::new([]);
    let evaluated = evaluate_build(
        BuildEvaluationRequest {
            root_path: std::path::Path::new("main.omg"),
            typed: &typed,
            target_machine_declarations: &Default::default(),
            build_source_id: None,
            sources: &sources,
            package_inputs: None,
            generated_source_custody: &[],
            execution_profile: None,
            build_directory: None,
            filesystem_sponsor: None,
            evaluation_sponsor: None,
            snapshot: Some(&snapshot),
            restricted_build_grants: None,
            artifact_only: false,
        },
        &mut artifacts::compile_timings::CompileTimings::default(),
    )
    .expect("without an activation there is no source snapshot to capture");
    assert_eq!(evaluated.configuration.config, BuildConfig::default());
    assert_eq!(evaluated.configuration.observation_summary, None);
}

#[test]
fn package_without_activation_retains_source_identity_without_preliminary_checking() {
    let fixture = PackageFixture::new("machine answer() -> u32 { 7 }");
    let mut timings = artifacts::compile_timings::CompileTimings::enabled();
    // A companion source may contain declarations but no Build entry.
    let evaluated = evaluate_build(fixture.request(true), &mut timings).expect("default Build");
    let expected = package_compilation::derive_package_compilation_subject(
        fixture.typed.symbols.source_files(),
        &fixture.packages,
        &[],
    )
    .expect("retained source identity")
    .source_consumption_commitment();
    assert_eq!(evaluated.base_source_consumption_commitment, Some(expected));
    assert!(
        timings.phases().is_empty(),
        "no preliminary check: {:#?}",
        timings.phases()
    );
    typed_trees_to_checked_trees::lower_typed_trees(
        fixture.typed,
        &typed_trees_to_checked_trees::CheckingRequest::settled()
            .with_declaration_authority(Some(&fixture.packages)),
    )
    .expect("the ordinary final pass checks the product");
}

#[test]
fn absent_activation_does_not_admit_an_invalid_product() {
    let fixture =
        PackageFixture::new("machine false_order(i: u64, j: u64) requires i < j ensures j < i {}");
    let mut timings = artifacts::compile_timings::CompileTimings::enabled();
    evaluate_build(fixture.request(false), &mut timings).expect("no Build to execute");
    assert!(timings.phases().is_empty());
    let errors = typed_trees_to_checked_trees::lower_typed_trees(
        fixture.typed,
        &typed_trees_to_checked_trees::CheckingRequest::settled()
            .with_declaration_authority(Some(&fixture.packages)),
    )
    .expect_err("the final checker must still reject the false contract");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("false_order")),
        "{errors:#?}"
    );
}

#[test]
fn actual_activation_keeps_declaration_admission_before_snapshot_capture() {
    let fixture = PackageFixture::new(
        "data Build {} machine build(builder: &mut Build) {}\n\
         machine false_order(i: u64, j: u64) requires i < j ensures j < i {}",
    );
    let snapshot = package_compilation::BuildSnapshotRequest::new([]);
    let mut request = fixture.request(true);
    request.snapshot = Some(&snapshot);
    let mut timings = artifacts::compile_timings::CompileTimings::enabled();
    let errors = match evaluate_build(request, &mut timings) {
        Ok(_) => panic!("invalid declarations must reject before a Build activation"),
        Err(errors) => errors,
    };
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("false_order")),
        "{errors:#?}"
    );
    assert!(
        !errors
            .iter()
            .any(|error| error.message.contains("snapshot")),
        "{errors:#?}"
    );
    assert!(
        !timings.phases().is_empty(),
        "the real activation keeps its preliminary check"
    );
}

#[test]
fn build_spelling_outside_the_companion_does_not_create_an_activation() {
    let fixture = PackageFixture::new("machine build() -> u32 { 7 }");
    let mut timings = artifacts::compile_timings::CompileTimings::enabled();
    let evaluated = evaluate_build(fixture.request(false), &mut timings).expect("ordinary machine");
    assert_eq!(evaluated.configuration.selected_build_machine_symbol, None);
    assert!(timings.phases().is_empty());
}

#[test]
fn selected_activation_executes_and_retains_its_evidence() {
    let fixture = PackageFixture::new(
        "data Build { freestanding: bool; }\n\
         machine allowed(value: bool) -> bool requires value { value }\n\
         machine build(builder: &mut Build) { builder.freestanding = allowed(true); }",
    );
    for packages in [None, Some(&fixture.packages)] {
        let mut request = fixture.request(true);
        request.package_inputs = packages;
        let mut timings = artifacts::compile_timings::CompileTimings::enabled();
        let evaluated = evaluate_build(request, &mut timings).expect("actual Build");
        assert!(evaluated.configuration.config.settings.freestanding);
        assert!(evaluated.configuration.evaluation_usage.is_some());
        assert!(evaluated.configuration.observation_summary.is_some());
        assert!(
            evaluated
                .configuration
                .selected_build_machine_symbol
                .is_some()
        );
        assert_eq!(timings.phases().len(), 1, "one semantic admission");
    }
}

#[test]
fn standalone_build_checks_helper_contracts_before_execution() {
    let fixture = PackageFixture::new(
        "data Build { freestanding: bool; }
         machine forbidden() -> bool requires false { true }
         machine build(builder: &mut Build) {
             builder.freestanding = true;
             builder.freestanding = forbidden();
         }",
    );
    let snapshot = package_compilation::BuildSnapshotRequest::new([]);
    for capture in [None, Some(&snapshot)] {
        let mut request = fixture.request(true);
        request.package_inputs = None;
        request.snapshot = capture;
        let errors = match evaluate_build(request, &mut Default::default()) {
            Ok(_) => panic!("standalone Build must not execute a call with a false premise"),
            Err(errors) => errors,
        };
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("forbidden")),
            "helper contract must reject before snapshot preparation: {errors:?}"
        );
    }
}

#[test]
fn ambiguous_build_entries_reject_before_snapshot_capture() {
    let fixture = PackageFixture::new("machine build() {} machine build(value: u32) {}");
    let snapshot = package_compilation::BuildSnapshotRequest::new([]);
    let mut request = fixture.request(true);
    request.snapshot = Some(&snapshot);
    let mut timings = artifacts::compile_timings::CompileTimings::enabled();
    let errors = match evaluate_build(request, &mut timings) {
        Ok(_) => panic!("two entries must reject before activation"),
        Err(errors) => errors,
    };
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("two build machines")),
        "{errors:#?}"
    );
    assert!(
        timings.phases().is_empty(),
        "entry discovery precedes checking"
    );
}

#[test]
fn required_outputs_and_artifact_only_still_require_an_activation() {
    let fixture = PackageFixture::new("machine answer() -> u32 { 7 }");
    let outputs = package_compilation::BuildSnapshotRequest::new([b"generated.omg".to_vec()]);
    for artifact_only in [true, false] {
        let mut request = fixture.request(false);
        request.artifact_only = artifact_only;
        request.snapshot = (!artifact_only).then_some(&outputs);
        let errors = match evaluate_build(request, &mut Default::default()) {
            Ok(_) => panic!("default settings cannot satisfy required Build outputs"),
            Err(errors) => errors,
        };
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("requires a build machine")),
            "{errors:#?}"
        );
    }
}
