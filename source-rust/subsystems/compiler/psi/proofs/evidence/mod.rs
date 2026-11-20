/*
Mathematical proof declarations, theorem obligations (`TheoremObligation`, `ObligationPhase`),
callable contracts, integer certificates, and proof-term representations in `TerminalModule`.
*/

pub(crate) mod content;
pub(crate) mod contracts;
pub(crate) mod declarations;
pub(crate) mod operation_crash_contracts;
pub(crate) mod outputs;
pub(crate) mod recursion;
pub(crate) mod scalar_block_invariants;
pub(crate) mod values;

pub use content::{
    ClaimContentProjection, ContentBackingAddressTransport, ContentBodyProjectionSubstitution,
    ContentBoundaryBodyRequirement, ContentClaimAddress, ContentClaimProjection, ContentEntryClaim,
    ContentIdentityReshuffle, ContentLineageStep, ContentPartitionComposition,
    ContentPlaceSubstitution, ContentTransformationCustody, ContentTransformationDerivation,
};
pub use contracts::{
    ContractClause, CrashPredicateTerm, CrashRouteBucket, CrashRouteGuard, MachineContract,
    OutcomeSpecificCallArgumentSubstitution, OutcomeSpecificCallEvidence,
    OutcomeSpecificCallEvidenceValidity, OutcomeSpecificCallResultSubstitution,
    OutcomeSpecificEnsure, OutcomeSpecificEvidence, OutcomeSpecificEvidenceUse,
    OutcomeSpecificGuard, PublishedWriteFrame, RuntimeDomainMembership,
};
pub use declarations::{
    EvidenceArgumentSubject, EvidenceCallableOwner, EvidenceContractLane, EvidenceContractLaneKind,
    EvidenceInterfaceIdentity, EvidenceProjectionIdentity, EvidenceRequirementIdentity,
    EvidenceTermDeclaration, PropositionApplicationIdentity, PropositionBinderArgumentIdentity,
    PropositionBinderArgumentKind, PropositionBinderDeclaration, PropositionBinderKind,
    PropositionDeclaration, PropositionEvidence, ScalarPredicateDefinition,
};
pub use operation_crash_contracts::TerminalOperationCrashContract;
pub use outputs::{
    EvidenceCallableTarget, ProofOutput, ProofOutputCall, ProofOutputEvidenceArgument,
    ProofOutputRuntimeCall, ProofOutputRuntimeResult, StaticRequirementDispatch,
};
pub use recursion::{
    TerminalProofRankingRelation, TerminalProofRecursiveCallSite, TerminalProofRecursiveComponent,
    TerminalProofRecursiveEdge, TerminalProofRecursiveField, TerminalProofRecursiveMember,
    TerminalProofRecursiveTransitionLane, TerminalProofRecursiveType,
};
pub use scalar_block_invariants::{ScalarBlockInvariant, ScalarBlockInvariantArrival};
pub use values::{
    DirectBlockFloatParameter, DirectCallFloatResult, DirectMachineFloatParameter,
    DirectMachineFloatResult, DirectOperationFloatResult, DirectStructuralFloatLeaf,
    FloatMeaningEqualityProposition, FloatMeaningProjection, FloatMeaningProjectionOperation,
    FloatMeaningSource, FloatProjectionContractIdentity, FloatProjectionInput,
    FloatProjectionInputId, FloatSemanticApplication, FloatSemanticApplicationOperand,
    FloatSemanticContractIdentity, ProofOnlyValueType, ProofPropositionId, ProofValueDeclaration,
    ProofValueId, float_meaning_equality_proposition_id,
};
