//! Checked terminal plans: the plan carriers every terminal machine kind
//! produces for lowering.
//!
//! `terminal_selections.rs` selects machines, `scalar_graph_plans.rs`,
//! `structural_control_plans.rs` and `composed_unit_control_plans.rs` carry
//! control graphs, `scalar_return_plans.rs`, `structural_return_plans.rs`
//! and `structural_return.rs` carry returns, `structural_type_plans.rs` and
//! `structural_argument_plans.rs` carry structural types and arguments,
//! `affine_cleanup_plans.rs` carries affine cleanup, `result_binding_plans.rs`
//! carries result bindings, `unit_effect_plans.rs` carries effect
//! operations and `boundary_machine_plans.rs` carries boundary machines.

mod affine_cleanup_plans;
mod boundary_machine_plans;
mod composed_unit_control_plans;
mod result_binding_plans;
mod scalar_graph_plans;
mod scalar_return_plans;
mod structural_argument_plans;
mod structural_control_plans;
mod structural_return;
mod structural_return_plans;
mod structural_type_plans;
mod terminal_selections;
mod unit_effect_plans;

pub use affine_cleanup_plans::{
    CheckedNominalAffineUnitCleanupMachinePlan, CheckedNominalAffineUnitCleanupPlans,
    CheckedPartialAffineUnitCleanupMachinePlan, CheckedPartialAffineUnitCleanupPlans,
    CheckedStructuralCallCustodyPlan, CheckedStructuralReturnedClaimTransferPlan,
    CheckedUnitClaimTransferPlan, CheckedUnitNominalAffineCallerRequirementPlan,
    CheckedUnitNominalAffineCleanupPlan, CheckedUnitNominalAffineCleanupRequirementPlan,
    CheckedUnitPartialAffineDiscardPlan,
};
pub use boundary_machine_plans::{
    CheckedBoundaryMachinePlan, CheckedBoundaryMachineResultPlan,
    CheckedProviderAttachmentRequirementPlan,
};
pub use composed_unit_control_plans::{
    CheckedClosedSumCaseSuccessorPlan, CheckedClosedSumPayloadTransferPlan,
    CheckedComposedUnitControlMachinePlan, CheckedComposedUnitControlStatePlan,
    CheckedComposedUnitControlTerminatorPlan, CheckedControlResultPlan, CheckedNaturalRankMeasure,
    CheckedScalarCaseFieldPlan, CheckedStateNaturalRank, CheckedStructuralCaseReturnPlan,
};
pub use result_binding_plans::{
    CheckedByteSequenceStoreValue, CheckedByteSequenceWritePlan, CheckedPrimitiveStoreDestination,
    CheckedStructuralByteSequenceFieldByteStorePlan, CheckedStructuralByteSequenceFieldStorePlan,
    CheckedUnitScalarResultBindingPlan, CheckedUnitStructuralResultBindingPlan,
};
pub use scalar_graph_plans::{
    CheckedScalarBinding, CheckedScalarBindingDestination, CheckedScalarBindingValue,
    CheckedScalarBranchDestination, CheckedScalarGraphPlans, CheckedScalarGuardedExit,
    CheckedScalarGuardedTail, CheckedScalarMachineGraph, CheckedScalarParameterStorage,
    CheckedScalarPrimitiveLocalPlan, CheckedScalarStateGraph, CheckedScalarStateTerminator,
    CheckedScalarSuccessor,
};
pub use scalar_return_plans::{
    CheckedBoundaryScalarReturnMachinePlan, CheckedBoundaryScalarReturnPlans,
    CheckedSelectedOperatorStructuralScalarReturnMachinePlan,
    CheckedStructuralBooleanConvergencePlan, CheckedStructuralScalarIntegerBoundKind,
    CheckedStructuralScalarIntegerBoundPlan, CheckedStructuralScalarIntegerBoundRequirementPlan,
    CheckedStructuralScalarParameterPlan, CheckedStructuralScalarReturnCleanupAction,
    CheckedStructuralScalarReturnMachinePlan, CheckedStructuralScalarReturnPlans,
    CheckedTraitOperatorScalarReturnMachinePlan,
};
pub use structural_argument_plans::{
    CheckedCallScalarArgument, CheckedUnitCallCoordinate, CheckedUnitEntryClaimPlan,
    CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralArgumentSourcePlan,
};
pub use structural_control_plans::{
    CheckedStructuralControlCleanupPlans, CheckedStructuralControlEdgeCleanupPlan,
    CheckedStructuralControlProjectedEdgeCleanupPlan,
    CheckedStructuralControlProjectedTransferPlan, CheckedStructuralControlStateCleanupPlan,
    CheckedStructuralControlSuccessorPlan, CheckedStructuralControlTransferPlan,
    CheckedStructuralControlTransferSourcePlan, CheckedStructuralRankedArgumentPlan,
    CheckedStructuralRankedGuardPlan, CheckedStructuralRankedSccEdgePlan,
    CheckedStructuralRankedSccPlan, CheckedStructuralScalarArgumentPlan,
    CheckedStructuralScalarArgumentSourcePlan, CheckedStructuralUnitControlMachinePlan,
    CheckedStructuralUnitControlPlans, CheckedStructuralUnitControlStatePlan,
    CheckedStructuralUnitControlTerminatorPlan,
};
pub use structural_return::{CheckedReferenceResultSourcePlan, CheckedUnitStructuralReturnPlan};
pub use structural_return_plans::{
    CheckedClaimFreeAffineStructuralReturnMachinePlan, CheckedPayloadlessCaseReturnMachinePlan,
    CheckedPayloadlessGuardedCallEvidencePlan, CheckedPayloadlessGuardedCallEvidenceUsePlan,
    CheckedPayloadlessGuardedCallReturnMachinePlan, CheckedStructuralCallReturnPlans,
    CheckedStructuralReturnMachinePlan, CheckedStructuralReturnPlans,
};
pub use structural_type_plans::{
    CheckedAffineConstructionElementPlan, CheckedByteSequenceCarrier,
    CheckedFusedServiceErasureReceipt, CheckedFusedServiceParameterReceipt,
    CheckedStructuralAccess, CheckedStructuralPathQualification, CheckedStructuralResultPlan,
    CheckedTrivialAffineStructuralLocalPlan, CheckedUnitStructuralCasePlan,
    CheckedUnitStructuralDomainPlan, CheckedUnitStructuralDomainRequirementPlan,
    CheckedUnitStructuralFieldPlan, CheckedUnitStructuralFieldType,
    CheckedUnitStructuralParameterPlan, CheckedUnitStructuralPathSegment,
    CheckedUnitStructuralTypePlan, CheckedUnitStructuralTypeShape,
};
pub use terminal_selections::{
    CheckedTerminalDebugPlans, CheckedTerminalMachineDebugPlan, CheckedTerminalMachineSelection,
    CheckedTerminalMachineSelections, CheckedTerminalSignatureEligibility,
    CheckedTerminalStateDebugPlan,
};
pub use unit_effect_plans::{
    CheckedStructuralValueCall, CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan,
    CheckedUnitEffectPlans, CheckedUnitPlanOmission, CheckedUnitPlanOmissionStage,
    CheckedUnitScalarControlPlan,
};
