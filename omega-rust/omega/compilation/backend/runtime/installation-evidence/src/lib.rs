#![forbid(unsafe_code)]

//! Read-only projections of admitted installation evidence consumed below
//! Omega orchestration.
//!
//! These traits carry no constructors and grant no authority. Orchestration
//! owns the sealed values that implement them; lowering and image emission can
//! inspect only the exact identities required to preserve those decisions.

mod bootstrap_environment;
mod installation_environment;
pub use bootstrap_environment::{BootstrapAssumptionInterval, BootstrapEnvironmentAssumption};
pub use installation_environment::{
    InstallationEnvironmentAssumption, InterruptDeliveryInterval, X86InterruptDeliveryAssumption,
    X86InterruptDeliveryProfile,
};

use std::collections::BTreeSet;

/// Collision-resistant commitment to one exact installed executable
/// occurrence. The executable-installation owner derives the bytes from its
/// private artifact, placement, final-byte, and provider evidence; lower
/// lifecycle representations only retain and compare the commitment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InstalledArtifactOccurrenceDigest([u8; 32]);

impl InstalledArtifactOccurrenceDigest {
    pub const fn from_sha256(digest: [u8; 32]) -> Self {
        Self(digest)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Exact admitted provider-execution identity projected into terminal
/// lowering and installation records.
pub trait ProviderExecutionEvidence: std::fmt::Debug {
    /// Canonical terminal requirement identity selected by this admitted
    /// execution. Lowering compares this value with the exact bodyless
    /// boundary declaration before it projects the numeric execution record.
    fn requirement_identity(&self) -> &str;
    fn provider_plan_report_identity(&self) -> u64;
    fn provider_execution_report_identity(&self) -> u64;
    fn provider_execution_report_fingerprint(&self) -> u64;
    fn normalized_root_report_identity(&self) -> u64;
    fn boundary_contract_report_fingerprint(&self) -> u64;
}

/// Exact caller-local claim source retained by one admitted installed-provider
/// call. This mirrors terminal abstract custody without making the
/// installation evidence crate depend on an Omega lowering representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledProviderCompletionClaimSource {
    pub claim: semantic_vocabulary::ClaimId,
    pub entry: Option<terminal_psi::EntryClaim>,
    pub content: Option<terminal_psi::ContentEntryClaim>,
}

/// Read-only projection of one boundary occurrence admitted as a call to an
/// exact checked provider machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledAssumedBindingEvidence {
    pub required: semantic_vocabulary::BoundaryMachineId,
    pub supplied_requirement: semantic_vocabulary::BoundaryMachineId,
    pub implementation: semantic_vocabulary::MachineId,
    pub selected_requirement: semantic_vocabulary::BoundaryMachineId,
    pub supply_path: Vec<semantic_vocabulary::BoundaryMachineId>,
}

impl InstalledAssumedBindingEvidence {
    /// Correspondence only. Root authorization and compatibility are admitted
    /// before projection; retaining this value cannot create either authority.
    pub fn matches(
        &self,
        required: semantic_vocabulary::BoundaryMachineId,
        provider: &terminal_psi::ProviderCandidateConformance,
    ) -> bool {
        self.required == required
            && self.implementation == provider.candidate
            && self.selected_requirement == provider.boundary
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledProviderCallEvidence {
    pub caller: semantic_vocabulary::MachineId,
    pub invocation_site: semantic_vocabulary::InvocationSite,
    pub boundary: semantic_vocabulary::BoundaryMachineId,
    pub provider: terminal_psi::ProviderCandidateConformance,
    pub assumption: Option<InstalledAssumedBindingEvidence>,
    /// Exact caller-local result established by successful provider completion.
    pub result: terminal_psi::OperationResult,
    /// Ordered scalar values supplied by the exact admitted call occurrence.
    pub scalar_arguments: Vec<semantic_vocabulary::ValueId>,
    pub structural_arguments: Vec<terminal_psi::StructuralArgument>,
    pub completion_claim_sources: Vec<InstalledProviderCompletionClaimSource>,
    pub completion_receipts: Vec<terminal_psi::CompletionReceipt>,
}

/// Dependency-light view of an opaque provider installation admitted against
/// one exact terminal-Psi artifact. Returning owned projections exposes no
/// mutable installation map or construction authority.
pub trait ProviderInstallationEvidence: std::fmt::Debug {
    fn psi(&self) -> terminal_psi::TerminalPsiIdentity;
    fn installed_provider_calls(&self) -> Vec<InstalledProviderCallEvidence>;
}

/// Relocation-free terminal object facts required to bind installed entry and
/// fixed-fuel evidence.
pub trait ObjectEvidence {
    fn psi(&self) -> terminal_psi::TerminalPsiIdentity;
    fn target(&self) -> target::NativeTarget;
    fn architecture(&self) -> target::Architecture {
        self.target().architecture
    }
    fn text_bytes(&self) -> &[u8];
    fn function_text_offset(&self, machine: semantic_vocabulary::MachineId) -> Option<usize>;
}

/// Emitter-derived stack closure for one terminal entry.
pub trait StackDemandEvidence {
    fn psi(&self) -> terminal_psi::TerminalPsiIdentity;
    fn architecture(&self) -> target::Architecture;
    fn entry(&self) -> semantic_vocabulary::MachineId;
    fn ceiling_bytes(&self) -> u64;
    fn stack_alignment(&self) -> u32;
    fn contributing_machines(&self) -> &BTreeSet<semantic_vocabulary::MachineId>;
    /// Non-authoritative compact report coordinates for every admitted opaque
    /// same-stack leaf contributing to this exact closure.
    fn admitted_stack_contribution_report_identities(&self) -> BTreeSet<u64>;
    /// Strong commitments to the complete admitted opaque same-stack claims.
    /// These remain distinct from the compact report coordinates above.
    fn admitted_stack_contribution_commitments(&self) -> BTreeSet<[u8; 32]>;
}

/// Stack capacity requested by one installed entry's physical implementation.
/// These numbers are conditional inputs to instruction selection, not admitted
/// resource authority. Final object publication must derive them again from
/// the exact retained private body and arrival bridge before accepting the
/// guard that uses them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledInterruptStackRequirement {
    /// Conditional composition for this exact installation. The runtime guards
    /// check dedicated backing; physical entry publication supplies root backing.
    pub arrival: Option<InstalledArrivalStackDemand>,
    /// Conditional body result, never a grant of valid RAM or stack capacity.
    /// Final composition re-derives it from the exact private body, and the
    /// installation must independently establish the listed entry premises.
    pub fault_exclusion: Option<InstalledBodyFaultPremises>,
    pub psi: terminal_psi::TerminalPsiIdentity,
    pub target: target::NativeTarget,
    pub operation: semantic_vocabulary::OperationId,
    pub placement_index: usize,
    pub entry: semantic_vocabulary::MachineId,
    pub entry_plan: runtime_abi::static_interrupts::InstalledInterruptEntryPlan,
    pub body_bytes: u64,
    pub bridge_bytes: u32,
    pub call_stack_bias: u32,
    pub alignment: u32,
}

impl InstalledInterruptStackRequirement {
    pub fn required_bytes(&self) -> Option<u64> {
        let own = self.own_bytes()?;
        Some(
            self.arrival
                .as_ref()
                .map_or(own, |arrival| own.max(arrival.domain_bytes)),
        )
    }

    pub fn own_bytes(&self) -> Option<u64> {
        self.body_bytes
            .checked_add(u64::from(self.bridge_bytes))?
            .checked_add(u64::from(self.call_stack_bias))
    }
}

/// This exact emitted body has no remaining synchronous fault source when its
/// admitted CPL0 entry, code/CPU support, checked borrowed memory and sufficient
/// mapped stack hold. It does not exclude physical faults, NMI or machine checks.
/// The full installation still owes those memory and stack premises.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledBodyFaultPremises {
    pub physical_program_identity: [u8; 32],
}

/// Demand from every admitted finite arrival history. These fields grant no
/// backing. Final composition re-derives them from exact entries and bodies;
/// every composed cohort requires the named physical entry's owned stack,
/// including Work's ordinary live frames when its added reserve is zero.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledArrivalStackDemand {
    pub physical_entry: InstalledPhysicalStackRoot,
    pub root_reserve_bytes: u64,
    pub domain_bytes: u64,
    /// Exact placement identities of simultaneously live dedicated entries.
    /// Sorted, unique and reciprocal; semantic classes never name hardware ISTs.
    pub simultaneous_placements: Vec<usize>,
}

/// The two declaration identities already joined by ProgramEntry settlement.
/// The semantic entry receives application arguments; the selected adapter
/// receives firmware's ABI and executes on the image-owned stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstalledPhysicalStackRoot {
    pub semantic_entry: semantic_vocabulary::MachineId,
    pub adapter: semantic_vocabulary::MachineId,
}
