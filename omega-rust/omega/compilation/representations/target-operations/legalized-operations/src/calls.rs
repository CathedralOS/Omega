// Structural call contracts, boundary settlements, and call-source validation errors in the
// legalized operation plan.
//
// `LegalizedStructuralContract` pairs a function's semantic structural signature, place table,
// entry claims, and service ceiling with its target-evaluated `TargetStructuralParameter`
// placements (`LegalizedCallUnitParameter`). `LegalizedBoundarySettlement` records a
// claim-completion-only boundary settlement alongside its admitted provider execution, work
// settlements, effect link, and ownership events.

use abstract_operations::CompletionClaimSource;
use optimization_unit::EffectLink;
use optimization_unit::OwnershipEvent;
use optimization_unit::WorkSettlement;
use semantic_vocabulary::BoundaryMachineId;
use semantic_vocabulary::OperationId;
use semantic_vocabulary::ServiceId;
use target_operations::ClaimCompletionOnlyRealization;
use target_operations::ProviderExecutionBinding;
use terminal_psi::CompletionReceipt;
use terminal_psi::EntryClaim;
use terminal_psi::StructuralArgument;
use terminal_psi::StructuralParameterDeclaration;
use terminal_psi::StructuralPlaceDeclaration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedStructuralContract {
    pub result: Option<terminal_psi::StructuralResultDeclaration>,
    pub structural_types: abstract_operations::StructuralTypeCatalog,
    pub parameters: Vec<LegalizedCallUnitParameter>,
    pub structural_places: Vec<StructuralPlaceDeclaration>,
    pub entry_claims: Vec<EntryClaim>,
    pub published_service_ceiling: Vec<ServiceId>,
}

impl LegalizedStructuralContract {
    pub fn structural_place(
        &self,
        place: semantic_vocabulary::PlaceId,
    ) -> Option<&StructuralPlaceDeclaration> {
        self.structural_places.iter().find(|row| row.id == place)
    }

    pub fn structural_type(
        &self,
        id: semantic_vocabulary::StructuralTypeId,
    ) -> Option<&terminal_psi::StructuralTypeDeclaration> {
        self.structural_types
            .as_slice()
            .iter()
            .find(|declaration| declaration.id == id)
    }
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
    pub work: Vec<WorkSettlement>,
    pub effect: EffectLink,
    pub ownership: Vec<OwnershipEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedCallUnitParameter {
    pub semantic: StructuralParameterDeclaration,
    pub target: target_operations::TargetStructuralParameter,
}

pub use target_operations::NativeCallOrigin;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegalizedCallSourceError {
    ProviderIdentityMismatch,
    ArgumentSignatureMismatch,
    CompletionEvidenceMismatch,
    OwnershipMismatch,
}
