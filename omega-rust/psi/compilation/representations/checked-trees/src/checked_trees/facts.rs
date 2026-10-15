//! `CheckFacts`, the facts half of `CheckedTrees`, and the fact families that
//! the sibling `borrow`, `flow`, `operators`, `proof` and `values` modules do
//! not define.
//!
//! `CheckFacts` (in `collection`) holds a field for each fact family,
//! including the families those five sibling modules define.
//! `typed-trees-to-checked-trees` assembles it in `build_check_facts`
//! (`src/facts.rs`), and `checked-trees-to-lowered-psi` reads it through
//! `CheckedTrees::facts`. The module list below groups the families defined
//! here.

// `CheckFacts`, the root that holds every fact family.
mod collection;
mod constant_predicate_obligations;
mod constant_recipe_obligations;
mod integer_range_obligations;
pub use constant_predicate_obligations::ConstantPredicateObligation;
pub use integer_range_obligations::{IntegerRangeObligation, IntegerRangeRecipe};
mod contextual_cleanup;
pub mod scalar_predicates;

// Machine contract and interface axes: contract and crash plans with
// operator crash sites, termination, blocking, suspension, synchronous
// invocation, service reach, mutation frames, and qualifications with their
// content projections.
mod blocking;
mod checked_crash_operator_site;
mod content;
mod contract_plans;
mod mutation;
mod qualifications;
mod service_reaches;
mod suspensions;
mod synchronous_invocations;
mod termination;

// Calls and selected machines: fact-call projections, intrinsic calls and
// boundary adapter dispatch, nominal machine uses, generic requirement-call
// specializations, dynamic conformance selections, and the closed
// requirement each provider refines.
mod calls;
mod quotient_operations;
pub use quotient_operations::{CheckedQuotientFormation, CheckedQuotientOperation};
mod dynamic_conformances;
mod nominal_machine_uses;
mod provider_closed_requirements;
mod requirement_call_specializations;

// Carry, domains and placement: carry policy, domain dependency and element
// facts, index compatibility conditions, placed view inputs, each type
// reference's borrow custody and content classification, each state's
// reference-result custody, and frozen expression/place/carrier types.
mod accepted_template_identities;
mod carry;
mod content_classes;
mod domains;
mod index_compatibility;
mod placement;
mod reference_custody;
mod resolved_types;
mod specialization_identities;
pub use accepted_template_identities::{
    CheckedAcceptedTemplateClassification, CheckedAcceptedTemplateIdentities,
    CheckedAcceptedTemplateIdentity,
};
pub use constant_recipe_obligations::{ConstantRecipeObligation, ConstantRecipeRequirement};
pub use specialization_identities::{
    CheckedMachineSpecializationIdentity, CheckedSpecializationIdentities,
};
mod state_locals;
mod stored_integer_writes;
pub use stored_integer_writes::{CheckedStoredIntegerWrite, StoredIntegerWriteSite};

pub use blocking::{BlockingFacts, MachineBlockingFact};
pub use calls::{CheckedBoundaryAdapterDispatch, CheckedFactCallProjection};
pub use carry::{
    CarryFacts, ClaimCarryPolicyFact, DataCarryFact, MachineActivationCarryFact,
    SuspensionCrossingBorrowFrontier, SuspensionCrossingCarryFact, SuspensionCrossingLiveValueFact,
    SuspensionCrossingLoanBacking, SuspensionCrossingLoanFact, SuspensionCrossingLoanSource,
    SuspensionCrossingStorage, SuspensionCrossingValueOrigin, SuspensionValueBorrowFrontier,
    canonical_suspension_crossing_id,
};
pub use checked_crash_operator_site::{CheckedCrashOperatorSite, OperatorSiteLocation};
pub use collection::{CheckFacts, DecidedCallRequires, ProvenAssignmentIndex};
pub use content::{
    ContentIdentityReshuffleFact, ContentPartitionCompositionFact,
    ContentPartitionInputClaimBinding, ContentPartitionPlaceSubstitution,
    ContentPartitionResultRewrite, ContentProjectionFacts, RetainedBorrowCustodyFact,
};
pub use content_classes::{ContentClassFacts, ContentClassRow};
pub use contextual_cleanup::{
    ContextualCleanupApplication, ContextualCleanupDisposition, ContextualCleanupEntryRequirement,
    ContextualCleanupEntryRequirements, ContextualCleanupFacts, ContextualStorageEstablishment,
    StorageWindowInvocation,
};
pub use contract_plans::{
    CheckedCrashCallSite, CheckedCrashSite, CheckedEntryResourceEnvelope,
    CheckedMachineResourceEnvelopes, CheckedResourceAxisAnchor,
    CheckedResourceDerivationObligation, CheckedRuntimeMembershipSubject,
    ClosedFloatRangeRequirement, ClosedIntegerRangeRequirement, ClosedScalarContractValue,
    ClosedScalarValueContractPlan, CrashCallSiteLocation, CrashCause, CrashContractCapsule,
    CrashInterface, CrashPlan, CrashPredicateExpression, CrashPredicateIdentity, CrashRouteBucket,
    CrashRouteBucketId, CrashRouteGuard, CrashSiteLocation, DeclaredCallableContract,
    DeclaredContractClause, DeclaredContractFact, DeclaredContractKind, DeclaredContractParameter,
    DeclaredContractStructure, DeclaredProgressPremise, DeclaredTermination,
    MachineContractCommitment, MachineContractIdentity, MachineContractPlan, MachineContractPlans,
    RealizedMachineContractEnvelope, contract_declarations_commitment, contract_identity,
    contract_plan_by_machine, declared_termination_commitment,
};
pub use domains::{DomainDependencyFact, DomainDependencyPathFact, DomainFacts};
pub use dynamic_conformances::{
    DynamicConformanceFacts, DynamicConformanceRowFact, DynamicConformanceRowSource,
    DynamicSelectionFact,
};
pub use index_compatibility::{
    IndexCompatibilityDischarge, IndexCompatibilityFact, IndexCompatibilityFacts,
};
pub use mutation::{
    MachineMutationFact, MutationFacts, PerformedIntegerWrite, StateWriteFramePlan,
};
pub use nominal_machine_uses::{
    CheckedCallbackResourceReceipt, CheckedMachineContractEnvelopeIdentity,
    CheckedMachineContractRefinement, CheckedNominalMachineUse, NominalMachineUseFacts,
    NominalMachineUseSite,
};
pub use placement::CheckedPlacedViewInput;
pub use provider_closed_requirements::{
    CheckedProviderClosedRequirement, CheckedProviderClosedRequirements,
};
pub use qualifications::{
    CheckedValueQualification, MachineQualifications, QualificationFacts, VacuousQualificationUse,
};
pub use reference_custody::{ReferenceCustodyFacts, ReferenceCustodyRow};
pub use requirement_call_specializations::{
    CheckedRequirementCallMachineSelection, CheckedRequirementCallSpecialization,
    CheckedRequirementCallTypeBinding, RequirementCallSpecializationFacts,
};
pub use resolved_types::{ResolvedCarrierType, ResolvedExpressionType, ResolvedTypeFacts};
pub use service_reaches::{
    CallServiceReachRows, MachineServiceReachRows, ServiceReachFacts, StateServiceReachRows,
};
pub use state_locals::{CheckedStateLocal, StateLocalFacts};
pub use suspensions::{MachineSuspensionFact, SuspensionFacts};
pub use synchronous_invocations::{MachineSynchronousInvocationFact, SynchronousInvocationFacts};
pub use termination::{
    BuildBoundProgressDemand, CheckedMachineRankCorrespondence, CheckedProofRankingRelation,
    CheckedProofRecursiveCallSite, CheckedProofRecursiveComponent, CheckedProofRecursiveEdge,
    CheckedProofRecursiveMember, CheckedProofRecursiveTransitionLane, CheckedRankArrivalEdge,
    CheckedRankArrivalPlan, CheckedRankBodyJudgment, CheckedRankCoordinate,
    CheckedRankCoordinateBinding, CheckedRankCoordinateHandle, CheckedRankCoordinateTransport,
    CheckedRankLocalCapture, CheckedRankPremiseRegime, CheckedRankReceivingCoordinate,
    CheckedRankValueDerivation, CheckedRuntimeNaturalState, CheckedRuntimeRankComponent,
    CheckedRuntimeRankEdge, CheckedRuntimeRankMember, CheckedStateRankCorrespondence,
    MachineBuildBoundProgressDemands, MachineTerminationFact, ProgressDemandCallSite,
    TerminationFacts,
};
