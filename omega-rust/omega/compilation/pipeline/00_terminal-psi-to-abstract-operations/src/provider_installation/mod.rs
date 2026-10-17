//! Optimizer module role: executable entrance.
//! Provider installation joins exact canonical candidates and occurrences
//! inside the invocation's selected abstract scope. Canonical Terminal keeps
//! the complete catalog; scope selection precedes installation so no later
//! consumer must discard orphaned call receipts. Optimizations do not select
//! providers or redirect family heads.

mod admission;
mod claim_content;
mod error;
mod replay;

pub use error::ProviderInstallationError;

use abstract_operations::CompletionClaimSource;
pub(crate) use admission::admit_provider_installation_with_projection;
use semantic_vocabulary::{MachineId, OperationId};
use terminal_psi::{CompletionReceipt, ProviderCandidateConformance, StructuralArgument};

/// Bind Omega's provider policy only to exact rows preserved from the verified
/// terminal catalog. Installation checks selections against the retained
/// artifact view before returning the private-field carrier used by execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedProviderAdapter {
    /// Selecting declaration, independently of the realizing callable.
    pub provider_declaration: String,
    /// Full originating plan identity; report fingerprints cannot select it.
    pub provider_plan_digest: effects::provider_plan::ProviderPlanDigest,
    /// Original row before specialization or target-family aliasing.
    pub selected_requirement_identity: String,
    pub requirement_identity: String,
    pub provider_identity: String,
    pub machine_identity: String,
}

/// One exact boundary occurrence bound to the checked provider
/// row selected for its requirement. Private fields prevent target lowering
/// from reconstructing this authority from a candidate machine ID alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedInstalledProviderCall {
    pub(crate) caller: MachineId,
    pub(crate) psi_operation: OperationId,
    pub(crate) boundary: semantic_vocabulary::BoundaryMachineId,
    pub(crate) provider: ProviderCandidateConformance,
    pub(crate) result: terminal_psi::OperationResult,
    pub(crate) scalar_arguments: Vec<semantic_vocabulary::ValueId>,
    pub(crate) structural_arguments: Vec<StructuralArgument>,
    pub(crate) completion_claim_sources: Vec<CompletionClaimSource>,
    pub(crate) completion_receipts: Vec<CompletionReceipt>,
}

impl AdmittedInstalledProviderCall {
    pub const fn caller(&self) -> MachineId {
        self.caller
    }

    pub const fn psi_operation(&self) -> OperationId {
        self.psi_operation
    }

    pub const fn boundary(&self) -> semantic_vocabulary::BoundaryMachineId {
        self.boundary
    }

    pub const fn provider(&self) -> &ProviderCandidateConformance {
        &self.provider
    }

    pub const fn result(&self) -> &terminal_psi::OperationResult {
        &self.result
    }

    pub fn scalar_arguments(&self) -> &[semantic_vocabulary::ValueId] {
        &self.scalar_arguments
    }

    pub fn structural_arguments(&self) -> &[StructuralArgument] {
        &self.structural_arguments
    }

    pub fn completion_claim_sources(&self) -> &[CompletionClaimSource] {
        &self.completion_claim_sources
    }

    pub fn completion_receipts(&self) -> &[CompletionReceipt] {
        &self.completion_receipts
    }
}

/// Omega-owned installation custody. Shared Terminal selection evidence stays
/// sealed; physical consumers use the admitted rows and call occurrences beside
/// it. No interpreter dispatch map or live runtime state is built here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedProviderInstallation {
    pub(crate) terminal_selection: terminal_installation::AdmittedProviderSelections,
    pub(crate) psi: terminal_psi::TerminalPsiIdentity,
    pub(crate) installed_candidates: Vec<ProviderCandidateConformance>,
    pub(crate) installed_calls: Vec<AdmittedInstalledProviderCall>,
}

impl AdmittedProviderInstallation {
    pub const fn psi(&self) -> terminal_psi::TerminalPsiIdentity {
        self.psi
    }

    pub const fn terminal_selection(&self) -> &terminal_installation::AdmittedProviderSelections {
        &self.terminal_selection
    }

    pub(crate) fn installed_candidates(&self) -> &[ProviderCandidateConformance] {
        &self.installed_candidates
    }

    pub fn installed_calls(&self) -> &[AdmittedInstalledProviderCall] {
        &self.installed_calls
    }
}

impl installation_evidence::ProviderInstallationEvidence for AdmittedProviderInstallation {
    fn psi(&self) -> terminal_psi::TerminalPsiIdentity {
        self.psi
    }

    fn installed_provider_calls(
        &self,
    ) -> Vec<installation_evidence::InstalledProviderCallEvidence> {
        self.installed_calls
            .iter()
            .map(
                |call| installation_evidence::InstalledProviderCallEvidence {
                    caller: call.caller,
                    psi_operation: call.psi_operation,
                    boundary: call.boundary,
                    provider: call.provider.clone(),
                    result: call.result.clone(),
                    scalar_arguments: call.scalar_arguments.clone(),
                    structural_arguments: call.structural_arguments.clone(),
                    completion_claim_sources: call
                        .completion_claim_sources
                        .iter()
                        .map(|source| {
                            installation_evidence::InstalledProviderCompletionClaimSource {
                                claim: source.claim,
                                entry: source.entry.clone(),
                                content: source.content.clone(),
                            }
                        })
                        .collect(),
                    completion_receipts: call.completion_receipts.clone(),
                },
            )
            .collect()
    }
}
