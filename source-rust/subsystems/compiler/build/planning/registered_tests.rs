//! Execute each Build-registered test as a separately published Product.
#[path = "registered_tests/native.rs"]
mod native;
#[path = "registered_tests/plan.rs"]
mod plan;
#[path = "registered_tests/result.rs"]
mod result;
use self::result::{EngineError, NativeTermination, Observed, PsiTermination, Run, assess};
use super::ProductStage;
use compiler::{Artifact, CompileRequest, Program};
use diagnostics::Diagnostic;
use installation_conditions::InstallationEnvironmentAssumption;
use product_publication::CompletedProduct;
use source_compiler::registered_tests::{RegisteredTests, TestExpectations, TestInvocationInputs};
use target::TargetProfile;
use terminal_runtime::{
    TerminalDomainExitDelegation, TerminalExecutionResult, TerminalExecutionStatus,
    TerminalRuntimeRequest,
};

/// Bound the total retained file bytes independently for each engine.
const TEST_FILE_CAPTURE_BYTES: usize = 64 << 20;

const TEST_PSI_WORK_ALLOWANCE: u64 = 10_000_000;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TestExecution {
    /// Run every available engine and require at least one passing execution.
    Available,
    /// Require both engines to pass and produce matching observations.
    Compare,
}

/// The invocation plan owns the exact compiled products it announced.
#[derive(Default)]
pub struct RegisteredTestPlan {
    tests: Vec<plan::PlannedTest>,
    notes: Vec<String>,
}

impl ProductStage<'_> {
    pub fn plan_registered_tests(
        &self,
        profile: TargetProfile,
        assumptions: &[InstallationEnvironmentAssumption],
        execution: TestExecution,
    ) -> Result<RegisteredTestPlan, Vec<Diagnostic>> {
        let target = profile.target_name();
        let disabled = self
            .product
            .evaluated()
            .configuration
            .config
            .settings
            .tests_disabled;
        let mut plan = RegisteredTestPlan::default();
        if let Some(mut suite) = RegisteredTests::discover(self.source(), target, disabled)? {
            let assumptions: Vec<_> = assumptions
                .iter()
                .filter(|a| a.profile() == profile)
                .cloned()
                .collect();
            for root in std::mem::take(&mut suite.roots) {
                plan.tests.push(plan::PlannedTest::prepare(
                    self,
                    &suite,
                    root,
                    profile,
                    &assumptions,
                    execution,
                ));
            }
            plan.notes = suite.notes;
        }
        if execution == TestExecution::Compare && plan.tests.is_empty() {
            plan.notes.insert(
                0,
                format!("0 registered test comparisons (target `{target}`)"),
            );
        }
        Ok(plan)
    }
}

impl RegisteredTestPlan {
    /// Describe the same admitted products that execution will consume.
    pub fn rows(&self) -> impl Iterator<Item = String> + '_ {
        self.tests
            .iter()
            .map(plan::PlannedTest::describe)
            .chain(self.notes.iter().map(|note| format!("test plan: {note}")))
    }

    pub fn execute(self) -> Result<Vec<String>, Vec<Diagnostic>> {
        let mut roster: Vec<_> = self
            .tests
            .into_iter()
            .flat_map(plan::PlannedTest::execute)
            .collect();
        roster.extend(self.notes.into_iter().map(Diagnostic::note));
        if roster.iter().any(Diagnostic::is_error) {
            for row in &mut roster {
                row.build_context =
                    Some(diagnostics::BuildDiagnosticContext::RunningRegisteredTests);
            }
            Err(roster)
        } else {
            Ok(roster.into_iter().map(|row| row.message).collect())
        }
    }
}

fn run_test(
    report: CompletedProduct,
    engines: &plan::Engines,
    inputs: &TestInvocationInputs,
    expected: &TestExpectations,
    profile: TargetProfile,
    execution: TestExecution,
) -> Result<String, String> {
    let interpreted = engines.psi.run(|| interpret(&report, profile, inputs, expected));
    let native = engines.native.run(|| {
        let program = report.retained_native_program().ok_or_else(|| {
            EngineError::Failed("the scheduled native test lost its retained Psi program".into())
        })?;
        let can_exit = program
            .psi_artifact()
            .semantic_module()
            .can_end_the_process();
        execute_native(report, can_exit, inputs, expected, profile)
    });
    assess(interpreted, native, execution, expected)
}

/// The private descriptor table starts with pipes only, including EOF when the
/// Build supplies no input bytes. Each invocation starts its own read cursor.
fn standard_input(inputs: &TestInvocationInputs, target: TargetProfile) -> Option<&[u8]> {
    inputs
        .stdin
        .as_deref()
        .or_else(|| matches!(target, TargetProfile::LinuxX64 | TargetProfile::LinuxArm64).then_some(&[]))
}

fn interpret(
    report: &CompletedProduct,
    target: TargetProfile,
    inputs: &TestInvocationInputs,
    expected: &TestExpectations,
) -> Run<PsiTermination> {
    let Some(artifact) = report
        .retained_native_program()
        .map(|program| program.psi_artifact())
        .or_else(|| report.artifact())
    else {
        return Err(EngineError::Failed(
            "the test report lost its Psi artifact".to_owned(),
        ));
    };
    let limits = crate::NativeEntryLimits::INSTALLED;
    let target = target.target_name();
    let x86_flags = match terminal_runtime::capture_host_x86_flags(target) {
        Ok(flags) => flags,
        Err(error) => return Err(EngineError::Unavailable(error.to_string())),
    };
    let observation = retained_image_runtime::execute(
        TerminalRuntimeRequest {
            entry: None,
            native_provider: None,
            native_services: terminal_runtime::TerminalNativeServiceDelegation::Unprovided,
            filesystem: if inputs.private_filesystem {
                terminal_runtime::TerminalFilesystemDelegation::Private {
                    initial_files: inputs.files.clone(),
                    output_paths: expected.files.keys().cloned().collect(),
                    capture_bytes: TEST_FILE_CAPTURE_BYTES,
                }
            } else {
                terminal_runtime::TerminalFilesystemDelegation::StandardStreamsOnly
            },
            x86_flags,
            memory_ordering:
                terminal_runtime::TerminalMemoryOrderingEnvironment::private_sequential(target),
            artifact,
            target: Some(target),
            provisions: report
                .runtime_provisions()
                .map(|runtime| runtime.execution()),
            provider_proposal: None,
            work_budget: TEST_PSI_WORK_ALLOWANCE,
            domain_exit: TerminalDomainExitDelegation::IsolatedContainingDomain,
            output_limits: Some(terminal_runtime::TerminalOutputLimits {
                stdout_bytes: limits.capture.stdout_bytes,
                stderr_bytes: limits.capture.stderr_bytes,
                combined_bytes: usize::try_from(limits.combined_output_bytes).unwrap_or(usize::MAX),
            }),
            standard_input: standard_input(inputs, target).map(|bytes| {
                Box::new(std::io::Cursor::new(bytes)) as Box<dyn std::io::Read + Send>
            }),
            clock: None,
        },
        report.retained_native_artifact(),
    );
    let observation = observation.map_err(|error| {
        if let Some(reason) = error.sponsor_exhaustion() {
            EngineError::Incomplete(reason.to_owned())
        } else if matches!(
            error,
            terminal_runtime::TerminalRuntimeError::FileObservation(_)
        ) {
            EngineError::Incomplete(error.to_string())
        } else if error.unavailable_before_guest_effects() {
            EngineError::Unavailable(error.to_string())
        } else {
            EngineError::Failed(error.to_string())
        }
    })?;
    let termination = match observation.status {
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit) => {
            PsiTermination::Returned
        }
        TerminalExecutionStatus::Crashed(crash) => PsiTermination::Crashed(crash.cause),
        TerminalExecutionStatus::ExternalCompletion(completion) => {
            PsiTermination::Exited(completion.return_code)
        }
        TerminalExecutionStatus::SponsorExhausted(exhaustion) => {
            return Err(EngineError::Incomplete(exhaustion.to_string()));
        }
        status => {
            return Err(EngineError::Failed(format!(
                "did not return normally: {status:?}"
            )));
        }
    };
    Ok(Observed {
        files: observation.files,
        termination,
        stdout: observation.stdout,
        stderr: observation.stderr,
    })
}

fn execute_native(
    report: CompletedProduct,
    can_exit: bool,
    inputs: &TestInvocationInputs,
    expected: &TestExpectations,
    target: TargetProfile,
) -> Run<NativeTermination> {
    let directory =
        platform_custody::temporary_directory::TemporaryDirectory::create("test", false).map_err(
            |error| EngineError::Unavailable(format!("cannot create a test directory: {error}")),
        )?;
    let published = report
        .publish(directory.path(), directory.path())
        .map_err(|reason| EngineError::Failed(format!("cannot be published: {reason}")))?;
    let executable = published.checked_native_executable_path().ok_or_else(|| {
        EngineError::Failed("test publication has no checked native executable".into())
    })?;
    published
        .validate_component_process_inputs(executable)
        .map_err(EngineError::Failed)?;
    let working = directory.path().join("fixture");
    std::fs::create_dir(&working).map_err(|error| {
        EngineError::Unavailable(format!(
            "cannot create the test's private working directory: {error}"
        ))
    })?;
    let mut command = std::process::Command::new(executable);
    command.env_clear().current_dir(&working);
    let native::NativeCapture { output, files } = native::capture(
        command,
        crate::NativeEntryLimits::INSTALLED,
        inputs.private_filesystem.then(|| {
            (
                working.as_path(),
                bounded_process::PrivateFilesystemFiles {
                    initial: inputs.files.clone(),
                    outputs: expected.files.keys().cloned().collect(),
                },
            )
        }),
        standard_input(inputs, target),
    )?;
    Ok(Observed {
        files: expected.files.keys().cloned().zip(files).collect(),
        termination: NativeTermination {
            status: output.status.into(),
            can_exit,
        },
        stdout: output.stdout,
        stderr: output.stderr,
    })
}
