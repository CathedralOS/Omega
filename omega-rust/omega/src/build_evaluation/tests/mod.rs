//! Tests for build admission.

use crate::build_evaluation::{
    AdmittedBuildAuthorityVerdict, AdmittedBuildProgramDisposition, BuildConfig,
    BuildMachineFilesystemScope, admit_build_program,
};
use std::path::PathBuf;

#[test]
fn admitted_no_build_checkpoint_preserves_default_configuration_and_evidence() {
    let typed = symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees::default();
    let scope = BuildMachineFilesystemScope::for_root(
        std::path::Path::new("main.omg"),
        PathBuf::from("build"),
        None,
    );
    let admitted = admit_build_program(&typed, None, &scope, None, false)
        .expect("the empty program has an explicit no-build disposition");

    assert_eq!(
        admitted.disposition(),
        AdmittedBuildProgramDisposition::NoBuildMachine
    );
    assert_eq!(
        admitted.authority_verdict(),
        AdmittedBuildAuthorityVerdict::NoBuildMachine
    );
    assert_eq!(admitted.selected_build_machine_symbol(), None);
    assert_eq!(admitted.selected_build_machine_callable_identity(), None);
    assert_eq!(admitted.initial_build_snapshot(), None);
    assert!(admitted.operational_plan().machines().is_empty());
    assert!(admitted.service_reach_plan().machines().is_empty());

    let executed = admitted.execute().expect("consume no-build checkpoint");
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
