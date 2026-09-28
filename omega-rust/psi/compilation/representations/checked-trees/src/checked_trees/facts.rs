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
// specializations, and dynamic conformance selections.
mod calls;
mod dynamic_conformances;
mod nominal_machine_uses;
mod requirement_call_specializations;

// Carry, domains and placement: carry policy, domain dependency and element
// facts, index compatibility conditions, and placed view inputs.
mod carry;
mod domains;
mod index_compatibility;
mod placement;

pub use blocking::{BlockingFacts, MachineBlockingFact};
pub use calls::{
    CheckedBoundaryAdapterDispatch, CheckedFactCallProjection, CheckedIntrinsicCallFact,
};
pub use carry::{
    CarryFacts, ClaimCarryPolicyFact, ContainedMachineFieldFact, ContainedMachineTargetFact,
    DataCarryFact, MachineActivationCarryFact, MachineCarryTopologyFact,
    SuspensionCrossingCarryFact, SuspensionCrossingLiveValueFact, SuspensionCrossingStorage,
    SuspensionCrossingValueOrigin, canonical_suspension_crossing_id,
};
pub use checked_crash_operator_site::{CheckedCrashOperatorSite, OperatorSiteLocation};
pub use collection::CheckFacts;
pub use content::{
    ContentIdentityReshuffleFact, ContentPartitionCompositionFact,
    ContentPartitionInputClaimBinding, ContentPartitionPlaceSubstitution,
    ContentPartitionResultRewrite, ContentProjectionFacts, RetainedBorrowCustodyFact,
};
pub use contract_plans::{
    CheckedCrashCallSite, CheckedCrashSite, CheckedEntryResourceEnvelope,
    CheckedMachineResourceEnvelopes, CheckedResourceAxisAnchor,
    CheckedResourceDerivationObligation, ClosedFloatRangeRequirement,
    ClosedIntegerRangeRequirement, ClosedScalarContractValue, ClosedScalarValueContractPlan,
    CrashCallSiteLocation, CrashCause, CrashContractCapsule, CrashInterface, CrashPlan,
    CrashPredicateExpression, CrashPredicateIdentity, CrashRouteBucket, CrashRouteBucketId,
    CrashRouteGuard, CrashSiteLocation, MachineContractCommitment, MachineContractIdentity,
    MachineContractPlan, MachineContractPlans, RealizedMachineContractEnvelope, contract_identity,
    contract_report_fingerprint,
};
pub use domains::{DomainDependencyFact, DomainDependencyPathFact, DomainElementFact, DomainFacts};
pub use dynamic_conformances::{
    DynamicConformanceBindingFact, DynamicConformanceBindingFacts, DynamicConformanceCandidateFact,
    DynamicConformanceFacts, DynamicConformanceRowFact, DynamicConformanceRowSource,
    DynamicConformanceSelectionFact, DynamicDescriptorStorageFact,
};
pub use index_compatibility::{
    IndexCompatibilityDischarge, IndexCompatibilityFact, IndexCompatibilityFacts,
};
pub use mutation::{MachineMutationFact, MutationFacts, StateWriteFramePlan};
pub use nominal_machine_uses::{
    CheckedCallbackPlacementIdentity, CheckedCallbackResourceReceipt,
    CheckedMachineContractEnvelopeIdentity, CheckedMachineContractRefinement,
    CheckedNominalMachineUse, NominalMachineUseFacts, NominalMachineUseSite,
};
pub use placement::CheckedPlacedViewInput;
pub use qualifications::{MachineQualifications, QualificationFacts, VacuousQualificationUse};
pub use requirement_call_specializations::{
    CheckedRequirementCallMachineSelection, CheckedRequirementCallSpecialization,
    CheckedRequirementCallTypeBinding, RequirementCallSpecializationFacts,
};
pub use service_reaches::{
    CallServiceReachRows, MachineServiceReachRows, ServiceReachFacts, StateServiceReachRows,
};
pub use suspensions::{MachineSuspensionFact, SuspensionFacts};
pub use synchronous_invocations::{MachineSynchronousInvocationFact, SynchronousInvocationFacts};
pub use termination::{
    BuildBoundProgressDemand, CheckedProofRankingRelation, CheckedProofRecursiveCallSite,
    CheckedProofRecursiveComponent, CheckedProofRecursiveEdge, CheckedProofRecursiveMember,
    CheckedProofRecursiveTransitionLane, MachineBuildBoundProgressDemands, MachineTerminationFact,
    ProgressDemandCallSite, TerminationFacts,
};
