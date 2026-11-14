/*
We define the read-only projections of admitted installation evidence consumed
below Omega orchestration (`ProviderExecutionEvidence`,
`ProviderInstallationEvidence`, `ObjectEvidence`, `StackDemandEvidence`, and
`InstalledInterruptStackRequirement`).

We expose traits and immutable value carriers here without public authority
constructors: orchestration owns the sealed values that implement these traits,
while instruction selection, machine emission, and image emission inspect only
the exact identities and byte demands needed to preserve those decisions.
*/

/*
Why keep installation evidence in a tiny dependency-light crate separate from
provider admission and lowering? If instruction selection or object
emission imported the full installation state machine or provider-selection
arenas, every backend lowerer would couple to source-derived planning and
mutable installation registries. By projecting admitted provider calls, object
text offsets, and interrupt stack requirements into plain `terminal_psi` and
`semantic_vocabulary` coordinates, we keep the backend's lower layers closed
against authority escalation while still re-deriving stack and fault premises
at final object publication.
*/

/*
Start in this file for the core evidence traits and interrupt-stack demand
records (`InstalledInterruptStackRequirement`, `InstalledArrivalStackDemand`,
`InstalledBodyFaultPremises`). `installation_environment.rs` holds the
caller-supplied bootstrap and x86 interrupt-delivery platform assumptions
(`InstallationEnvironmentAssumption`).
*/

#![forbid(unsafe_code)]

mod installation_environment;
pub use installation_environment::{
    BootstrapAssumptionInterval, BootstrapEnvironmentAssumption, InstallationEnvironmentAssumption,
    InterruptDeliveryInterval, X86InterruptDeliveryAssumption, X86InterruptDeliveryProfile,
};

use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InstalledArtifactOccurrenceDigest([u8; 32]); // Collision-resistant SHA-256 commitment to one exact installed executable occurrence.

impl InstalledArtifactOccurrenceDigest {
    pub const fn from_sha256(digest: [u8; 32]) -> Self {
        Self(digest)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

pub trait ProviderExecutionEvidence: std::fmt::Debug {
    fn requirement_identity(&self) -> &str;
    fn provider_plan_report_identity(&self) -> u64;
    fn provider_execution_report_identity(&self) -> u64;
    fn provider_execution_report_fingerprint(&self) -> u64;
    fn normalized_root_report_identity(&self) -> u64;
    fn boundary_contract_report_fingerprint(&self) -> u64;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledProviderCompletionClaimSource {
    pub claim: semantic_vocabulary::ClaimId,
    pub entry: Option<terminal_psi::EntryClaim>,
    pub content: Option<terminal_psi::ContentEntryClaim>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledAssumedBindingEvidence {
    pub required: semantic_vocabulary::BoundaryMachineId,
    pub supplied_requirement: semantic_vocabulary::BoundaryMachineId,
    pub implementation: semantic_vocabulary::MachineId,
    pub selected_requirement: semantic_vocabulary::BoundaryMachineId,
    pub supply_path: Vec<semantic_vocabulary::BoundaryMachineId>,
}

impl InstalledAssumedBindingEvidence {
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
    pub result: terminal_psi::OperationResult, // Exact caller-local result established by successful provider completion.
    pub scalar_arguments: Vec<semantic_vocabulary::ValueId>, // Ordered scalar values supplied by the admitted call occurrence.
    pub structural_arguments: Vec<terminal_psi::StructuralArgument>,
    pub completion_claim_sources: Vec<InstalledProviderCompletionClaimSource>,
    pub completion_receipts: Vec<terminal_psi::CompletionReceipt>,
}

pub trait ProviderInstallationEvidence: std::fmt::Debug {
    fn psi(&self) -> terminal_psi::TerminalPsiIdentity;
    fn installed_provider_calls(&self) -> Vec<InstalledProviderCallEvidence>;
}

pub trait ObjectEvidence {
    fn psi(&self) -> terminal_psi::TerminalPsiIdentity;
    fn target(&self) -> target::NativeTarget;
    fn architecture(&self) -> target::Architecture {
        self.target().architecture
    }
    fn text_bytes(&self) -> &[u8];
    fn function_text_offset(&self, machine: semantic_vocabulary::MachineId) -> Option<usize>;
}

pub trait StackDemandEvidence {
    fn psi(&self) -> terminal_psi::TerminalPsiIdentity;
    fn architecture(&self) -> target::Architecture;
    fn entry(&self) -> semantic_vocabulary::MachineId;
    fn ceiling_bytes(&self) -> u64;
    fn stack_alignment(&self) -> u32;
    fn contributing_machines(&self) -> &BTreeSet<semantic_vocabulary::MachineId>;
    fn admitted_stack_contribution_report_identities(&self) -> BTreeSet<u64>;
    fn admitted_stack_contribution_commitments(&self) -> BTreeSet<[u8; 32]>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledInterruptStackRequirement {
    pub arrival: Option<InstalledArrivalStackDemand>, // Conditional composition for this installation; runtime guards check dedicated backing.
    pub fault_exclusion: Option<InstalledBodyFaultPremises>, // Conditional body result re-derived at final composition from the private body.
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledBodyFaultPremises {
    pub physical_program_identity: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledArrivalStackDemand {
    pub physical_entry: InstalledPhysicalStackRoot,
    pub root_reserve_bytes: u64,
    pub domain_bytes: u64,
    pub simultaneous_placements: Vec<usize>, // Sorted, unique, reciprocal placement indices of simultaneously live dedicated entries.
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstalledPhysicalStackRoot {
    pub semantic_entry: semantic_vocabulary::MachineId,
    pub adapter: semantic_vocabulary::MachineId,
}
