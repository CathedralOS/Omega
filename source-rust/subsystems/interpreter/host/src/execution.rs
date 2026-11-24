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
pub use entry::establish as establish_host_entry;
pub use host_imports::{HostContext, bind_numeric_import_slot};

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
    /// Only the sponsor's standard streams exist. A Linux descriptor call on any
    /// other number has the same EBADF result as in the bounded native child.
    StandardStreamsOnly,
    /// The host namespace as this process sees it, working directory included.
    HostNamespace,
    /// The host namespace with relative paths resolved beneath this directory
    /// rather than the process's working directory, so a harness can give each
    /// execution its own scratch directory. Kernel calls resolve against an
    /// open descriptor for it, and the native sponsor starts in it.
    HostDirectory(std::path::PathBuf),
    /// A private in-memory namespace created empty for this execution, which
    /// a test runner supplies in place of the host's
    /// (wiki/spec/build/testing.md#group-configuration-and-authority). The
    /// guest's kernel filesystem calls reach only this state; nothing falls
    /// back to the host.
    Private {
        initial_files: std::collections::BTreeMap<Vec<u8>, std::sync::Arc<[u8]>>,
        output_paths: Vec<Vec<u8>>,
        capture_bytes: usize,
    },
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub enum TerminalNativeServiceDelegation {
    #[default]
    Unprovided,
    MatchingHost,
}

use published_runtime_input::RuntimeExecutionProvisions as TerminalRuntimeProvisions;
use terminal_interpreter::{
    TerminalArtifactInterpretError, TerminalBindingRejection, TerminalExecutionStatus,
    TerminalInstanceError, TerminalInterpretError, TerminalProgram, TerminalRuntimeSupplyRequest,
};

/// Host capabilities for execution of a canonical Terminal program.
///
/// ```compile_fail
/// fn inject(request: &mut terminal_runtime::TerminalRuntimeRequest<'_>) {
///     request.native_image = Some(([1u8; 32], vec![(1_u32, 0x1000_u64)]));
/// }
/// ```
pub struct TerminalRuntimeRequest<'program> {
    pub entry: Option<&'program psi_publication_records::CheckedProgramEntryTerminalReceipt>, // Optional retained entry from the Build companion.
    pub native_provider: Option<native_provider_runtime::NativeProviderCommand>, // Optional isolated native sponsor worker command.
    pub native_services: TerminalNativeServiceDelegation,
    pub filesystem: TerminalFilesystemDelegation,
    pub x86_flags: Option<terminal_interpreter::TerminalX86FlagsEnvironment>,
    pub memory_ordering: Option<terminal_interpreter::TerminalMemoryOrderingEnvironment>,
    pub artifact: &'program terminal_codec::CanonicalTerminalArtifact,
    pub target: Option<&'program str>, // Exact deployment tag admitted by this invocation.
    pub provisions: Option<&'program TerminalRuntimeProvisions>,
    pub provider_proposal: Option<&'program target_provider_admission::TerminalTargetProviderProposal>, // Checked candidate selection from the Build companion.
    pub work_budget: u64,
    pub domain_exit: TerminalDomainExitDelegation,
    pub standard_input: Option<Box<dyn std::io::Read + Send + 'program>>, // Explicit finite replay stream; `None` supplies no stdin capability.
    pub clock: Option<TerminalClockEnvironment>,
}

pub type ObservedFiles = std::collections::BTreeMap<Vec<u8>, Option<Vec<u8>>>;

pub struct TerminalRuntimeObservation {
    pub files: ObservedFiles,
    pub status: TerminalExecutionStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub private_entry_ram: Vec<(u32, Vec<u8>)>,
    pub effects: Vec<terminal_interpreter::TerminalEffect>,
    pub work_usage: terminal_work::WorkUsage, // Logical work consumed, including independently executing children.
    pub work_suspensions: u64,                // Work pauses resumed by this sponsor.
    pub component_suspensions: u64, // Pauses where a producing child still owned its invocation.
}

#[derive(Debug)]
pub enum TerminalRuntimeError {
    MissingProvision {
        capability: &'static str,
        reason: String,
    },
    InvalidProvision(&'static str),
    NativeArtifactInspection(String),
    PrivateFilesystemSetup(std::io::Error),
    FileObservation(std::io::Error),
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

    pub fn missing(capability: &'static str, reason: impl Into<String>) -> Self {
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
            Self::PrivateFilesystemSetup(error) => {
                write!(formatter, "cannot prepare private files: {error}")
            }
            Self::FileObservation(error) => {
                write!(formatter, "cannot observe declared files: {error}")
            }
            Self::NativeArtifactInspection(reason) => {
                write!(formatter, "native artifact inspection failed: {reason}")
            }
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
    let host = HostedTerminalProgram::prepare(request)?;
    let installation = host
        .program()
        .admit_runtime_supplies(host.supplies())
        .map_err(TerminalRuntimeError::Requirements)?;
    hosted_invocation::execute_installed(host, installation, quantum)
}

mod hosted_invocation;
/// Use the linked code observations from an embedder's checked installation.
/// All service, entry, memory and provider grants come from this host's request.
pub fn execute_with_image_installation(
    host: HostedTerminalProgram<'_>,
    image: &terminal_interpreter::TerminalRuntimeSupplies,
    quantum: std::num::NonZeroU64,
) -> Result<TerminalRuntimeObservation, TerminalRuntimeError> {
    let layouts = image.native_code_layouts().ok_or(TerminalRuntimeError::InvalidProvision(
        "published image supplied no checked linked code layouts",
    ))?;
    let installation = host
        .program()
        .admit_runtime_supplies(TerminalRuntimeSupplyRequest {
            native_code_layouts: Some(layouts),
            ..host.supplies()
        })
        .map_err(TerminalRuntimeError::Requirements)?;
    hosted_invocation::execute_installed(host, installation, quantum)
}

/// Checked host choices for PSI's explicit low-level installation API.
pub struct HostedTerminalProgram<'program> {
    request: TerminalRuntimeRequest<'program>,
    program: TerminalProgram<'program>,
    providers:
        Option<std::borrow::Cow<'program, psi_provider_selections::AdmittedProviderSelections>>,
    host_boundaries: host_imports::HostBoundarySelections,
    target_machines: Option<terminal_interpreter::TerminalTargetContext>,
    target_layouts: Option<terminal_interpreter::AdmittedTargetLayouts>,
}

impl<'program> HostedTerminalProgram<'program> {
    pub fn prepare(
        request: TerminalRuntimeRequest<'program>,
    ) -> Result<Self, TerminalRuntimeError> {
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
            && !provisions.matches_artifact(request.artifact)
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
                    .map_err(|error| {
                        TerminalRuntimeError::missing("target-provider-selection", error)
                    })
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
        let execution_target = request.target.or_else(|| {
            request
                .provisions
                .map(|provisions| provisions.target_slot().owner.target_name())
        });
        let target_machines = execution_target
            .map(|target| terminal_interpreter::terminal_target_context(artifact, target));
        let target_layouts = target_machines
            .as_ref()
            .map(|selected| {
                let facts = layout_target_facts(selected.target())?;
                terminal_interpreter::admit_target_layouts(artifact, selected, &facts)
            })
            .transpose()
            .map_err(|error| {
                TerminalRuntimeError::Requirements(TerminalInterpretError::TargetLayoutSelection(
                    error,
                ))
            })?;
        let mut host_boundaries = host_imports::HostBoundarySelections::default();
        if let Some(provisions) = request.provisions {
            for provision in provisions.host() {
                if matches!(&provision.realization,
                target_vocabulary::BoundarySettlementRealization::NormalizedForeignCall(binding)
                    if matches!(binding.mechanism, target_vocabulary::NormalizedForeignCallMechanism::Component { .. }))
                {
                    continue;
                }
                host_boundaries.add(&program, provision);
            }
        }

        let providers = proposed_installation
            .map(std::borrow::Cow::Owned)
            .or_else(|| retained_installation.map(std::borrow::Cow::Borrowed));
        Ok(Self {
            request,
            program,
            providers,
            host_boundaries,
            target_machines,
            target_layouts,
        })
    }

    pub fn request(&self) -> &TerminalRuntimeRequest<'program> {
        &self.request
    }

    pub fn into_request(self) -> TerminalRuntimeRequest<'program> {
        self.request
    }

    pub fn program(&self) -> &TerminalProgram<'program> {
        &self.program
    }

    pub fn supplies(&self) -> TerminalRuntimeSupplyRequest<'_> {
        let request = &self.request;
        TerminalRuntimeSupplyRequest {
            native_code_layouts: None,
            x86_flags: request.x86_flags,
            memory_ordering: request.memory_ordering,
            entry: request
                .entry
                .map(|entry| entry.terminal_entry())
                .or_else(|| request.provisions.map(|provisions| provisions.entry()))
                .map(terminal_psi::TargetMachineCallable::Machine),
            installation: self.providers.as_deref(),
            host_imports: &self.host_boundaries.imports,
            external_completions: &self.host_boundaries.external_completions,
            target_machines: self.target_machines.as_ref(),
            target_layouts: self.target_layouts.as_ref(),
        }
    }
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
) -> Result<psi_target_layouts::LayoutTargetFacts, psi_target_layouts::TargetLayoutSelectionError> {
    target::TargetProfile::from_canonical_target_name(name)
        .map_err(|reason| {
            psi_target_layouts::TargetLayoutSelectionError::InvalidTargetInput(
                reason.to_string(),
            )
        })?
        .layout_target_facts()
        .ok_or_else(|| {
            psi_target_layouts::TargetLayoutSelectionError::InvalidTargetInput(
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
