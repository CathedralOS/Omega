//! Checked terminal plans: the plan carriers every terminal machine kind
//! produces for lowering.
//!
//! `terminal_selections.rs` selects machines, `scalar_state_plans.rs`
//! carries a state's scalar bindings, terminator and guarded exits,
//! `structural_control_plans.rs` and `composed_unit_control_plans.rs` carry
//! control graphs, `structural_return.rs` carries a Unit body's structural result,
//! `structural_type_plans.rs` and `structural_argument_plans.rs` carry
//! structural types and arguments,
//! `affine_cleanup_plans.rs` carries affine cleanup and `unit_plans.rs` names
//! the one Unit-bodied plan a machine carries across the cleanup and control
//! rosters, `result_binding_plans.rs` carries result bindings,
//! `unit_effect_plans.rs` carries effect operations, `unit_call_plans.rs`
//! carries the one call operation's route, result and transfers,
//! `boundary_machine_plans.rs` carries boundary machines, `atomic_plans.rs`
//! carries normalized atomic events and `place_loan_plans.rs` carries the
//! place each silent `&`/`&mut` loan local names.

mod affine_cleanup_plans;
mod atomic_plans;
mod boundary_machine_plans;
mod composed_unit_control_plans;
mod dynamic_slot_plans;
mod place_loan_plans;
mod result_binding_plans;
mod scalar_state_plans;
mod structural_argument_plans;
mod structural_control_plans;
mod structural_return;
mod structural_type_plans;
mod terminal_selections;
mod unit_call_plans;
mod unit_effect_plans;
mod unit_plans;

pub use affine_cleanup_plans::{
    CheckedCleanupCaseCondition, CheckedCleanupControlPlans,
    CheckedNominalAffineUnitCleanupMachinePlan, CheckedNominalAffineUnitCleanupPlans,
    CheckedNormalCleanupAction, CheckedNormalExitCleanupPlan,
    CheckedPartialAffineUnitCleanupMachinePlan, CheckedPartialAffineUnitCleanupPlans,
    CheckedStructuralCallCustodyPlan, CheckedStructuralResultClaimPlan,
    CheckedUnitClaimTransferPlan, CheckedUnitNominalAffineCallerRequirementPlan,
    CheckedUnitNominalAffineCleanupPlan, CheckedUnitNominalAffineCleanupRequirementPlan,
    CheckedUnitPartialAffineDiscardPlan,
};
pub use atomic_plans::{CheckedAtomicAccessPlan, CheckedAtomicEvent, CheckedAtomicReadModifyWrite};
pub use boundary_machine_plans::{
    CheckedBoundaryLifetimeDependency, CheckedBoundaryMachinePlan,
    CheckedBoundaryMachineResultPlan, CheckedBoundaryReferenceDestination,
    CheckedBoundaryReferenceOutput, CheckedBoundaryScalarGuarantee,
    CheckedProviderAttachmentRequirementPlan,
};
pub use composed_unit_control_plans::{
    CheckedClosedSumCaseOutcomePlan, CheckedClosedSumCasePlan, CheckedClosedSumPayloadTransferPlan,
    CheckedClosedSumSubjectKind, CheckedComposedUnitControlMachinePlan,
    CheckedComposedUnitControlStatePlan, CheckedComposedUnitControlTerminatorPlan,
    CheckedConditionalReturnArm, CheckedControlResultPlan, CheckedGuardedJumpArmPlan,
    CheckedGuardedJumpFallbackPlan, CheckedGuardedJumpPlan, CheckedGuardedJumpReturnPlan,
    CheckedNaturalRankMeasure, CheckedRankEdgeCoordinate, CheckedScalarCaseFieldPlan,
    CheckedScalarReturnPlan, CheckedStateContractRequirement, CheckedStateNaturalRank,
};
pub use dynamic_slot_plans::{CheckedDynamicClaimPath, CheckedDynamicSlotPlan};
pub use place_loan_plans::{CheckedPlaceLoanPlan, CheckedPlaceLoanRoot};
pub use result_binding_plans::{
    CheckedByteSequenceValue, CheckedByteSequenceValuePlan, CheckedInitializedLocal,
    CheckedUnitScalarResultBindingPlan, CheckedUnitStructuralResultBindingPlan,
};
pub use scalar_state_plans::{
    CheckedGuardedExitPlans, CheckedScalarBranchDestination, CheckedScalarGuardedExit,
    CheckedScalarGuardedTail, CheckedScalarStateTerminator,
};
pub use structural_argument_plans::{
    CheckedCallScalarArgument, CheckedReferenceStorage, CheckedStructuralScalarParameterPlan,
    CheckedUnitCallCoordinate, CheckedUnitEntryClaimPlan, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan,
};
pub use structural_control_plans::{
    CheckedStructuralControlCleanupPlans, CheckedStructuralControlEdgeCleanupPlan,
    CheckedStructuralControlStateCleanupPlan, CheckedStructuralControlSuccessorPlan,
    CheckedStructuralControlTransferPlan, CheckedStructuralControlTransferSourcePlan,
    CheckedStructuralScalarArgumentPlan, CheckedStructuralScalarArgumentSourcePlan,
};
pub use structural_return::{
    CheckedReferenceOriginProjection, CheckedReferenceOriginRoot, CheckedReferenceOriginSourcePlan,
    CheckedReferenceResultSourcePlan, CheckedReturnedReferencePlan,
    CheckedStructuralReturnCompletion, CheckedUnitStructuralReturnPlan,
};
pub use structural_type_plans::{
    CheckedAffineConstructionElementPlan, CheckedBindingFieldReceipt,
    CheckedBindingParameterReceipt, CheckedByteSequenceCarrier, CheckedExtent,
    CheckedMemberIdentity, CheckedRuntimeIndex, CheckedStructuralAccess,
    CheckedStructuralPathQualification, CheckedStructuralResultPlan,
    CheckedTrivialAffineStructuralLocalPlan, CheckedUnitRecordStorage,
    CheckedUnitStructuralCasePlan, CheckedUnitStructuralDomainPlan,
    CheckedUnitStructuralDomainRequirementPlan, CheckedUnitStructuralFieldPlan,
    CheckedUnitStructuralFieldType, CheckedUnitStructuralParameterPlan,
    CheckedUnitStructuralPathSegment, CheckedUnitStructuralTypePlan,
    CheckedUnitStructuralTypeShape, checked_structural_type_by_identity, is_borrowed_view,
    is_bounded_nominal_cleanup_record, is_partial_affine_field_type,
};
pub use terminal_selections::{
    CheckedTerminalDebugPlans, CheckedTerminalMachineDebugPlan, CheckedTerminalMachineSelection,
    CheckedTerminalMachineSelections, CheckedTerminalSignatureEligibility,
    CheckedTerminalStateDebugPlan,
};
pub use unit_call_plans::{
    CheckedUnitCallParts, CheckedUnitCallPlan, CheckedUnitCallResult, CheckedUnitCallRoute,
    CheckedUnitCallTarget, CheckedUnitCallTransfers,
};
pub use unit_effect_plans::{
    CheckedOperandSequence, CheckedStructuralValueCall, CheckedUnitCallableEntry,
    CheckedUnitCleanupBoundary, CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan,
    CheckedUnitEffectPlans, CheckedUnitLocalBindingPlan, CheckedUnitParameterStoragePlan,
    CheckedUnitPlanGap, CheckedUnitPlanGapReason, CheckedUnitScalarControlPlan,
    CheckedUnitSelectedReferenceRetirementPlan,
};
pub use unit_plans::CheckedUnitPlan;
