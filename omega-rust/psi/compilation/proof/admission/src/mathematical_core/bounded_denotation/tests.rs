use crate::proof::{assumption_proof, semantic_axiom_proof};
use std::collections::BTreeSet;

use semantic_vocabulary::{
    ContentAlgebra, ContentAlgebraKind, ContentConservation, ContentDomainId, ContentPlaceSegment,
    ContentPlaceVersion, ContentProjectionIdentity, ContentStructuralPlace, ContentTerm,
    IntegerSign, IntegerType, IntegerValue, PlaceId, PropositionId, ScalarType,
    StructuralPlaceKind, ValueId,
};
use terminal_psi::PrimitiveJudgment;

use super::test_utilities::denote_via_assumption_premise;
use super::{
    AcceptedPremise, AcceptedProofRule, BoundedDenotation, BoundedDenotationError, Elaboration,
    ProofError, ProofNode, ProofRule, Proposition, PropositionContext, ScalarTerm, Term,
    denote_bounded_certificate, verify_bounded_certificate,
};
use crate::mathematical_core::{
    Budget, DEFAULT_CONVERSION_STEPS, certificate_assumption_closure,
    verify_mathematical_certificate,
};

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

fn boolean_value(id: u64) -> (ValueId, ScalarTerm) {
    let id = ValueId::new(id).expect("value id");
    (id, ScalarTerm::value(id, ScalarType::Boolean))
}

fn literal(value: u128) -> ScalarTerm {
    ScalarTerm::integer(unsigned64(), IntegerValue::Unsigned(value)).expect("u64 literal")
}

fn atom(index: u64) -> Proposition {
    Proposition::Atom(PropositionId::new(index).expect("atom id"))
}

/// The contract-entailment discharge shape: `[x <= y] ⊢ x <= y` by
/// `Assumption{0}` — the judgment every shipped producer emits for an
/// assumption discharge, re-decided by the kernel here.
#[test]
fn contract_entailment_discharge_shape_is_a_kernel_judgment() {
    let (x_id, x) = value(1);
    let (y_id, y) = value(2);
    let bound = Proposition::LessOrEqual(x, y);
    let context = PropositionContext::from_value_types([
        (x_id, unsigned64_type()),
        (y_id, unsigned64_type()),
    ])
    .expect("context");
    let proof = assumption_proof(bound.clone(), 0);
    let denoted = verify_bounded_certificate(
        &context,
        &bound,
        std::slice::from_ref(&bound),
        &[],
        &proof,
        &mut budget(),
    )
    .expect("the discharge judgment verifies in the kernel");

    // `x <= y` over integers lifts to `IntLe m_x m_y`: the
    // signature is `{Int : Type 0, IntLe : Π(_:Int).Π(_:Int).Type 0,
    // m_x : Int, m_y : Int}` and the judgment
    // `Γ; [IntLe m_x m_y] ⊢ p : IntLe m_x m_y` — the term is the
    // bound variable.
    assert_eq!(denoted.certificate.signature.len(), 4);
    assert!(denoted.certificate.signature[0].is_assumption());
    assert_eq!(denoted.certificate.context.len(), 1);
    assert_eq!(
        denoted.arena.get(denoted.certificate.term),
        Term::Variable(0)
    );
    assert_eq!(
        certificate_assumption_closure(&denoted.arena, &denoted.certificate),
        BTreeSet::from([0, 1, 2, 3]),
        "the judgment commits to the carrier, the relation and both endpoints",
    );
    assert_eq!(
        denoted.assumptions,
        vec![AcceptedPremise {
            index: 0,
            proposition: bound,
        }],
    );
    assert_eq!(denoted.rules, vec![AcceptedProofRule::Assumption],);
}

/// A discharged hypothesis is a λ binder inside the evidence, never a
/// judgment premise: `(P ∧ Q) → (Q ∧ P)` is a closed proof whose kernel
/// judgment has an empty context.
#[test]
fn discharged_hypotheses_stay_inside_the_evidence_term() {
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
    let denoted = verify_bounded_certificate(
        &PropositionContext::default(),
        &goal,
        &[],
        &[],
        &proof,
        &mut budget(),
    )
    .expect("the swap proof verifies");
    assert_eq!(denoted.certificate.signature.len(), 2);
    assert!(
        denoted.certificate.context.is_empty(),
        "the pushed hypothesis is bound inside the term, not the context",
    );
    assert!(
        denoted.assumptions.is_empty(),
        "a discharged hypothesis is not an ambient premise",
    );
    assert_eq!(
        certificate_assumption_closure(&denoted.arena, &denoted.certificate),
        BTreeSet::from([0, 1]),
        "the judgment names both atoms the goal commits to",
    );
}

/// `Equal` denotes the relevant `Id`, so equality symmetry and
/// transitivity become `J` eliminations the kernel re-decides.
#[test]
fn equality_rules_become_identity_eliminations() {
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
    let second = Proposition::Equal(y.clone(), z.clone());
    let goal = Proposition::Equal(x, z);
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::EqualityTransitivity {
            left_equals_middle: Box::new(assumption_proof(first.clone(), 0)),
            middle_equals_right: Box::new(assumption_proof(second.clone(), 1)),
        },
    };
    let denoted = verify_bounded_certificate(
        &context,
        &goal,
        &[first, second],
        &[],
        &proof,
        &mut budget(),
    )
    .expect("transitivity verifies as J");
    // `Int` plus one assumption constant per mathematical endpoint;
    // the lifted `Id Int` chain composes by `J`.
    assert_eq!(denoted.certificate.signature.len(), 4);
    assert_eq!(denoted.certificate.context.len(), 2);
    assert_eq!(
        certificate_assumption_closure(&denoted.arena, &denoted.certificate),
        BTreeSet::from([0, 1, 2, 3]),
    );

    // Symmetry: `x = y ⊢ y = x`.
    let flipped = Proposition::Equal(
        ScalarTerm::value(y_id, unsigned64_type()),
        ScalarTerm::value(x_id, unsigned64_type()),
    );
    let source = Proposition::Equal(
        ScalarTerm::value(x_id, unsigned64_type()),
        ScalarTerm::value(y_id, unsigned64_type()),
    );
    let proof = ProofNode {
        conclusion: flipped.clone(),
        rule: ProofRule::EqualitySymmetry {
            equality: Box::new(assumption_proof(source.clone(), 0)),
        },
    };
    let denoted =
        verify_bounded_certificate(&context, &flipped, &[source], &[], &proof, &mut budget())
            .expect("symmetry verifies");
    // `{Int, m_x, m_y}` — canonical `IntegerMathEqual` order makes
    // the flipped goal's denotation the premise's own, so the cited
    // variable is the whole evidence.
    assert_eq!(denoted.certificate.signature.len(), 3);
    assert_eq!(
        denoted.arena.get(denoted.certificate.term),
        Term::Variable(0),
    );
    assert_eq!(
        certificate_assumption_closure(&denoted.arena, &denoted.certificate),
        BTreeSet::from([0, 1, 2]),
    );
}

/// An integer order rule whose instance lands wholly in the `Int`
/// vocabulary denotes a roster-law application: `x < y ⊢ x <= y`
/// elaborates to `lt_le m_x m_y p`, the shared relation check
/// re-decides the instance before the law is cited, and the closure
/// names the law exactly.
#[test]
fn order_rules_become_named_law_citations() {
    let (x_id, x) = value(1);
    let (y_id, y) = value(2);
    let context = PropositionContext::from_value_types([
        (x_id, unsigned64_type()),
        (y_id, unsigned64_type()),
    ])
    .expect("context");
    let strict = Proposition::LessThan(x.clone(), y.clone());
    let weakened = Proposition::LessOrEqual(x.clone(), y.clone());
    let proof = ProofNode {
        conclusion: weakened.clone(),
        rule: ProofRule::IntegerOrderWeakening {
            relation: Box::new(assumption_proof(strict.clone(), 0)),
        },
    };
    let denoted = verify_bounded_certificate(
        &context,
        &weakened,
        std::slice::from_ref(&strict),
        &[],
        &proof,
        &mut budget(),
    )
    .expect("order weakening verifies as a roster citation");
    // The signature is `{Int, IntLt, m_x, m_y, IntLe, lt_le}`: the
    // evidence is the roster constant applied to the endpoints and
    // the cited premise variable — `lt_le m_x m_y p`.
    assert_eq!(denoted.certificate.signature.len(), 6);
    let mut head = denoted.certificate.term;
    for _ in 0..3 {
        let Term::Apply { function, .. } = denoted.arena.get(head) else {
            panic!("a roster citation applies endpoints then evidence");
        };
        head = function;
    }
    assert!(matches!(denoted.arena.get(head), Term::Constant { .. }));
    let Term::Apply { argument, .. } = denoted.arena.get(denoted.certificate.term) else {
        unreachable!("checked above");
    };
    assert_eq!(denoted.arena.get(argument), Term::Variable(0));
    assert_eq!(
        certificate_assumption_closure(&denoted.arena, &denoted.certificate),
        BTreeSet::from([0, 1, 2, 3, 4, 5]),
        "the judgment names the vocabulary, both endpoints and the law",
    );
    assert_eq!(
        denoted.rules,
        vec![
            AcceptedProofRule::Assumption,
            AcceptedProofRule::IntegerOrderWeakening
        ],
    );

    // A relation the rule does not license — a nonstrict premise —
    // is the same `RulePremiseMismatch` the bounded checker reports,
    // never an axiom.
    let proof = ProofNode {
        conclusion: weakened.clone(),
        rule: ProofRule::IntegerOrderWeakening {
            relation: Box::new(assumption_proof(weakened.clone(), 0)),
        },
    };
    assert!(matches!(
        denote_bounded_certificate(
            &context,
            &weakened,
            std::slice::from_ref(&weakened),
            &[],
            &proof,
        ),
        Err(BoundedDenotationError::Certificate(
            ProofError::RulePremiseMismatch(_)
        )),
    ));
}

/// Mathematical-integer transitivity — `IntegerMathEqual` premises
/// and the normalized `IntegerMathEqual` conclusion of an `Equal`
/// chain alike — denotes `J` on the `Int` vocabulary; the
/// orientation fixups ride on canonical endpoint order.
#[test]
fn integer_math_transitivity_becomes_identity_eliminations() {
    // `IntegerMathEqual` transitivity: `a = b`, `b = c` ⊢ `a = c`
    // over mathematical literals.
    let math =
        |value: u128| semantic_vocabulary::IntegerMathTerm::literal(IntegerValue::Unsigned(value));
    let first = Proposition::IntegerMathEqual(math(1), math(2));
    let second = Proposition::IntegerMathEqual(math(2), math(3));
    let goal = Proposition::IntegerMathEqual(math(1), math(3));
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::EqualityTransitivity {
            left_equals_middle: Box::new(assumption_proof(first.clone(), 0)),
            middle_equals_right: Box::new(assumption_proof(second.clone(), 1)),
        },
    };
    let denoted = verify_bounded_certificate(
        &PropositionContext::default(),
        &goal,
        &[first, second],
        &[],
        &proof,
        &mut budget(),
    )
    .expect("mathematical-integer transitivity verifies as J");
    assert!(matches!(
        denoted.arena.get(denoted.certificate.term),
        Term::IdElim { .. },
    ));
    assert_eq!(
        denoted.rules,
        vec![
            AcceptedProofRule::Assumption,
            AcceptedProofRule::EqualityTransitivity
        ],
    );

    // The normalized crossing: `Equal` premises composing to the
    // lifted `IntegerMathEqual` conclusion.
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
    let second = Proposition::Equal(y.clone(), z.clone());
    let goal = crate::lift_fixed_integer_relation(&Proposition::Equal(x, z)).expect("lifts");
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::EqualityTransitivity {
            left_equals_middle: Box::new(assumption_proof(first.clone(), 0)),
            middle_equals_right: Box::new(assumption_proof(second.clone(), 1)),
        },
    };
    let denoted = verify_bounded_certificate(
        &context,
        &goal,
        &[first, second],
        &[],
        &proof,
        &mut budget(),
    )
    .expect("the normalized conclusion verifies as J");
    assert!(matches!(
        denoted.arena.get(denoted.certificate.term),
        Term::IdElim { .. },
    ));
}

/// Identical denotations reuse evidence; changing a value endpoint
/// transports it by J without assuming an order-substitution law.
#[test]
fn conversion_and_value_transport_use_existing_evidence_and_identity_elimination() {
    let (x_id, x) = value(1);
    let (y_id, y) = value(2);
    let (z_id, z) = value(3);
    let context = PropositionContext::from_value_types([
        (x_id, unsigned64_type()),
        (y_id, unsigned64_type()),
        (z_id, unsigned64_type()),
    ])
    .expect("context");

    // `PredicateDenotation`: `y == x` converts to its canonical `x
    // == y` — different propositions, one canonical `Id Int`
    // denotation, so the cited premise variable is the whole
    // evidence with no axiom at all.
    let premise = Proposition::Equal(y.clone(), x.clone());
    let goal = Proposition::Equal(x, y);
    let denoted = denote_via_assumption_premise(
        &context,
        &goal,
        &premise,
        "predicate denotation verifies definitionally",
    );
    assert_eq!(
        denoted.arena.get(denoted.certificate.term),
        Term::Variable(0),
    );

    // `ValueEqualityTransport`: `x <= y` under the proved `x == z`
    // transports to `z <= y` using the supplied equation itself.
    let premise = Proposition::LessOrEqual(
        ScalarTerm::value(x_id, unsigned64_type()),
        ScalarTerm::value(y_id, unsigned64_type()),
    );
    let equation = Proposition::Equal(ScalarTerm::value(x_id, unsigned64_type()), z.clone());
    let goal = Proposition::LessOrEqual(z, ScalarTerm::value(y_id, unsigned64_type()));
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::ValueEqualityTransport {
            premise: Box::new(assumption_proof(premise.clone(), 0)),
            equalities: vec![assumption_proof(equation.clone(), 1)],
        },
    };
    let denoted = verify_bounded_certificate(
        &context,
        &goal,
        &[premise, equation],
        &[],
        &proof,
        &mut budget(),
    )
    .expect("value equality transport verifies through J");
    assert!(matches!(
        denoted.arena.get(denoted.certificate.term),
        Term::IdElim { .. }
    ));
    // Int, its three values and IntLe; no substitution-law assumption.
    assert_eq!(denoted.certificate.signature.len(), 5);
    assert_eq!(
        denoted.rules,
        vec![
            AcceptedProofRule::Assumption,
            AcceptedProofRule::ValueEqualityTransport
        ],
    );
}

/// `IntegerOrderSubstitution` cites the roster's substitution law
/// over the `Int` denotations of the relation and equality
/// premises; the substituted endpoint is the shared check's
/// decision, not a separate denotation.
#[test]
fn order_substitution_becomes_a_named_law_citation() {
    let (x_id, x) = value(1);
    let (y_id, y) = value(2);
    let (z_id, z) = value(3);
    let context = PropositionContext::from_value_types([
        (x_id, unsigned64_type()),
        (y_id, unsigned64_type()),
        (z_id, unsigned64_type()),
    ])
    .expect("context");
    let relation = Proposition::LessOrEqual(x.clone(), y);
    let equality = Proposition::Equal(x, z.clone());
    let goal = Proposition::LessOrEqual(z.clone(), ScalarTerm::value(y_id, unsigned64_type()));
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::IntegerOrderSubstitution {
            relation: Box::new(assumption_proof(relation.clone(), 0)),
            equality: Box::new(assumption_proof(equality.clone(), 1)),
            endpoint: 0,
        },
    };
    let denoted = verify_bounded_certificate(
        &context,
        &goal,
        &[relation, equality],
        &[],
        &proof,
        &mut budget(),
    )
    .expect("order substitution verifies as a roster citation");
    // `le_subst_left m_x m_z m_y eq p` — five `Apply`s to the law
    // constant: three endpoints, the equation, the relation.
    let mut head = denoted.certificate.term;
    for _ in 0..5 {
        let Term::Apply { function, .. } = denoted.arena.get(head) else {
            panic!("a substitution citation applies endpoints then evidence");
        };
        head = function;
    }
    assert!(matches!(denoted.arena.get(head), Term::Constant { .. }));

    // Substituting an endpoint the equality does not license is the
    // shared checker's mismatch, never an axiom.
    let wrong = Proposition::LessOrEqual(z.clone(), literal(9));
    let proof = ProofNode {
        conclusion: wrong.clone(),
        rule: ProofRule::IntegerOrderSubstitution {
            relation: Box::new(assumption_proof(
                Proposition::LessOrEqual(
                    ScalarTerm::value(x_id, unsigned64_type()),
                    ScalarTerm::value(y_id, unsigned64_type()),
                ),
                0,
            )),
            equality: Box::new(assumption_proof(
                Proposition::Equal(ScalarTerm::value(x_id, unsigned64_type()), z.clone()),
                1,
            )),
            endpoint: 0,
        },
    };
    assert!(matches!(
        denote_bounded_certificate(
            &context,
            &wrong,
            &[
                Proposition::LessOrEqual(
                    ScalarTerm::value(x_id, unsigned64_type()),
                    ScalarTerm::value(y_id, unsigned64_type()),
                ),
                Proposition::Equal(ScalarTerm::value(x_id, unsigned64_type()), z),
            ],
            &[],
            &proof,
        ),
        Err(BoundedDenotationError::Certificate(_)),
    ));
}

/// A decided closed equality denotes a reflexive identity: `2 + 0 = 2`
/// is proved by the kernel's own `refl`, not by an admitted decision.
#[test]
fn a_decided_closed_equality_is_refl_in_the_kernel() {
    let two = literal(2);
    let zero = literal(0);
    let sum =
        ScalarTerm::wrapping_integer_add(unsigned64(), two.clone(), zero).expect("wrapping add");
    let goal = Proposition::Equal(sum, two);
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
    };
    let denoted = verify_bounded_certificate(
        &PropositionContext::default(),
        &goal,
        &[],
        &[],
        &proof,
        &mut budget(),
    )
    .expect("the decided equality denotes a refl-provable Id");
    // Int, zero, odd and double plus two checked numeral definitions;
    // the evidence is `refl Int 2`, with no decision assumption.
    assert_eq!(denoted.certificate.signature.len(), 6);
    assert!(denoted.certificate.context.is_empty());
    assert!(matches!(
        denoted.arena.get(denoted.certificate.term),
        Term::Refl { .. }
    ));
    assert_eq!(
        certificate_assumption_closure(&denoted.arena, &denoted.certificate),
        BTreeSet::from([0, 1, 2, 4]),
    );
}

/// A `Primitive` leaf the calculus cannot prove definitionally — a
/// decided closed order `1 < 2` — is a named decision assumption in the
/// signature, so the closure names the bounded decision exactly.
#[test]
fn a_decision_assumption_is_named_in_the_closure() {
    let goal = Proposition::LessThan(literal(1), literal(2));
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
    };
    let denoted = verify_bounded_certificate(
        &PropositionContext::default(),
        &goal,
        &[],
        &[],
        &proof,
        &mut budget(),
    )
    .expect("the decided order verifies");
    // `IntLt` is a relation, not an `Id`, so the decided `1 < 2`
    // names a decision assumption of type `IntLt m₁ m₂` — the
    // Endpoints are interned before the relation, so the signature is
    // `{Int, zero, odd, one:=odd zero, double, two:=double one,
    // IntLt, decision}`. Literal definitions do
    // not appear as assumptions; their vocabulary does.
    assert_eq!(denoted.certificate.signature.len(), 8);
    assert_eq!(
        certificate_assumption_closure(&denoted.arena, &denoted.certificate),
        BTreeSet::from([0, 1, 2, 4, 6, 7]),
        "carrier, relation, numeral vocabulary and the named decision",
    );

    // `Truth` is definitional: no signature, no decision assumption.
    let proof = ProofNode {
        conclusion: Proposition::Truth,
        rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
    };
    let denoted = verify_bounded_certificate(
        &PropositionContext::default(),
        &Proposition::Truth,
        &[],
        &[],
        &proof,
        &mut budget(),
    )
    .expect("truth verifies");
    assert!(denoted.certificate.signature.is_empty());
    assert!(certificate_assumption_closure(&denoted.arena, &denoted.certificate).is_empty());
}

/// A disjunction denotes the tagged sum `Σ(t : Two). caseTwo(M, d₀,
/// rest, t)`: introduction pairs a `zero`/`one` tag with the nested
/// payload, and elimination is the dependent `caseTwo` applied to the
/// payload — for three disjuncts the one-branch's nested sum is an
/// intermediate binder while each branch's hypothesis stays one cited
/// premise.
#[test]
fn disjunction_rules_become_tagged_sum_constructions() {
    let p = atom(1);
    let q = atom(2);
    let r = atom(3);
    let truth_node = || ProofNode {
        conclusion: Proposition::Truth,
        rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
    };

    // Introduction: `[P] ⊢ P ∨ Q` pairs `zero` with the cited premise.
    let disjunction = Proposition::Disjunction(vec![p.clone(), q.clone()]);
    let proof = ProofNode {
        conclusion: disjunction.clone(),
        rule: ProofRule::DisjunctionIntroduction {
            disjunct: Box::new(assumption_proof(p.clone(), 0)),
            index: 0,
        },
    };
    let denoted = verify_bounded_certificate(
        &PropositionContext::default(),
        &disjunction,
        std::slice::from_ref(&p),
        &[],
        &proof,
        &mut budget(),
    )
    .expect("disjunction introduction verifies as a tagged pair");
    assert!(matches!(
        denoted.arena.get(denoted.certificate.term),
        Term::Pair { .. }
    ));
    assert_eq!(
        certificate_assumption_closure(&denoted.arena, &denoted.certificate),
        BTreeSet::from([0, 1]),
        "the tagged sum names both disjunct atoms",
    );

    // Every other position pairs `one` with the payload the sum's
    // `one` branch actually holds: the bare last disjunct of a binary
    // sum, or the nested tagged pair of a wider one. The kernel checks
    // the payload at `caseTwo(M, d₀, rest, one) ≡ rest`, so a payload
    // nested one level too deep is a type error, not a shape quibble.
    for (disjuncts, index) in [
        (vec![p.clone(), q.clone()], 1),
        (vec![p.clone(), q.clone(), r.clone()], 1),
        (vec![p.clone(), q.clone(), r.clone()], 2),
    ] {
        let selected = disjuncts[index].clone();
        let disjunction = Proposition::Disjunction(disjuncts);
        let proof = ProofNode {
            conclusion: disjunction.clone(),
            rule: ProofRule::DisjunctionIntroduction {
                disjunct: Box::new(assumption_proof(selected.clone(), 0)),
                index,
            },
        };
        let denoted = verify_bounded_certificate(
            &PropositionContext::default(),
            &disjunction,
            std::slice::from_ref(&selected),
            &[],
            &proof,
            &mut budget(),
        )
        .unwrap_or_else(|error| {
            panic!("disjunct {index} of {disjunction:?} verifies as a tagged pair: {error}")
        });
        let Term::Pair { first, .. } = denoted.arena.get(denoted.certificate.term) else {
            panic!("a disjunction introduction denotes a pair");
        };
        assert!(matches!(denoted.arena.get(first), Term::TwoOne));
    }

    // Two-branch elimination under one cited disjunction.
    let proof = ProofNode {
        conclusion: Proposition::Truth,
        rule: ProofRule::DisjunctionElimination {
            disjunction: Box::new(assumption_proof(disjunction.clone(), 0)),
            branches: vec![truth_node(), truth_node()],
        },
    };
    let denoted = verify_bounded_certificate(
        &PropositionContext::default(),
        &Proposition::Truth,
        &[disjunction],
        &[],
        &proof,
        &mut budget(),
    )
    .expect("two-branch elimination verifies as dependent caseTwo");
    assert!(matches!(
        denoted.arena.get(denoted.certificate.term),
        Term::Apply { .. }
    ));

    // Three-branch elimination nests one intermediate tagged-sum
    // hypothesis the branches never cite.
    let disjunction = Proposition::Disjunction(vec![p, q, r]);
    let proof = ProofNode {
        conclusion: Proposition::Truth,
        rule: ProofRule::DisjunctionElimination {
            disjunction: Box::new(assumption_proof(disjunction.clone(), 0)),
            branches: vec![truth_node(), truth_node(), truth_node()],
        },
    };
    let denoted = verify_bounded_certificate(
        &PropositionContext::default(),
        &Proposition::Truth,
        std::slice::from_ref(&disjunction),
        &[],
        &proof,
        &mut budget(),
    )
    .expect("three-branch elimination verifies through the nested sum");
    assert_eq!(denoted.certificate.signature.len(), 3);
    assert_eq!(
        certificate_assumption_closure(&denoted.arena, &denoted.certificate),
        BTreeSet::from([0, 1, 2]),
    );
    assert_eq!(
        denoted.assumptions,
        vec![AcceptedPremise {
            index: 0,
            proposition: disjunction,
        }],
    );
}

/// Semantic axioms bind after the ambient assumptions; a cited axiom is
/// a context variable and a recorded premise.
#[test]
fn semantic_axioms_bind_after_the_assumption_roster() {
    let p = atom(1);
    let implication = Proposition::Implication {
        premise: Box::new(p.clone()),
        conclusion: Box::new(p.clone()),
    };
    let goal = Proposition::Implication {
        premise: Box::new(implication.clone()),
        conclusion: Box::new(p.clone()),
    };
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::ImplicationIntroduction {
            body: Box::new(ProofNode {
                conclusion: p.clone(),
                rule: ProofRule::ImplicationElimination {
                    implication: Box::new(assumption_proof(implication, 0)),
                    premise: Box::new(semantic_axiom_proof(p.clone(), 0)),
                },
            }),
        },
    };
    let denoted = verify_bounded_certificate(
        &PropositionContext::default(),
        &goal,
        &[],
        std::slice::from_ref(&p),
        &proof,
        &mut budget(),
    )
    .expect("the axiom application verifies");
    assert_eq!(denoted.certificate.signature.len(), 1);
    assert_eq!(denoted.certificate.context.len(), 1);
    assert_eq!(
        denoted.semantic_axioms,
        vec![AcceptedPremise {
            index: 0,
            proposition: p,
        }],
    );
    assert_eq!(
        certificate_assumption_closure(&denoted.arena, &denoted.certificate),
        BTreeSet::from([0]),
    );
}

/// Forged and malformed certificates reject at the same points the
/// bounded checker rejects them; a tampered denoted certificate rejects
/// in the kernel.
#[test]
fn forged_and_malformed_certificates_reject() {
    let (x_id, x) = value(1);
    let (y_id, y) = value(2);
    let context = PropositionContext::from_value_types([
        (x_id, unsigned64_type()),
        (y_id, unsigned64_type()),
    ])
    .expect("context");
    let bound = Proposition::LessOrEqual(x, y.clone());
    let swapped = Proposition::LessOrEqual(y, ScalarTerm::value(x_id, unsigned64_type()));

    // A citation past the roster is an unknown assumption.
    let proof = assumption_proof(bound.clone(), 1);
    assert!(matches!(
        denote_bounded_certificate(&context, &bound, std::slice::from_ref(&bound), &[], &proof),
        Err(BoundedDenotationError::Certificate(
            ProofError::UnknownAssumption(1)
        )),
    ));

    // A citation whose conclusion is not the retained premise mismatches.
    let proof = assumption_proof(swapped.clone(), 0);
    assert!(matches!(
        denote_bounded_certificate(
            &context,
            &swapped,
            std::slice::from_ref(&bound),
            &[],
            &proof
        ),
        Err(BoundedDenotationError::Certificate(
            ProofError::AssumptionConclusionMismatch(0)
        )),
    ));

    // A proof of the obligation's sibling does not discharge it.
    let proof = assumption_proof(bound.clone(), 0);
    assert!(matches!(
        denote_bounded_certificate(
            &context,
            &Proposition::Truth,
            std::slice::from_ref(&bound),
            &[],
            &proof
        ),
        Err(BoundedDenotationError::Certificate(
            ProofError::CertificateConclusionMismatch
        )),
    ));

    // A primitive label on a false relation never decides it.
    let falsehood = Proposition::LessThan(literal(2), literal(1));
    let proof = ProofNode {
        conclusion: falsehood.clone(),
        rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
    };
    assert!(matches!(
        denote_bounded_certificate(&PropositionContext::default(), &falsehood, &[], &[], &proof,),
        Err(BoundedDenotationError::Certificate(
            ProofError::PrimitiveJudgment(_)
        )),
    ));

    // A kernel-tampered term rejects: the variable is unbound.
    let proof = assumption_proof(bound.clone(), 0);
    let mut denoted =
        denote_bounded_certificate(&context, &bound, std::slice::from_ref(&bound), &[], &proof)
            .expect("denotation succeeds before tampering");
    denoted.certificate.term = denoted.arena.insert(Term::Variable(9));
    assert!(matches!(
        verify_mathematical_certificate(&mut denoted.arena, &denoted.certificate, &mut budget(),),
        Err(crate::CoreError::UnboundVariable { .. }),
    ));
}

/// The citation-level `Equal`↔`IntegerMathEqual` crossing denotes
/// identically now: both sides name `Id Int` over the shared `Int`
/// vocabulary, so the normalized citation binds the premise's own
/// variable — the `Id`-versus-atom shape crossing is closed.
#[test]
fn the_equal_integer_math_crossing_denotes_identically() {
    let (x_id, x) = value(1);
    let (y_id, y) = value(2);
    let context = PropositionContext::from_value_types([
        (x_id, unsigned64_type()),
        (y_id, unsigned64_type()),
    ])
    .expect("context");
    let fixed = Proposition::Equal(x, y);
    let lifted = crate::lift_fixed_integer_relation(&fixed).expect("lifts");
    let proof = assumption_proof(lifted.clone(), 0);
    assert_eq!(
        crate::check_certificate(&context, &lifted, std::slice::from_ref(&fixed), &[], &proof),
        Ok(()),
        "the bounded checker accepts the normalized citation",
    );
    let denoted = verify_bounded_certificate(
        &context,
        &lifted,
        std::slice::from_ref(&fixed),
        &[],
        &proof,
        &mut budget(),
    )
    .expect("the crossing denotes the same `Id Int` and verifies");
    // `{Int, m_x, m_y}`; the cited variable is the evidence.
    assert_eq!(denoted.certificate.signature.len(), 3);
    assert_eq!(
        denoted.arena.get(denoted.certificate.term),
        Term::Variable(0),
    );
}

/// An uncited ambient premise still belongs to the judgment: the
/// closure is exact about what the *judgment* commits to, which is the
/// full context — the same premise list `accept_certificate` scopes.
#[test]
fn the_closure_names_every_judgment_premise() {
    let p = atom(1);
    let q = atom(2);
    let proof = assumption_proof(p.clone(), 0);
    let denoted = verify_bounded_certificate(
        &PropositionContext::default(),
        &p,
        &[p.clone(), q],
        &[],
        &proof,
        &mut budget(),
    )
    .expect("the citation verifies");
    assert_eq!(
        certificate_assumption_closure(&denoted.arena, &denoted.certificate),
        BTreeSet::from([0, 1]),
        "both context bindings enter the closure even though only the first is cited",
    );
    assert_eq!(
        denoted.assumptions,
        vec![AcceptedPremise {
            index: 0,
            proposition: p,
        }],
        "the acceptance projection still records only the cited premise",
    );
}
mod connectives;
mod content_algebra;
