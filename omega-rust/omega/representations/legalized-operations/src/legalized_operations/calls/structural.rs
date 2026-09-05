//! calls structural in the legalized operations program.

use abstract_operations::CompletionClaimSource;
use calling_conventions::CallPlan;
use optimization_unit::EffectLink;
use optimization_unit::FuelSettlement;
use optimization_unit::OwnershipEvent;
use semantic_vocabulary::BlockId;
use semantic_vocabulary::BoundaryMachineId;
use semantic_vocabulary::EdgeId;
use semantic_vocabulary::MachineId;
use semantic_vocabulary::ObligationId;
use semantic_vocabulary::OperationId;
use semantic_vocabulary::ServiceId;
use target_operations::ClaimCompletionOnlyRealization;
use target_operations::ProviderExecutionBinding;
use target_operations::TerminalPsiProvenance;
use terminal_psi::ClaimTransfer;
use terminal_psi::CompletionReceipt;
use terminal_psi::CrashRouteBucket;
use terminal_psi::EntryClaim;
use terminal_psi::ProviderCandidateConformance;
use terminal_psi::StructuralArgument;
use terminal_psi::StructuralParameterDeclaration;
use terminal_psi::StructuralPlaceDeclaration;
use terminal_psi::StructuralTypeDeclaration;

/// One structural-signature `ReturnUnit`, optionally preceded by one
/// claim-preserving `CallUnit`.
///
/// Semantic declarations remain paired with their target ABI projections so
/// instruction selection can consume only the legalized carrier. This record
/// assigns no virtual/physical homes, stack frame, instruction encoding,
/// symbol, relocation, object span, or publication authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedStructuralUnitFunction {
    pub machine: MachineId,
    pub attachment: Option<semantic_vocabulary::StructuralTypeId>,
    pub provenance: TerminalPsiProvenance,
    pub recipe: crate::StructuralUnitLegalizationRecipe,
    /// Canonical verifier-owned structural declaration closure required by
    /// the function signature and its call arguments.
    pub structural_types: Vec<StructuralTypeDeclaration>,
    /// Exact target-native call plan for the function's structural ABI.
    pub call_plan: CallPlan,
    /// Ordered semantic parameters paired one-for-one with their target ABI
    /// shape and placement.
    pub parameters: Vec<LegalizedCallUnitParameter>,
    /// Complete declared structural-place roster used to replay the parameter
    /// roots named by claims and call arguments.
    pub structural_places: Vec<StructuralPlaceDeclaration>,
    /// Ordered whole/projection claims present at function entry.
    pub entry_claims: Vec<EntryClaim>,
    /// Exact normalized published-service ceiling. The ProgramStorage slice
    /// requires this to be empty, but legalization must not erase it.
    pub published_service_ceiling: Vec<ServiceId>,
    pub entry_block: BlockId,
    /// Ordered metadata-only provider settlements. These consume claims and
    /// effects but select no instruction.
    pub boundary_settlements: Vec<LegalizedBoundarySettlement>,
    /// Optional sole structural call. `None` represents the terminal callee
    /// needed to close an acyclic `[CallUnit, ReturnUnit]` program while still
    /// retaining its nonempty structural ABI.
    pub call: Option<LegalizedCallUnit>,
    pub return_edge: EdgeId,
    pub return_fuel: Vec<FuelSettlement>,
    pub return_effect: EffectLink,
    pub return_ownership: Vec<OwnershipEvent>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedCallUnitArgument {
    pub semantic: StructuralArgument,
    pub target: target_operations::TargetStructuralArgument,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedCallUnit {
    pub source: LegalizedCallUnitSource,
    pub operation: OperationId,
    pub callee: MachineId,
    pub arguments: Vec<LegalizedCallUnitArgument>,
    pub claim_transfers: Vec<ClaimTransfer>,
    pub requirement_obligations: Vec<ObligationId>,
    pub crash_continuations: Vec<CrashRouteBucket>,
    pub fuel: Vec<FuelSettlement>,
    pub effect: EffectLink,
    pub ownership: Vec<OwnershipEvent>,
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
