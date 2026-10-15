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
mod operand;
mod ownership;
mod semantic_dependencies;
mod state_rows;
pub use state_rows::FlowStateTable;

// Terminal plans the checker publishes for lowering: `terminal` holds the
// plan carriers for each terminal machine kind, and `store_plans` the
// one store plan the Unit-effect plans use.
mod store_plans;
mod terminal;

// Lookup methods on `FlowFacts`.
mod queries;

pub use self::terminal::{
    CheckedAffineConstructionElementPlan, CheckedAtomicAccessPlan, CheckedAtomicEvent,
    CheckedAtomicReadModifyWrite, CheckedBindingFieldReceipt, CheckedBindingParameterReceipt,
    CheckedBoundaryMachinePlan, CheckedBoundaryMachineResultPlan, CheckedBoundaryScalarGuarantee,
    CheckedByteSequenceCarrier, CheckedByteSequenceValue, CheckedByteSequenceValuePlan,
    CheckedCallScalarArgument, CheckedCleanupCaseCondition, CheckedCleanupControlPlans,
    CheckedClosedSumCaseOutcomePlan, CheckedClosedSumCasePlan, CheckedClosedSumPayloadTransferPlan,
    CheckedClosedSumSubjectKind, CheckedComposedUnitControlMachinePlan,
    CheckedComposedUnitControlStatePlan, CheckedComposedUnitControlTerminatorPlan,
    CheckedConditionalReturnArm, CheckedControlResultPlan, CheckedDynamicClaimPath,
    CheckedDynamicSlotPlan, CheckedExtent, CheckedGuardedExitPlans, CheckedGuardedJumpArmPlan,
    CheckedGuardedJumpFallbackPlan, CheckedGuardedJumpPlan, CheckedGuardedJumpReturnPlan,
    CheckedInitializedLocal, CheckedMemberIdentity, CheckedNaturalRankMeasure,
    CheckedNominalAffineUnitCleanupMachinePlan, CheckedNominalAffineUnitCleanupPlans,
    CheckedNormalCleanupAction, CheckedNormalExitCleanupPlan, CheckedOperandSequence,
    CheckedPartialAffineUnitCleanupMachinePlan, CheckedPartialAffineUnitCleanupPlans,
    CheckedPlaceLoanPlan, CheckedPlaceLoanRoot, CheckedProviderAttachmentRequirementPlan,
    CheckedRankEdgeCoordinate, CheckedReferenceOriginProjection, CheckedReferenceOriginSourcePlan,
    CheckedReferenceResultSourcePlan, CheckedReferenceStorage, CheckedReturnedReferencePlan,
    CheckedRuntimeIndex, CheckedScalarBranchDestination, CheckedScalarCaseFieldPlan,
    CheckedScalarGuardedExit, CheckedScalarGuardedTail, CheckedScalarReturnPlan,
    CheckedScalarStateTerminator, CheckedStateContractRequirement, CheckedStateNaturalRank,
    CheckedStructuralAccess, CheckedStructuralCallCustodyPlan,
    CheckedStructuralControlCleanupPlans, CheckedStructuralControlEdgeCleanupPlan,
    CheckedStructuralControlStateCleanupPlan, CheckedStructuralControlSuccessorPlan,
    CheckedStructuralControlTransferPlan, CheckedStructuralControlTransferSourcePlan,
    CheckedStructuralPathQualification, CheckedStructuralResultClaimPlan,
    CheckedStructuralResultPlan, CheckedStructuralReturnCompletion,
    CheckedStructuralScalarArgumentPlan, CheckedStructuralScalarArgumentSourcePlan,
    CheckedStructuralScalarParameterPlan, CheckedStructuralValueCall, CheckedTerminalDebugPlans,
    CheckedTerminalMachineDebugPlan, CheckedTerminalMachineSelection,
    CheckedTerminalMachineSelections, CheckedTerminalSignatureEligibility,
    CheckedTerminalStateDebugPlan, CheckedTrivialAffineStructuralLocalPlan,
    CheckedUnitCallCoordinate, CheckedUnitCallParts, CheckedUnitCallPlan, CheckedUnitCallResult,
    CheckedUnitCallRoute, CheckedUnitCallTarget, CheckedUnitCallTransfers,
    CheckedUnitCallableEntry, CheckedUnitClaimTransferPlan, CheckedUnitCleanupBoundary,
    CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan, CheckedUnitEffectPlans,
    CheckedUnitEntryClaimPlan, CheckedUnitLocalBindingPlan,
    CheckedUnitNominalAffineCallerRequirementPlan, CheckedUnitNominalAffineCleanupPlan,
    CheckedUnitNominalAffineCleanupRequirementPlan, CheckedUnitParameterStoragePlan,
    CheckedUnitPartialAffineDiscardPlan, CheckedUnitPlan, CheckedUnitPlanGap,
    CheckedUnitPlanGapReason, CheckedUnitRecordStorage, CheckedUnitScalarControlPlan,
    CheckedUnitScalarResultBindingPlan, CheckedUnitSelectedReferenceRetirementPlan,
    CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralArgumentSourcePlan,
    CheckedUnitStructuralCasePlan, CheckedUnitStructuralDomainPlan,
    CheckedUnitStructuralDomainRequirementPlan, CheckedUnitStructuralFieldPlan,
    CheckedUnitStructuralFieldType, CheckedUnitStructuralParameterPlan,
    CheckedUnitStructuralPathSegment, CheckedUnitStructuralResultBindingPlan,
    CheckedUnitStructuralReturnPlan, CheckedUnitStructuralTypePlan, CheckedUnitStructuralTypeShape,
    checked_structural_type_by_identity, is_borrowed_view, is_bounded_nominal_cleanup_record,
    is_partial_affine_field_type,
};
pub use borrow_lifetimes::{
    FlowBorrowActivationFact, FlowBorrowWeakeningFact, FlowBorrowWeakeningReason,
};
pub use boundaries::FlowBoundaryEdgeFact;
pub use contexts::{FlowConstraintKind, FlowConstraintRef, FlowSemanticContextRef};
pub use control::{
    FlowCallDispatchRole, FlowCallFact, FlowCallResult, FlowCallSite, FlowExitFact,
    FlowExitParameterOrigin, FlowOperatorInvocationFact, FlowStateFact, FlowStateOutgoingFact,
    FlowStatementFact, RetiredFlowCall,
};
pub use invalidations::{FlowInvalidationFact, FlowInvalidationSource};
pub use operand::{FlowCaseAccessFact, FlowOperandChild, FlowOperandFact, FlowOperandReferent};
pub use ownership::{
    FlowBorrowClaimFact, FlowClaimJoinAlternative, FlowClaimJoinAlternativeSource,
    FlowClaimJoinExit, FlowClaimJoinExitKind, FlowClaimJoinReceipt, FlowClaimOutcomeEntryFact,
    FlowClaimOutcomeMapFact, FlowClaimOutcomeSource, FlowLoanCompletionFact,
    FlowOwnedSelectionClaim, FlowOwnedSelectionReceipt, FlowOwnedSelectionSource,
    FlowOwnedSelectionTransfer, FlowPermissionEvaluation, FlowPermissionEventFact,
};
pub use roots::{
    FlowBorrowLifetimeFacts, FlowBoundaryFacts, FlowContextFacts, FlowControlFacts, FlowFacts,
    FlowInvalidationFacts, FlowOwnershipFacts,
};
pub use semantic_dependencies::{
    CheckedSemanticDependencies, CheckedSemanticDependency, CheckedSemanticDependencyExposure,
    CheckedSemanticDependencyKind,
};
pub use store_plans::{
    CheckedCaseLiteralValuePlan, CheckedIndexedWriteDecisions, CheckedPrimitiveReceivingRange,
    CheckedScalarStoreValue, CheckedStoreDestination, CheckedStoreLeaf, CheckedStorePlan,
    CheckedStoreRoot, CheckedStoreValue,
};
