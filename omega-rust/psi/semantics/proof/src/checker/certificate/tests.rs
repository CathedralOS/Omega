//! Certificate-route tests: the admission kernel, not the producer, decides
//! every package. Valid packages must verify; forged premises, mismatched
//! steps and out-of-range values must be rejected by the kernel itself.

use super::{bigint_math_literal, closed_bounds_certificate, declared_interval_certificate};
use crate::obligations::IntegerRange;
use numerics::bignum::BigInt;
use proof_admission::{
    AcceptedFactRoute, AcceptedProofRule, AdmissionProfile, EvidenceRoute, Obligation,
    ObligationClass, PrimitiveJudgment, ProofRule, verify_obligation,
};
use semantic_vocabulary::{
    IntegerMathLiteral, IntegerMathTerm, IntegerSign, IntegerType, ObligationId, Proposition,
    PropositionContext,
};

fn math_literal(value: i64) -> IntegerMathLiteral {
    bigint_math_literal(&BigInt::from_i64(value)).expect("literal bound")
}

fn literal_term(value: i64) -> IntegerMathTerm {
    IntegerMathTerm::IntegerLiteral(math_literal(value))
}

fn integer_range(minimum: i64, maximum: i64) -> IntegerRange {
    IntegerRange {
        minimum: BigInt::from_i64(minimum),
        maximum: BigInt::from_i64(maximum),
    }
}

fn u8_type() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 8).expect("u8")
}

#[test]
fn closed_literal_within_target_verifies() {
    let certificate =
        closed_bounds_certificate(literal_term(5), math_literal(0), math_literal(10), 7)
            .expect("certificate");
    let fact = certificate.verify().expect("kernel accepts");
    assert_eq!(fact.proposition, certificate.obligation.proposition);
    assert!(matches!(
        fact.route,
        AcceptedFactRoute::CertificateDerived { ref acceptance, .. }
            if acceptance.assumptions.is_empty()
    ));
}

#[test]
fn closed_arithmetic_term_verifies_under_kernel_evaluation() {
    // `2 + 3 <= 10` is arithmetic the kernel itself evaluates; the producer's
    // encoder never decides it.
    let term = IntegerMathTerm::Add(Box::new(literal_term(2)), Box::new(literal_term(3)));
    let certificate =
        closed_bounds_certificate(term, math_literal(0), math_literal(10), 8).expect("certificate");
    certificate.verify().expect("kernel accepts");
}

#[test]
fn closed_literal_outside_target_is_rejected_by_the_kernel() {
    let certificate =
        closed_bounds_certificate(literal_term(300), math_literal(0), math_literal(5), 9)
            .expect("certificate");
    assert!(certificate.verify().is_err());
}

#[test]
fn closed_arithmetic_term_outside_target_is_rejected() {
    // `200 + 100` is a closed 300; the kernel's own evaluation refuses it
    // against a `[0, 5]` target.
    let term = IntegerMathTerm::Add(Box::new(literal_term(200)), Box::new(literal_term(100)));
    let certificate =
        closed_bounds_certificate(term, math_literal(0), math_literal(5), 10).expect("certificate");
    assert!(certificate.verify().is_err());
}

#[test]
fn declared_interval_within_target_verifies() {
    let certificate = declared_interval_certificate(
        &integer_range(0, 255),
        u8_type(),
        math_literal(0),
        math_literal(300),
        11,
    )
    .expect("certificate");
    let fact = certificate.verify().expect("kernel accepts");
    let AcceptedFactRoute::CertificateDerived { acceptance, .. } = fact.route else {
        panic!("certificate-derived route");
    };
    // Both declared endpoints are recorded as cited premises: the certificate
    // states exactly which declared facts the leg consumed.
    assert_eq!(acceptance.assumptions.len(), 2);
    assert!(
        acceptance
            .rules
            .contains(&AcceptedProofRule::IntegerLessOrEqualTransitivity)
    );
}

#[test]
fn declared_interval_certificate_rejects_an_uncited_premise() {
    let mut certificate = declared_interval_certificate(
        &integer_range(0, 255),
        u8_type(),
        math_literal(0),
        math_literal(300),
        12,
    )
    .expect("certificate");
    // Forge: cite an assumption index that was never supplied.
    let ProofRule::ConjunctionIntroduction(conjuncts) = &mut certificate.envelope.proof.rule else {
        panic!("conjunction root");
    };
    let ProofRule::IntegerLessOrEqualTransitivity {
        middle_less_or_equal_right,
        ..
    } = &mut conjuncts[0].rule
    else {
        panic!("transitivity leg");
    };
    middle_less_or_equal_right.rule = ProofRule::Assumption { index: 9 };
    assert!(certificate.verify().is_err());
}

#[test]
fn declared_interval_certificate_rejects_a_withheld_premise() {
    let mut certificate = declared_interval_certificate(
        &integer_range(0, 255),
        u8_type(),
        math_literal(0),
        math_literal(300),
        13,
    )
    .expect("certificate");
    // Forge: drop the lower premise from the roster the kernel sees. The
    // certificate still cites it; verification must fail.
    certificate.assumptions.remove(0);
    assert!(certificate.verify().is_err());
}

#[test]
fn declared_interval_certificate_rejects_a_false_premise_swap() {
    let mut certificate = declared_interval_certificate(
        &integer_range(0, 255),
        u8_type(),
        math_literal(0),
        math_literal(300),
        14,
    )
    .expect("certificate");
    // Forge: swap the roster so index 0 states `atom <= declared_upper`
    // instead of `declared_lower <= atom`. Citation matching must fail.
    certificate.assumptions.swap(0, 1);
    assert!(certificate.verify().is_err());
}

#[test]
fn tampered_obligation_conclusion_is_rejected() {
    let mut certificate =
        closed_bounds_certificate(literal_term(5), math_literal(0), math_literal(10), 15)
            .expect("certificate");
    // Forge: weaken the obligation to `0 <= 5 <= 400` while the certificate
    // still proves `[0, 10]`. Obligation/certificate mismatch must fail.
    certificate.obligation.proposition = Proposition::Conjunction(vec![
        Proposition::IntegerMathLessOrEqual(literal_term(0), literal_term(5)),
        Proposition::IntegerMathLessOrEqual(literal_term(5), literal_term(400)),
    ]);
    assert!(certificate.verify().is_err());
}

#[test]
fn transitivity_middle_mismatch_is_rejected() {
    let mut certificate = declared_interval_certificate(
        &integer_range(0, 255),
        u8_type(),
        math_literal(0),
        math_literal(300),
        16,
    )
    .expect("certificate");
    // Forge: the lower leg's closed step now ends at `200` while the premise
    // still starts from the declared lower bound, so the two children no
    // longer share the middle the transitivity chain requires.
    let ProofRule::ConjunctionIntroduction(conjuncts) = &mut certificate.envelope.proof.rule else {
        panic!("conjunction root");
    };
    let ProofRule::IntegerLessOrEqualTransitivity {
        left_less_or_equal_middle,
        ..
    } = &mut conjuncts[0].rule
    else {
        panic!("transitivity leg");
    };
    left_less_or_equal_middle.conclusion =
        Proposition::IntegerMathLessOrEqual(literal_term(0), literal_term(200));
    assert!(certificate.verify().is_err());
}

#[test]
fn kernel_derived_route_still_decides_the_same_closed_claim() {
    // The same conclusion routed through `KernelDerived` is decided by
    // `decide_primitive` directly -- evidence-route plumbing sanity.
    let conclusion = Proposition::IntegerMathLessOrEqual(literal_term(3), literal_term(9));
    let fact = verify_obligation(
        &PropositionContext::default(),
        &Obligation {
            id: ObligationId::new(21).expect("obligation id"),
            proposition: conclusion.clone(),
            class: ObligationClass::Derivable,
        },
        &[],
        &[],
        EvidenceRoute::KernelDerived(PrimitiveJudgment::ClosedIntegerRelation),
        &AdmissionProfile::default(),
    )
    .expect("kernel accepts a true closed relation");
    assert_eq!(fact.proposition, conclusion);
}
