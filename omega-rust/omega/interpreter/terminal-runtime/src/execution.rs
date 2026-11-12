/*
We execute a produced Terminal program under an explicit isolated host root
(`execute`, `execute_sliced`).

The native producer retains its admitted entry and provider provisions
(`TerminalRuntimeProvisions`). Reference execution consumes those exact records
without rebuilding source or re-running Build. They identify what to establish;
we create the actual zero-initialized private entry RAM backing and explicitly
supplied host bindings (`host_imports`).

Containing-domain exit (`TerminalDomainExitDelegation`), host filesystem access
(`TerminalFilesystemDelegation`), and package-owned native service reach
(`TerminalNativeServiceDelegation`) require explicit receiving choices; none is
inferred from a provider name, reach declaration, or artifact possession.
Guest storage never escapes, no placed host loans are admitted, and host
standard output/error is copied into `TerminalRuntimeObservation`.
*/

mod components;
mod entry;
mod host_imports;
pub use host_imports::bind_admitted_intrinsics;
mod native_code_layouts;

pub use host_imports::clock::{TerminalClockEnvironment, TerminalClockSnapshot};

#[derive(Clone, Copy, Default)]
pub enum TerminalDomainExitDelegation {
    #[default]
    Unprovided,
    IsolatedContainingDomain,
}

#[derive(Clone, Default)]
pub enum TerminalFilesystemDelegation {
    #[default]
    Unprovided,
    /// The host namespace as this process sees it, working directory included.
    HostNamespace,
    /// The host namespace with relative paths resolved beneath this directory
    /// rather than the process's working directory, so a harness can give each
    /// execution its own scratch directory. Kernel calls resolve against an
    /// open descriptor for it, and the native sponsor starts in it.
    HostDirectory(std::path::PathBuf),
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub enum TerminalNativeServiceDelegation {
    #[default]
    Unprovided,
    MatchingHost,
}

use native_artifact::TerminalRuntimeProvisions;
use terminal_interpreter::{
    TerminalArtifactInterpretError, TerminalBindingRejection, TerminalExecutionStatus,
    TerminalInstanceError, TerminalInterpretError, TerminalProcessExitAdmission, TerminalProgram,
    TerminalRuntimeSupplyRequest,
};

pub struct TerminalRuntimeRequest<'program> {
    pub native_image: Option<&'program native_artifact::NativeArtifact>, // Optional completed image for code-layout metadata inspection.
    pub entry: Option<&'program terminal_psi::CheckedProgramEntryTerminalReceipt>, // Optional retained entry from the Build companion.
    pub native_provider: Option<std::process::Command>, // Optional isolated native sponsor worker command.
    pub native_services: TerminalNativeServiceDelegation,
    pub filesystem: TerminalFilesystemDelegation,
    pub x86_flags: Option<terminal_interpreter::TerminalX86FlagsEnvironment>,
    pub memory_ordering: Option<terminal_interpreter::TerminalMemoryOrderingEnvironment>,
    pub artifact: &'program terminal_codec::CanonicalTerminalArtifact,
    pub target: Option<&'program str>, // Exact deployment tag admitted by this invocation.
    pub provisions: Option<&'program TerminalRuntimeProvisions>,
    pub provider_proposal: Option<&'program backend_plan::TerminalTargetProviderProposal>, // Checked candidate selection from the Build companion.
    pub work_budget: u64,
    pub domain_exit: TerminalDomainExitDelegation,
    pub standard_input: Option<Box<dyn std::io::Read + Send + 'program>>, // Explicit finite replay stream; `None` supplies no stdin capability.
    pub clock: Option<TerminalClockEnvironment>,
}

pub struct TerminalRuntimeObservation {
    pub status: TerminalExecutionStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub private_entry_ram: Vec<(u32, Vec<u8>)>,
    pub effects: Vec<terminal_interpreter::TerminalEffect>,
    pub work_usage: psi_work::WorkUsage, // Logical work consumed, including independently executing children.
    pub work_suspensions: u64,           // Work pauses resumed by this sponsor.
    pub component_suspensions: u64, // Pauses where a producing child still owned its invocation.
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
    ComponentBinding(String),
    Execution(TerminalInstanceError),
}

const NATIVE_PROVIDER_RETIREMENT: &str = "native-provider-retirement";

impl TerminalRuntimeError {
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
                | TerminalInterpretError::AffineProjectionNotRepresentable { .. }
        )
    }

    pub fn unavailable_before_guest_effects(&self) -> bool {
        match self {
            Self::Execution(_) => false,
            Self::MissingProvision { capability, .. }
                if *capability == NATIVE_PROVIDER_RETIREMENT =>
            {
                false
            }
            _ => self.comparison_unavailable(),
        }
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

pub fn execute(
    request: TerminalRuntimeRequest<'_>,
) -> Result<TerminalRuntimeObservation, TerminalRuntimeError> {
    execute_sliced(request, std::num::NonZeroU64::MAX)
}

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
        .map(|selected| {
            let facts = layout_target_facts(selected.target())?;
            terminal_interpreter::admit_target_layouts(artifact, selected, &facts)
        })
        .transpose()
        .map_err(|error| {
            TerminalRuntimeError::Requirements(TerminalInterpretError::TargetLayoutSelection(error))
        })?;
    let mut host_imports = Vec::new();
    if let Some(provisions) = request.provisions {
        for provision in provisions.host() {
            if matches!(&provision.realization,
                target_operations::BoundarySettlementRealization::NormalizedForeignCall(binding)
                    if matches!(binding.mechanism, target_operations::NormalizedForeignCallMechanism::Component { .. }))
            {
                continue;
            }
            if host_imports::executes_internally(&program, provision) {
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
    let native_code_layouts =
        native_code_layouts::inspect(&request, program.terminal_psi(), execution_target)?;
    let mut installation = program
        .admit_runtime_supplies(TerminalRuntimeSupplyRequest {
            native_code_layouts: native_code_layouts.as_ref(),
            x86_flags: request.x86_flags,
            memory_ordering: request.memory_ordering,
            entry: request
                .entry
                .map(|entry| entry.terminal_entry())
                .or_else(|| request.provisions.map(TerminalRuntimeProvisions::entry))
                .map(terminal_psi::TargetMachineCallable::Machine),
            installation: checked_installation,
            host_imports: &host_imports,
            target_machines: target_machines.as_ref(),
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
        request.native_services,
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
    let first_quantum = request.work_budget.min(quantum.get());
    let mut remaining = request.work_budget - first_quantum;
    let mut work = psi_work::WorkMeter::with_budget(first_quantum);
    let mut work_suspensions = 0u64;
    let mut component_suspensions = 0u64;
    let status = loop {
        let status = instance
            .invoke(&mut work)
            .map_err(TerminalRuntimeError::Execution)?;
        if !matches!(status, TerminalExecutionStatus::SponsorExhausted(_)) || remaining == 0 {
            break status;
        }
        work_suspensions += 1;
        component_suspensions += u64::from(instance.active_component_invocation_count() != 0);
        let additional = remaining.min(quantum.get());
        remaining -= additional;
        work.replenish(additional).map_err(|_| {
            TerminalRuntimeError::InvalidProvision("sponsor work replenishment overflows")
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
        work_usage: work.into_usage(),
        work_suspensions,
        component_suspensions,
        effects,
        private_entry_ram,
        status,
        stdout,
        stderr,
    })
}

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

#[cfg(test)]
mod unavailability_tests {
    use super::{NATIVE_PROVIDER_RETIREMENT, TerminalRuntimeError};

    #[test]
    fn only_refusals_before_the_guest_runs_are_standalone_unavailability() {
        let before = TerminalRuntimeError::missing("runtime-provider-installation", "absent");
        assert!(before.comparison_unavailable());
        assert!(before.unavailable_before_guest_effects());
        let after = TerminalRuntimeError::missing(NATIVE_PROVIDER_RETIREMENT, "worker failed");
        assert!(after.comparison_unavailable());
        assert!(!after.unavailable_before_guest_effects());
        let malformed = TerminalRuntimeError::InvalidProvision("malformed");
        assert!(!malformed.unavailable_before_guest_effects());
    }
}
