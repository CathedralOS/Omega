#![forbid(unsafe_code)]

//! Read-only projections of admitted installation evidence consumed below
//! Omega orchestration.
//!
//! These traits carry no constructors and grant no authority. Orchestration
//! owns the sealed values that implement them; lowering and image emission can
//! inspect only the exact identities required to preserve those decisions.

use std::collections::BTreeSet;

mod native_fuel;
pub use native_fuel::{
    NativeFuelActivationStateSlot, NativeFuelContextLayout, NativeFuelRuntimeEntryIdentity,
    NativeFuelRuntimeTextEvidence, NativeFuelRuntimeTextSpan, NativeFuelSavedValue,
    NativeFuelSponsorStackPlan, NativeFuelTargetPlanProjection, NativeFuelTransferEvidenceError,
    NativeFuelTransferPlanError, NativeFuelTransferRuntimePlanProjection, SponsorContextTransport,
    TerminalNativeFuelTransferRuntimeEvidence,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TerminalFuelAttributionSite {
    Operation(psi_core::OperationId),
    Edge(psi_core::EdgeId),
}

/// Read-only normalized projection of one byte-validated native fuel site.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalFuelAttributionEvidence {
    pub machine: psi_core::MachineId,
    pub schedule: psi_core::FuelScheduleIdentity,
    pub site: TerminalFuelAttributionSite,
    pub units: u64,
    pub operation_ordinal: usize,
    pub text_offset: usize,
    pub byte_count: usize,
}

/// Exact source, hot-charge, semantic, and cold-dispatch locations retained by
/// an independently replayed metered image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalNativeFuelChargeEvidence {
    pub attribution: TerminalFuelAttributionEvidence,
    pub charge_text_offset: usize,
    pub charge_byte_count: usize,
    pub semantic_text_offset: usize,
    pub cold_dispatch_text_offset: usize,
    pub cold_dispatch_byte_count: usize,
}

/// Dependency-light projection of a final, independently replayed dynamic-
/// fuel image. This is input evidence, not installation authority; external-
/// root admission still binds both unrelocated and materialized bytes to one
/// exact installed-code value on its side of the
/// dependency boundary.
pub trait TerminalNativeFuelImageEvidence {
    fn terminal_psi(&self) -> psi_terminal::TerminalPsiIdentity;
    fn target(&self) -> omega_target::NativeTarget;
    fn target_policy(&self) -> NativeFuelTargetPlanProjection;
    fn source_text_bytes(&self) -> &[u8];
    fn metered_text_bytes(&self) -> &[u8];
    fn final_text_bytes(&self) -> &[u8];
    fn function_text_offset(&self, machine: psi_core::MachineId) -> Option<usize>;
    fn charges(&self) -> Vec<TerminalNativeFuelChargeEvidence>;
}

/// Dependency-light view of a final image containing the compiler-owned
/// exhaustion-transfer runtime. This exposes both complete text coordinates
/// and the independently replayed runtime intervals, but grants no authority
/// to install or execute either entry.
pub trait TerminalNativeFuelTransferRuntimeImageEvidence {
    fn terminal_psi(&self) -> psi_terminal::TerminalPsiIdentity;
    fn target(&self) -> omega_target::NativeTarget;
    fn unrelocated_text_bytes(&self) -> &[u8];
    fn final_text_bytes(&self) -> &[u8];
    /// Exact compiler-owned `.text` coordinate named by the replayed sponsor
    /// call relocation. This remains a coordinate, not a callable reference.
    fn sponsor_text_offset(&self) -> usize;
    fn transfer_runtime_evidence(&self) -> &TerminalNativeFuelTransferRuntimeEvidence;
}

/// Exact admitted provider-execution identity projected into terminal
/// lowering and installation records.
pub trait TerminalProviderExecutionEvidence: std::fmt::Debug {
    /// Canonical terminal requirement identity selected by this admitted
    /// execution. Lowering compares this value with the exact bodyless
    /// boundary declaration before it projects the numeric execution record.
    fn requirement_identity(&self) -> &str;
    fn provider_plan(&self) -> u64;
    fn provider_execution_identity(&self) -> u64;
    fn provider_execution_fingerprint(&self) -> u64;
    fn normalized_root_identity(&self) -> u64;
    fn boundary_contract_fingerprint(&self) -> u64;
}

/// Exact caller-local claim source retained by one admitted installed-provider
/// Unit call. This mirrors terminal abstract custody without making the
/// installation evidence crate depend on an Omega lowering representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalInstalledProviderCompletionClaimSource {
    pub claim: psi_core::ClaimId,
    pub entry: Option<psi_terminal::EntryClaim>,
    pub content: Option<psi_terminal::ContentEntryClaim>,
}

/// Read-only projection of one boundary occurrence admitted as a call to an
/// exact checked provider machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalInstalledProviderUnitCallEvidence {
    pub caller: psi_core::MachineId,
    pub psi_operation: psi_core::OperationId,
    pub boundary: psi_core::BoundaryMachineId,
    pub provider: psi_terminal::ProviderCandidateConformance,
    pub structural_arguments: Vec<psi_terminal::StructuralArgument>,
    pub completion_claim_sources: Vec<TerminalInstalledProviderCompletionClaimSource>,
    pub completion_receipts: Vec<psi_terminal::CompletionReceipt>,
}

/// Dependency-light view of an opaque provider installation admitted against
/// one exact terminal-Psi artifact. Returning owned projections exposes no
/// mutable installation map or construction authority.
pub trait TerminalProviderInstallationEvidence: std::fmt::Debug {
    fn terminal_psi(&self) -> psi_terminal::TerminalPsiIdentity;
    fn installed_provider_unit_calls(&self) -> Vec<TerminalInstalledProviderUnitCallEvidence>;
}

/// Read-only projection of one opaque, installation-owned component progress
/// acceptance. These compact identities enter the terminal installation
/// record and artifact fingerprint, but never substitute for the retained
/// acceptance value at publication.
pub trait TerminalComponentProgressAcceptanceEvidence: std::fmt::Debug {
    fn component_progress_manifest_identity(&self) -> u64;
    fn component_progress_acceptance_identity(&self) -> u64;
}

/// Relocation-free terminal object facts required to bind installed entry and
/// fixed-fuel evidence.
pub trait TerminalObjectEvidence {
    fn terminal_psi(&self) -> psi_terminal::TerminalPsiIdentity;
    fn target(&self) -> omega_target::NativeTarget;
    fn architecture(&self) -> omega_target::Architecture {
        self.target().architecture
    }
    fn text_bytes(&self) -> &[u8];
    fn function_text_offset(&self, machine: psi_core::MachineId) -> Option<usize>;
    fn fuel_attribution(&self) -> Vec<TerminalFuelAttributionEvidence>;
}

/// Emitter-derived stack closure for one terminal entry.
pub trait TerminalStackDemandEvidence {
    fn terminal_psi(&self) -> psi_terminal::TerminalPsiIdentity;
    fn architecture(&self) -> omega_target::Architecture;
    fn entry(&self) -> psi_core::MachineId;
    fn ceiling_bytes(&self) -> u64;
    fn stack_alignment(&self) -> u32;
    fn contributing_machines(&self) -> &BTreeSet<psi_core::MachineId>;
}
