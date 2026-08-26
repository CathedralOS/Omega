//! Product-local proof and admission checker for terminal Psi.
//!
//! The checker never searches. It re-decides closed primitive judgments,
//! checks explicit proof nodes, or validates an admission against an obligation
//! site and installation-profile decision. Executable terminal-Psi lowering
//! reconstructs obligations; a proof bundle cannot choose their class. This
//! crate is distinct from the generic derivation kernel under
//! `bootstrap/assurance/proof-kernel/`.

#![forbid(unsafe_code)]

mod evidence;
mod integer_affine;
mod integer_cast;
mod integer_forbidden_root;
mod integer_shift;
mod kernel;
mod normalization;
mod proof;
mod recursion;

pub use evidence::{
    AcceptedFact, AcceptedFactRoute, AdmissionAcceptance, AdmissionEvidence, AdmissionKind,
    AdmissionProfile, AuthorizedAdmission, CertificateEnvelope, EvidenceError, EvidenceRoute,
    Obligation, ObligationClass, ProofSystemMarker, verify_obligation,
};
pub use integer_affine::{
    CheckedIntegerAffineForm, IntegerAffineBoundConversionError, IntegerAffineWitness,
    IntegerAffineWitnessError, check_integer_affine_bound_conversion, check_integer_affine_witness,
};
pub use integer_cast::{
    CheckedIntegerCastChain, IntegerCastBoundConversionError, IntegerCastChainWitness,
    IntegerCastChainWitnessError, check_integer_cast_bound_conversion,
    check_integer_cast_chain_witness,
};
pub use integer_forbidden_root::{
    CheckedIntegerCorrelatedForbiddenRoots, CorrelatedAffineBranch, CorrelatedAffineBranchWitness,
    CorrelatedAffineStepWitness, IntegerCorrelatedForbiddenRootWitness,
    IntegerCorrelatedForbiddenRootWitnessError, check_integer_correlated_forbidden_root_witness,
};
pub use integer_shift::{
    CheckedIntegerShiftChain, CheckedIntegerShiftStep, IntegerShiftChainWitness,
    IntegerShiftChainWitnessError, IntegerShiftDirection, IntegerShiftStepWitness,
    check_integer_shift_chain_witness,
};
pub use kernel::{KernelError, PrimitiveJudgment, decide_primitive};
pub use normalization::{
    NormalizationAcceptance, NormalizationCertificate, NormalizationError,
    NormalizationLawAcceptance, NormalizationLawCertificate, NormalizationLawObligation,
    NormalizationObligation, verify_normalization,
};
pub use proof::{
    AcceptedPremise, AcceptedProofRule, CertificateAcceptance, ProofError, ProofNode, ProofRule,
    accept_certificate, check_certificate,
};
pub use recursion::{
    CertificateObligation, RecursiveComponentAcceptance, RecursiveComponentCertificate,
    RecursiveComponentError, RecursiveComponentObligation, RecursiveEdgeCertificate,
    RecursiveEdgeObligation, verify_recursive_component,
};
