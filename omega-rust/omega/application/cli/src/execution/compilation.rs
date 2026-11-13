/*

Build and Terminal Psi execution glue for `omega run`.

We request a single-target `ProjectProduct::Native` build through
`orchestration::build` (either `NativeDestination::Retained` for native /
`--both` runs, or `NativeDestination::RuntimeAdmission` for `--engine terminal`
runs) and replay any supplied `reference_inputs` against the produced
`CompileReport` before publication consumes the retained native payload.
Consuming the exact `CompileReport` rather than re-checking source or
interpreting Checked Psi ensures that Build-generated inputs, package bindings,
and settled runtime provisions are identical between Terminal Psi and native
execution.

In `interpret_product`, a retained native build exposes its Terminal Psi
artifact via `report.retained_native_program()`, whereas a `RuntimeAdmission`
build exposes it directly via `report.artifact()`. We run that artifact with
`TerminalNativeServiceDelegation::MatchingHost` (since `omega run` executes on
the host) and `TerminalDomainExitDelegation::IsolatedContainingDomain` (so a
guest exit or crash inside the interpreter does not terminate the `omega` CLI
process itself).

*/

use compiler::{CompileOptions, CompileReport};
use diagnostics::Diagnostic;
use orchestration::{BuildError, BuildRequest, BuildTargets, NativeDestination, ProjectProduct};
use terminal_runtime::{
    TerminalDomainExitDelegation, TerminalRuntimeError, TerminalRuntimeObservation,
    TerminalRuntimeRequest,
};

pub(super) struct ProbeCompilation {
    pub report: CompileReport,
    pub target_name: &'static str,
    pub interpretations: Vec<Result<TerminalRuntimeObservation, TerminalRuntimeError>>, // One reference run per supplied input, in input order.
}

pub(super) fn compile(
    private_entry_ram: Vec<compilation_products::PrivateEntryRamInitialization>,
    build_snapshot: Option<compiler::BuildSnapshotRequest>,
    options: CompileOptions,
    destination: NativeDestination,
    reference_inputs: Vec<std::fs::File>,
    psi_work_budget: u64,
) -> Result<ProbeCompilation, Vec<Diagnostic>> {
    let targets = match options.target_name.as_deref() {
        Some(name) => BuildTargets::Single(
            target::TargetProfile::from_omega_target_name(Some(name))
                .map_err(|diagnostic| vec![diagnostic])?,
        ),
        None => BuildTargets::Host,
    };
    let outcomes = orchestration::build(BuildRequest {
        root_path: options.root_path,
        build_directory: options.build_dir,
        targets,
        product: ProjectProduct::Native {
            installation_assumptions: Vec::new(),
            destination,
            private_entry_ram,
        },
        timings: false,
        offline: false,
        verify: false,
        require_package_project: false,
        optimization_rollback: Default::default(),
        build_snapshot,
    })
    .map_err(BuildError::into_diagnostics)?;
    let [outcome]: [_; 1] = outcomes.try_into().map_err(|_| {
        vec![Diagnostic::error(
            "execution requires exactly one project Build outcome",
        )]
    })?;
    let target_name = outcome.target.target_name();
    let outcome = outcome.result.map_err(BuildError::into_diagnostics)?;
    let report = outcome
        .into_report()
        .map_err(BuildError::into_diagnostics)?
        .report;
    let interpretations = reference_inputs
        .into_iter()
        .map(|input| {
            interpret_product(&report, Some(Box::new(input)), target_name, psi_work_budget)
        })
        .collect();
    Ok(ProbeCompilation {
        report,
        target_name,
        interpretations,
    })
}

pub(super) fn interpret_product(
    report: &CompileReport,
    input: Option<Box<dyn std::io::Read + Send>>,
    target: &str,
    psi_work_budget: u64,
) -> Result<TerminalRuntimeObservation, TerminalRuntimeError> {
    let artifact = report
        .retained_native_program()
        .map(|program| program.psi_artifact())
        .or_else(|| report.artifact())
        .ok_or(TerminalRuntimeError::InvalidProvision(
            "report lost its produced Terminal artifact before execution",
        ))?;
    terminal_runtime::execute(TerminalRuntimeRequest {
        native_image: report.retained_native_artifact(),
        entry: None,
        filesystem: terminal_runtime::TerminalFilesystemDelegation::HostNamespace,
        native_services: terminal_runtime::TerminalNativeServiceDelegation::MatchingHost,
        native_provider: Some(std::process::Command::new(
            std::env::current_exe().map_err(|error| TerminalRuntimeError::MissingProvision {
                capability: "native-provider-sponsor",
                reason: error.to_string(),
            })?,
        )),
        x86_flags: terminal_runtime::capture_host_x86_flags(target)?,
        memory_ordering: terminal_runtime::TerminalMemoryOrderingEnvironment::private_sequential(
            target,
        ),
        provider_proposal: None,
        artifact,
        target: Some(target),
        provisions: report.runtime_provisions(),
        work_budget: psi_work_budget,
        domain_exit: TerminalDomainExitDelegation::IsolatedContainingDomain,
        standard_input: input,
        clock: Some(terminal_runtime::TerminalClockEnvironment::Host),
    })
}
