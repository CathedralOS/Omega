//! An authored Build returns its declaration and target selections to the project host.

#[path = "../../../../../tests/support/fixture_repository.rs"]
mod fixture_repository;

use artifacts::compile_timings::{CompileTimings, counters::BuildExecutionPurpose};
use build_declarations::BuildDeclaration;
use build_settings::{BuildEvaluationRequest, prepare_build_evaluation};
use package_compilation::{BuildExecutionInputs, source_assembly::ImmutableSourceParseCheckpoint};
use project_build::planning::interpret;
use semantic_vocabulary::NativeBindingLocator;
use source_acquisition::local::operations::capture_local_source_input;
use source_compiler::{
    ResolveAndTypeRequest, UnmeasuredProduction, source_inputs::SourceCatalogue,
};
use std::sync::Arc;

#[test]
fn authored_build_returns_its_application_and_target_bindings() {
    let repo = fixture_repository::root();
    package_compilation::toolchain_library::configure(&repo.join("source/library")).unwrap();
    let fixture = repo.join("tests/fixtures/run/providers/native_boundary_supply");
    let capture = capture_local_source_input(&fixture, Default::default()).unwrap();
    let root = fixture.join("main.omg");
    let mut timings = CompileTimings::default();
    let mut catalogue = SourceCatalogue::default();
    let checkpoint = ImmutableSourceParseCheckpoint::prepare_from_sources(
        &root,
        None,
        Some(&capture),
        &mut timings,
        &mut catalogue,
    )
    .unwrap();
    let (_, assembled) = checkpoint
        .assemble(None, &mut timings, &mut catalogue)
        .unwrap();
    let build_source = assembled.build_source.expect("authored Build source");
    let source = ResolveAndTypeRequest {
        syntax: assembled.syntax_trees,
        source_items: assembled.source_items,
        root_sources: assembled.root_sources,
        frontends: Default::default(),
        build_helper_identity: "linux_x86_64",
        build_sources: assembled.build_scope_sources,
        build_stage: Some((build_source.source_id(), 0)),
        sources: Arc::clone(&assembled.sources),
        top_level_bindings: assembled.source_scoped_top_level_bindings,
        selection_authority: None,
    }
    .resolve_and_type(&mut UnmeasuredProduction)
    .unwrap();
    let evaluation = prepare_build_evaluation(
        BuildEvaluationRequest {
            execution_purpose: BuildExecutionPurpose::Source,
            pure_build_compilation: None,
            root_path: &root,
            typed: &source.typed,
            build_source: Some(build_source),
            sources: &assembled.sources,
            package_inputs: None,
            anonymous_source: assembled.anonymous_owner.as_ref(),
            generated_source_custody: &assembled.generated_source_custody,
            execution: BuildExecutionInputs::default().resolve_compilation_defaults(None),
            artifact_only: false,
        },
        &mut timings,
    )
    .unwrap();

    let stage = interpret(evaluation, &mut ()).unwrap();
    let Some(BuildDeclaration::Application(application)) = stage.returned().declaration() else {
        panic!("the captured Build must return an application");
    };
    assert_eq!(application.name.as_str(), "native_boundary_supply");
    let computed = stage.complete(None).unwrap().configuration;
    assert_eq!(computed.root_binding_requests.len(), 4);
    assert_eq!(computed.native_binding_requests.len(), 4);
    for (target, syscall) in [("linux_x86_64", 39), ("linux_arm64", 172)] {
        let binding = computed
            .native_binding_requests
            .iter()
            .find(|row| row.target.canonical_name() == target)
            .expect("Build issued the target's native binding");
        assert_eq!(
            binding.locator,
            NativeBindingLocator::Syscall { number: syscall }
        );
    }
}
