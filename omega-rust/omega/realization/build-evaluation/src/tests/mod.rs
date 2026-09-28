//! Tests for complete Build evaluation.

use crate::{BuildConfig, BuildEvaluationRequest, evaluate_build};

#[test]
fn no_build_evaluation_preserves_default_configuration_and_evidence() {
    let typed = typed_trees::TypedTrees::default();
    let sources = source::SourceMap::default();
    let evaluated = evaluate_build(
        BuildEvaluationRequest {
            root_path: std::path::Path::new("main.omg"),
            typed: &typed,
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
