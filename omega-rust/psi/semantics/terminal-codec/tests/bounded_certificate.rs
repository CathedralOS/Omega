//! The bounded certificate language crossing the canonical mathematical
//! wire: a `terminal_psi::ProofNode` certificate — the shape every shipped
//! producer emits — is denoted by `proof_admission::denote_bounded_certificate`
//! into the common mathematical core, encoded, decoded into a fresh arena,
//! and re-decided by `verify_mathematical_certificate`. The kernel never
//! sees the bounded checker or a producer success flag on this route —
//! only the judgment and its exact assumption closure survive the wire.

use std::collections::BTreeSet;

use proof_admission::{
    Budget, CoreError, DEFAULT_CONVERSION_STEPS, Term, certificate_assumption_closure,
    denote_bounded_certificate, verify_mathematical_certificate,
};
use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext, PropositionId,
    ScalarTerm, ScalarType, ValueId,
};
use terminal_codec::{decode_mathematical_certificate, encode_mathematical_certificate};
use terminal_psi::{PrimitiveJudgment, ProofNode, ProofRule};

fn budget() -> Budget {
    Budget::new(DEFAULT_CONVERSION_STEPS)
}

fn unsigned64() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 64).expect("u64")
}

fn unsigned64_type() -> ScalarType {
    ScalarType::Integer(unsigned64())
}

fn value(id: u64) -> (ValueId, ScalarTerm) {
    let id = ValueId::new(id).expect("value id");
    (id, ScalarTerm::value(id, unsigned64_type()))
}

fn atom(index: u64) -> Proposition {
    Proposition::Atom(PropositionId::new(index).expect("atom id"))
}

/// The contract-entailment discharge shape `[x <= y] ⊢ x <= y` — the
/// judgment `typed-trees-to-checked-trees`'s assumption discharge emits —
/// denoted, encoded, decoded and re-decided end to end.
#[test]
fn a_bounded_discharge_certificate_crosses_the_wire() {
    let (x_id, x) = value(1);
    let (y_id, y) = value(2);
    let bound = Proposition::LessOrEqual(x, y);
    let context = PropositionContext::from_value_types([
        (x_id, unsigned64_type()),
        (y_id, unsigned64_type()),
    ])
    .expect("context");
    let proof = ProofNode {
        conclusion: bound.clone(),
        rule: ProofRule::Assumption { index: 0 },
    };
    let denoted =
        denote_bounded_certificate(&context, &bound, std::slice::from_ref(&bound), &[], &proof)
            .expect("denote");
    let bytes =
        encode_mathematical_certificate(&denoted.arena, &denoted.certificate).expect("encode");

    let mut decoded = decode_mathematical_certificate(&bytes).expect("decode");
    verify_mathematical_certificate(&mut decoded.arena, &decoded.certificate, &mut budget())
        .expect("the decoded judgment re-verifies");
    assert_eq!(
        encode_mathematical_certificate(&decoded.arena, &decoded.certificate)
            .expect("canonical re-encode"),
        bytes,
    );
    assert_eq!(
        certificate_assumption_closure(&decoded.arena, &decoded.certificate),
        BTreeSet::from([0, 1, 2, 3]),
        "the judgment retains Int, IntLe and the two integer endpoints",
    );

    // A receiver that swaps the evidence for a free variable — the forged
    // wire decodes fine but the kernel re-decides it away.
    decoded.certificate.term = decoded.arena.insert(Term::Variable(7));
    assert!(matches!(
        verify_mathematical_certificate(&mut decoded.arena, &decoded.certificate, &mut budget()),
        Err(CoreError::UnboundVariable { .. }),
    ));

    // A claimed type the judgment does not establish — `⟦Falsehood⟧` —
    // rejects on re-decision.
    let mut decoded = decode_mathematical_certificate(&bytes).expect("decode");
    let ty = decoded.arena.insert(Term::Two);
    let left = decoded.arena.insert(Term::TwoZero);
    let right = decoded.arena.insert(Term::TwoOne);
    decoded.certificate.expected = decoded.arena.insert(Term::Id { ty, left, right });
    assert!(matches!(
        verify_mathematical_certificate(&mut decoded.arena, &decoded.certificate, &mut budget()),
        Err(CoreError::TypeMismatch { .. }),
    ));

    // Malformed wire shapes never reach the kernel.
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(decode_mathematical_certificate(&trailing).is_err());
    assert!(decode_mathematical_certificate(&bytes[..bytes.len() - 1]).is_err());
    let mut forged_magic = bytes.clone();
    forged_magic[0] = b'X';
    assert!(decode_mathematical_certificate(&forged_magic).is_err());
}

/// A discharged hypothesis denoted into a λ crosses the wire: the swap
/// proof `(P ∧ Q) → (Q ∧ P)` exercises `Pair`, `Fst`, `Snd` and `Lambda`
/// through the table, and re-decision keeps the same two atoms.
#[test]
fn a_bounded_implication_certificate_crosses_the_wire() {
    let p = atom(1);
    let q = atom(2);
    let conjunction = Proposition::Conjunction(vec![p.clone(), q.clone()]);
    let swapped = Proposition::Conjunction(vec![q.clone(), p.clone()]);
    let goal = Proposition::Implication {
        premise: Box::new(conjunction.clone()),
        conclusion: Box::new(swapped.clone()),
    };
    let hypothesis = |conclusion: Proposition| ProofNode {
        conclusion,
        rule: ProofRule::Assumption { index: 0 },
    };
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::ImplicationIntroduction {
            body: Box::new(ProofNode {
                conclusion: swapped.clone(),
                rule: ProofRule::ConjunctionIntroduction(vec![
                    ProofNode {
                        conclusion: q,
                        rule: ProofRule::ConjunctionElimination {
                            conjunction: Box::new(hypothesis(conjunction.clone())),
                            conjunct: 1,
                        },
                    },
                    ProofNode {
                        conclusion: p,
                        rule: ProofRule::ConjunctionElimination {
                            conjunction: Box::new(hypothesis(conjunction)),
                            conjunct: 0,
                        },
                    },
                ]),
            }),
        },
    };
    let denoted =
        denote_bounded_certificate(&PropositionContext::default(), &goal, &[], &[], &proof)
            .expect("denote");
    let bytes =
        encode_mathematical_certificate(&denoted.arena, &denoted.certificate).expect("encode");
    let mut decoded = decode_mathematical_certificate(&bytes).expect("decode");
    verify_mathematical_certificate(&mut decoded.arena, &decoded.certificate, &mut budget())
        .expect("the discharged judgment re-verifies");
    assert_eq!(
        encode_mathematical_certificate(&decoded.arena, &decoded.certificate)
            .expect("canonical re-encode"),
        bytes,
    );
    assert_eq!(
        certificate_assumption_closure(&decoded.arena, &decoded.certificate),
        BTreeSet::from([0, 1]),
    );
}

/// Scalar equalities denote `Id` over a carrier assumption, so a bounded
/// transitivity chain crosses the wire as the kernel's `J` elimination and
/// re-decides with the carrier-and-endpoints closure.
#[test]
fn a_bounded_equality_certificate_crosses_the_wire() {
    let (x_id, x) = value(1);
    let (y_id, y) = value(2);
    let (z_id, z) = value(3);
    let context = PropositionContext::from_value_types([
        (x_id, unsigned64_type()),
        (y_id, unsigned64_type()),
        (z_id, unsigned64_type()),
    ])
    .expect("context");
    let first = Proposition::Equal(x.clone(), y.clone());
    let second = Proposition::Equal(y, z.clone());
    let goal = Proposition::Equal(x, z);
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::EqualityTransitivity {
            left_equals_middle: Box::new(ProofNode {
                conclusion: first.clone(),
                rule: ProofRule::Assumption { index: 0 },
            }),
            middle_equals_right: Box::new(ProofNode {
                conclusion: second.clone(),
                rule: ProofRule::Assumption { index: 1 },
            }),
        },
    };
    let denoted =
        denote_bounded_certificate(&context, &goal, &[first, second], &[], &proof).expect("denote");
    let bytes =
        encode_mathematical_certificate(&denoted.arena, &denoted.certificate).expect("encode");
    let mut decoded = decode_mathematical_certificate(&bytes).expect("decode");
    verify_mathematical_certificate(&mut decoded.arena, &decoded.certificate, &mut budget())
        .expect("the J elimination re-verifies after decode");
    assert_eq!(
        encode_mathematical_certificate(&decoded.arena, &decoded.certificate)
            .expect("canonical re-encode"),
        bytes,
    );
    assert_eq!(
        certificate_assumption_closure(&decoded.arena, &decoded.certificate),
        BTreeSet::from([0, 1, 2, 3]),
        "carrier plus one constant per endpoint",
    );
}

/// Integer weakening applies one shared law quantified over both endpoints.
/// The wire retains that law and the shared integer vocabulary; it does not
/// replace the application with an assumption of this particular conclusion.
#[test]
fn a_bounded_shared_order_law_crosses_the_wire() {
    let (x_id, x) = value(1);
    let (y_id, y) = value(2);
    let context = PropositionContext::from_value_types([
        (x_id, unsigned64_type()),
        (y_id, unsigned64_type()),
    ])
    .expect("context");
    let strict = Proposition::LessThan(x.clone(), y.clone());
    let goal = Proposition::LessOrEqual(x, y);
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::IntegerOrderWeakening {
            relation: Box::new(ProofNode {
                conclusion: strict.clone(),
                rule: ProofRule::Assumption { index: 0 },
            }),
        },
    };
    let denoted =
        denote_bounded_certificate(&context, &goal, std::slice::from_ref(&strict), &[], &proof)
            .expect("denote");
    let bytes =
        encode_mathematical_certificate(&denoted.arena, &denoted.certificate).expect("encode");
    let mut decoded = decode_mathematical_certificate(&bytes).expect("decode");
    verify_mathematical_certificate(&mut decoded.arena, &decoded.certificate, &mut budget())
        .expect("the shared law application re-verifies after decode");
    assert_eq!(
        encode_mathematical_certificate(&decoded.arena, &decoded.certificate)
            .expect("canonical re-encode"),
        bytes,
    );
    assert_eq!(
        certificate_assumption_closure(&decoded.arena, &decoded.certificate),
        BTreeSet::from([0, 1, 2, 3, 4, 5]),
        "Int, both endpoints, IntLt, IntLe and the universally quantified weakening law",
    );
    let mut law_type = decoded.certificate.signature[5].ty;
    let mut binders = 0;
    while let Term::Pi { codomain, .. } = decoded.arena.get(law_type) {
        binders += 1;
        law_type = codomain;
    }
    assert_eq!(
        binders, 3,
        "the law binds two integers and their strict-order premise"
    );
}

/// A decided closed relation is a named decision assumption on the wire:
/// `1 < 2` uses shared binary literal definitions and a decision of
/// `IntLt one two` in the empty context.
#[test]
fn a_bounded_decision_certificate_crosses_the_wire() {
    let literal = |value: u128| {
        ScalarTerm::integer(unsigned64(), IntegerValue::Unsigned(value)).expect("u64 literal")
    };
    let goal = Proposition::LessThan(literal(1), literal(2));
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
    };
    let denoted =
        denote_bounded_certificate(&PropositionContext::default(), &goal, &[], &[], &proof)
            .expect("denote");
    let bytes =
        encode_mathematical_certificate(&denoted.arena, &denoted.certificate).expect("encode");
    let mut decoded = decode_mathematical_certificate(&bytes).expect("decode");
    verify_mathematical_certificate(&mut decoded.arena, &decoded.certificate, &mut budget())
        .expect("the decision judgment re-verifies");
    assert_eq!(
        certificate_assumption_closure(&decoded.arena, &decoded.certificate),
        BTreeSet::from([0, 1, 2, 3, 5, 7]),
        "Int, IntLt, zero, odd, double and the bounded decision; numeral definitions are not assumptions",
    );
}

/// Fixed and mathematical integer equality share one Id Int judgment.
/// Citation conversion survives the wire without a conversion assumption;
/// changing an endpoint still rejects before encoding.
#[test]
fn a_fixed_integer_equality_citation_crosses_the_mathematical_wire() {
    let (x_id, x) = value(1);
    let (y_id, y) = value(2);
    let context = PropositionContext::from_value_types([
        (x_id, unsigned64_type()),
        (y_id, unsigned64_type()),
    ])
    .expect("context");
    let fixed = Proposition::Equal(x.clone(), y);
    let lifted = proof_admission::lift_fixed_integer_relation(&fixed).expect("lifts");
    let proof = ProofNode {
        conclusion: lifted.clone(),
        rule: ProofRule::Assumption { index: 0 },
    };
    let denoted =
        denote_bounded_certificate(&context, &lifted, std::slice::from_ref(&fixed), &[], &proof)
            .expect("both integer equality forms share their denotation");
    let bytes =
        encode_mathematical_certificate(&denoted.arena, &denoted.certificate).expect("encode");
    let mut decoded = decode_mathematical_certificate(&bytes).expect("decode");
    verify_mathematical_certificate(&mut decoded.arena, &decoded.certificate, &mut budget())
        .expect("the equality citation re-verifies after decode");
    assert_eq!(
        encode_mathematical_certificate(&decoded.arena, &decoded.certificate)
            .expect("canonical re-encode"),
        bytes,
    );
    assert_eq!(
        certificate_assumption_closure(&decoded.arena, &decoded.certificate),
        BTreeSet::from([0, 1, 2]),
        "Int and the two endpoints, without a conversion law",
    );
    assert!(matches!(
        decoded.arena.get(decoded.certificate.expected),
        Term::Id { .. }
    ));

    let changed = Proposition::Equal(x.clone(), x);
    let changed = proof_admission::lift_fixed_integer_relation(&changed).expect("lifts");
    let proof = ProofNode {
        conclusion: changed.clone(),
        rule: ProofRule::Assumption { index: 0 },
    };
    assert!(denote_bounded_certificate(&context, &changed, &[fixed], &[], &proof).is_err());
}
