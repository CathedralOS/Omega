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
            "produce_terminal_product(",
        ] {
            assert!(
                !source.contains(forbidden),
                "package operation drives {forbidden}"
            );
        }
    }
}
