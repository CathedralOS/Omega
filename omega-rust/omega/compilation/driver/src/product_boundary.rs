//! Guard the production boundaries already owned by this driver. Shared
//! representations remain legitimate dependencies; transform crates do not.

#[test]
fn application_does_not_depend_on_pipeline_stages() {
    let manifest = include_str!("../../../application/cli/Cargo.toml");
    assert!(
        !manifest.contains("/pipeline/"),
        "application operations must call compiler drivers, not numbered transforms"
    );
}

#[test]
fn package_operations_do_not_assemble_compiler_products() {
    for source in [
        include_str!("../../../packages/package-manager/src/operations/check_project.rs"),
        include_str!("../../../packages/package-manager/src/operations/compile_project.rs"),
    ] {
        for forbidden in [
            ".admit_trust(",
            ".prepare_product_report(",
            "CheckedProductReport::",
            "produce_native_product(",
            "produce_retained_terminal_artifacts(",
        ] {
            assert!(
                !source.contains(forbidden),
                "package operation drives {forbidden}"
            );
        }
    }
}

#[test]
fn native_product_consumes_terminal_without_producing_it() {
    let source = include_str!("native.rs");
    assert!(source.contains("terminal: compilation_products::RetainedTerminalArtifact"));
    assert!(!source.contains("crate::terminal::"));
    assert!(!source.contains("TerminalProductionRequest"));
}

#[test]
fn selected_execution_does_not_copy_checked_programs() {
    for source in [
        include_str!("../../realization/selected-dispatch/src/selected_dispatch.rs"),
        include_str!("../../realization/selected-dispatch/src/boundary_dispatch.rs"),
    ] {
        assert!(!source.contains("Arc::make_mut"));
    }
}

#[test]
fn retained_runtime_borrows_terminal_storage() {
    let source = include_str!("runtime.rs");
    assert!(source.contains("TerminalProgram::borrow(artifact)"));
    assert!(!source.contains("TerminalProgram::load("));
    assert!(!source.contains("semantic_bytes()"));
}
