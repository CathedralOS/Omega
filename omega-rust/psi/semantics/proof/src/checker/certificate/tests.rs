//! Certificate-route tests: the admission kernel, not the producer, decides
//! every package. Valid packages must verify; forged premises, mismatched
//! steps and out-of-range values must be rejected by the kernel itself.

use super::{
    bigint_math_literal, closed_bounds_certificate, declared_interval_certificate,
    guarded_bounds_certificate, refold_bounds_certificate, scalar_integer_term,
};
use crate::obligations::IntegerRange;
use numerics::bignum::BigInt;
use proof_admission::{
    AcceptedFactRoute, AcceptedProofRule, AdmissionProfile, EvidenceRoute, Obligation,
    ObligationClass, PrimitiveJudgment, ProofRule, verify_obligation,
};
use semantic_vocabulary::{
    IntegerMathLiteral, IntegerMathTerm, IntegerSign, IntegerType, ObligationId, Proposition,
    PropositionContext, ScalarTerm, ScalarType, ValueId,
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

fn math_atom(integer_type: IntegerType, seed: u64) -> (ValueId, IntegerMathTerm) {
    let value = ValueId::new(seed).expect("value id");
    (
        value,
        IntegerMathTerm::MathValue {
            source_type: integer_type,
            value,
        },
    )
}

/// The direct guarded leg's certificate: declared interval plus guard facts
/// (`pending > 0 && pending <= 100`) as premises on one opaque atom, claimed
/// against `u8 [0..=200]`.
fn guarded_argument_certificate(seed: u64) -> super::BoundedValueCertificate {
    let integer_type = u8_type();
    let (value, atom) = math_atom(integer_type, seed);
    let context =
        PropositionContext::from_value_types([(value, ScalarType::Integer(integer_type))])
            .expect("context");
    let assumptions = vec![
        // Declared `u8` interval.
        Proposition::IntegerMathLessOrEqual(literal_term(0), atom.clone()),
        Proposition::IntegerMathLessOrEqual(atom.clone(), literal_term(255)),
        // Guard facts: `pending > 0` restates as `1 <= pending`, and
        // `pending <= 100` enters verbatim.
        Proposition::IntegerMathLessOrEqual(literal_term(1), atom.clone()),
        Proposition::IntegerMathLessOrEqual(atom.clone(), literal_term(100)),
    ];
    guarded_bounds_certificate(atom, assumptions, context, &integer_range(0, 200), seed)
        .expect("certificate")
}

#[test]
fn guarded_argument_premises_verify_under_the_kernel() {
    let certificate = guarded_argument_certificate(31);
    let fact = certificate.verify().expect("kernel accepts");
    assert_eq!(fact.proposition, certificate.obligation.proposition);
    let AcceptedFactRoute::CertificateDerived { acceptance, .. } = fact.route else {
        panic!("certificate-derived route");
    };
    // Only the two strongest anchors are cited; the roster may carry more.
    assert_eq!(acceptance.assumptions.len(), 2);
    assert!(
        acceptance
            .rules
            .contains(&AcceptedProofRule::IntegerLessOrEqualTransitivity)
    );
}

#[test]
fn guarded_argument_gap_stays_uncovered() {
    // Premises `1 <= pending <= 100` cannot reach a `[50, 200]` target; the
    // leg emits nothing and the trusted derivation keeps the verdict.
    let integer_type = u8_type();
    let (value, atom) = math_atom(integer_type, 32);
    let context =
        PropositionContext::from_value_types([(value, ScalarType::Integer(integer_type))])
            .expect("context");
    let assumptions = vec![
        Proposition::IntegerMathLessOrEqual(literal_term(1), atom.clone()),
        Proposition::IntegerMathLessOrEqual(atom.clone(), literal_term(100)),
    ];
    assert!(
        guarded_bounds_certificate(atom, assumptions, context, &integer_range(50, 200), 32)
            .is_none()
    );
}

#[test]
fn guarded_argument_certificate_rejects_a_miscited_premise() {
    let mut certificate = guarded_argument_certificate(33);
    // Forge: the lower leg cites index 0 (`0 <= pending`) where it derived
    // `1 <= pending` -- the transitivity middle no longer matches.
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
    middle_less_or_equal_right.rule = ProofRule::Assumption { index: 0 };
    assert!(certificate.verify().is_err());
}

#[test]
fn guarded_argument_certificate_rejects_a_dropped_premise() {
    let mut certificate = guarded_argument_certificate(34);
    // Forge: drop the cited guard upper bound from the roster the kernel sees.
    certificate.assumptions.remove(3);
    assert!(certificate.verify().is_err());
}

/// The refold leg's certificate for `fuel - 1`: premises `2 <= fuel` (the
/// `fuel > 1` guard) and `fuel <= 128` (declared), mapped through the affine
/// witness to `1 <= fuel - 1 <= 127`, claimed against `u8 [0..=127]`.
fn fuel_decrement_certificate(seed: u64) -> super::BoundedValueCertificate {
    let integer_type = u8_type();
    let value = ValueId::new(seed).expect("value id");
    let place = ScalarTerm::value(value, ScalarType::Integer(integer_type));
    let place_math = IntegerMathTerm::MathValue {
        source_type: integer_type,
        value,
    };
    let context =
        PropositionContext::from_value_types([(value, ScalarType::Integer(integer_type))])
            .expect("context");
    let scalar_bound = |bound| scalar_integer_term(integer_type, &BigInt::from_i64(bound));
    let assumptions = vec![
        Proposition::LessOrEqual(scalar_bound(2).expect("scalar"), place.clone()),
        Proposition::LessOrEqual(place.clone(), scalar_bound(128).expect("scalar")),
    ];
    refold_bounds_certificate(
        place,
        place_math,
        integer_type,
        BigInt::from_i64(1),
        true,
        assumptions,
        (0, BigInt::from_i64(2)),
        (1, BigInt::from_i64(128)),
        context,
        &integer_range(0, 127),
        seed,
    )
    .expect("certificate")
}

#[test]
fn affine_refold_bound_verifies_under_the_kernel() {
    let certificate = fuel_decrement_certificate(41);
    let fact = certificate.verify().expect("kernel accepts");
    assert_eq!(fact.proposition, certificate.obligation.proposition);
    let AcceptedFactRoute::CertificateDerived { acceptance, .. } = fact.route else {
        panic!("certificate-derived route");
    };
    assert_eq!(acceptance.assumptions.len(), 2);
    assert!(
        acceptance
            .rules
            .contains(&AcceptedProofRule::IntegerAffineBound)
    );
}

#[test]
fn affine_refold_rejects_a_wrong_mapped_endpoint() {
    let mut certificate = fuel_decrement_certificate(42);
    // Forge: claim `5 <= fuel - 1` where the cited premise `2 <= fuel` maps
    // only to `1 <= fuel - 1`; the kernel's affine map must refuse it.
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
    let ProofRule::IntegerAffineBound { .. } = middle_less_or_equal_right.rule else {
        panic!("affine leg");
    };
    let Proposition::IntegerMathLessOrEqual(_, argument) = &middle_less_or_equal_right.conclusion
    else {
        panic!("integer <= conclusion");
    };
    let argument = argument.clone();
    middle_less_or_equal_right.conclusion =
        Proposition::IntegerMathLessOrEqual(literal_term(5), argument);
    assert!(certificate.verify().is_err());
}

#[test]
fn affine_refold_rejects_a_foreign_witness_root() {
    let mut certificate = fuel_decrement_certificate(43);
    // Forge: repoint the affine witness at a different atom, so the cited
    // `2 <= fuel` premise no longer binds the witness's root.
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
    let ProofRule::IntegerAffineBound { witness, .. } = &mut middle_less_or_equal_right.rule else {
        panic!("affine leg");
    };
    let foreign = ValueId::new(999).expect("value id");
    witness.root = ScalarTerm::value(foreign, ScalarType::Integer(u8_type()));
    assert!(certificate.verify().is_err());
}

#[test]
fn affine_refold_gap_stays_uncovered() {
    // Premise `1 <= fuel` maps to `0 <= fuel - 1`, which cannot reach a
    // `[1, 127]` target's lower endpoint: the leg emits nothing rather than a
    // certificate the kernel would refuse.
    let integer_type = u8_type();
    let value = ValueId::new(44).expect("value id");
    let place = ScalarTerm::value(value, ScalarType::Integer(integer_type));
    let place_math = IntegerMathTerm::MathValue {
        source_type: integer_type,
        value,
    };
    let context =
        PropositionContext::from_value_types([(value, ScalarType::Integer(integer_type))])
            .expect("context");
    let scalar_bound = |bound| scalar_integer_term(integer_type, &BigInt::from_i64(bound));
    let assumptions = vec![
        Proposition::LessOrEqual(scalar_bound(1).expect("scalar"), place.clone()),
        Proposition::LessOrEqual(place.clone(), scalar_bound(128).expect("scalar")),
    ];
    assert!(
        refold_bounds_certificate(
            place,
            place_math,
            integer_type,
            BigInt::from_i64(1),
            true,
            assumptions,
            (0, BigInt::from_i64(1)),
            (1, BigInt::from_i64(128)),
            context,
            &integer_range(1, 127),
            44,
        )
        .is_none()
    );
}

#[test]
fn plan_measurement_records_the_kernel_receipt() {
    use crate::checker::measurement::ProofPlanMeasurements;
    use proof_admission::MathematicalCoreDecision;

    let certificate =
        closed_bounds_certificate(literal_term(5), math_literal(0), math_literal(10), 55)
            .expect("certificate");
    let fact = certificate.verify().expect("kernel accepts");

    let mut measurements = ProofPlanMeasurements::default();
    measurements.record_certificate_verdict(super::CertificateVerdict::Certified);
    measurements.record_accepted_fact(&fact);

    let AcceptedFactRoute::CertificateDerived { acceptance, .. } = fact.route else {
        panic!("certificate-derived route");
    };
    let MathematicalCoreDecision::Judged(receipt) = acceptance.mathematical_core else {
        panic!("a closed literal certificate judges in the mathematical core");
    };
    assert_eq!(measurements.certificate_certified, 1);
    assert_eq!(measurements.certificate_attempts(), 1);
    assert_eq!(measurements.kernel_judgments, 1);
    assert_eq!(measurements.kernel_refusals, 0);
    assert_eq!(
        measurements.kernel_declarations,
        u64::from(receipt.declarations)
    );
    assert_eq!(
        measurements.kernel_assumption_closure,
        u64::from(receipt.assumption_closure)
    );
    assert_eq!(
        measurements.kernel_context_depth,
        u64::from(receipt.context_depth)
    );
    assert_eq!(
        measurements.kernel_arena_slots,
        u64::from(receipt.arena_slots)
    );
}

// Derivation-recheck consultation: retained certificates are re-decided by
// the admission kernel on every consult — a hit discharges the leg, a
// kernel-rejected candidate is passed over, a miss and a capacity refusal
// are explicit outcomes. The key scaffold mirrors `derivation_store::tests`:
// a minimal TypedTrees with one machine and two data symbols so
// `proof_obligation_key` produces canonical obligation identities.

use crate::checker::derivation_cache::{DerivationConsultation, ProofDerivationCache};
use crate::obligations::{
    BoundedValueObligation, ProofObligation, ProofObligationOwner, ProofPlan, proof_obligation_key,
};
use symbols::{SymbolHandle, SymbolKind, SymbolNameRef, SymbolTableBuilder};
use typed_trees::TypedTrees;
use typed_trees::name::Identifier;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

struct ConsultationProgram {
    typed_trees: TypedTrees,
    machine: SymbolHandle,
    data: [SymbolHandle; 2],
    int_type: SymbolHandle,
}

fn consultation_program() -> ConsultationProgram {
    let mut builder = SymbolTableBuilder::new();
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let machine = SymbolTableBuilder::child_handles(
        builder.insert_children(root, [(SymbolKind::Machine, SymbolNameRef::Static("Main"))]),
    )
    .next()
    .expect("machine");
    let members = SymbolTableBuilder::child_handles(builder.insert_children(
        machine,
        [
            (SymbolKind::Data, SymbolNameRef::Static("count")),
            (SymbolKind::Data, SymbolNameRef::Static("total")),
            (SymbolKind::BuiltinType, SymbolNameRef::Static("Int")),
        ],
    ))
    .collect::<Vec<_>>();
    ConsultationProgram {
        typed_trees: TypedTrees {
            symbols: builder.finish(),
            ..TypedTrees::default()
        },
        machine,
        data: [members[0], members[1]],
        int_type: members[2],
    }
}

impl ConsultationProgram {
    fn int_reference(&mut self) -> TypeReferenceHandle {
        self.typed_trees
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: self.int_type,
                name: Identifier::generated("Int"),
            })
    }
}

fn key_obligation(
    machine: SymbolHandle,
    machine_name: &str,
    data_symbol: SymbolHandle,
    data_name: &str,
    base: TypeReferenceHandle,
) -> ProofObligation {
    ProofObligation::BoundedValue(BoundedValueObligation {
        owner: ProofObligationOwner::MachineOwnedData {
            machine_symbol: machine,
            machine: Identifier::generated(machine_name),
            data_symbol,
            data: Identifier::generated(data_name),
        },
        base_type: base,
        constraints: arena::HandleSpan::empty(),
    })
}

#[test]
fn retained_certificate_re_decides_for_a_semantically_identical_obligation() {
    let mut program = consultation_program();
    let base = program.int_reference();
    let plan = ProofPlan::new(&program.typed_trees);
    let mut cache = ProofDerivationCache::new();

    let certificate =
        closed_bounds_certificate(literal_term(5), math_literal(0), math_literal(10), 61)
            .expect("certificate");
    let proposition = certificate.obligation.proposition.clone();
    let obligation = key_obligation(program.machine, "Main", program.data[0], "count", base);
    DerivationConsultation::new(&plan, &obligation, &mut cache).retain(certificate);
    assert_eq!(cache.report().retained, 1);
    assert_eq!(cache.len(), 1);

    // A display rename beside the same resolved symbols is the same
    // semantic obligation: the retained package is re-decided by the kernel
    // and the leg discharges without the producer running again.
    let renamed = key_obligation(program.machine, "Renamed", program.data[0], "renamed", base);
    let fact = DerivationConsultation::new(&plan, &renamed, &mut cache)
        .recheck()
        .expect("retained candidate re-decides");
    assert_eq!(fact.proposition, proposition);
    let report = cache.report();
    assert_eq!(report.consultations, 1);
    assert_eq!(report.reused, 1);
    assert_eq!(report.rejected_candidates, 0);
}

#[test]
fn recheck_misses_an_obligation_with_no_retained_candidates() {
    let mut program = consultation_program();
    let base = program.int_reference();
    let plan = ProofPlan::new(&program.typed_trees);
    let mut cache = ProofDerivationCache::new();

    let certificate =
        closed_bounds_certificate(literal_term(5), math_literal(0), math_literal(10), 62)
            .expect("certificate");
    let stored = key_obligation(program.machine, "Main", program.data[0], "count", base);
    DerivationConsultation::new(&plan, &stored, &mut cache).retain(certificate);

    // A different resolved owner is a different obligation: a miss, not a
    // collision.
    let other = key_obligation(program.machine, "Main", program.data[1], "count", base);
    assert!(
        DerivationConsultation::new(&plan, &other, &mut cache)
            .recheck()
            .is_none()
    );
    let report = cache.report();
    assert_eq!(report.consultations, 1);
    assert_eq!(report.reused, 0);
}

#[test]
fn kernel_rejected_candidates_are_passed_over_not_accepted() {
    let mut program = consultation_program();
    let base = program.int_reference();
    let plan = ProofPlan::new(&program.typed_trees);
    let mut cache = ProofDerivationCache::new();
    let obligation = key_obligation(program.machine, "Main", program.data[0], "count", base);

    // A stale or hostile entry under the right key gets the same treatment
    // as an honest one: the kernel re-decides it. This package's own claim
    // (`300 <= 5`) is false, so it can never discharge a leg.
    let forged = closed_bounds_certificate(literal_term(300), math_literal(0), math_literal(5), 63)
        .expect("certificate builds; the kernel rejects it");
    DerivationConsultation::new(&plan, &obligation, &mut cache).retain(forged);
    let sound = closed_bounds_certificate(literal_term(5), math_literal(0), math_literal(10), 64)
        .expect("certificate");
    DerivationConsultation::new(&plan, &obligation, &mut cache).retain(sound);

    let fact = DerivationConsultation::new(&plan, &obligation, &mut cache)
        .recheck()
        .expect("the sound candidate still discharges the leg");
    assert!(matches!(
        fact.route,
        AcceptedFactRoute::CertificateDerived { .. }
    ));
    let report = cache.report();
    assert_eq!(report.consultations, 1);
    assert_eq!(report.rejected_candidates, 1);
    assert_eq!(report.reused, 1);
}

#[test]
fn capacity_refusal_is_explicit_and_keeps_the_verdict_path() {
    let mut program = consultation_program();
    let base = program.int_reference();
    let plan = ProofPlan::new(&program.typed_trees);
    let mut cache = ProofDerivationCache::with_capacity(0);
    let obligation = key_obligation(program.machine, "Main", program.data[0], "count", base);

    let certificate =
        closed_bounds_certificate(literal_term(5), math_literal(0), math_literal(10), 65)
            .expect("certificate");
    DerivationConsultation::new(&plan, &obligation, &mut cache).retain(certificate);

    // The refusal is counted, nothing is silently evicted, and the leg that
    // produced the certificate was already discharged by the kernel.
    assert!(cache.is_empty());
    assert_eq!(cache.report().capacity_refusals, 1);
}

#[test]
fn dependency_change_misses_and_invalidate_reclaims_the_stale_row() {
    let mut program = consultation_program();
    let base = program.int_reference();
    let plan = ProofPlan::new(&program.typed_trees);
    let mut cache = ProofDerivationCache::new();

    let certificate =
        closed_bounds_certificate(literal_term(5), math_literal(0), math_literal(10), 66)
            .expect("certificate");
    let count = key_obligation(program.machine, "Main", program.data[0], "count", base);
    DerivationConsultation::new(&plan, &count, &mut cache).retain(certificate);

    // A dependency change — here a different resolved owner — produces a
    // different key: the stale row is never consulted, not even to reject.
    let moved = key_obligation(program.machine, "Main", program.data[1], "count", base);
    assert!(
        DerivationConsultation::new(&plan, &moved, &mut cache)
            .recheck()
            .is_none()
    );

    // Unreachable evidence still occupies arena storage until the cache's
    // key-granularity invalidate reclaims it; neighbors survive untouched.
    // `total` on `data[1]` is a distinct row — `moved` consulted earlier
    // before anything was retained under its key, so it stayed a miss.
    let total = key_obligation(program.machine, "Main", program.data[1], "total", base);
    let neighbor =
        closed_bounds_certificate(literal_term(3), math_literal(0), math_literal(10), 67)
            .expect("certificate");
    DerivationConsultation::new(&plan, &total, &mut cache).retain(neighbor);

    let stale_key = proof_obligation_key(&plan, &count);
    assert_eq!(cache.invalidate(&stale_key), 1);
    assert_eq!(cache.len(), 1);
    assert_eq!(cache.key_count(), 1);

    // What was unreachable is now absent: re-consulting misses, the neighbor
    // still discharges its leg, and dropping a never-stored key is an
    // explicit no-op.
    assert!(
        DerivationConsultation::new(&plan, &count, &mut cache)
            .recheck()
            .is_none()
    );
    assert!(
        DerivationConsultation::new(&plan, &total, &mut cache)
            .recheck()
            .is_some(),
        "the surviving row still discharges its leg"
    );
    let ghost = key_obligation(program.machine, "Main", program.int_type, "ghost", base);
    assert_eq!(cache.invalidate(&proof_obligation_key(&plan, &ghost)), 0);

    let report = cache.report();
    assert_eq!(report.consultations, 3);
    assert_eq!(report.reused, 1);
    assert_eq!(report.retained, 2);
}
