//! Proof facts: the proof obligations, contract evidence and proof-only
//! declarations checking retains.
//!
//! `ProofFacts` (in `roots`) holds them and is stored as `CheckFacts::proof`.
//! `typed-trees-to-checked-trees` builds it in `build_check_facts`
//! (`src/facts.rs`), starting from `build_proof_facts`
//! (`src/proof.rs`); `checked-trees-to-lowered-psi` reads it through
//! `checked.facts.proof`.
//!
//! `obligations` defines the proof obligation rows; `contracts` the contract
//! proof facts, evidence terms and arguments, proof-output calls and
//! outcome-specific guarantees; `float_meaning` the proof-only float meaning projections and semantic
//! applications; `propositions` the nominal proposition vocabulary; and
//! `mathematical` the checked mathematical declarations. `lemmas` defines
//! `LemmaFacts` (length, bounds and window lemmas and quantified range
//! facts), which is not a field of `ProofFacts`.

mod contracts;
mod float_meaning;
mod lemmas;
mod mathematical;
mod obligations;
mod propositions;
mod roots;

pub use contracts::{
    BoundaryQualificationAuthorization, CheckedEvidenceTerm, ContractCallFact,
    ContractEvidenceArgument, ContractExitFact, ContractExpressionEvidenceArgumentFact,
    ContractExpressionEvidenceCallFact, ContractExpressionStaticConformanceApplicationFact,
    ContractOperatorUseFact, ContractProofFact, ContractProofFactKind, ContractProofFactOwner,
    ContractProofFactRef, EvidenceAssignmentSource, EvidenceForwardingFact, InheritedContractScope,
    OutcomeOccurrenceRoot, OutcomeReferencedOccurrence, OutcomeSpecificArmFact,
    OutcomeSpecificArmRowFact, OutcomeSpecificEvidenceInterfaceScopeFact,
    OutcomeSpecificGuaranteeFact, OutcomeSpecificValidityFact, ProofOutputCallFact,
    ProofOutputEvidenceArgumentFact, ProofOutputFact, ProofOutputRuntimeCallFact,
    StaticRequirementDispatchFact,
};
pub use float_meaning::{
    CheckedDirectBlockFloatParameter, CheckedDirectCallFloatResult,
    CheckedDirectMachineFloatParameter, CheckedDirectMachineFloatResult,
    CheckedDirectOperationFloatResult, CheckedDirectStructuralFloatLeaf,
    CheckedFloatMeaningEqualityProposition, CheckedFloatMeaningProjection,
    CheckedFloatMeaningProjectionError, CheckedFloatMeaningProjectionOccurrence,
    CheckedFloatMeaningProjectionOccurrenceId, CheckedFloatProjectionContractIdentity,
    CheckedFloatProjectionInput, CheckedFloatProjectionInputId, CheckedFloatProjectionSource,
    CheckedFloatSemanticApplication, CheckedFloatSemanticApplicationError,
    CheckedFloatSemanticApplicationOperand, CheckedFloatUseSite, CheckedProofOnlyValueType,
    CheckedProofPropositionId, CheckedProofValueDeclaration, CheckedProofValueId,
};
pub use lemmas::{
    LemmaFacts, ProofLemmaFact, ProofLemmaKind, QuantifiedBoundFact, QuantifiedRangeFact,
};
pub use mathematical::{
    CheckedMathematicalBinder, CheckedMathematicalBinderKind, CheckedMathematicalBody,
    CheckedMathematicalDeclaration, CheckedMathematicalParameter, CheckedMathematicalSignature,
};
pub use obligations::{
    CheckedIntegerAssignmentDecision, ProofFactKind, ProofObligationFact, ProofObligationOwner,
};
pub use propositions::{
    CheckedEvidenceInterfaceIdentity, CheckedEvidenceProjection,
    CheckedEvidenceRequirementIdentity, CheckedPropositionApplication, CheckedPropositionBinder,
    CheckedPropositionBinderArgument, CheckedPropositionBinderArgumentKind,
    CheckedPropositionBinderKind, CheckedPropositionDeclaration, CheckedPropositionEvidence,
    CheckedPropositionVocabulary,
};
pub use roots::ProofFacts;
