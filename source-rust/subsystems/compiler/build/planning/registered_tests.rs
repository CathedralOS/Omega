//! Execute each Build-registered test as a separately published Product.
#[path = "registered_tests/native.rs"]
mod native;
#[path = "registered_tests/result.rs"]
mod result;
use self::result::{EngineError, NativeTermination, Observed, PsiTermination, Run, assess};
use super::ProductStage;
use compiler::{Artifact, CompileRequest, Program};
use diagnostics::Diagnostic;
use installation_conditions::InstallationEnvironmentAssumption;
use product_publication::CompletedProduct;
use provider_admission::IsolatedExecutionGrant;
use source_compiler::registered_tests::{RegisteredTests, TestInvocationInputs};
use target::TargetProfile;
use terminal_runtime::{
    TerminalDomainExitDelegation, TerminalExecutionResult, TerminalExecutionStatus,
    TerminalRuntimeRequest,
};

const TEST_PSI_WORK_ALLOWANCE: u64 = 10_000_000;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TestExecution {
    /// Run every available engine and require at least one passing execution.
    Available,
    /// Require both engines to pass and produce matching observations.
    Compare,
}

impl ProductStage<'_> {
    pub fn execute_registered_tests(
        &self,
        profile: TargetProfile,
        assumptions: &[InstallationEnvironmentAssumption],
        execution: TestExecution,
    ) -> Result<Vec<String>, Vec<Diagnostic>> {
        let target = profile.target_name();
        let disabled = self
            .product
            .evaluated()
            .configuration
            .config
            .settings
            .tests_disabled;
        let Some(suite) = RegisteredTests::discover(self.source(), target, disabled)? else {
            return Ok(if execution == TestExecution::Compare {
                vec![format!("0 registered test comparisons (target `{target}`)")]
            } else {
                Vec::new()
            });
        };
        let mut roster = Vec::new();
        for test in &suite.roots {
            let prepare = || {
                let mut native_target = self.native_target(profile, assumptions.to_vec())?;
                let private_filesystem = test
                    .inputs
                    .private_filesystem
                    .then(|| IsolatedExecutionGrant::private_linux_filesystem(profile))
                    .flatten();
                native_target.isolated_execution_grant = Some(
                    private_filesystem.unwrap_or_else(IsolatedExecutionGrant::captured_process),
                );
                let program = Program::CheckedSource {
                    publication: suite.compile(test, target)?,
                    suppliers: self.product,
                    runtime_consent: self.runtime_consent(profile)?,
                    target: native_target,
                };
                let request = if TargetProfile::host_if_supported() == Some(profile) {
                    CompileRequest::Native {
                        program,
                        entry_memory: &[],
                    }
                } else {
                    // A foreign target can run in Psi without emitting a native test image.
                    CompileRequest::Runtime {
                        program,
                        entry_memory: &[],
                    }
                };
                compiler::compile(request, Default::default())
                    .map(|output| output.artifact)
                    .map_err(compiler::stage_validators::NativeValidationError::into_diagnostics)
            };
            let name = format!("test `{}` in group `{}`", test.name, test.group);
            let result = prepare()
                .map_err(|errors| {
                    format!(
                        "cannot be produced: {}",
                        diagnostics::join_messages(&errors)
                    )
                })
                .and_then(|product| run_test(product, profile, execution, &test.inputs));
            roster.push(match result {
                Ok(status) => Diagnostic::note(format!("{name} (target `{target}`): {status}")),
                Err(reason) => Diagnostic::error(format!("{name} {reason} (target `{target}`)")),
            });
        }
        if execution == TestExecution::Compare && suite.roots.is_empty() {
            roster.push(Diagnostic::note(format!(
                "0 registered test comparisons (target `{target}`)"
            )));
        }
        roster.extend(suite.notes.into_iter().map(Diagnostic::note));
        if roster.iter().any(Diagnostic::is_error) {
            if execution == TestExecution::Available {
                roster.retain(Diagnostic::is_error);
            }
            Err(roster)
        } else {
            Ok(roster.into_iter().map(|row| row.message).collect())
        }
    }
}

fn run_test(
    product: Artifact,
    profile: TargetProfile,
    execution: TestExecution,
    inputs: &TestInvocationInputs,
) -> Result<String, String> {
    if inputs.private_filesystem
        && !matches!(profile, TargetProfile::LinuxX64 | TargetProfile::LinuxArm64)
    {
        let unavailable = EngineError::Unavailable(format!(
            "no private filesystem adapter for {}",
            profile.target_name()
        ));
        return assess(Err(unavailable.clone()), Err(unavailable), execution);
    }
    let (interpreted, native) = match product {
        Artifact::Runtime(report) => (
            interpret(&report, profile, inputs),
            Err(EngineError::Unavailable(
                "its target does not execute on this host".into(),
            )),
        ),
        Artifact::Native(report) => {
            let interpreted = interpret(&report, profile, inputs);
            let program = report
                .retained_native_program()
                .ok_or("the native test lost its Psi artifact")?;
            let can_exit = program
                .psi_artifact()
                .semantic_module()
                .can_end_the_process();
            let native = execute_native(report, can_exit, inputs);
            (interpreted, native)
        }
        Artifact::Psi(_) => return Err("the test lacks an admitted execution product".into()),
    };
    assess(interpreted, native, execution)
}

fn interpret(
    report: &CompletedProduct,
    target: TargetProfile,
    inputs: &TestInvocationInputs,
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
                terminal_runtime::TerminalFilesystemDelegation::Private
            } else {
                terminal_runtime::TerminalFilesystemDelegation::Unprovided
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
            standard_input: inputs
                .private_filesystem
                .then(|| Box::new(std::io::empty()) as Box<dyn std::io::Read + Send>),
            clock: None,
        },
        report.retained_native_artifact(),
    );
    let observation = observation.map_err(|error| {
        if error.unavailable_before_guest_effects() {
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
        termination,
        stdout: observation.stdout,
        stderr: observation.stderr,
    })
}

fn execute_native(
    report: CompletedProduct,
    can_exit: bool,
    inputs: &TestInvocationInputs,
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
    let output = native::capture(
        command,
        crate::NativeEntryLimits::INSTALLED,
        inputs.private_filesystem.then_some(working.as_path()),
    )?;
    Ok(Observed {
        termination: NativeTermination {
            status: output.status.into(),
            can_exit,
        },
        stdout: output.stdout,
        stderr: output.stderr,
    })
}
