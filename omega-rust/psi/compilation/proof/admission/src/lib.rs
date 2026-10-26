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
//!
//! The producer-side certificate search and the supplied-certificate denotation
//! replay were removed with the PCC verifier and certificate stack in
//! `9718604e847`. Verification no longer consumes certificates produced by a
//! search inside this crate; `admission/` keeps the evidence routes and the
//! checker still re-decides every judgment it is given.

#![forbid(unsafe_code)]
//!
//! `proof.rs` checks explicit proof nodes — and, on every acceptance,
//! denotes the certificate into the `mathematical_core.rs` term model so
//! the kernel re-decides the judgment — while `kernel.rs` re-decides closed
//! primitive judgments over that model and `predicate_denotation.rs`;
//! `integer_rules/` holds the checked integer normalizations and
//! `admission/` the evidence routes.

mod admission;
mod classicality;
mod integer_rules;
mod kernel;
mod mathematical_core;
mod predicate_denotation;
mod proof;
mod term_matchers;

pub use admission::evidence::{
    AcceptedFact, AcceptedFactRoute, AdmissionEvidence, AdmissionKind, AuthorizedAdmission,
    CertificateEnvelope, EvidenceError, EvidenceRoute, Obligation, ObligationClass,
    ProofSystemMarker, accept_source_checked_operation, verify_obligation,
    verify_obligation_with_machine_parameters,
};
pub use classicality::ProofRuleFoundation;
pub use integer_rules::closed_integer::{
    ClosedIntegerEvaluationError, ClosedIntegerEvaluator, big_integer_literal,
    compare_integer_math_terms,
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
    INDEXED_W, IndexedFamily, Level, MathematicalCertificate, MathematicalCertificateBatchError,
    QUOTIENT, QUOTIENT_BETA, QUOTIENT_BOX_PROP, QUOTIENT_COVERAGE, QUOTIENT_EFFECTIVE,
    QUOTIENT_ELIM, QUOTIENT_ID_CANCEL, QUOTIENT_ID_SYM, QUOTIENT_ID_TRANS, QUOTIENT_IND_PROP,
    QUOTIENT_IS_SET, QUOTIENT_LIFT, QUOTIENT_LIFT_PRECONDITION, QUOTIENT_PROJECT,
    QUOTIENT_PROP_IS_SET, QUOTIENT_SET, QUOTIENT_SOUND, QUOTIENT_TRANSPORT,
    QUOTIENT_TRANSPORT_CONST, QUOTIENT_UNIQUE, QuotientFamily, Signature, SignatureCheckError,
    Sort, SourceIntegerTheory, SourceIntegerVocabulary, Term, TermArena, TermHandle,
    assumption_closure, certificate_assumption_closure, check_signature,
    check_signature_with_attribution, check_type, convertible, convertible_retaining,
    denote_bounded_certificate, identity_substitution, indexed_correctness, indexed_scheme,
    infer_sort, infer_type, instantiate_levels, judgment_assumption_closure, quotient_scheme,
    shift, substitute, verify_bounded_certificate, verify_mathematical_certificate,
    verify_mathematical_certificates, weak_head_normalize, weak_head_normalize_retaining,
};
pub use predicate_denotation::{
    CheckedPredicateDenotations, PredicateDenotationError, check_predicate_denotations,
    check_predicate_denotations_with_value_equalities, check_predicate_evaluation_size,
    check_value_equality_denotation,
};
pub use proof::{
    AcceptedPremise, AcceptedProofRule, CertificateAcceptance, MathematicalCoreDecision,
    MathematicalJudgmentReceipt, ProofError, ProofNode, ProofRule, accept_certificate,
    accept_certificate_with_machine_parameters, check_certificate, lift_exact_integer_order,
    lift_fixed_integer_relation, lower_integer_math_relation,
};

/// Forge the certificate's mathematical term to a `TwoZero` node, assert the
/// kernel rejects the forged certificate, and return the original term so
/// the caller may restore it and keep checking.
#[cfg(test)]
pub(crate) fn forge_term_and_assert_rejected(denoted: &mut BoundedDenotation) -> TermHandle {
    let evidence = denoted.certificate.term;
    denoted.certificate.term = denoted.arena.insert(Term::TwoZero);
    assert!(
        verify_mathematical_certificate(
            &mut denoted.arena,
            &denoted.certificate,
            &mut Budget::default()
        )
        .is_err()
    );
    evidence
}

macro_rules! debug_display {
    ($type:ty) => {
        impl std::fmt::Display for $type {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                std::fmt::Debug::fmt(self, formatter)
            }
        }
    };
}
pub(crate) use debug_display;

/// The cited-semantic-axiom lookup each witness module reads its roster
/// through: the index must exist and its proposition must validate against
/// the context. The module names its own refusal type, so the two refusals
/// stay in its vocabulary and no lookup body is written twice.
macro_rules! checked_axiom {
    ($error:path) => {
        fn checked_axiom<'a>(
            context: &semantic_vocabulary::PropositionContext,
            semantic_axioms: &'a [semantic_vocabulary::Proposition],
            index: usize,
        ) -> Result<&'a semantic_vocabulary::Proposition, $error> {
            let proposition = semantic_axioms
                .get(index)
                .ok_or(<$error>::UnknownSemanticAxiom(index))?;
            context
                .validate(proposition)
                .map_err(<$error>::MalformedProposition)?;
            Ok(proposition)
        }
    };
}
pub(crate) use checked_axiom;
