//! Shared native/Terminal oracle for source-discovery SDK journeys.

pub fn execute_joined_product(
    compiler: &mut compiler::CompilationSession,
    build: compiler::PackageBuild,
    output: &std::path::Path,
) -> Result<(), compiler::CompilationProductError> {
    let product = compiler::CompilationProduct::Native(compilation_products::NativeProductRequest {
        private_entry_ram: Vec::new(),
        terminal_authority_policy: terminal_psi_to_abstract_operations::provider_admission::current_terminal_authority_policy(),
        terminal_authority_permission_policy: None,
        optimization_rollback: Default::default(),
    });
    let mut outcomes = compiler
        .complete_package_products(vec![(
            (),
            compiler::PreparedPackageProduct {
                build,
                product,
                permissions: None,
            },
        )])
        .expect("joined Product batch");
    let (_, outcome) = outcomes.pop().expect("one Product outcome");
    let report = outcome.and_then(compiler::CompilationOutput::into_report)?;
    execute_report(report, output);
    Ok(())
}

pub fn execute_report(report: compiler::CompileReport, output: &std::path::Path) {
    let artifact = report
        .retained_native_artifact()
        .expect("native product")
        .psi_artifact();
    let reloaded = terminal_codec::CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes())
        .expect("named Product publishes portable Terminal");
    let observation = terminal_runtime::execute(terminal_runtime::TerminalRuntimeRequest {
        x86_flags: None,
        provider_proposal: None,
        artifact: &reloaded,
        target: Some(
            target::TargetProfile::host_if_supported()
                .unwrap()
                .target_name(),
        ),
        provisions: report.runtime_provisions(),
        fuel_allowance: 100_000,
        domain_exit: terminal_runtime::TerminalDomainExitDelegation::IsolatedContainingDomain,
        standard_input: None,
        clock: None,
    })
    .expect("execute joined Terminal Product");
    assert!(
        matches!(
            observation.status,
            terminal_interpreter::TerminalExecutionStatus::Complete(
                terminal_interpreter::TerminalExecutionResult::Unit
            )
        ),
        "{:?}",
        observation.status
    );
    let published = compilation_products::publication::publish_compilation(report, output)
        .expect("publish native Product");
    let observed = std::process::Command::new(
        published
            .checked_native_executable_path()
            .expect("native executable"),
    )
    .output()
    .expect("execute joined native Product");
    assert_eq!(observed.status.code(), Some(0), "{observed:?}");
}
