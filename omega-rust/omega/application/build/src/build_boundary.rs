//! Source guards for the application/compiler ownership boundary.

#[test]
fn application_depends_on_facades_not_language_stages() {
    let manifest = include_str!("../Cargo.toml");
    for forbidden in [
        "syntax-trees",
        "typed-trees",
        "checked-trees",
        "lowered-psi",
        "pipeline/",
        "provider-planning",
    ] {
        assert!(
            !manifest.contains(forbidden),
            "Build reaches behind a facade: {forbidden}"
        );
    }
    let cli = include_str!("../../cli/src/cli/compilation.rs");
    assert!(cli.contains("use omega_build::{"));
    assert!(!cli.contains("prepare_local_project_for_targets("));
    assert!(!cli.contains("publish_compilation("));
    let execution = include_str!("../../cli/src/execution/compilation.rs");
    assert!(execution.contains("omega_build::build("));
    assert!(!execution.contains("prepare_local_project("));
    assert!(!execution.contains("compiler::compile("));
    let inspection = include_str!("../../cli/src/inspection/mod.rs");
    assert!(inspection.contains("omega_build::build("));
    assert!(!inspection.contains("prepare_local_project("));
    assert!(!inspection.contains("CompilationSession::default()"));
}

#[test]
fn build_entrance_shows_the_ordered_project_lifecycle() {
    let mut source = include_str!("build.rs");
    for operation in [
        "build_directory::admit_outputs(",
        "prepare_local_project_for_targets(",
        "products::compile(",
        "publication::publish_products(",
    ] {
        let position = source
            .find(operation)
            .expect("operation follows its inputs");
        source = &source[position + operation.len()..];
    }
    let products = include_str!("products.rs");
    assert!(
        !products.contains(".zip("),
        "product identity comes from its outcome"
    );
}

#[test]
fn ordinary_package_operations_admit_instead_of_execute() {
    let admission =
        include_str!("../../../packages/package-manager/src/operations/accepted_compilation.rs");
    for forbidden in [
        ".compile_packages(",
        ".check_package(",
        "CompilationSession",
    ] {
        assert!(
            !admission.contains(forbidden),
            "admission executes {forbidden}"
        );
    }
    let exports = include_str!("../../../packages/package-manager/src/operations/mod.rs");
    for removed in [
        "mod compile_project",
        "mod check_project",
        "compile_prepared_local_project",
    ] {
        assert!(
            !exports.contains(removed),
            "obsolete package execution route: {removed}"
        );
    }
    let execution = include_str!("project_compilation.rs");
    let admit = execution
        .find("admit_project_compilation(request)")
        .unwrap();
    let compile = execution
        .find("compiler.compile_packages(admitted.requests)")
        .unwrap();
    let reconcile = execution
        .find("admitted.completion.complete(products)")
        .unwrap();
    assert!(admit < compile && compile < reconcile);
}
