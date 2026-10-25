//! `run` requests a retained native product through the shared Build owner.
//! It owns no package preparation or compilation dispatch. Reference execution
//! consumes the exact produced Terminal artifact and its retained runtime
//! provisions before publication
//! transfers that payload. Rechecking source or interpreting checked trees would
//! bypass the product contract and could repeat Build or lose generated inputs.

use compiler::runtime::{
    TerminalDomainExitDelegation, TerminalRuntimeError, TerminalRuntimeObservation,
    TerminalRuntimeRequest,
};
use compiler::{CompileOptions, CompileReport};
use diagnostics::Diagnostic;
use omega_build::{BuildError, BuildRequest, BuildTargets, NativeDestination, ProjectProduct};

// This is an explicit sponsor budget, not a divergence verdict. Exhaustion
// remains an unavailable comparison and may be raised for a measured workload.
const REFERENCE_EXECUTION_FUEL_ALLOWANCE: u64 = 10_000_000;

pub(super) struct ProbeCompilation {
    pub report: CompileReport,
    /// Target name selected for this compilation.
    pub target_name: &'static str,
    /// One reference execution per supplied input, in input order.
    pub interpretations: Vec<Result<TerminalRuntimeObservation, TerminalRuntimeError>>,
}

/// Compiles once and replays the produced Terminal artifact against each
/// reference input before publication transfers the payload, so many inputs
/// share one compilation.
pub(super) fn compile(
    private_entry_ram: Vec<compilation_products::PrivateEntryRamInitialization>,
    options: CompileOptions,
    reference_inputs: Vec<std::fs::File>,
) -> Result<ProbeCompilation, Vec<Diagnostic>> {
    let targets = match options.target_name.as_deref() {
        Some(name) => BuildTargets::Single(
            target::TargetProfile::from_omega_target_name(Some(name))
                .map_err(|diagnostic| vec![diagnostic])?,
        ),
        None => BuildTargets::Host,
    };
    let outcomes = omega_build::build(BuildRequest {
        root_path: options.root_path,
        build_directory: options.build_dir,
        targets,
        product: ProjectProduct::Native {
            destination: NativeDestination::Retained,
            private_entry_ram,
        },
        timings: false,
        offline: false,
        verify: false,
        require_package_project: false,
        optimization_rollback: Default::default(),
        build_snapshot: None,
    })
    .map_err(BuildError::into_diagnostics)?;
    let [outcome]: [_; 1] = outcomes.try_into().map_err(|_| {
        vec![Diagnostic::error(
            "execution requires exactly one project Build outcome",
        )]
    })?;
    let target_name = outcome.target.target_name();
    let report = outcome.result.map_err(BuildError::into_diagnostics)?.report;
    let interpretations = reference_inputs
        .into_iter()
        .map(|input| interpret_product(&report, Some(input), target_name))
        .collect();
    Ok(ProbeCompilation {
        report,
        target_name,
        interpretations,
    })
}

pub(super) fn interpret_product(
    report: &CompileReport,
    input: Option<std::fs::File>,
    target: &str,
) -> Result<TerminalRuntimeObservation, TerminalRuntimeError> {
    let artifact = report
        .retained_native_artifact()
        .map(|artifact| artifact.psi_artifact())
        .or_else(|| {
            report
                .retained_dynamic_elf_artifact()
                .map(|artifact| artifact.psi_artifact())
        })
        .ok_or(TerminalRuntimeError::InvalidProvision(
            "native report lost its produced Terminal artifact before comparison",
        ))?;
    compiler::runtime::execute(TerminalRuntimeRequest {
        x86_flags: compiler::runtime::capture_host_x86_flags(target)?,
        provider_proposal: None,
        artifact,
        target: Some(target),
        provisions: report.runtime_provisions(),
        fuel_allowance: REFERENCE_EXECUTION_FUEL_ALLOWANCE,
        // `run` sponsors one standalone private domain. Its death cannot end
        // this CLI or invalidate another instance's storage/capabilities.
        domain_exit: TerminalDomainExitDelegation::IsolatedContainingDomain,
        standard_input: input.map(|file| Box::new(file) as Box<dyn std::io::Read + Send>),
        clock: Some(compiler::runtime::TerminalClockSnapshot::capture_host_epoch()?),
    })
}
