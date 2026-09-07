//! Optimizer module role: stage group.
use crate::*;
pub(super) use abstract_operations::CompletionClaimSource;
pub(super) use calling_conventions::{
    CallSignature, CallingPolicy, ValueShape, evaluate_call_plan,
};
pub(super) use optimization_core::OptimizationUnitIdentity;
pub(super) use optimization_unit::{EffectLink, FuelSettlement, OwnershipEvent, PsiProvenance};
pub(super) use semantic_vocabulary::{
    BoundaryMachineId, ClaimId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType,
    IntegerValue, OperationId, ScalarType, StructuralDomainId, StructuralFieldId,
    StructuralPlaceKind, StructuralTypeId, ValueId,
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
mod scalar_calls;
mod validation;

use fixtures::*;

fn structural_call_mut(plan: &mut LegalizedOperationPlan) -> &mut LegalizedScalarCall {
    let LegalizedScalarInstructionKind::Call(call) =
        &mut plan.scalar_functions[0].blocks[0].instructions[0].kind
    else {
        panic!("structural call fixture");
    };
    call
}

fn scalar_argument_mut(
    argument: &mut LegalizedScalarArgument,
) -> (&mut ValueId, &mut calling_conventions::ValuePlacement) {
    let LegalizedScalarArgument::Scalar { source, placement } = argument else {
        panic!("scalar argument fixture");
    };
    (source, placement)
}

fn structural_argument_mut(
    argument: &mut LegalizedScalarArgument,
) -> (
    &mut StructuralArgument,
    &mut target_operations::TargetStructuralArgument,
) {
    let LegalizedScalarArgument::Structural { semantic, target } = argument else {
        panic!("structural argument fixture");
    };
    (semantic, target)
}

pub(super) fn id<T>(raw: u64) -> T
where
    T: semantic_vocabulary::PsiSemanticId,
{
    T::new(raw).expect("nonzero test identity")
}
