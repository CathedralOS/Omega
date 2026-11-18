//! Shared and mutable visitors use the active case and the declared field order.
#[path = "support/fixture_package_inputs.rs"]
mod fixture_package_inputs;
#[path = "support/joined_product_execution.rs"]
mod joined_product_execution;

#[test]
fn active_case_visitors_report_and_update_fields_in_order() {
    run_visitation("active_case_visitation");
}

#[test]
fn active_case_visitors_omit_only_provably_empty_cases() {
    run_visitation("active_case_uninhabited_payload");
    rejects_visitation("active_case_uninhabited_payload");
}

fn run_visitation(name: &str) {
    let Some(target) = target::TargetProfile::host_if_supported() else {
        return;
    };
    let fixture = fixture_package_inputs::repo_root()
        .join("tests/fixtures/run/reflection")
        .join(name)
        .join("main.omg");
    let discovered = fixture_package_inputs::discover_fixture(&fixture, Default::default())
        .unwrap_or_else(|errors| panic!("{errors:#?}"));
    let product = compiler::CompilationProduct::Native(products::NativeProductRequest {
        installation_assumptions: Vec::new(),
        retain_native_inputs: false,
        retain_native_stage_inputs: false,
        image: Default::default(),
        private_entry_ram: Vec::new(),
        terminal_authority_policy:
            provider_admission::provider_admission::current_terminal_authority_policy(),
        terminal_authority_permission_policy: None,
        optimization_rollback: Default::default(),
    });
    let mut prepared = fixture_package_inputs::review_fixture(discovered, &[target], product)
        .unwrap_or_else(|errors| panic!("{errors:#?}"));
    let output = fixture_package_inputs::produce_fixture(&mut prepared, Some(target.target_name()))
        .unwrap_or_else(|errors| panic!("{errors:#?}"));
    let report = orchestration::complete_product(output)
        .and_then(compiler::CompilationOutput::into_report)
        .unwrap_or_else(|error| panic!("{error:?}"));
    let output =
        platform_custody::temporary_directory::TemporaryDirectory::create(name, false).unwrap();
    joined_product_execution::execute_report(report, &output.path().join("native"));
}

#[test]
fn active_case_visitors_check_both_callback_contracts() {
    for name in [
        "active_case_callback_precondition",
        "active_case_mutable_callback_type",
    ] {
        rejects_visitation(name);
    }
}

fn rejects_visitation(name: &str) {
    let source = fixture_package_inputs::repo_root()
        .join("tests/fixtures/fail/reflection")
        .join(name)
        .join("main.omg");
    let diagnostics = match fixture_package_inputs::reviewed_repository_fixture_package_inputs(
        &source,
        Some("macos_arm64"),
    ) {
        Err(diagnostics) => diagnostics,
        Ok(mut prepared) => match fixture_package_inputs::check_fixture(
            &mut prepared,
            "macos_arm64",
            Default::default(),
        ) {
            Err(diagnostics) => diagnostics,
            Ok(_) => panic!("{name} accepted an incompatible callback"),
        },
    };
    let expected = std::fs::read_to_string(source.with_file_name("expected.txt")).unwrap();
    assert!(
        diagnostics.iter().any(|diagnostic| {
            expected
                .lines()
                .all(|line| diagnostic.message.contains(line))
        }),
        "{name}: {diagnostics:#?}"
    );
}
