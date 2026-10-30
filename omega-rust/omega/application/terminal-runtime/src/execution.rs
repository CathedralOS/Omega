//! Execute the produced Terminal program under an explicit isolated host root.
//!
//! The native producer retains its admitted entry and provider provisions.
//! Reference execution consumes those exact records without rebuilding source
//! or rerunning Build. They identify what to establish; this owner creates the
//! actual zero backing and explicitly supplied host bindings. Containing-domain exit
//! requires a separate explicit host delegation; its default is absent even
//! when entry or provider provisions are present. Guest storage never escapes,
//! no placed host loans are admitted, and host output is copied into an
//! observation. A separate filesystem delegation can perform persistent effects
//! in the host namespace; private domain death does not undo those effects.
//! These are explicit receiving choices, not
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
//! Exact Darwin imports and Linux clock syscalls consume an explicit clock
//! environment shared across slots. Snapshots retain private simulated waits;
//! live host observations remain coherent with real filesystem timestamps.

mod components;
mod entry;
mod host_imports;

pub use host_imports::clock::{TerminalClockEnvironment, TerminalClockSnapshot};

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

/// Explicit access to this host's filesystem namespace. Naming filesystem
/// effects or selecting a native provider does not supply this authority.
#[derive(Clone, Copy, Default)]
pub enum TerminalFilesystemDelegation {
    #[default]
    Unprovided,
    HostNamespace,
}

use native_artifact::TerminalRuntimeProvisions;
use terminal_interpreter::{
    TerminalArtifactInterpretError, TerminalBindingRejection, TerminalExecutionStatus,
    TerminalInstanceError, TerminalInterpretError, TerminalProcessExitAdmission, TerminalProgram,
    TerminalRuntimeSupplyRequest,
};

pub struct TerminalRuntimeRequest<'program> {
    /// An explicit retained entry from this artifact's Build companion. This
    /// selects code only; entry storage and host authority remain independent.
    /// Without it, native provisions select their entry or the artifact default
    /// is used. An explicit entry must agree with any supplied native provisions.
    pub entry: Option<&'program terminal_psi::CheckedProgramEntryTerminalReceipt>,
    /// Explicit private native sponsor. Its executable must dispatch the
    /// runtime worker hook before normal arguments. None admits no native C
    /// execution. This worker supplies no standard stream or external service
    /// authority on its own; filesystem access needs the separate delegation.
    pub native_provider: Option<std::process::Command>,
    pub filesystem: TerminalFilesystemDelegation,
    pub x86_flags: Option<terminal_interpreter::TerminalX86FlagsEnvironment>,
    pub memory_ordering: Option<terminal_interpreter::TerminalMemoryOrderingEnvironment>,
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
    /// Explicit clock supply. Absence supplies no clock capability. A snapshot
    /// remains private modeled time; Host observes the actual matching host.
    pub clock: Option<TerminalClockEnvironment>,
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
    /// Actual logical work, including independently executing children.
    pub fuel_usage: terminal_fuel::TerminalFuelUsage,
    /// Fuel pauses resumed by this sponsor, never replayed invocations.
    pub fuel_suspensions: u64,
    /// Pauses at which a real producing child still owns its invocation.
    pub component_suspensions: u64,
}

#[derive(Debug)]
pub enum TerminalRuntimeError {
    MissingProvision {
        capability: &'static str,
        reason: String,
    },
    InvalidProvision(&'static str),
    InvalidProviderSelection(String),
    Artifact(TerminalArtifactInterpretError),
    Requirements(TerminalInterpretError),
    Binding(TerminalBindingRejection),
    ComponentBinding(String),
    Execution(TerminalInstanceError),
}

impl TerminalRuntimeError {
    /// Unsupported execution can surface at admission, instantiation or runtime.
    /// Keep those unavailable comparisons distinct from malformed artifacts and
    /// missing custody, which are failures rather than unsupported execution.
    pub fn comparison_unavailable(&self) -> bool {
        let interpretation = match self {
            Self::MissingProvision { .. } => return true,
            Self::Requirements(error)
            | Self::Artifact(TerminalArtifactInterpretError::Execution(error))
            | Self::Execution(TerminalInstanceError::Execution(error)) => error,
            _ => return false,
        };
        matches!(
            interpretation,
            TerminalInterpretError::UnsupportedCapability(_)
                | TerminalInterpretError::UnsupportedSemanticVariant(_)
                | TerminalInterpretError::ScalarEntryQualificationUnsupported
                | TerminalInterpretError::AffineProjectionNotRepresentable
        )
    }

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
            Self::InvalidProviderSelection(reason) => formatter.write_str(reason),
            Self::Artifact(error) => std::fmt::Display::fmt(error, formatter),
            Self::Requirements(error) => std::fmt::Display::fmt(error, formatter),
            Self::Binding(error) => write!(formatter, "runtime binding rejected: {error:?}"),
            Self::ComponentBinding(error) => {
                write!(formatter, "component binding rejected: {error}")
            }
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
    execute_sliced(request, std::num::NonZeroU64::MAX)
}

/// Execute the same admitted root with a bounded sponsor scheduling quantum.
/// The driver owns every invocation and its observed outcome. Replenishment
/// keeps the same meter and instance, never exceeds the request's total fuel
/// allowance, and grants no new host binding or result-producing callback.
pub fn execute_sliced(
    request: TerminalRuntimeRequest<'_>,
    quantum: std::num::NonZeroU64,
) -> Result<TerminalRuntimeObservation, TerminalRuntimeError> {
    let artifact = request.artifact.view();
    let program = TerminalProgram::borrow(artifact);
    if let Some(entry) = request.entry {
        if entry.terminal_psi_identity() != program.terminal_psi() {
            return Err(TerminalRuntimeError::InvalidProvision(
                "explicit entry names another Terminal artifact",
            ));
        }
        if request
            .provisions
            .is_some_and(|provisions| provisions.entry() != entry.terminal_entry())
        {
            return Err(TerminalRuntimeError::InvalidProvision(
                "explicit entry and runtime provisions disagree",
            ));
        }
    }
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
    let retained_installation = request.provisions.and_then(|provisions| {
        provisions
            .scalar_applications()
            .map(|completed| completed.primary().providers())
            .or_else(|| provisions.installation())
    });
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
    {
        retained
            .validate_projection_of(proposed)
            .map_err(|reason| {
                TerminalRuntimeError::InvalidProviderSelection(format!(
                    "runtime provisions and explicit provider proposal disagree: {reason}"
                ))
            })?;
    }
    let checked_installation = proposed_installation.as_ref().or(retained_installation);
    let execution_target = request.target.or_else(|| {
        request
            .provisions
            .map(|provisions| provisions.target_slot().owner.target_name())
    });
    let target_context =
        execution_target.map(|target| terminal_interpreter::target_context(artifact, target));
    let target_layouts = target_context
        .as_ref()
        .map(|selected| {
            let facts = layout_target_facts(selected.target())?;
            terminal_interpreter::admit_target_layouts(artifact, selected, &facts)
        })
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
            if matches!(&provision.realization,
                target_operations::BoundarySettlementRealization::NormalizedForeignCall(binding)
                    if matches!(binding.mechanism, target_operations::NormalizedForeignCallMechanism::Component { .. }))
            {
                continue;
            }
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
    let mut installation = program
        .admit_runtime_supplies(TerminalRuntimeSupplyRequest {
            x86_flags: request.x86_flags,
            memory_ordering: request.memory_ordering,
            entry: request
                .entry
                .map(|entry| entry.terminal_entry())
                .or_else(|| request.provisions.map(TerminalRuntimeProvisions::entry))
                .map(terminal_psi::TerminalCallable::Machine),
            installation: checked_installation,
            host_imports: &host_imports,
            target_context: target_context.as_ref(),
            target_layouts: target_layouts.as_ref(),
        })
        .map_err(TerminalRuntimeError::Requirements)?;
    if let Some(provisions) = request.provisions {
        for component in provisions.component_products() {
            installation = installation
                .with_component_provider(component.selection())
                .map_err(TerminalRuntimeError::Requirements)?;
        }
    }
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
        .instantiate(terminal_interpreter::TerminalInstantiation {
            scalar_arguments: &[],
            structural_inputs: inputs,
            installation: Some(&installation),
            scalar_admission: request
                .provisions
                .and_then(TerminalRuntimeProvisions::scalar_applications),
            ..Default::default()
        })
        .map_err(TerminalRuntimeError::Artifact)?;
    let host = host_imports::HostContext::new(
        request.standard_input,
        request.clock,
        request.native_provider,
        request.filesystem,
    );
    let component_slots = request
        .provisions
        .map(|provisions| {
            provisions
                .component_products()
                .iter()
                .map(|component| component.selection().receiving_boundary())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let imports = requirements
        .imports
        .iter()
        .copied()
        .filter(|boundary| !component_slots.contains(boundary))
        .collect::<Vec<_>>();
    let output = host_imports::bind(
        &program,
        &mut instance,
        request
            .provisions
            .map(TerminalRuntimeProvisions::host_provisions),
        &imports,
        &host,
    )?;
    if let Some(provisions) = request.provisions {
        for component in provisions.component_products() {
            let binding =
                components::prepare(component, &host, request.x86_flags, request.memory_ordering)?;
            instance
                .bind_component(binding)
                .map_err(TerminalRuntimeError::ComponentBinding)?;
        }
    }
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
    let first_quantum = request.fuel_allowance.min(quantum.get());
    let mut remaining = request.fuel_allowance - first_quantum;
    let mut fuel = terminal_fuel::TerminalFuelMeter::with_allowance(first_quantum);
    let mut fuel_suspensions = 0u64;
    let mut component_suspensions = 0u64;
    let status = loop {
        let status = instance
            .invoke(&mut fuel)
            .map_err(TerminalRuntimeError::Execution)?;
        if !matches!(status, TerminalExecutionStatus::SponsorExhausted(_)) || remaining == 0 {
            break status;
        }
        fuel_suspensions += 1;
        component_suspensions += u64::from(instance.active_component_invocation_count() != 0);
        let additional = remaining.min(quantum.get());
        remaining -= additional;
        fuel.replenish(additional).map_err(|_| {
            TerminalRuntimeError::InvalidProvision("sponsor fuel replenishment overflows")
        })?;
    };
    let stdout = output.stdout.snapshot()?;
    let stderr = output.stderr.snapshot()?;
    let private_entry_ram = instance
        .observe_private_entry_ram()
        .map_err(TerminalRuntimeError::Execution)?;
    let effects = instance.effects().to_vec();
    host.finish_native_provider()?;
    Ok(TerminalRuntimeObservation {
        fuel_usage: fuel.into_usage(),
        fuel_suspensions,
        component_suspensions,
        effects,
        private_entry_ram,
        status,
        stdout,
        stderr,
    })
}

/// Observe this host's flags image for an explicitly selected x86 environment.
/// Cross-architecture sponsors must supply their own modeled processor state.
pub fn capture_host_x86_flags(
    target: &str,
) -> Result<Option<terminal_interpreter::TerminalX86FlagsEnvironment>, TerminalRuntimeError> {
    if !terminal_psi::RegisterTargetApplicability::X86_64.accepts_target_name(target) {
        return Ok(None);
    }
    let Some(image) = runtime_abi::capture_host_x86_flags() else {
        return Ok(None);
    };
    terminal_interpreter::TerminalX86FlagsEnvironment::new(image)
        .map(Some)
        .map_err(TerminalRuntimeError::Requirements)
}

/// Rejoin portable layout inputs to the receiving compiler target catalogue.
/// A retained artifact cannot choose the pointer geometry used to validate it.
fn layout_target_facts(
    name: &str,
) -> Result<
    terminal_installation::LayoutTargetFacts,
    terminal_installation::TargetLayoutSelectionError,
> {
    target::TargetProfile::from_canonical_target_name(name)
        .map_err(|reason| {
            terminal_installation::TargetLayoutSelectionError::InvalidTargetInput(
                reason.to_string(),
            )
        })?
        .layout_target_facts()
        .ok_or_else(|| {
            terminal_installation::TargetLayoutSelectionError::InvalidTargetInput(
                "selected target has no representation context".into(),
            )
        })
}
