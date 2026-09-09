//! calls structural in the legalized operations program.

use abstract_operations::CompletionClaimSource;
use optimization_unit::EffectLink;
use optimization_unit::FuelSettlement;
use optimization_unit::OwnershipEvent;
use semantic_vocabulary::BoundaryMachineId;
use semantic_vocabulary::OperationId;
use semantic_vocabulary::ServiceId;
use target_operations::ClaimCompletionOnlyRealization;
use target_operations::ProviderExecutionBinding;
use terminal_psi::CompletionReceipt;
use terminal_psi::EntryClaim;
use terminal_psi::ProviderCandidateConformance;
use terminal_psi::StructuralArgument;
use terminal_psi::StructuralParameterDeclaration;
use terminal_psi::StructuralPlaceDeclaration;
use terminal_psi::StructuralTypeDeclaration;

/// Current structural signature and ownership declarations, without executable rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedStructuralContract {
    pub result: Option<terminal_psi::StructuralResultDeclaration>,
    pub structural_types: Vec<StructuralTypeDeclaration>,
    pub parameters: Vec<LegalizedCallUnitParameter>,
    pub structural_places: Vec<StructuralPlaceDeclaration>,
    pub entry_claims: Vec<EntryClaim>,
    pub published_service_ceiling: Vec<ServiceId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedBoundarySettlement {
    pub operation: OperationId,
    pub boundary: BoundaryMachineId,
    pub provider_execution: ProviderExecutionBinding,
    pub realization: ClaimCompletionOnlyRealization,
    pub arguments: Vec<StructuralArgument>,
    pub completion_claim_sources: Vec<CompletionClaimSource>,
    pub completion_receipts: Vec<CompletionReceipt>,
    pub fuel: Vec<FuelSettlement>,
    pub effect: EffectLink,
    pub ownership: Vec<OwnershipEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedCallUnitParameter {
    pub semantic: StructuralParameterDeclaration,
    pub target: target_operations::TargetStructuralParameter,
}

/// Exact semantic origin of one legalized structural Unit call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegalizedCallUnitSource {
    AuthoredCallUnit,
    InstalledProvider {
        boundary: BoundaryMachineId,
        provider: ProviderCandidateConformance,
        completion_claim_sources: Vec<CompletionClaimSource>,
        completion_receipts: Vec<CompletionReceipt>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegalizedCallSourceError {
    ProviderIdentityMismatch,
    ArgumentSignatureMismatch,
    CompletionEvidenceMismatch,
    OwnershipMismatch,
}
