//! Non-executable proof vocabulary and machine contracts.

mod content;
mod contracts;
mod declarations;
mod operation_crash_contracts;
mod outputs;
mod quotient;
mod recursion;
mod scalar_block_invariants;
mod values;

pub use content::{
    ClaimContentProjection, ContentEntryClaim, ContentIdentityReshuffle,
    ContentPartitionComposition, ContentPlaceSubstitution,
};
pub use contracts::{
    ContractClause, CrashPredicateTerm, CrashRouteBucket, CrashRouteGuard, MachineContract,
    OutcomeSpecificCallEvidence, OutcomeSpecificCallEvidenceValidity,
    OutcomeSpecificCallResultSubstitution, OutcomeSpecificEnsure, OutcomeSpecificEvidence,
    OutcomeSpecificEvidenceUse, OutcomeSpecificGuard,
};
pub use declarations::{
    EvidenceContractLane, EvidenceContractLaneKind, EvidenceInterfaceIdentity,
    EvidenceProjectionIdentity, EvidenceRequirementIdentity, EvidenceTermDeclaration,
    PropositionApplicationIdentity, PropositionBinderArgumentIdentity,
    PropositionBinderArgumentKind, PropositionBinderDeclaration, PropositionBinderKind,
    PropositionDeclaration, PropositionEvidence,
};
pub use operation_crash_contracts::TerminalOperationCrashContract;
pub use outputs::{
    ProofOutput, ProofOutputCall, ProofOutputEvidenceArgument, ProofOutputRuntimeCall,
    ProofOutputRuntimeResult, StaticRequirementDispatch,
};
pub use quotient::{
    QuotientCorrespondenceIdentity, RetainedQuotientCorrespondence,
    retain_non_executable_quotient_correspondence,
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
