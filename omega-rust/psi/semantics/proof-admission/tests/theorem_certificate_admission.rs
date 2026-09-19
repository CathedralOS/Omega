//! A theorem machine's obligation discharged through the admission and
//! judged by the common mathematical core.
//!
//! The authored theorem is `machine equal_symmetric(a: u64, b: u64)
//! requires a == b ensures b == a { }`. Lowering reconstructs its
//! `ensures` clause as a Terminal obligation whose assumption roster is
//! the machine's `requires`, and ships the certificate
//! `EqualitySymmetry { Assumption 0 }` in the proof bundle. The verifier
//! hands that obligation, roster and decoded certificate to
//! `verify_obligation`, which is what these tests call directly: the
//! admission re-decides the certificate with the bounded rules *and*
//! denotes it into the core's judgment — `a == b` and `b == a` share
//! the canonical `Id Int m_a m_b` denotation, so the judgment is
//! `[h : Id Int m_a m_b] ⊢ h : Id Int m_a m_b`, which the kernel
//! checks. The accepted fact records the kernel's decision and the
//! judgment's measurements.
//!
//! The invalid controls are the four the board names: a wrong goal, a
//! wrong proof, a forged closure and a missing dependency each reject —
//! at the admission for the first two, and at the kernel on the exact
//! elaborated judgment for the last two, so the kernel is shown to be
//! load-bearing over this certificate rather than a bystander.

use std::collections::BTreeSet;

use proof_admission::{
    AcceptedFactRoute, AcceptedProofRule, AdmissionProfile, Budget, CertificateEnvelope, CoreError,
    DEFAULT_CONVERSION_STEPS, EvidenceError, EvidenceRoute, MathematicalCoreDecision,
    MathematicalJudgmentReceipt, Obligation, ObligationClass, ProofError, ProofNode, ProofRule,
    ProofSystemMarker, Term, certificate_assumption_closure, denote_bounded_certificate,
    lift_fixed_integer_relation, verify_mathematical_certificate, verify_obligation,
};
use semantic_vocabulary::{
    EvidenceIdentity, IntegerSign, IntegerType, ObligationId, Proposition, PropositionContext,
    ScalarTerm, ScalarType, ValueId,
};

fn u64_type() -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).expect("u64"))
}

/// The theorem's parameters `a`, `b`, `c` as the verifier's scalar values.
fn parameter(identity: u64) -> ScalarTerm {
    ScalarTerm::value(
        ValueId::new(identity).expect("parameter identity"),
        u64_type(),
    )
}

fn context() -> PropositionContext {
    PropositionContext::from_value_types(
        [1, 2, 3].map(|identity| (ValueId::new(identity).expect("identity"), u64_type())),
    )
    .expect("parameter context")
}

fn equal(left: u64, right: u64) -> Proposition {
    Proposition::Equal(parameter(left), parameter(right))
}

fn obligation(proposition: Proposition) -> Obligation {
    Obligation {
        id: ObligationId::new(7).expect("obligation identity"),
        proposition,
        class: ObligationClass::Derivable,
    }
}

fn envelope(proof: ProofNode) -> EvidenceRoute {
    EvidenceRoute::CertificateDerived(CertificateEnvelope {
        identity: EvidenceIdentity::new(7).expect("evidence identity"),
        proof_system_marker: ProofSystemMarker::CURRENT,
        proof,
    })
}

fn cite(index: usize, proposition: Proposition) -> ProofNode {
    ProofNode {
        conclusion: proposition,
        rule: ProofRule::Assumption { index },
    }
}

/// `EqualitySymmetry { Assumption 0 }` concluding `b == a` — the shape
/// lowering ships for `requires a == b ensures b == a`.
fn symmetry_certificate() -> ProofNode {
    ProofNode {
        conclusion: equal(2, 1),
        rule: ProofRule::EqualitySymmetry {
            equality: Box::new(cite(0, equal(1, 2))),
        },
    }
}

/// `EqualityTransitivity { Assumption 0, Assumption 1 }` concluding
/// `a == c` — the shape for `requires a == b, b == c ensures a == c`.
fn transitivity_certificate() -> ProofNode {
    ProofNode {
        conclusion: equal(1, 3),
        rule: ProofRule::EqualityTransitivity {
            left_equals_middle: Box::new(cite(0, equal(1, 2))),
            middle_equals_right: Box::new(cite(1, equal(2, 3))),
        },
    }
}

fn admit(
    goal: Proposition,
    requires: &[Proposition],
    proof: ProofNode,
) -> Result<MathematicalCoreDecision, EvidenceError> {
    let accepted = verify_obligation(
        &context(),
        &obligation(goal),
        requires,
        &[],
        envelope(proof),
        &AdmissionProfile::default(),
    )?;
    let AcceptedFactRoute::CertificateDerived { acceptance, .. } = accepted.route else {
        panic!("a certificate envelope is accepted on the certificate route");
    };
    Ok(acceptance.mathematical_core)
}

#[test]
fn the_symmetry_theorem_obligation_is_judged_by_the_kernel_at_admission() {
    let decision = admit(equal(2, 1), &[equal(1, 2)], symmetry_certificate())
        .expect("the shipped symmetry certificate discharges the ensures obligation");
    // Three declarations — `Int` and the two mathematical endpoints —
    // all of which the judgment's closure commits to; one bound premise;
    // 34 arena slots once the kernel has checked the judgment, the
    // elaborated terms plus the checker's own working terms.
    assert_eq!(
        decision,
        MathematicalCoreDecision::Judged(MathematicalJudgmentReceipt {
            declarations: 3,
            assumption_closure: 3,
            context_depth: 1,
            arena_slots: 34,
        })
    );
}

#[test]
fn the_transitivity_theorem_obligation_is_judged_with_both_premises_bound() {
    let decision = admit(
        equal(1, 3),
        &[equal(1, 2), equal(2, 3)],
        transitivity_certificate(),
    )
    .expect("the shipped transitivity certificate discharges the ensures obligation");
    assert_eq!(
        decision,
        MathematicalCoreDecision::Judged(MathematicalJudgmentReceipt {
            declarations: 4,
            assumption_closure: 4,
            context_depth: 2,
            arena_slots: 156,
        })
    );
}

#[test]
fn a_wrong_goal_rejects_at_the_admission() {
    // The verifier reconstructs `b == c`; the certificate proves `b == a`.
    // The goal is bound by the receiver, never read from the certificate.
    assert_eq!(
        admit(equal(2, 3), &[equal(1, 2)], symmetry_certificate()).expect_err("wrong goal"),
        EvidenceError::Certificate(ProofError::CertificateConclusionMismatch)
    );
}

#[test]
fn a_wrong_proof_rejects_at_the_admission() {
    // Citing the premise as if it already read `b == a`: the cited roster
    // entry is `a == b`, and symmetry is not applied for free.
    assert_eq!(
        admit(equal(2, 1), &[equal(1, 2)], cite(0, equal(2, 1))).expect_err("wrong proof"),
        EvidenceError::Certificate(ProofError::AssumptionConclusionMismatch(0))
    );
    // A symmetry step over a premise that is not in the roster.
    assert_eq!(
        admit(equal(2, 1), &[equal(1, 3)], symmetry_certificate()).expect_err("missing premise"),
        EvidenceError::Certificate(ProofError::AssumptionConclusionMismatch(0))
    );
}

#[test]
fn the_kernel_rejects_a_forged_claim_over_the_same_elaborated_judgment() {
    // The exact judgment the admission hands the kernel, re-elaborated
    // here so the forgeries land on the term the kernel really checks.
    let mut denoted = denote_bounded_certificate(
        &context(),
        &equal(2, 1),
        &[equal(1, 2)],
        &[],
        &symmetry_certificate(),
    )
    .expect("the certificate denotes");
    verify_mathematical_certificate(
        &mut denoted.arena,
        &denoted.certificate,
        &mut Budget::new(DEFAULT_CONVERSION_STEPS),
    )
    .expect("the honest judgment verifies");
    assert_eq!(
        denoted.rules,
        vec![
            AcceptedProofRule::Assumption,
            AcceptedProofRule::EqualitySymmetry
        ]
    );

    // Wrong goal at the kernel: the honest evidence is the cited
    // premise variable `h : Id Int m_a m_b`; claiming the non-canonical
    // orientation `Id Int m_b m_a` — a different denoted type, however
    // propositionally equivalent — mismatches in the kernel.
    let Term::Id { ty, left, right } = denoted.arena.get(denoted.certificate.expected) else {
        panic!("the goal denotes to an identity");
    };
    let forged_goal = denoted.arena.insert(Term::Id {
        ty,
        left: right,
        right: left,
    });
    let mut forged = denoted.certificate.clone();
    forged.expected = forged_goal;
    assert!(matches!(
        verify_mathematical_certificate(
            &mut denoted.arena,
            &forged,
            &mut Budget::new(DEFAULT_CONVERSION_STEPS)
        ),
        Err(CoreError::TypeMismatch { .. })
    ));

    // Wrong proof at the kernel: `refl Int m_a` inhabits
    // `Id Int m_a m_a`, offered for the goal `Id Int m_a m_b` — the
    // endpoints never convert, so the kernel rejects the term.
    let Term::Id {
        ty: carrier,
        left: endpoint,
        ..
    } = denoted.arena.get(denoted.certificate.expected)
    else {
        panic!("the goal denotes to an identity");
    };
    let mut forged = denoted.certificate.clone();
    forged.term = denoted.arena.insert(Term::Refl {
        ty: carrier,
        value: endpoint,
    });
    assert!(matches!(
        verify_mathematical_certificate(
            &mut denoted.arena,
            &forged,
            &mut Budget::new(DEFAULT_CONVERSION_STEPS)
        ),
        Err(CoreError::TypeMismatch { .. })
    ));

    // Forged closure: the judgment commits to exactly the carrier and the
    // two parameters. Dropping the last declaration leaves the constant
    // naming it dangling, and the kernel refuses the signature — a
    // missing dependency, not a weaker judgment.
    assert_eq!(
        certificate_assumption_closure(&denoted.arena, &denoted.certificate),
        BTreeSet::from([0, 1, 2])
    );
    let mut forged = denoted.certificate.clone();
    forged.signature.pop();
    assert_eq!(
        verify_mathematical_certificate(
            &mut denoted.arena,
            &forged,
            &mut Budget::new(DEFAULT_CONVERSION_STEPS)
        ),
        Err(CoreError::UnknownDeclaration {
            declaration: 2,
            signature_len: 2,
        })
    );
}

#[test]
fn the_equal_integer_math_citation_crossing_is_judged_by_the_kernel() {
    // The bounded citation matcher accepts `a == b` cited where its lifted
    // `IntegerMathEqual` form is the goal: one normalized relation under
    // the bounded rules. Both sides denote the same `Id Int m_a m_b` now
    // — the `Id`-versus-atom shape crossing is closed — so the kernel
    // judges the citation's own variable as evidence.
    let goal = lift_fixed_integer_relation(&equal(1, 2)).expect("the fixed equality lifts");
    let decision = admit(goal.clone(), &[equal(1, 2)], cite(0, goal))
        .expect("the bounded rules accept the normalized citation");
    assert!(matches!(decision, MathematicalCoreDecision::Judged(_)));
}
