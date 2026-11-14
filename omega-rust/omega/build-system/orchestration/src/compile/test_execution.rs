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

A group without services delegates no filesystem, native service, clock or
standard input. A group that names std's `FilesystemService` slot runs every
invocation over a private filesystem this runner creates empty for it, as an
operating system supplies a real one: the interpreter answers the program's
kernel filesystem calls from its own in-memory namespace, and the native
executable runs in a fresh mount namespace whose root is an empty tmpfs
(`bounded_process::run_in_private_root`). Neither can reach a host path, so the program cannot widen
the backing to the real filesystem; where a host cannot supply the private
filesystem, that engine is reported unavailable rather than run on the host's.
*/

use compilation_products::{
    CompileReport, PendingTestProduct, TestFilesystem, TestRootIdentity, TestRosterEntry,
    TestRosterStatus,
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
) -> Result<
    (
        Vec<TestRosterEntry>,
        compilation_products::SourceCompletionAccounting,
    ),
    compiler::CompilationProductError,
> {
    let mut accounting = compilation_products::SourceCompletionAccounting::default();
    let mut failures = Vec::new();
    let mut roster = Vec::with_capacity(tests.len() + notes.len());
    for test in tests {
        match run_test(test.product, &test.identity, &mut accounting) {
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
        return Err(compiler::CompilationProductError::Production(failures)
            .with_source_completion_accounting(accounting));
    }
    roster.extend(notes);
    Ok((roster, accounting))
}

fn run_test(
    product: Result<
        Box<compilation_products::PendingNativeProduct>,
        compilation_products::CompileFailure,
    >,
    identity: &TestRootIdentity,
    accounting: &mut compilation_products::SourceCompletionAccounting,
) -> Result<TestRosterStatus, String> {
    let product = product.map_err(|failure| {
        if let Some(charged) = failure.source_completion_accounting() {
            accounting.append(charged.clone());
        }
        format!("cannot be produced: {}", join(failure.into_diagnostics()))
    })?;
    let report = match super::native_completion::complete_native(*product) {
        Ok(CompilationOutput::Report(report)) => report,
        Ok(_) => return Err("native completion produced no test report".to_owned()),
        Err(error) => {
            let failure = compilation_products::CompileFailure::from(error);
            if let Some(charged) = failure.source_completion_accounting() {
                accounting.append(charged.clone());
            }
            return Err(format!(
                "cannot be produced: {}",
                join(failure.into_diagnostics())
            ));
        }
    };
    accounting.append(report.source_completion_accounting().clone());
    let interpreted = interpret(&report, identity.target, identity.filesystem);
    let native = if target::TargetProfile::host_if_supported() == Some(identity.target) {
        execute_native(report, identity.filesystem)
    } else {
        EngineOutcome::Unavailable("its target does not execute on this host".to_owned())
    };
    let mut engines = Vec::new();
    let mut unavailable = Vec::new();
    let mut failures = Vec::new();
    for (engine, outcome) in [("Psi interpreter", interpreted), ("native", native)] {
        match outcome {
            EngineOutcome::Passed => engines.push(engine.to_owned()),
            // Every engine's failure is reported, so a disagreement between
            // them is visible rather than hidden behind the first.
            EngineOutcome::Failed(reason) => {
                failures.push(format!("failed on the {engine} engine: {reason}"));
            }
            EngineOutcome::Unavailable(reason) => unavailable.push(format!("{engine}: {reason}")),
        }
    }
    if !failures.is_empty() {
        if !engines.is_empty() {
            failures.push(format!("passed on the {} engine", engines.join(" and ")));
        }
        return Err(failures.join("; "));
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

fn interpret(
    report: &CompileReport,
    target: target::TargetProfile,
    filesystem: TestFilesystem,
) -> EngineOutcome {
    // The interpreter's private namespace answers Linux kernel filesystem
    // calls; other targets' filesystem imports have no private backing yet.
    let filesystem = match filesystem {
        TestFilesystem::Unprovided => terminal_runtime::TerminalFilesystemDelegation::Unprovided,
        TestFilesystem::Private
            if matches!(
                target,
                target::TargetProfile::LinuxX64 | target::TargetProfile::LinuxArm64
            ) =>
        {
            terminal_runtime::TerminalFilesystemDelegation::Private
        }
        TestFilesystem::Private => {
            return EngineOutcome::Unavailable(format!(
                "the interpreter supplies its private test filesystem only for Linux targets, not `{}`",
                target.target_name()
            ));
        }
    };
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
        filesystem,
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

fn execute_native(report: CompileReport, filesystem: TestFilesystem) -> EngineOutcome {
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
    let output = match filesystem {
        TestFilesystem::Unprovided => std::process::Command::new(&executable)
            .stdin(std::process::Stdio::null())
            .output(),
        TestFilesystem::Private => {
            match bounded_process::run_in_private_root(&executable, directory.path()) {
                Ok(output) => output,
                Err(bounded_process::PrivateRootUnavailable(reason)) => {
                    return EngineOutcome::Unavailable(reason);
                }
            }
        }
    };
    let output = match output {
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
