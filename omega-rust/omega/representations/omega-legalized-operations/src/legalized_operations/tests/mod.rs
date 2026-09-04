//! Optimizer module role: stage group.
use crate::*;
pub(super) use omega_abstract_operations::CompletionClaimSource;
pub(super) use omega_calling_conventions::{
    CallSignature, CallingPolicy, MachineRegister, ValueShape, evaluate_call_plan,
};
pub(super) use omega_optimization_core::OptimizationUnitIdentity;
pub(super) use omega_optimization_unit::{
    EffectLink, FuelSettlement, OwnershipEvent, PsiProvenance,
};
pub(super) use omega_target::NativeTarget;
pub(super) use omega_target_operations::TerminalPsiProvenance;
pub(super) use psi_core::{
    BlockId, BoundaryMachineId, ClaimId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType,
    IntegerValue, OperationId, StructuralDomainId, StructuralFieldId, StructuralPlaceKind,
    StructuralTypeId, ValueId,
};
pub(super) use psi_terminal::{
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
    T: psi_core::PsiSemanticId,
{
    T::new(raw).expect("nonzero test identity")
}
