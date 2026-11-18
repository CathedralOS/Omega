//! Terminal execution oracle for a realized Product report: reload the portable
//! Terminal artifact, run it, and execute the published native executable.

/// `process_exit` requires that exact external completion; `None` requires
/// the entry to return Unit, which the CLI maps to native exit zero.
pub fn execute_report_with_completion(
    report: compiler::CompileReport,
    output: &std::path::Path,
    process_exit: Option<i32>,
) {
    let artifact = report
        .retained_native_artifact()
        .expect("native product")
        .psi_artifact();
    let reloaded = terminal_codec::CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes())
        .expect("named Product publishes portable Terminal");
    let observation = terminal_runtime::execute(terminal_runtime::TerminalRuntimeRequest {
        native_image: report
            .retained_native_artifact()
            .map(|image| image.inspect_for_runtime()),
        entry: None,
        filesystem: Default::default(),
        native_services: Default::default(),
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
        work_budget: 100_000,
        domain_exit: terminal_runtime::TerminalDomainExitDelegation::IsolatedContainingDomain,
        standard_input: None,
        clock: None,
    })
    .expect("execute joined Terminal Product");
    let completed = match process_exit {
        None => matches!(
            &observation.status,
            terminal_interpreter::TerminalExecutionStatus::Complete(
                terminal_interpreter::TerminalExecutionResult::Unit
            )
        ),
        Some(code) => matches!(
            &observation.status,
            terminal_interpreter::TerminalExecutionStatus::ExternalCompletion(completion)
                if completion.kind == semantic_vocabulary::ExternalCompletionKind::ProcessExit
                    && completion.return_code == code
        ),
    };
    assert!(completed, "{:?}", observation.status);
    let published =
        products::publication::publish_compilation(report, output).expect("publish native Product");
    let observed = std::process::Command::new(
        published
            .checked_native_executable_path()
            .expect("native executable"),
    )
    .output()
    .expect("execute joined native Product");
    assert_eq!(
        observed.status.code(),
        Some(process_exit.unwrap_or(0)),
        "{observed:?}"
    );
}

/// Keep the original exact-Unit oracle for callers that return normally.
#[allow(
    dead_code,
    reason = "external-exit fixtures use the completion-aware oracle"
)]
pub fn execute_report(report: compiler::CompileReport, output: &std::path::Path) {
    execute_report_with_completion(report, output, None);
}
