//! Terminal execution oracle for a realized Product report: reload the portable
//! Terminal artifact, run it, and execute the published native executable.

pub fn execute_report(report: compiler::CompileReport, output: &std::path::Path) {
    let artifact = report
        .retained_native_artifact()
        .expect("native product")
        .psi_artifact();
    let reloaded = terminal_codec::CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes())
        .expect("named Product publishes portable Terminal");
    let observation = terminal_runtime::execute(terminal_runtime::TerminalRuntimeRequest {
        entry: None,
        native_provider: None,
        x86_flags: None,
        memory_ordering: None,
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
