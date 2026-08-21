//! Small total proof and evidence checker for terminal Psi.
//!
//! The kernel never searches. It re-decides closed primitive judgments, checks
//! explicit proof nodes, or validates an admission against an obligation site
//! and installation-profile decision. Executable terminal-Psi lowering will
//! reconstruct obligations; a proof bundle cannot choose their class.

#![forbid(unsafe_code)]

mod evidence;
mod integer_affine;
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
    CheckedIntegerAffineForm, IntegerAffineWitness, IntegerAffineWitnessError,
    check_integer_affine_witness,
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
