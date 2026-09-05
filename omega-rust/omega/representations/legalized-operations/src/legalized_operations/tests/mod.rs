//! Optimizer module role: stage group.
use crate::*;
pub(super) use abstract_operations::CompletionClaimSource;
pub(super) use calling_conventions::{
    CallSignature, CallingPolicy, MachineRegister, ValueShape, evaluate_call_plan,
};
pub(super) use optimization_core::OptimizationUnitIdentity;
pub(super) use optimization_unit::{EffectLink, FuelSettlement, OwnershipEvent, PsiProvenance};
pub(super) use semantic_vocabulary::{
    BlockId, BoundaryMachineId, ClaimId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType,
    IntegerValue, OperationId, StructuralDomainId, StructuralFieldId, StructuralPlaceKind,
    StructuralTypeId, ValueId,
};
pub(super) use target::NativeTarget;
pub(super) use target_operations::TerminalPsiProvenance;
pub(super) use terminal_psi::{
    BindingRelevance, ClaimTransfer, CompletionReceipt, EntryClaim, ProviderCandidateConformance,
    SemanticFingerprint, StructuralAccess, StructuralArgument, StructuralFieldDeclaration,
    StructuralFieldType, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPathSegment, StructuralPlaceDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalPsiIdentity, VocabularyMarker,
};

mod fixtures;
mod identity;
mod validation;

use fixtures::*;

pub(super) fn id<T>(raw: u64) -> T
where
    T: semantic_vocabulary::PsiSemanticId,
{
    T::new(raw).expect("nonzero test identity")
}
