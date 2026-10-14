//! Execute the produced Terminal program under an explicit isolated host root.
//!
//! The native producer retains its admitted entry and provider provisions.
//! Reference execution consumes those exact records without rebuilding source
//! or rerunning Build. They identify what to establish; this owner creates the
//! actual zero backing and pure buffered host bindings. Containing-domain exit
//! requires a separate explicit host delegation; its default is absent even
//! when entry or provider provisions are present. Guest storage never escapes,
//! no placed host loans are admitted, and host output is copied into an
//! observation rather than performed on a
//! surviving process. Those are the domain-death premises of this runtime, not
//! permissions inferred from a provider name or from a reach declaration.
//! Selected numeric imports reuse exact shared IEEE semantics, including directed
//! rounding and integer conversion. Their trapping responses still require the
//! core's published same-cause guard under the actual invocation snapshot.
//! Selected hosted byte imports consume an explicit finite replay stream;
//! absence is not fabricated EOF. Scalar sum completions retain actual case
//! payloads, so checked line readers execute their own byte-by-byte loop.
//! Descriptor adapters join complete admitted locator/ABI rows: MSVCRT reads
//! and writes use captured integer loans, while Darwin writes also accept the
//! exact readonly byte-view descriptor. Counts cannot widen the loan's extent.
//! Linux read/write syscalls use those same captured loans and native placement;
//! raw kernel errors are not guessed from libc or host replay errors.
//! Slots share finite input and independent ordered stdout/stderr captures;
//! checked Console wrappers still run in Psi.
//! Exact Darwin imports and Linux clock syscalls consume a supplied private time
//! model shared across slots. Guest calibration/retry code remains executable;
//! simulated waits neither sleep the host nor establish native latency evidence.

mod entry;
mod host_imports;

pub use host_imports::clock::TerminalClockSnapshot;

/// A trusted embedding choice, not a permission inferred from guest metadata.
/// The default supplies no exit authority. A standalone sponsor may explicitly
/// delegate death of this invocation's private simulated containing domain;
/// the core still seals each grant to the actual instance and exact demand.
/// This never delegates death of the embedding host or a sibling domain.
#[derive(Clone, Copy, Default)]
pub enum TerminalDomainExitDelegation {
    #[default]
    Unprovided,
    IsolatedContainingDomain,
}

use compilation_products::TerminalRuntimeProvisions;
use terminal_interpreter::{
    TerminalArtifactInterpretError, TerminalBindingRejection, TerminalExecutionStatus,
    TerminalInstanceError, TerminalInterpretError, TerminalProcessExitAdmission, TerminalProgram,
    TerminalRuntimeSupplyRequest,
};

pub struct TerminalRuntimeRequest<'program> {
    pub artifact: &'program terminal_codec::CanonicalTerminalArtifact,
    /// Exact deployment tag admitted by this invocation. Absence permits only
    /// execution scopes that do not demand a portable target-family head.
    pub target: Option<&'program str>,
    pub provisions: Option<&'program TerminalRuntimeProvisions>,
    /// Explicit receiving-root choice from the Build companion. It supplies
    /// checked candidate selection only, never entry backing or host authority.
    pub provider_proposal: Option<&'program backend_plan::TerminalTargetProviderProposal>,
    pub fuel_allowance: u64,
    /// Explicit host acceptance of domain death through the driver's private
    /// storage and buffered crossings. Binding establishment, reach, provider
    /// selection and artifact possession supply none of this permission.
    pub domain_exit: TerminalDomainExitDelegation,
    /// Explicit finite replay stream. A zero read establishes exhaustion of
    /// this snapshot, never a temporarily empty live pipe. Absence supplies no
    /// stdin capability; input-demanding imports reject before guest effects.
    pub standard_input: Option<Box<dyn std::io::Read + Send + 'program>>,
    /// Explicit clock model for this isolated invocation. Absence supplies no
    /// clock capability. Modeled time is not evidence of native timing behavior.
    pub clock: Option<TerminalClockSnapshot>,
}

/// The exact semantic outcome and preceding buffered output. An external exit
/// retains its full i32 argument; physical status presentation belongs to the
/// consumer's host contract, not this reference execution.
pub struct TerminalRuntimeObservation {
    pub status: TerminalExecutionStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub private_entry_ram: Vec<(u32, Vec<u8>)>,
    pub effects: Vec<terminal_interpreter::TerminalEffect>,
}

#[derive(Debug)]
pub enum TerminalRuntimeError {
    MissingProvision {
        capability: &'static str,
        reason: String,
    },
    InvalidProvision(&'static str),
    Artifact(TerminalArtifactInterpretError),
    Requirements(TerminalInterpretError),
    Binding(TerminalBindingRejection),
    Execution(TerminalInstanceError),
}

impl TerminalRuntimeError {
    pub(super) fn missing(capability: &'static str, reason: impl Into<String>) -> Self {
        Self::MissingProvision {
            capability,
            reason: reason.into(),
        }
    }
}

impl std::fmt::Display for TerminalRuntimeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingProvision { capability, reason } => {
                write!(formatter, "{capability}: {reason}")
            }
            Self::InvalidProvision(reason) => formatter.write_str(reason),
            Self::Artifact(error) => std::fmt::Display::fmt(error, formatter),
            Self::Requirements(error) => std::fmt::Display::fmt(error, formatter),
            Self::Binding(error) => write!(formatter, "runtime binding rejected: {error:?}"),
            Self::Execution(error) => write!(formatter, "runtime execution failed: {error:?}"),
        }
    }
}

impl std::error::Error for TerminalRuntimeError {}

/// Establish this driver's actual host root before invoking any guest effect.
/// Absence of retained provisions permits only a genuinely input/import-free
/// Terminal program; it never enables synthetic context or host bindings.
pub fn execute(
    request: TerminalRuntimeRequest<'_>,
) -> Result<TerminalRuntimeObservation, TerminalRuntimeError> {
    let artifact = request.artifact.view();
    let program = TerminalProgram::borrow(artifact);
    if let Some(provisions) = request.provisions
        && provisions.identity() != program.terminal_psi()
    {
        return Err(TerminalRuntimeError::InvalidProvision(
            "runtime provisions name another Terminal program or entry",
        ));
    }
    if let (Some(provisions), Some(target)) = (request.provisions, request.target)
        && provisions.target_slot().owner.target_name() != target
    {
        return Err(TerminalRuntimeError::InvalidProvision(
            "runtime provisions name another deployment target",
        ));
    }
    let retained_installation = request
        .provisions
        .and_then(TerminalRuntimeProvisions::installation);
    let proposed_installation = request
        .provider_proposal
        .map(|proposal| {
            if proposal.terminal_psi != artifact.identity()
                || request.target != Some(proposal.target.as_str())
            {
                return Err(TerminalRuntimeError::InvalidProvision(
                    "provider proposal names another Terminal artifact or deployment target",
                ));
            }
            let selection = proposal.selection.as_ref().map_err(|reasons| {
                TerminalRuntimeError::missing("target-provider-selection", reasons.join("; "))
            })?;
            selection
                .admit(artifact, &proposal.target)
                .map_err(|error| TerminalRuntimeError::missing("target-provider-selection", error))
        })
        .transpose()?;
    if let (Some(retained), Some(proposed)) =
        (retained_installation, proposed_installation.as_ref())
        && retained != proposed
    {
        return Err(TerminalRuntimeError::InvalidProvision(
            "runtime provisions and explicit provider proposal disagree",
        ));
    }
    let checked_installation = proposed_installation.as_ref().or(retained_installation);
    let execution_target = request.target.or_else(|| {
        request
            .provisions
            .map(|provisions| provisions.target_slot().owner.target_name())
    });
    let target_machines = execution_target
        .map(|target| terminal_interpreter::admit_target_machine_selections(artifact, target));
    let target_layouts = target_machines
        .as_ref()
        .map(|selected| terminal_interpreter::admit_target_layouts(artifact, selected))
        .transpose()
        .map_err(|error| {
            TerminalRuntimeError::Requirements(
                terminal_interpreter::TerminalInterpretError::TargetLayoutSelection(error),
            )
        })?;
    // These are this native request's admitted external settlements, not every
    // boundary with a retained checked alternative. Completion authority is
    // established separately below, never selected as a normal import.
    let mut host_imports = Vec::new();
    if let Some(provisions) = request.provisions {
        for provision in provisions.host() {
            // A placed RAM event is an internal semantic handler, not a host
            // crossing. Leave its demand unresolved here: the entry-relative
            // origin analysis below must prove captured or checked private RAM
            // before removing it, and execution still validates the live loan.
            if matches!(
                provision.realization,
                target_operations::BoundarySettlementRealization::Builtin(
                    target_operations::BoundaryRealization::PlacedScalarAccess(_)
                )
            ) {
                continue;
            }
            if program
                .external_completion_requirement(provision.boundary)
                .is_none()
            {
                host_imports.push(provision.boundary);
            }
        }
    }
    let installation = program
        .admit_runtime_supplies(TerminalRuntimeSupplyRequest {
            entry: request
                .provisions
                .map(|provisions| terminal_psi::TargetMachineCallable::Machine(provisions.entry())),
            installation: checked_installation,
            host_imports: &host_imports,
            target_machines: target_machines.as_ref(),
            target_layouts: target_layouts.as_ref(),
        })
        .map_err(TerminalRuntimeError::Requirements)?;
    let installed_entry = program
        .entry_installed(&installation)
        .map_err(TerminalRuntimeError::Requirements)?;
    if request
        .provisions
        .is_some_and(|provisions| provisions.entry() != installed_entry.machine)
    {
        return Err(TerminalRuntimeError::InvalidProvision(
            "runtime provisions name another installed executable entry",
        ));
    }
    let requirements = program
        .execution_requirements_with_private_entry_ram(
            Some(&installation),
            request
                .provisions
                .map(TerminalRuntimeProvisions::private_entry_ram)
                .unwrap_or_default(),
        )
        .map_err(TerminalRuntimeError::Requirements)?;
    if !requirements.missing_providers.is_empty() {
        return Err(TerminalRuntimeError::missing(
            "runtime-provider-installation",
            format!(
                "uninstalled exact provider slots {:?}",
                requirements.missing_providers
            ),
        ));
    }
    if !requirements.external_completions.is_empty()
        && matches!(
            request.domain_exit,
            TerminalDomainExitDelegation::Unprovided
        )
    {
        return Err(TerminalRuntimeError::missing(
            "process-exit-domain-delegation",
            "the host has not delegated exit of this isolated containing domain",
        ));
    }
    let arrival = entry::establish(&program, &installation, request.provisions)?;
    let inputs = arrival.inputs();
    let mut instance = program
        .instantiate_installed(&[], inputs, &installation)
        .map_err(TerminalRuntimeError::Artifact)?;
    let output = host_imports::bind(
        &program,
        &mut instance,
        request.provisions,
        &requirements.imports,
        request.standard_input,
        request.clock,
    )?;
    // Service occurrences and containing-domain authority are independent.
    // This host explicitly accepted domain death above. All guest storage is
    // private, outputs are buffered, and placed host loans are refused. The
    // core grants exact instance/slot custody under those crossing premises;
    // a Binding roster, Console reach or selected exit provider cannot grant it.
    for demand in &requirements.external_completions {
        let admission = TerminalProcessExitAdmission::admit_isolated_simulated_domain(
            &instance,
            demand.boundary,
        )
        .map_err(TerminalRuntimeError::Binding)?;
        instance
            .bind_external_completion(demand.boundary, admission)
            .map_err(TerminalRuntimeError::Binding)?;
    }
    let mut fuel = terminal_fuel::TerminalFuelMeter::with_allowance(request.fuel_allowance);
    let status = instance
        .invoke(&mut fuel)
        .map_err(TerminalRuntimeError::Execution)?;
    let stdout = output.stdout.snapshot()?;
    let stderr = output.stderr.snapshot()?;
    let private_entry_ram = instance
        .observe_private_entry_ram()
        .map_err(TerminalRuntimeError::Execution)?;
    let effects = instance.effects().to_vec();
    Ok(TerminalRuntimeObservation {
        effects,
        private_entry_ram,
        status,
        stdout,
        stderr,
    })
}
