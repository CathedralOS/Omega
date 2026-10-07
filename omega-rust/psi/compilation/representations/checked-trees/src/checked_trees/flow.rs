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

// Terminal plans the checker publishes for lowering: `terminal` holds the
// plan carriers for each terminal machine kind, and `store_plans` the
// structural field store plans the Unit-effect plans use.
mod store_plans;
mod terminal;

// Lookup methods on `FlowFacts`.
mod queries;

pub use self::terminal::{
    CheckedAffineConstructionElementPlan, CheckedAtomicAccessPlan, CheckedAtomicEvent,
    CheckedAtomicReadModifyWrite, CheckedBoundaryMachinePlan, CheckedBoundaryMachineResultPlan,
    CheckedBoundaryScalarGuarantee, CheckedBoundaryScalarReturnMachinePlan,
    CheckedBoundaryScalarReturnPlans, CheckedByteSequenceCarrier,
    CheckedByteSequenceStoreDestination, CheckedByteSequenceStorePlan, CheckedByteSequenceValue,
    CheckedByteSequenceValuePlan, CheckedByteSequenceWritePlan, CheckedCallScalarArgument,
    CheckedClaimFreeAffineStructuralReturnMachinePlan, CheckedClosedSumCaseOutcomePlan,
    CheckedClosedSumCasePlan, CheckedClosedSumPayloadTransferPlan, CheckedClosedSumSubjectKind,
    CheckedComposedUnitControlMachinePlan, CheckedComposedUnitControlStatePlan,
    CheckedComposedUnitControlTerminatorPlan, CheckedConditionalReturnArm,
    CheckedControlResultPlan, CheckedElementViewWritePlan, CheckedFusedServiceErasureReceipt,
    CheckedFusedServiceParameterReceipt, CheckedGuardedJumpArmPlan, CheckedGuardedJumpFallbackPlan,
    CheckedGuardedJumpPlan, CheckedGuardedJumpReturnPlan, CheckedNaturalRankMeasure,
    CheckedNominalAffineUnitCleanupMachinePlan, CheckedNominalAffineUnitCleanupPlans,
    CheckedNormalCleanupAction, CheckedNormalExitCleanupPlan, CheckedOperandSequence,
    CheckedPartialAffineUnitCleanupMachinePlan, CheckedPartialAffineUnitCleanupPlans,
    CheckedPayloadlessGuardedCallEvidencePlan, CheckedPayloadlessGuardedCallEvidenceUsePlan,
    CheckedPayloadlessGuardedCallReturnMachinePlan, CheckedProviderAttachmentRequirementPlan,
    CheckedReferenceResultSourcePlan, CheckedRepresentationViewStorePlan, CheckedReturnPlan,
    CheckedRuntimeIndex, CheckedScalarBinding, CheckedScalarBindingDestination,
    CheckedScalarBindingValue, CheckedScalarBranchDestination, CheckedScalarCaseFieldPlan,
    CheckedScalarGraphPlans, CheckedScalarGuardedExit, CheckedScalarGuardedTail,
    CheckedScalarMachineGraph, CheckedScalarParameterStorage, CheckedScalarPrimitiveLocalPlan,
    CheckedScalarReturnPlan, CheckedScalarStateGraph, CheckedScalarStateTerminator,
    CheckedScalarSuccessor, CheckedStateNaturalRank, CheckedStructuralAccess,
    CheckedStructuralBooleanConvergencePlan, CheckedStructuralByteSequenceFieldByteStorePlan,
    CheckedStructuralCallCustodyPlan, CheckedStructuralCallReturnPlans,
    CheckedStructuralCaseReturnPlan, CheckedStructuralControlCleanupPlans,
    CheckedStructuralControlEdgeCleanupPlan, CheckedStructuralControlStateCleanupPlan,
    CheckedStructuralControlSuccessorPlan, CheckedStructuralControlTransferPlan,
    CheckedStructuralControlTransferSourcePlan, CheckedStructuralPathQualification,
    CheckedStructuralRankedArgumentPlan, CheckedStructuralRankedGuardPlan,
    CheckedStructuralRankedSccEdgePlan, CheckedStructuralRankedSccPlan,
    CheckedStructuralResultPlan, CheckedStructuralReturnMachinePlan, CheckedStructuralReturnPlans,
    CheckedStructuralReturnedClaimTransferPlan, CheckedStructuralScalarArgumentPlan,
    CheckedStructuralScalarArgumentSourcePlan, CheckedStructuralScalarIntegerBoundKind,
    CheckedStructuralScalarIntegerBoundPlan, CheckedStructuralScalarIntegerBoundRequirementPlan,
    CheckedStructuralScalarParameterPlan, CheckedStructuralScalarReturnMachinePlan,
    CheckedStructuralScalarReturnPlans, CheckedStructuralUnitControlMachinePlan,
    CheckedStructuralUnitControlPlans, CheckedStructuralUnitControlStatePlan,
    CheckedStructuralUnitControlTerminatorPlan, CheckedStructuralValueCall,
    CheckedTerminalDebugPlans, CheckedTerminalMachineDebugPlan, CheckedTerminalMachineSelection,
    CheckedTerminalMachineSelections, CheckedTerminalSignatureEligibility,
    CheckedTerminalStateDebugPlan, CheckedTraitOperatorScalarReturnMachinePlan,
    CheckedTrivialAffineStructuralLocalPlan, CheckedUnitCallCoordinate, CheckedUnitCallParts,
    CheckedUnitCallPlan, CheckedUnitCallResult, CheckedUnitCallRoute, CheckedUnitCallTarget,
    CheckedUnitCallTransfers, CheckedUnitCallableEntry, CheckedUnitClaimTransferPlan,
    CheckedUnitCleanupBoundary, CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan,
    CheckedUnitEffectPlans, CheckedUnitEntryClaimPlan,
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
    FlowCallDispatchRole, FlowCallFact, FlowCallResult, FlowCallSite, FlowExitFact,
    FlowExitParameterOrigin, FlowOperatorInvocationFact, FlowStateFact, FlowStateOutgoingFact,
    FlowStatementFact, RetiredFlowCall,
};
pub use invalidations::{FlowInvalidationFact, FlowInvalidationSource};
pub use operand::{FlowOperandChild, FlowOperandFact, FlowOperandReferent};
pub use ownership::{
    FlowClaimJoinAlternative, FlowClaimJoinAlternativeSource, FlowClaimJoinExit,
    FlowClaimJoinExitKind, FlowClaimJoinReceipt, FlowClaimOutcomeEntryFact,
    FlowClaimOutcomeMapFact, FlowClaimOutcomeSource, FlowOwnedSelectionClaim,
    FlowOwnedSelectionReceipt, FlowOwnedSelectionSource, FlowOwnedSelectionTransfer,
    FlowPermissionEvaluation, FlowPermissionEventFact,
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
    CheckedStoreRoot, CheckedStoreValue, CheckedStructuralCaseFieldStorePlan,
    CheckedStructuralCaseLiteralFieldStorePlan, CheckedStructuralScalarFieldStorePlan,
};
