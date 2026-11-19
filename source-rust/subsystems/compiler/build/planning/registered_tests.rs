//! Execute each Build-registered test as a separately published Product.
use super::ProductStage;
use compiler::{Artifact, CompileRequest, Program};
use diagnostics::Diagnostic;
use installation_conditions::InstallationEnvironmentAssumption;
use product_publication::CompletedProduct;
use source_compiler::registered_tests::RegisteredTests;
use target::TargetProfile;
use terminal_runtime::{
    TerminalDomainExitDelegation, TerminalExecutionResult, TerminalExecutionStatus,
    TerminalRuntimeRequest,
};

const TEST_PSI_WORK_ALLOWANCE: u64 = 10_000_000;

enum EngineOutcome {
    Passed,
    Unavailable(String),
    Failed(String),
}

impl ProductStage<'_> {
    pub fn execute_registered_tests(
        &self,
        profile: TargetProfile,
        assumptions: &[InstallationEnvironmentAssumption],
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
            return Ok(Vec::new());
        };
        let mut roster = Vec::new();
        let mut failures = Vec::new();
        for test in &suite.roots {
            let prepare = || {
                let program = Program::CheckedSource {
                    publication: suite.compile(test, target)?,
                    target: self.native_target(profile, assumptions.to_vec())?,
                };
                let Artifact::Native(product) = compiler::compile(CompileRequest::Native {
                    program,
                    entry_memory: &[],
                })?
                else {
                    return Err(vec![Diagnostic::error(
                        "the requested native test product was absent",
                    )]);
                };
                Ok(product)
            };
            let name = format!("test `{}` in group `{}`", test.name, test.group);
            let result = prepare()
                .map_err(|errors| {
                    format!(
                        "cannot be produced: {}",
                        diagnostics::join_messages(&errors)
                    )
                })
                .and_then(|product| run_test(product, profile));
            match result {
                Ok(status) => roster.push(format!("{name} (target `{target}`): {status}")),
                Err(reason) => failures.push(Diagnostic::error(format!(
                    "{name} {reason} (target `{target}`)"
                ))),
            }
        }
        roster.extend(suite.notes);
        if failures.is_empty() {
            Ok(roster)
        } else {
            Err(failures)
        }
    }
}

fn run_test(report: CompletedProduct, profile: TargetProfile) -> Result<String, String> {
    let interpreted = interpret(&report, profile);
    let can_exit = report.retained_native_program().is_none_or(|program| {
        program
            .psi_artifact()
            .semantic_module()
            .can_end_the_process()
    });
    let native = if TargetProfile::host_if_supported() != Some(profile) {
        EngineOutcome::Unavailable("its target does not execute on this host".into())
    } else if can_exit {
        EngineOutcome::Unavailable(
            "the test can end the process, so its exit status cannot show that it returned to the runner".into()
        )
    } else {
        execute_native(report)
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
    let mut status = format!("passed on {}", engines.join(", "));
    if !unavailable.is_empty() {
        status.push_str(&format!("; not run on {}", unavailable.join("; ")));
    }
    Ok(status)
}

fn interpret(report: &CompletedProduct, target: TargetProfile) -> EngineOutcome {
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
    let observation = crate::published_terminal::execute_published_terminal(
        TerminalRuntimeRequest {
            entry: None,
            native_provider: None,
            native_services: terminal_runtime::TerminalNativeServiceDelegation::Unprovided,
            filesystem: terminal_runtime::TerminalFilesystemDelegation::Unprovided,
            x86_flags,
            memory_ordering:
                terminal_runtime::TerminalMemoryOrderingEnvironment::private_sequential(target),
            artifact,
            target: Some(target),
            provisions: report.runtime_provisions(),
            provider_proposal: None,
            work_budget: TEST_PSI_WORK_ALLOWANCE,
            domain_exit: TerminalDomainExitDelegation::IsolatedContainingDomain,
            standard_input: None,
            clock: None,
        },
        report.retained_native_artifact(),
    );
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

fn execute_native(report: CompletedProduct) -> EngineOutcome {
    let directory =
        match platform_custody::temporary_directory::TemporaryDirectory::create("test", false) {
            Ok(directory) => directory,
            Err(error) => {
                return EngineOutcome::Unavailable(format!(
                    "cannot create a test directory: {error}"
                ));
            }
        };
    let published = match report.publish(directory.path(), directory.path()) {
        Ok(product) => product,
        Err(reason) => return EngineOutcome::Failed(format!("cannot be published: {reason}")),
    };
    let Some(executable) = published.checked_native_executable_path() else {
        return EngineOutcome::Failed("test publication has no checked native executable".into());
    };
    if let Err(reason) = published.validate_component_process_inputs(executable) {
        return EngineOutcome::Failed(reason);
    }
    let output = std::process::Command::new(executable)
        .stdin(std::process::Stdio::null())
        .output();
    let output = match output {
        Ok(output) => output,
        Err(error) => {
            return EngineOutcome::Unavailable(format!("cannot be spawned: {error}"));
        }
    };
    // The caller ran only a test that cannot end the process, so exit status
    // 0 means its entry returned normally.
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
