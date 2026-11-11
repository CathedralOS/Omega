/*
Run every scheduled test invocation before its application report succeeds
(wiki/spec/build/testing.md#execution-plan).

Each test product arrives as its own pending native product whose only
`ProgramEntry` root is the test machine. For each one we complete the native
image, then run the same product on both engines with fresh instance state:
the Terminal Psi interpreter (target-neutral, so it is available for every
target) and the native executable when the test's target is this host. A test
passes on an engine only by returning normally. A contract-permitted crash, a
process exit instead of a return, and work exhaustion each fail or leave the
run incomplete, and every such outcome refuses the application. Every test
must execute on at least one engine; an engine that is unavailable before any
guest effect is reported, never counted as a pass.

This slice provides only pure execution: no filesystem, native service, clock
or standard input is delegated to the interpreter, and selected test services
(virtual filesystems) are not yet supported.
*/

use compilation_products::{
    CompileReport, PendingTestProduct, TestRootIdentity, TestRosterEntry, TestRosterStatus,
};
use compiler::CompilationOutput;
use diagnostics::Diagnostic;
use terminal_runtime::{
    TerminalDomainExitDelegation, TerminalExecutionResult, TerminalExecutionStatus,
    TerminalRuntimeRequest,
};

// Tests inherit the reference-run sponsor policy rather than run unlimited.
const TEST_PSI_WORK_ALLOWANCE: u64 = 10_000_000;

enum EngineOutcome {
    Passed,
    Unavailable(String),
    Failed(String),
}

// The executed roster, in roster order, followed by the groups that ran
// nothing. Any failed or incomplete invocation refuses the whole run.
pub(crate) fn run_tests(
    tests: Vec<PendingTestProduct>,
    notes: Vec<TestRosterEntry>,
) -> Result<Vec<TestRosterEntry>, Vec<Diagnostic>> {
    let mut failures = Vec::new();
    let mut roster = Vec::with_capacity(tests.len() + notes.len());
    for test in tests {
        match run_test(test.product.map_err(join), &test.identity) {
            Ok(status) => roster.push(TestRosterEntry {
                group: test.identity.group.clone(),
                test: Some(test.identity.test.clone()),
                target: test.identity.target,
                status,
            }),
            Err(reason) => failures.push(Diagnostic::error(format!(
                "{} {reason} (target `{}`)",
                test.identity,
                test.identity.target.target_name()
            ))),
        }
    }
    if !failures.is_empty() {
        return Err(failures);
    }
    roster.extend(notes);
    Ok(roster)
}

fn run_test(
    product: Result<Box<compilation_products::PendingNativeProduct>, String>,
    identity: &TestRootIdentity,
) -> Result<TestRosterStatus, String> {
    let product = product.map_err(|reason| format!("cannot be produced: {reason}"))?;
    let report = match super::native_completion::complete_native(*product) {
        Ok(CompilationOutput::Report(report)) => report,
        Ok(_) => return Err("native completion produced no test report".to_owned()),
        Err(error) => {
            return Err(format!(
                "cannot be produced: {}",
                join(error.into_diagnostics())
            ));
        }
    };
    let interpreted = interpret(&report, identity.target);
    let native = if target::TargetProfile::host_if_supported() == Some(identity.target) {
        execute_native(report)
    } else {
        EngineOutcome::Unavailable("its target does not execute on this host".to_owned())
    };
    let mut engines = Vec::new();
    let mut unavailable = Vec::new();
    for (engine, outcome) in [("Psi interpreter", interpreted), ("native", native)] {
        match outcome {
            EngineOutcome::Passed => engines.push(engine.to_owned()),
            EngineOutcome::Failed(reason) => {
                return Err(format!("failed on the {engine} engine: {reason}"));
            }
            EngineOutcome::Unavailable(reason) => unavailable.push(format!("{engine}: {reason}")),
        }
    }
    if engines.is_empty() {
        return Err(format!(
            "has no runnable engine ({}); the test run is incomplete",
            unavailable.join("; ")
        ));
    }
    Ok(TestRosterStatus::Passed {
        engines,
        unavailable,
    })
}

fn interpret(report: &CompileReport, target: target::TargetProfile) -> EngineOutcome {
    let Some(artifact) = report
        .retained_native_program()
        .map(|program| program.psi_artifact())
    else {
        return EngineOutcome::Failed("the test report lost its Terminal artifact".to_owned());
    };
    let target = target.target_name();
    let x86_flags = match terminal_runtime::capture_host_x86_flags(target) {
        Ok(flags) => flags,
        Err(error) => return EngineOutcome::Unavailable(error.to_string()),
    };
    let observation = terminal_runtime::execute(TerminalRuntimeRequest {
        native_image: report.retained_native_artifact(),
        entry: None,
        native_provider: None,
        native_services: terminal_runtime::TerminalNativeServiceDelegation::Unprovided,
        filesystem: terminal_runtime::TerminalFilesystemDelegation::Unprovided,
        x86_flags,
        memory_ordering: terminal_runtime::TerminalMemoryOrderingEnvironment::private_sequential(
            target,
        ),
        artifact,
        target: Some(target),
        provisions: report.runtime_provisions(),
        provider_proposal: None,
        work_budget: TEST_PSI_WORK_ALLOWANCE,
        domain_exit: TerminalDomainExitDelegation::IsolatedContainingDomain,
        standard_input: None,
        clock: None,
    });
    match observation {
        Err(error) if error.unavailable_before_guest_effects() => {
            EngineOutcome::Unavailable(error.to_string())
        }
        Err(error) => EngineOutcome::Failed(error.to_string()),
        Ok(observation) => match observation.status {
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit) => {
                EngineOutcome::Passed
            }
            TerminalExecutionStatus::Crashed(crash) => {
                EngineOutcome::Failed(format!("crashed with {:?}", crash.cause))
            }
            TerminalExecutionStatus::ExternalCompletion(completion) => {
                EngineOutcome::Failed(format!(
                    "exited the process with status {} instead of returning to the runner",
                    completion.return_code
                ))
            }
            TerminalExecutionStatus::SponsorExhausted(exhaustion) => {
                EngineOutcome::Failed(format!("incomplete: {exhaustion}"))
            }
            status => EngineOutcome::Failed(format!("did not return normally: {status:?}")),
        },
    }
}

fn execute_native(report: CompileReport) -> EngineOutcome {
    let directory =
        match platform_custody::temporary_directory::TemporaryDirectory::create("test", false) {
            Ok(directory) => directory,
            Err(error) => {
                return EngineOutcome::Unavailable(format!(
                    "cannot create a test directory: {error}"
                ));
            }
        };
    let (_, executable) =
        match crate::publication::publish_native_artifact(report, directory.path()) {
            Ok(published) => published,
            Err(reason) => return EngineOutcome::Failed(format!("cannot be published: {reason}")),
        };
    let output = match std::process::Command::new(&executable)
        .stdin(std::process::Stdio::null())
        .output()
    {
        Ok(output) => output,
        Err(error) => {
            return EngineOutcome::Unavailable(format!("cannot be spawned: {error}"));
        }
    };
    if output.status.success() {
        return EngineOutcome::Passed;
    }
    match output.status.code() {
        Some(code) => EngineOutcome::Failed(format!(
            "terminated with status {code} instead of returning normally"
        )),
        None => EngineOutcome::Failed(format!("terminated abnormally: {}", output.status)),
    }
}

fn join(diagnostics: Vec<Diagnostic>) -> String {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("; ")
}
