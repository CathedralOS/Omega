//! Product checking's target-layout observations reach the ordinary native
//! completion owner, then both engines execute the unchanged kept program.
#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

#[path = "support/fixture_package_inputs.rs"]
mod fixture_package_inputs;

#[test]
fn native_and_psi_execute_with_extended_product_scalar_conditions() {
    let fixture = "tests/fixtures/pass/layouts/runtime_constant_layout_contract_exit/main.omg";
    let root = fixture_package_inputs::repo_root().join(fixture);
    let discovered = fixture_package_inputs::discover_fixture(&root, Default::default())
        .unwrap_or_else(|errors| panic!("{fixture} discovery: {errors:#?}"));
    let product = compiler::CompilationProduct::Native(products::NativeProductRequest {
        retain_native_inputs: false,
        retain_native_stage_inputs: false,
        installation_assumptions: Vec::new(),
        image: Default::default(),
        private_entry_ram: Vec::new(),
        terminal_authority_policy:
            provider_admission::current_terminal_authority_policy(),
        terminal_authority_permission_policy: None,
        optimization_rollback: Default::default(),
    });
    let mut prepared = fixture_package_inputs::review_fixture(
        discovered,
        &[target::TargetProfile::LinuxX64],
        product,
    )
    .unwrap_or_else(|errors| panic!("{fixture} review: {errors:#?}"));
    assert_eq!(prepared.plans.len(), 1);
    let (_, request) = prepared.plans.pop().unwrap();
    let build = orchestration::prepare_package(
        prepared.session.compiler(),
        request.occurrences,
        &mut prepared.completed,
    )
    .unwrap_or_else(|error| panic!("source preparation: {:?}", error.failure));
    let mut outcomes = prepared
        .session
        .compiler()
        .complete_package_products(vec![(
            (),
            compiler::PreparedPackageProduct {
                build,
                product: request.product,
                permissions: request.terminal_permissions,
            },
        )])
        .unwrap();
    assert_eq!(outcomes.len(), 1);
    let ((), output) = outcomes.pop().unwrap();
    let output = output.unwrap_or_else(|error| panic!("{fixture} Product checking: {error:?}"));
    let compiler::CompilationOutput::Native(pending) = &output else {
        panic!("source production must hand off pending native work");
    };
    assert!(
        pending
            .scalar_completion
            .admitted()
            .values()
            .observed()
            .next()
            .is_some(),
        "the witness must evaluate actual target-layout scalar conditions during Product checking"
    );
    let mut accounting = source_compiler::compile_time::SourceCompletionAccounting::default();
    let completed = native_scalar_inputs::prepare(
        pending.terminal.shared_artifact(),
        Some("linux_x86_64"),
        &[],
        pending.scalar_completion.proposals(),
        &mut accounting,
    )
    .unwrap();
    let proposal = pending
        .terminal
        .native_realization_proposal()
        .expect("native proposal");
    let demand = provider_realization::native_execution::entry_scalar_demand(
        proposal,
        pending.terminal.authorization(),
    )
    .expect("checked entry demand");
    let mut changed = proposal.target_provider_proposals().to_vec();
    changed[0].selection = Err(vec!["changed provider proposal".into()]);
    let error = completed
        .extend(
            pending.terminal.artifact(),
            proposal.target_profile().target_name(),
            &changed,
            demand.as_deref(),
            &mut accounting,
        )
        .expect_err("native demand cannot reuse another provider selection");
    assert!(error.contains("retained provider proposal"), "{error}");
    let report = orchestration::complete_product(output)
        .unwrap_or_else(|error| panic!("{fixture} native completion: {error:?}"))
        .into_report()
        .expect("a completed native report");
    let artifact = report.retained_native_artifact().expect("a native product");
    let observed = project_build::published_terminal::execute_published_terminal(
        terminal_runtime::TerminalRuntimeRequest {
            entry: None,
            filesystem: Default::default(),
            native_services: Default::default(),
            native_provider: None,
            x86_flags: None,
            memory_ordering: None,
            provider_proposal: None,
            artifact: artifact.psi_artifact(),
            target: Some("linux_x86_64"),
            provisions: report.runtime_provisions(),
            work_budget: 100_000,
            domain_exit: terminal_runtime::TerminalDomainExitDelegation::IsolatedContainingDomain,
            standard_input: None,
            clock: None,
        },
        Some(artifact),
    )
    .expect("Psi executes the completed application");
    // The authored entry returns unit, which the CLI's native/Psi comparison
    // maps to exit 0. It does not invoke an external exit operation.
    assert!(
        matches!(
            observed.status,
            terminal_interpreter::TerminalExecutionStatus::Complete(
                terminal_interpreter::TerminalExecutionResult::Unit
            )
        ),
        "Psi outcome: {:?}",
        observed.status
    );
    let directory = platform_custody::temporary_directory::TemporaryDirectory::create(
        "scalar-completion-handoff",
        false,
    )
    .unwrap();
    let published =
        products::publication::publish_compilation(report, &directory.path().join("native"))
            .unwrap();
    let native = std::process::Command::new(published.checked_native_executable_path().unwrap())
        .output()
        .expect("execute the emitted program");
    assert_eq!(native.status.code(), Some(0), "native outcome: {native:?}");
}
