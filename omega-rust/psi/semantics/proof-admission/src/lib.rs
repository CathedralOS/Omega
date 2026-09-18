//! Product-local proof and admission checker for terminal Psi.
//!
//! The checker never searches. It re-decides closed primitive judgments,
//! checks explicit proof nodes, or validates an admission against an obligation
//! site and installation-profile decision. Executable terminal-Psi lowering
//! reconstructs obligations; a proof bundle cannot choose their class. This
//! crate is distinct from the Gamma-written bootstrap derivation checker.
//! The crate also owns the common mathematical core: the shared dependent
//! term model and its independent checker for the reference predicative
//! calculus with stratified relevant and strict universes.

#![forbid(unsafe_code)]
//!
//! `proof.rs` checks explicit proof nodes — and, on every acceptance,
//! denotes the certificate into the `mathematical_core.rs` term model so
//! the kernel re-decides the judgment — while `kernel.rs` re-decides closed
//! primitive judgments over that model and `predicate_denotation.rs`;
//! `integer_rules/` holds the checked integer normalizations and
//! `admission/` the evidence routes.

mod admission;
mod integer_rules;
mod kernel;
mod mathematical_core;
mod predicate_denotation;
mod proof;

pub use admission::evidence::{
    AcceptedFact, AcceptedFactRoute, AdmissionAcceptance, AdmissionEvidence, AdmissionKind,
    AdmissionProfile, AuthorizedAdmission, CertificateEnvelope, EvidenceError, EvidenceRoute,
    Obligation, ObligationClass, ProofSystemMarker, verify_obligation,
    verify_obligation_with_machine_parameters,
};
pub use admission::normalization::{
    NormalizationAcceptance, NormalizationCertificate, NormalizationError,
    NormalizationLawAcceptance, NormalizationLawCertificate, NormalizationLawObligation,
    NormalizationObligation, verify_normalization,
};
pub use admission::recursion::{
    CertificateObligation, RecursiveComponentAcceptance, RecursiveComponentCertificate,
    RecursiveComponentError, RecursiveComponentObligation, RecursiveEdgeCertificate,
    RecursiveEdgeObligation, verify_recursive_component,
    verify_recursive_component_with_machine_parameters,
};
pub use integer_rules::closed_integer::{
    ClosedIntegerEvaluationError, ClosedIntegerEvaluator, compare_integer_math_terms,
};
pub use integer_rules::integer_affine::{
    CheckedIntegerAffineForm, IntegerAffineBoundConversionError, IntegerAffineWitness,
    IntegerAffineWitnessError, check_integer_affine_bound_conversion, check_integer_affine_witness,
    integer_affine_truth_bounds, integer_affine_wrapping_evidence, map_integer_affine_bound,
};
pub use integer_rules::integer_cast::{
    CheckedIntegerCastChain, IntegerCastBoundConversionError, IntegerCastChainWitness,
    IntegerCastChainWitnessError, check_integer_cast_bound_conversion,
    check_integer_cast_chain_witness, integer_cast_truth_bounds,
};
pub use integer_rules::integer_forbidden_root::{
    CheckedIntegerCorrelatedForbiddenRoots, CorrelatedAffineBranch, CorrelatedAffineBranchWitness,
    CorrelatedAffineStepWitness, IntegerCorrelatedForbiddenRootConversionError,
    IntegerCorrelatedForbiddenRootWitness, IntegerCorrelatedForbiddenRootWitnessError,
    check_integer_correlated_forbidden_root_conversion,
    check_integer_correlated_forbidden_root_witness,
};
pub use integer_rules::integer_shift::{
    CheckedIntegerShiftChain, CheckedIntegerShiftStep, IntegerShiftChainWitness,
    IntegerShiftChainWitnessError, IntegerShiftDirection, IntegerShiftStepWitness,
    check_integer_shift_chain_witness,
};
pub use kernel::{KernelError, PrimitiveJudgment, decide_primitive};
pub use mathematical_core::{
    BoundedDenotation, BoundedDenotationError, Budget, Context, CoreError,
    DEFAULT_CONVERSION_STEPS, Declaration, INDEXED_AT, INDEXED_IND, INDEXED_PACK, INDEXED_SUP,
    INDEXED_W, IndexedFamily, Level, MathematicalCertificate, QUOTIENT, QUOTIENT_BETA,
    QUOTIENT_BOX_PROP, QUOTIENT_COVERAGE, QUOTIENT_EFFECTIVE, QUOTIENT_ELIM, QUOTIENT_ID_CANCEL,
    QUOTIENT_ID_SYM, QUOTIENT_ID_TRANS, QUOTIENT_IND_PROP, QUOTIENT_IS_SET, QUOTIENT_LIFT,
    QUOTIENT_LIFT_PRECONDITION, QUOTIENT_PROJECT, QUOTIENT_PROP_IS_SET, QUOTIENT_SET,
    QUOTIENT_SOUND, QUOTIENT_TRANSPORT, QUOTIENT_TRANSPORT_CONST, QUOTIENT_UNIQUE, QuotientFamily,
    Signature, Sort, Term, TermArena, TermHandle, assumption_closure,
    certificate_assumption_closure, check_signature, check_type, convertible,
    denote_bounded_certificate, denote_bounded_certificate_with_machine_parameters,
    identity_substitution, indexed_correctness, indexed_scheme, infer_sort, infer_type,
    instantiate_levels, judgment_assumption_closure, quotient_scheme, shift, substitute,
    verify_bounded_certificate, verify_bounded_certificate_with_machine_parameters,
    verify_mathematical_certificate, weak_head_normalize,
};
pub use predicate_denotation::{
    CheckedPredicateDenotations, PredicateDenotationError, check_predicate_denotations,
    check_predicate_denotations_with_value_equalities, check_predicate_evaluation_size,
    check_value_equality_denotation,
};
pub use proof::{
    AcceptedPremise, AcceptedProofRule, CertificateAcceptance, MathematicalCoreDecision,
    MathematicalJudgmentReceipt, ProofError, ProofNode, ProofRule, accept_certificate,
    accept_certificate_with_machine_parameters, check_certificate,
    check_certificate_with_machine_parameters, lift_fixed_integer_relation,
    lower_integer_math_relation,
};
