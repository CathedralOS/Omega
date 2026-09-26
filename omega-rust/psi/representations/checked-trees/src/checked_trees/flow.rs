//! Flow facts: what checking recorded along each state's control flow, and
//! the terminal plans the lowering stage consumes.
//!
//! `FlowFacts` (in `roots`) holds the root of each fact family, the checked
//! semantic dependencies, and the `terminal_*` plan lanes; it is stored as
//! `CheckFacts::flow`. `typed-trees-to-checked-trees` builds the family
//! roots in its `flow` module (`FlowFacts::with_roots`) and assigns the
//! terminal lanes in `build_check_facts` (`src/facts.rs`) and its
//! `execution` module. `checked-trees-to-lowered-psi` reads it through
//! `checked.facts.flow`, and `lowered-psi-to-terminal-psi` reads its
//! `terminal_unit_effects` plans.

// `FlowFacts` and the root of each fact family.
mod roots;

// The fact families: semantic and constraint contexts, invalidations,
// borrow activations and weakenings, ownership permission events and claim
// outcomes, boundary edges, control records (states, statements, calls,
// exits and operator invocations) with their operand referents, and
// declaration-level semantic dependencies.
mod borrow_lifetimes;
mod boundaries;
mod contexts;
mod control;
mod invalidations;
mod operator_operand;
mod ownership;
mod semantic_dependencies;

// Terminal plans the checker publishes for lowering: `terminal` holds the
// plan carriers for each terminal machine kind, and `dynamic_scalar_calls`
// the dynamic dispatch and structural field store plans the Unit-effect
// plans use.
mod dynamic_scalar_calls;
mod terminal;

// Lookup methods on `FlowFacts`.
mod queries;

pub use self::terminal::{
    CheckedAffineConstructionElementPlan, CheckedAtomicAccessPlan, CheckedAtomicEvent,
    CheckedAtomicReadModifyWrite, CheckedBoundaryMachinePlan, CheckedBoundaryMachineResultPlan,
    CheckedBoundaryScalarReturnMachinePlan, CheckedBoundaryScalarReturnPlans,
    CheckedByteSequenceCarrier, CheckedByteSequenceStoreValue, CheckedByteSequenceWritePlan,
    CheckedCallScalarArgument, CheckedClaimFreeAffineStructuralReturnMachinePlan,
    CheckedClosedSumCaseSuccessorPlan, CheckedClosedSumPayloadTransferPlan,
    CheckedComposedUnitControlMachinePlan, CheckedComposedUnitControlStatePlan,
    CheckedComposedUnitControlTerminatorPlan, CheckedConditionalReturnArm,
    CheckedControlResultPlan, CheckedFusedServiceErasureReceipt,
    CheckedFusedServiceParameterReceipt, CheckedGuardedJumpPlan, CheckedNaturalRankMeasure,
    CheckedNominalAffineUnitCleanupMachinePlan, CheckedNominalAffineUnitCleanupPlans,
    CheckedPartialAffineUnitCleanupMachinePlan, CheckedPartialAffineUnitCleanupPlans,
    CheckedPayloadlessGuardedCallEvidencePlan, CheckedPayloadlessGuardedCallEvidenceUsePlan,
    CheckedPayloadlessGuardedCallReturnMachinePlan, CheckedPrimitiveStoreDestination,
    CheckedProviderAttachmentRequirementPlan, CheckedReferenceResultSourcePlan, CheckedReturnPlan,
    CheckedRuntimeIndex, CheckedScalarBinding, CheckedScalarBindingDestination,
    CheckedScalarBindingValue, CheckedScalarBranchDestination, CheckedScalarCaseFieldPlan,
    CheckedScalarGraphPlans, CheckedScalarGuardedExit, CheckedScalarGuardedTail,
    CheckedScalarMachineGraph, CheckedScalarParameterStorage, CheckedScalarPrimitiveLocalPlan,
    CheckedScalarReturnPlan, CheckedScalarStateGraph, CheckedScalarStateTerminator,
    CheckedScalarSuccessor, CheckedStateNaturalRank, CheckedStructuralAccess,
    CheckedStructuralBooleanConvergencePlan, CheckedStructuralByteSequenceFieldByteStorePlan,
    CheckedStructuralByteSequenceFieldStorePlan, CheckedStructuralCallCustodyPlan,
    CheckedStructuralCallReturnPlans, CheckedStructuralCaseReturnPlan,
    CheckedStructuralControlCleanupPlans, CheckedStructuralControlEdgeCleanupPlan,
    CheckedStructuralControlProjectedEdgeCleanupPlan,
    CheckedStructuralControlProjectedTransferPlan, CheckedStructuralControlStateCleanupPlan,
    CheckedStructuralControlSuccessorPlan, CheckedStructuralControlTransferPlan,
    CheckedStructuralControlTransferSourcePlan, CheckedStructuralPathQualification,
    CheckedStructuralRankedArgumentPlan, CheckedStructuralRankedGuardPlan,
    CheckedStructuralRankedSccEdgePlan, CheckedStructuralRankedSccPlan,
    CheckedStructuralResultPlan, CheckedStructuralReturnMachinePlan, CheckedStructuralReturnPlans,
    CheckedStructuralReturnedClaimTransferPlan, CheckedStructuralScalarArgumentPlan,
    CheckedStructuralScalarArgumentSourcePlan, CheckedStructuralScalarIntegerBoundKind,
    CheckedStructuralScalarIntegerBoundPlan, CheckedStructuralScalarIntegerBoundRequirementPlan,
    CheckedStructuralScalarParameterPlan, CheckedStructuralScalarReturnCleanupAction,
    CheckedStructuralScalarReturnMachinePlan, CheckedStructuralScalarReturnPlans,
    CheckedStructuralUnitControlMachinePlan, CheckedStructuralUnitControlPlans,
    CheckedStructuralUnitControlStatePlan, CheckedStructuralUnitControlTerminatorPlan,
    CheckedStructuralValueCall, CheckedTerminalDebugPlans, CheckedTerminalMachineDebugPlan,
    CheckedTerminalMachineSelection, CheckedTerminalMachineSelections,
    CheckedTerminalSignatureEligibility, CheckedTerminalStateDebugPlan,
    CheckedTraitOperatorScalarReturnMachinePlan, CheckedTrivialAffineStructuralLocalPlan,
    CheckedUnitCallCoordinate, CheckedUnitClaimTransferPlan, CheckedUnitEffectMachinePlan,
    CheckedUnitEffectOperationPlan, CheckedUnitEffectPlans, CheckedUnitEntryClaimPlan,
    CheckedUnitNominalAffineCallerRequirementPlan, CheckedUnitNominalAffineCleanupPlan,
    CheckedUnitNominalAffineCleanupRequirementPlan, CheckedUnitPartialAffineDiscardPlan,
    CheckedUnitPlan, CheckedUnitPlanOmission, CheckedUnitPlanOmissionStage,
    CheckedUnitScalarControlPlan, CheckedUnitScalarResultBindingPlan,
    CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralArgumentSourcePlan,
    CheckedUnitStructuralCasePlan, CheckedUnitStructuralDomainPlan,
    CheckedUnitStructuralDomainRequirementPlan, CheckedUnitStructuralFieldPlan,
    CheckedUnitStructuralFieldType, CheckedUnitStructuralParameterPlan,
    CheckedUnitStructuralPathSegment, CheckedUnitStructuralResultBindingPlan,
    CheckedUnitStructuralReturnPlan, CheckedUnitStructuralTypePlan, CheckedUnitStructuralTypeShape,
    is_borrowed_view,
};
pub use borrow_lifetimes::{
    FlowBorrowActivationFact, FlowBorrowWeakeningFact, FlowBorrowWeakeningReason,
};
pub use boundaries::FlowBoundaryEdgeFact;
pub use contexts::{FlowConstraintKind, FlowConstraintRef, FlowSemanticContextRef};
pub use control::{
    FlowCallFact, FlowExitFact, FlowExitParameterOrigin, FlowOperatorInvocationFact, FlowStateFact,
    FlowStatementFact, RetiredFlowCall,
};
pub use dynamic_scalar_calls::{
    CheckedDynamicBinding, CheckedDynamicBindingKind, CheckedDynamicDescriptorTransferEdge,
    CheckedDynamicDescriptorTransferPath, CheckedDynamicDescriptorTransferPlan,
    CheckedDynamicDescriptorTransferSource, CheckedDynamicDispatchPlan,
    CheckedDynamicDispatchPlans, CheckedDynamicJoinBranchPlan, CheckedDynamicJoinControlPlan,
    CheckedDynamicRealizationBodyPlan, CheckedDynamicRealizationCallablePlan,
    CheckedDynamicScalarCallOrigin, CheckedDynamicScalarCallPlan, CheckedDynamicScalarHelperPlan,
    CheckedDynamicSelectionPlan, CheckedDynamicStoredDescriptorPlan, CheckedDynamicUnitCallOrigin,
    CheckedDynamicUnitCallPlan, CheckedDynamicUnitContinuationPlan, CheckedDynamicUnitHelperPlan,
    CheckedStructuralCaseFieldStorePlan, CheckedStructuralScalarFieldStoreDestination,
    CheckedStructuralScalarFieldStorePlan, CheckedStructuralScalarFieldStoreValue,
};
pub use invalidations::{FlowInvalidationFact, FlowInvalidationSource};
pub use operator_operand::{FlowOperandReferent, FlowOperatorOperandFact};
pub use ownership::{
    FlowClaimJoinAlternative, FlowClaimJoinAlternativeSource, FlowClaimJoinExit,
    FlowClaimJoinExitKind, FlowClaimJoinReceipt, FlowClaimOutcomeEntryFact,
    FlowClaimOutcomeMapFact, FlowClaimOutcomeSource, FlowOwnedSelectionClaim,
    FlowOwnedSelectionReceipt, FlowOwnedSelectionSource, FlowOwnedSelectionTransfer,
    FlowPermissionEventFact,
};
pub use roots::{
    FlowBorrowLifetimeFacts, FlowBoundaryFacts, FlowContextFacts, FlowControlFacts, FlowFacts,
    FlowInvalidationFacts, FlowOwnershipFacts,
};
pub use semantic_dependencies::{
    CheckedSemanticDependencies, CheckedSemanticDependency, CheckedSemanticDependencyExposure,
    CheckedSemanticDependencyKind,
};
