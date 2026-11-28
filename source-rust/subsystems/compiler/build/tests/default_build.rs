//! One captured default Build supplies both deployments. These counters are process-wide.
#[path = "../../../../../tests/support/fixture_repository.rs"]
mod fixture_repository;

use artifacts::compile_timings::counters;
use package_manager::operations::begin_local_project_preparation;
use platform_custody::temporary_directory::TemporaryDirectory;
use project_build::{PlanningRequest, PlanningSources, interpret, prepare};
use target::TargetProfile;

#[test]
fn two_deployments_share_the_named_files_default_build() {
    package_compilation::toolchain_library::configure(
        &fixture_repository::root().join("source/library"),
    )
    .unwrap();
    let project = TemporaryDirectory::create("default-build-deployments", false).unwrap();
    let main = project.path().join("loose.omg");
    std::fs::write(&main, "machine main() {}").unwrap();
    // A neighboring Build has no effect on build-first entry selection
    // (wiki/spec/build/command_line.md). If it were selected, the returned
    // plan would carry its Application declaration. The neighbor still parses:
    // it is a source file of the captured root, and the Product's operator
    // family discovery parses every unit of its owning package.
    std::fs::write(
        project.path().join("build.omg"),
        "machine build(builder: @mutable BuildPlan) {\n    \
         builder.product = BuildProduct::Application { name: \"neighbor\" };\n}\n",
    )
    .unwrap();
    let targets = [TargetProfile::LinuxX64, TargetProfile::WindowsX64];
    let acquired = begin_local_project_preparation(&main, &targets, true)
        .unwrap()
        .acquire_root()
        .unwrap();
    let before = counters::source_build_execution_count();
    let providers = counters::provider_defaults_execution_count();
    let evaluation = prepare(
        PlanningRequest {
            root: &main,
            sources: PlanningSources::Captured(acquired.source_capture()),
            execution: Default::default(),
            artifact_only: false,
        },
        Default::default(),
        Default::default(),
    )
    .unwrap();
    let stage = interpret(evaluation, &mut ()).unwrap();
    assert!(stage.returned().declaration().is_none());
    let product = stage.open_product(None).unwrap();
    let planned = product.product_stage(&targets).unwrap();
    for target in targets {
        let source = planned.source();
        let published = source_compiler::compile_for_target(source, target.target_name(), None).unwrap();
        assert!(!published.into_parts().0.to_bytes().is_empty());
    }
    assert_eq!(counters::source_build_execution_count() - before, 1);
    assert_eq!(counters::provider_defaults_execution_count(), providers);
}
