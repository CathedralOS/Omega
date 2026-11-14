/*
    Search-free proof and admission checker for Terminal Psi and the common
    predicative mathematical core (`sMLTT` with `Two`, `Id`, and `W`).

    We re-decide closed primitive judgments (`kernel`), check explicit bounded
    `ProofNode` certificates (`proof`), verify integer affine, cast, shift, and
    forbidden-root witnesses (`integer_rules` and `integer_affine`), check
    predicate and value-equality denotations (`predicate_denotation`), decide a
    closed crash guard once (`closed_proposition`), and
    denote accepted bounded certificates into our dependent term calculus
    (`mathematical_core`, `mathematical_typing`, and `bounded_denotation`) so
    the kernel independently re-decides every judgment.
*/

/*
    Separating this small, deterministic, search-free verification kernel from
    the proof-search and obligation-collection machinery in `proof` ensures that
    neither Terminal Psi verification nor checked-tree admission ever trusts a
    producer's derivation flags, and that bounded proof nodes and universe-
    polymorphic mathematical declarations share one predicative kernel.
*/

/*
    Start at `admission` (in this file) for `verify_obligation`, `proof` for
    `accept_certificate` and bounded `ProofNode` traversal, and
    `mathematical_core` for `verify_mathematical_certificate` and `Signature`
    checking. `mathematical_typing` implements dependent typing and conversion
    judgments; `bounded_denotation` denotes bounded certificates into core
    terms; `integer_rules` and `integer_affine` replay integer normalizations
    and witnesses; `predicate_denotation` checks boolean predicate denotations;
    `kernel` decides closed primitive judgments; `closed_proposition` decides a
    closed crash guard for build-time admission and the interpreter; and
    `classicality` classifies proof-rule foundations.
*/

#![forbid(unsafe_code)]

mod bounded_denotation;
mod integer_affine;
mod mathematical_typing;
pub(crate) mod admission {
    pub(crate) mod evidence {
        use std::collections::BTreeSet;
        pub use terminal_psi::ProofSystemMarker;
        pub use terminal_psi::{
            AdmissionEvidence, AdmissionKind, CertificateEnvelope, EvidenceRoute,
        };

        use semantic_vocabulary::{
            AdmissionSiteId, EvidenceIdentity, ObligationId, Proposition, PropositionContext,
        };

        use crate::{
            CertificateAcceptance, PrimitiveJudgment, accept_certificate_with_machine_parameters,
            decide_primitive,
        };

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct AuthorizedAdmission {
            pub site: AdmissionSiteId,
            pub kind: AdmissionKind,
            pub authority_identity: EvidenceIdentity,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum ObligationClass {
            Derivable,
            AdmissionAuthorized(AuthorizedAdmission),
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct Obligation {
            pub id: ObligationId,
            pub proposition: Proposition,
            pub class: ObligationClass,
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub enum AcceptedFactRoute {
            KernelDerived(PrimitiveJudgment),
            CertificateDerived {
                identity: EvidenceIdentity,
                proof_system_marker: ProofSystemMarker,
                acceptance: CertificateAcceptance,
            },
            Admitted(AdmissionEvidence),
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct AcceptedFact {
            pub obligation: ObligationId,
            pub proposition: Proposition,
            pub route: AcceptedFactRoute,
        }

        pub fn verify_obligation(
            context: &PropositionContext,
            obligation: &Obligation,
            assumptions: &[Proposition],
            semantic_axioms: &[Proposition],
            route: EvidenceRoute,
        ) -> Result<AcceptedFact, EvidenceError> {
            verify_obligation_with_machine_parameters(
                context,
                obligation,
                assumptions,
                semantic_axioms,
                &BTreeSet::new(),
                route,
            )
        }

        pub fn verify_obligation_with_machine_parameters(
            context: &PropositionContext,
            obligation: &Obligation,
            assumptions: &[Proposition],
            semantic_axioms: &[Proposition],
            machine_parameter_values: &BTreeSet<semantic_vocabulary::ValueId>,
            route: EvidenceRoute,
        ) -> Result<AcceptedFact, EvidenceError> {
            context
                .validate(&obligation.proposition)
                .map_err(EvidenceError::MalformedProposition)?;
            let route = match route {
                EvidenceRoute::KernelDerived(judgment) => {
                    decide_primitive(context, &obligation.proposition, judgment)
                        .map_err(EvidenceError::Kernel)?;
                    AcceptedFactRoute::KernelDerived(judgment)
                }
                EvidenceRoute::CertificateDerived(certificate) => {
                    let acceptance = accept_certificate_with_machine_parameters(
                        context,
                        &obligation.proposition,
                        assumptions,
                        semantic_axioms,
                        machine_parameter_values,
                        &certificate.proof,
                    )
                    .map_err(EvidenceError::Certificate)?;
                    AcceptedFactRoute::CertificateDerived {
                        identity: certificate.identity,
                        proof_system_marker: certificate.proof_system_marker,
                        acceptance,
                    }
                }
                EvidenceRoute::Admitted(evidence) => {
                    if primitive_derivation(context, &obligation.proposition).is_some() {
                        return Err(EvidenceError::AdmissionCannotReplaceDerivation);
                    }
                    let ObligationClass::AdmissionAuthorized(authorized) = obligation.class else {
                        return Err(EvidenceError::AdmissionCannotReplaceDerivation);
                    };
                    if evidence.site != authorized.site
                        || evidence.kind != authorized.kind
                        || evidence.authority_identity != authorized.authority_identity
                    {
                        return Err(EvidenceError::AdmissionSiteMismatch);
                    }
                    return Err(EvidenceError::AdmissionNotAcceptedByProfile);
                }
                EvidenceRoute::SourceChecked => {
                    return Err(EvidenceError::SourceCheckedOutsideOperation);
                }
            };
            Ok(AcceptedFact {
                obligation: obligation.id,
                proposition: obligation.proposition.clone(),
                route,
            })
        }

        fn primitive_derivation(
            context: &PropositionContext,
            proposition: &Proposition,
        ) -> Option<PrimitiveJudgment> {
            [
                PrimitiveJudgment::Truth,
                PrimitiveJudgment::ReflexiveEquality,
                PrimitiveJudgment::ClosedIntegerRelation,
                PrimitiveJudgment::IntegerCarrierBound,
            ]
            .into_iter()
            .find(|judgment| decide_primitive(context, proposition, *judgment).is_ok())
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub enum EvidenceError {
            MalformedProposition(semantic_vocabulary::PropositionError),
            Kernel(crate::KernelError),
            Certificate(crate::ProofError),
            AdmissionCannotReplaceDerivation,
            AdmissionSiteMismatch,
            AdmissionNotAcceptedByProfile,
            SourceCheckedOutsideOperation,
        }

        crate::debug_display!(EvidenceError);

        impl std::error::Error for EvidenceError {}

        #[cfg(test)]
        mod tests {
            use super::{
                AdmissionEvidence, AdmissionKind, AdmissionSiteId, AuthorizedAdmission,
                EvidenceError, EvidenceIdentity, EvidenceRoute, Obligation, ObligationClass,
                ObligationId, ProofSystemMarker, Proposition, PropositionContext,
                verify_obligation,
            };
            use semantic_vocabulary::{ProfileDecisionId, PropositionId};
            fn obligation(proposition: Proposition, class: ObligationClass) -> Obligation {
                Obligation {
                    id: ObligationId::new(1).expect("obligation identity"),
                    proposition,
                    class,
                }
            }

            fn admission() -> AdmissionEvidence {
                AdmissionEvidence {
                    site: AdmissionSiteId::new(2).expect("site identity"),
                    kind: AdmissionKind::ProviderFact,
                    authority_identity: EvidenceIdentity::new(3).expect("authority identity"),
                    evidence_identity: EvidenceIdentity::new(4).expect("evidence identity"),
                    profile_decision: ProfileDecisionId::new(5).expect("profile decision"),
                }
            }

            #[test]
            fn admission_cannot_replace_a_derivable_obligation() {
                let evidence = admission();
                let authorized = AuthorizedAdmission {
                    site: evidence.site,
                    kind: evidence.kind,
                    authority_identity: evidence.authority_identity,
                };
                assert_eq!(
                    verify_obligation(
                        &PropositionContext::default(),
                        &obligation(
                            Proposition::Truth,
                            ObligationClass::AdmissionAuthorized(authorized),
                        ),
                        &[],
                        &[],
                        EvidenceRoute::Admitted(evidence),
                    )
                    .expect_err("admission must not replace derivation"),
                    EvidenceError::AdmissionCannotReplaceDerivation
                );
            }

            #[test]
            fn admission_rejects_authority_substitution_and_receiver_acceptance() {
                let evidence = admission();
                let proposition =
                    Proposition::Atom(PropositionId::new(6).expect("proposition identity"));
                let authorized = AuthorizedAdmission {
                    site: evidence.site,
                    kind: evidence.kind,
                    authority_identity: EvidenceIdentity::new(99).expect("other authority"),
                };
                assert_eq!(
                    verify_obligation(
                        &PropositionContext::default(),
                        &obligation(
                            proposition.clone(),
                            ObligationClass::AdmissionAuthorized(authorized),
                        ),
                        &[],
                        &[],
                        EvidenceRoute::Admitted(evidence),
                    )
                    .expect_err("substituted authority must reject"),
                    EvidenceError::AdmissionSiteMismatch
                );

                let authorized = AuthorizedAdmission {
                    site: evidence.site,
                    kind: evidence.kind,
                    authority_identity: evidence.authority_identity,
                };
                assert_eq!(
                    verify_obligation(
                        &PropositionContext::default(),
                        &obligation(
                            proposition,
                            ObligationClass::AdmissionAuthorized(authorized),
                        ),
                        &[],
                        &[],
                        EvidenceRoute::Admitted(evidence),
                    )
                    .expect_err("receiver admission evidence must reject"),
                    EvidenceError::AdmissionNotAcceptedByProfile
                );
            }

            #[test]
            fn proof_system_marker_has_no_compatibility_ladder() {
                assert_eq!(ProofSystemMarker::new(5), Some(ProofSystemMarker::CURRENT));
                assert!(ProofSystemMarker::new(4).is_none());
                assert_eq!(ProofSystemMarker::new(0), None);
                assert_eq!(ProofSystemMarker::new(1), None);
                assert_eq!(ProofSystemMarker::new(2), None);
                assert_eq!(ProofSystemMarker::new(3), None);
            }
        }
    }
}
mod classicality;
mod closed_proposition;
mod integer_rules;
mod kernel;
mod mathematical_core;
mod predicate_denotation;
mod proof;
pub(crate) mod term_matchers {
    use semantic_vocabulary::{IntegerMathTerm, IntegerType, ScalarTerm};

    pub(crate) type TermMatcher =
        fn(&ScalarTerm) -> Option<(&IntegerType, &ScalarTerm, &ScalarTerm)>;

    macro_rules! scalar_term_matcher {
    ($($matcher:ident => $variant:ident),+ $(,)?) => {
        $(
            pub(crate) fn $matcher(
                term: &ScalarTerm,
            ) -> Option<(&IntegerType, &ScalarTerm, &ScalarTerm)> {
                match term {
                    ScalarTerm::$variant { scalar_type, left, right } => {
                        Some((scalar_type, left, right))
                    }
                    _ => None,
                }
            }
        )+
    };
}

    scalar_term_matcher! {
        exact_integer_add => ExactIntegerAdd,
        wrapping_integer_add => WrappingIntegerAdd,
        exact_integer_subtract => ExactIntegerSubtract,
        exact_integer_multiply => ExactIntegerMultiply,
        exact_integer_divide => ExactIntegerDivide,
    }

    pub(crate) fn is_math_add(term: &IntegerMathTerm) -> bool {
        matches!(term, IntegerMathTerm::Add(..))
    }

    pub(crate) fn is_math_subtract(term: &IntegerMathTerm) -> bool {
        matches!(term, IntegerMathTerm::Subtract(..))
    }

    pub(crate) fn is_math_multiply(term: &IntegerMathTerm) -> bool {
        matches!(term, IntegerMathTerm::Multiply(..))
    }
}

pub use admission::evidence::{
    AcceptedFact, AcceptedFactRoute, AdmissionEvidence, AdmissionKind, AuthorizedAdmission,
    CertificateEnvelope, EvidenceError, EvidenceRoute, Obligation, ObligationClass,
    ProofSystemMarker, verify_obligation, verify_obligation_with_machine_parameters,
};
pub use classicality::ProofRuleFoundation;
pub use closed_proposition::{closed_guard, closed_guard_with};
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
    Sort, SourceContentTheory, SourceIntegerTheory, SourceIntegerVocabulary, Term, TermArena,
    TermHandle, assumption_closure, certificate_assumption_closure, check_signature,
    check_signature_with_attribution, check_type, convertible, convertible_retaining,
    denote_bounded_certificate, identity_substitution, indexed_correctness, indexed_scheme,
    infer_sort, infer_type, instantiate_levels, judgment_assumption_closure, quotient_scheme,
    shift, substitute, universe_parameter_arity, verify_bounded_certificate,
    verify_mathematical_certificate, verify_mathematical_certificates, weak_head_normalize,
    weak_head_normalize_retaining,
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

#[cfg(test)]
pub(crate) fn correlated_step(
    definition_axiom: usize,
    literal_axiom: Option<usize>,
) -> CorrelatedAffineStepWitness {
    CorrelatedAffineStepWitness {
        definition_axiom,
        literal_axiom,
    }
}

#[cfg(test)]
pub(crate) fn value_context(
    scalar: semantic_vocabulary::ScalarType,
    values: u64,
) -> semantic_vocabulary::PropositionContext {
    semantic_vocabulary::PropositionContext::from_value_types(
        (1..=values).map(|id| (semantic_vocabulary::ValueId::new(id).unwrap(), scalar)),
    )
    .unwrap()
}

#[cfg(test)]
pub(crate) fn affine_witness(
    root: semantic_vocabulary::ScalarTerm,
    target: semantic_vocabulary::ScalarTerm,
    definition_axioms: Vec<usize>,
    literal_axioms: Vec<Option<usize>>,
) -> IntegerAffineWitness {
    IntegerAffineWitness {
        root,
        target,
        definition_axioms,
        literal_axioms,
    }
}

#[cfg(test)]
macro_rules! proof_node {
    ($conclusion:expr => $rule:expr) => {
        ProofNode {
            conclusion: $conclusion,
            rule: $rule,
        }
    };
}
#[cfg(test)]
pub(crate) use proof_node;

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
