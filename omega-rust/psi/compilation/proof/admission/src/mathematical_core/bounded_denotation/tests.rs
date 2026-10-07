use crate::proof::{assumption_proof, semantic_axiom_proof};
use std::collections::BTreeSet;

use semantic_vocabulary::{
    ContentAlgebra, ContentAlgebraKind, ContentConservation, ContentDomainId, ContentPlaceSegment,
    ContentPlaceVersion, ContentProjectionIdentity, ContentStructuralPlace, ContentTerm,
    IntegerSign, IntegerType, IntegerValue, PlaceId, PropositionId, ScalarType,
    StructuralPlaceKind, ValueId,
};
use terminal_psi::PrimitiveJudgment;

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
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::PredicateDenotation {
            premise: Box::new(assumption_proof(premise.clone(), 0)),
        },
    };
    let denoted = verify_bounded_certificate(
        &context,
        &goal,
        std::slice::from_ref(&premise),
        &[],
        &proof,
        &mut budget(),
    )
    .expect("predicate denotation verifies definitionally");
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
    // signature is `{Int, IntLt, zero, odd, one:=odd zero,
    // double, two:=double one, decision}`. Literal definitions do
    // not appear as assumptions; their vocabulary does.
    assert_eq!(denoted.certificate.signature.len(), 8);
    assert_eq!(
        certificate_assumption_closure(&denoted.arena, &denoted.certificate),
        BTreeSet::from([0, 1, 2, 3, 5, 7]),
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

fn content_algebra() -> ContentAlgebra {
    ContentAlgebra {
        kind: ContentAlgebraKind::CountedQuantity,
        parameter: "Byte".to_owned(),
    }
}

fn content_root() -> PlaceId {
    PlaceId::new(1).expect("place")
}

fn content_context(root: PlaceId) -> PropositionContext {
    PropositionContext::from_value_types_and_places(
        [],
        [(root, StructuralPlaceKind::Parameter { position: 0 })],
    )
    .expect("content context")
}

fn content_term(root: PlaceId, field: &str) -> ContentTerm {
    ContentTerm::Projection {
        projection: ContentProjectionIdentity {
            domain: ContentDomainId::new(2).expect("domain"),
            projection_report_fingerprint: 3,
        },
        subject: ContentStructuralPlace {
            version: ContentPlaceVersion::Current,
            root,
            segments: vec![ContentPlaceSegment::Field(field.to_owned())],
        },
    }
}

fn conservation(algebra: &ContentAlgebra, left: &ContentTerm, right: &ContentTerm) -> Proposition {
    Proposition::ContentConservation(ContentConservation::new(
        algebra.clone(),
        left.clone(),
        right.clone(),
    ))
}

/// `ContentConservation` denotes `Id` over its algebra's carrier, so
/// its transitivity rule is a `J` composition the kernel re-decides
/// in every shared-endpoint orientation the bounded relation check
/// licenses — never an interned rule axiom.
#[test]
fn content_conservation_transitivity_is_a_j_composition() {
    let root = content_root();
    let algebra = content_algebra();
    let terms = |field: &str| content_term(root, field);
    let context = content_context(root);
    // Every shared-endpoint arrangement the checker's
    // `ContentConservation` arm accepts: direct chaining, right-right
    // and left-left sharing, and the cross-side middle — the `J`
    // composition wraps the needed premises in `sym`.
    for (first_pair, second_pair, goal_pair) in [
        (("a", "b"), ("b", "c"), ("a", "c")),
        (("a", "c"), ("b", "c"), ("a", "b")),
        (("a", "c"), ("a", "b"), ("b", "c")),
        (("b", "c"), ("a", "b"), ("a", "c")),
    ] {
        let first = conservation(&algebra, &terms(first_pair.0), &terms(first_pair.1));
        let second = conservation(&algebra, &terms(second_pair.0), &terms(second_pair.1));
        let goal = conservation(&algebra, &terms(goal_pair.0), &terms(goal_pair.1));
        let proof = ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::EqualityTransitivity {
                left_equals_middle: Box::new(semantic_axiom_proof(first.clone(), 0)),
                middle_equals_right: Box::new(semantic_axiom_proof(second.clone(), 1)),
            },
        };
        let denoted = verify_bounded_certificate(
            &context,
            &goal,
            &[],
            &[first, second],
            &proof,
            &mut budget(),
        )
        .unwrap_or_else(|error| {
            panic!("content transitivity {goal_pair:?} verifies as J: {error}")
        });
        // `{C, a', b', c'}` — the algebra carrier and one element
        // constant per distinct content term; the rule cites no
        // axiom of its own.
        assert_eq!(denoted.certificate.signature.len(), 4);
        assert!(
            denoted
                .certificate
                .signature
                .iter()
                .all(|declaration| declaration.is_assumption()),
        );
        assert!(
            matches!(
                denoted.arena.get(denoted.certificate.term),
                Term::IdElim { .. }
            ),
            "content transitivity {goal_pair:?} composes by J",
        );
        assert_eq!(
            certificate_assumption_closure(&denoted.arena, &denoted.certificate),
            BTreeSet::from([0, 1, 2, 3]),
        );
        assert_eq!(
            denoted.rules,
            vec![
                AcceptedProofRule::SemanticAxiom,
                AcceptedProofRule::EqualityTransitivity
            ],
        );
        assert_eq!(denoted.semantic_axioms.len(), 2);
    }
}

/// A reflexive content conservation denotes `refl` on the algebra
/// carrier — kernel-proved, not a named decision — over `Separate`
/// endpoints as well as bare projections.
#[test]
fn reflexive_content_conservation_is_refl_in_the_kernel() {
    let root = content_root();
    let algebra = content_algebra();
    let context = content_context(root);
    let separate = ContentTerm::separate([content_term(root, "a"), content_term(root, "b")])
        .expect("separation");
    let proposition = conservation(&algebra, &separate, &separate);
    let proof = ProofNode {
        conclusion: proposition.clone(),
        rule: ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality),
    };
    let denoted =
        verify_bounded_certificate(&context, &proposition, &[], &[], &proof, &mut budget())
            .expect("reflexive conservation verifies as refl");
    // `{C, s'}` — the carrier and the one endpoint; the evidence is
    // `refl C s'` with no decision assumption.
    assert_eq!(denoted.certificate.signature.len(), 2);
    assert!(matches!(
        denoted.arena.get(denoted.certificate.term),
        Term::Refl { .. }
    ));
    assert_eq!(
        certificate_assumption_closure(&denoted.arena, &denoted.certificate),
        BTreeSet::from([0, 1]),
    );
}

/// Content transitivity still rejects at the shared checker's own
/// points: different algebras, no shared endpoint, or a conclusion
/// that is not the composed equation.
#[test]
fn content_conservation_transitivity_rejects_at_the_shared_check() {
    let root = content_root();
    let algebra = content_algebra();
    let terms = |field: &str| content_term(root, field);
    let context = content_context(root);
    let transitivity = |first: Proposition, second: Proposition, goal: Proposition| ProofNode {
        conclusion: goal,
        rule: ProofRule::EqualityTransitivity {
            left_equals_middle: Box::new(semantic_axiom_proof(first, 0)),
            middle_equals_right: Box::new(semantic_axiom_proof(second, 1)),
        },
    };

    // A shared endpoint across different algebras is an algebra
    // mismatch — the denotation would put the equations over
    // different carriers anyway.
    let other = ContentAlgebra {
        kind: ContentAlgebraKind::CountedQuantity,
        parameter: "Gram".to_owned(),
    };
    let first = conservation(&algebra, &terms("a"), &terms("b"));
    let second = conservation(&other, &terms("b"), &terms("c"));
    let goal = conservation(&algebra, &terms("a"), &terms("c"));
    let proof = transitivity(first.clone(), second.clone(), goal.clone());
    assert!(matches!(
        denote_bounded_certificate(&context, &goal, &[], &[first, second], &proof),
        Err(BoundedDenotationError::Certificate(
            ProofError::EqualityAlgebraMismatch
        )),
    ));

    // No shared endpoint at all.
    let first = conservation(&algebra, &terms("a"), &terms("b"));
    let second = conservation(&algebra, &terms("c"), &terms("d"));
    let proof = transitivity(first.clone(), second.clone(), goal.clone());
    assert!(matches!(
        denote_bounded_certificate(&context, &goal, &[], &[first, second], &proof),
        Err(BoundedDenotationError::Certificate(
            ProofError::EqualityMiddleMismatch
        )),
    ));

    // A shared endpoint but a conclusion that is not the composed
    // equation.
    let first = conservation(&algebra, &terms("a"), &terms("b"));
    let second = conservation(&algebra, &terms("b"), &terms("c"));
    let proof = transitivity(first.clone(), second.clone(), first.clone());
    assert!(matches!(
        denote_bounded_certificate(&context, &first.clone(), &[], &[first, second], &proof),
        Err(BoundedDenotationError::Certificate(
            ProofError::EqualityConclusionMismatch
        )),
    ));
}

/// Unchanged content evidence is already the requested proof. Its
/// original context remains in the closure even when transport is empty.
#[test]
fn unchanged_content_transport_reuses_evidence_and_retains_its_context() {
    let root = content_root();
    let algebra = content_algebra();
    let premise = conservation(&algebra, &content_term(root, "a"), &content_term(root, "b"));
    let (x_id, x) = value(1);
    let (z_id, z) = value(3);
    let equation = Proposition::Equal(x, z);
    let context = PropositionContext::from_value_types_and_places(
        [(x_id, unsigned64_type()), (z_id, unsigned64_type())],
        [(root, StructuralPlaceKind::Parameter { position: 0 })],
    )
    .expect("context");
    let proof = ProofNode {
        conclusion: premise.clone(),
        rule: ProofRule::ValueEqualityTransport {
            premise: Box::new(semantic_axiom_proof(premise.clone(), 0)),
            equalities: vec![semantic_axiom_proof(equation.clone(), 1)],
        },
    };
    let denoted = verify_bounded_certificate(
        &context,
        &premise,
        &[],
        &[premise.clone(), equation],
        &proof,
        &mut budget(),
    )
    .expect("the original content evidence verifies");
    // C, a', b', Int, m_x and m_z remain; no new rule axiom.
    assert_eq!(denoted.certificate.signature.len(), 6);
    assert_eq!(
        denoted.arena.get(denoted.certificate.term),
        Term::Variable(1)
    );
    assert_eq!(
        certificate_assumption_closure(&denoted.arena, &denoted.certificate),
        (0..6).collect()
    );
    assert!(
        matches!(
            denoted.arena.get(denoted.certificate.context[0]),
            Term::Id { .. }
        ),
        "the first premise is the content identity",
    );
    assert_eq!(
        denoted.rules,
        vec![
            AcceptedProofRule::SemanticAxiom,
            AcceptedProofRule::ValueEqualityTransport
        ],
    );
}

/// Elaborate one `PredicateDenotation` node over the cited premise —
/// `Truth` premises are discharged by the primitive judgment, every
/// other premise is a cited assumption — and return the elaboration
/// so a test can inspect the interned rule axioms.
fn predicate_denotation_elaboration(
    premise: &Proposition,
    goal: &Proposition,
) -> Result<BoundedDenotation, BoundedDenotationError> {
    let (premise_proof, roster) = if matches!(premise, Proposition::Truth) {
        (
            ProofNode {
                conclusion: premise.clone(),
                rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
            },
            Vec::new(),
        )
    } else {
        (assumption_proof(premise.clone(), 0), vec![premise.clone()])
    };
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::PredicateDenotation {
            premise: Box::new(premise_proof),
        },
    };
    let context = PropositionContext::default();
    let machine_parameters = BTreeSet::new();
    let mut elaboration = Elaboration::new(&context, goal, &roster, &[], &machine_parameters)
        .expect("the normalized relation licenses the conversion");
    let term = elaboration.node(&proof)?;
    assert_eq!(
        elaboration.denotation.rule_axioms.len(),
        0,
        "{premise:?} -> {goal:?} derives without a rule-instance axiom",
    );
    let mut denoted = elaboration.finish(term);
    verify_mathematical_certificate(&mut denoted.arena, &denoted.certificate, &mut budget())
        .expect("the derived judgment re-decides in the kernel");
    Ok(denoted)
}

/// The closed `Int`-relation → `Id Two zero one` crossing: a decided
/// false order or identity refutes through the shared numeral-order
/// derivation and strict irreflexivity, then `EmptyElim` — the
/// `Id Two`/`Int` carrier crossing's Falsehood side, with no
/// rule-instance axiom.
#[test]
fn closed_false_integer_relations_refute_to_empty_elimination() {
    for premise in [
        Proposition::LessThan(literal(3), literal(2)),
        Proposition::LessThan(literal(2), literal(2)),
        Proposition::LessOrEqual(literal(3), literal(2)),
        Proposition::Equal(literal(2), literal(3)),
        Proposition::Equal(literal(3), literal(2)),
    ] {
        let denoted = predicate_denotation_elaboration(&premise, &Proposition::Falsehood)
            .expect("the closed-false relation denotes a refutation");
        assert!(
            matches!(
                denoted.arena.get(denoted.certificate.term),
                Term::EmptyElim { .. }
            ),
            "{premise:?} eliminates its checked `Empty` into Falsehood",
        );
    }
}

/// `Id Two zero one` itself is the discriminant case: `J`'s Boolean
/// discriminant lands on the goal type directly — the strict `Empty`
/// is no `Type 0` branch — so a Falsehood premise transports into
/// any licensed goal.
#[test]
fn the_falsehood_premise_transports_through_the_j_discriminant() {
    let goal = Proposition::Equal(literal(2), literal(3));
    let denoted = predicate_denotation_elaboration(&Proposition::Falsehood, &goal)
        .expect("the Falsehood premise denotes a discriminant elimination");
    assert!(
        matches!(
            denoted.arena.get(denoted.certificate.term),
            Term::IdElim { .. }
        ),
        "the evidence is the `J` discriminant landing on the goal",
    );
}

/// The `Id Two zero zero` → closed-true side: a Truth-normalized
/// goal is inhabited outright — `refl` for the reflexive identity,
/// the `eq_le`/`lt_le` roster laws or the numeral-order derivation
/// for a decided `IntLe`/`IntLt`, and the pair/tag constructions for
/// the connective denotations.
#[test]
fn closed_true_goals_are_inhabited_outright() {
    let denoted = predicate_denotation_elaboration(
        &Proposition::Equal(literal(2), literal(2)),
        &Proposition::Truth,
    )
    .expect("a closed-true identity crosses to Truth");
    assert!(
        matches!(
            denoted.arena.get(denoted.certificate.term),
            Term::Refl { .. }
        ),
        "the `Id Two zero zero` goal is `refl`",
    );

    for goal in [
        Proposition::LessThan(literal(1), literal(2)),
        Proposition::LessOrEqual(literal(2), literal(2)),
        Proposition::Equal(literal(2), literal(2)),
    ] {
        predicate_denotation_elaboration(&Proposition::Truth, &goal)
            .expect("a Truth premise inhabits the closed-true relation");
    }
}

/// The crossing reaches inside the connective denotations: a
/// conjunction with a refutable child projects the component into
/// `Empty`, and a Truth-normalized conjunction or disjunction goal
/// is built from its component inhabitants.
#[test]
fn the_crossing_composes_through_connective_denotations() {
    let premise = Proposition::Conjunction(vec![
        Proposition::LessThan(literal(3), literal(2)),
        Proposition::Equal(literal(2), literal(2)),
    ]);
    let denoted = predicate_denotation_elaboration(&premise, &Proposition::Falsehood)
        .expect("a conjunction with a refutable child denotes a projection");
    assert!(
        matches!(
            denoted.arena.get(denoted.certificate.term),
            Term::EmptyElim { .. }
        ),
        "the refuted component eliminates `Empty` into Falsehood",
    );

    let goal = Proposition::Conjunction(vec![
        Proposition::Equal(literal(2), literal(2)),
        Proposition::LessOrEqual(literal(2), literal(2)),
    ]);
    let denoted = predicate_denotation_elaboration(&Proposition::Truth, &goal)
        .expect("a Truth-normalized conjunction denotes a pair");
    assert!(
        matches!(
            denoted.arena.get(denoted.certificate.term),
            Term::Pair { .. }
        ),
        "the conjunction's components pair",
    );

    let goal = Proposition::Disjunction(vec![
        Proposition::Equal(literal(2), literal(3)),
        Proposition::Truth,
    ]);
    let denoted = predicate_denotation_elaboration(&Proposition::Truth, &goal)
        .expect("a Truth-normalized disjunction denotes a tagged pair");
    let Term::Pair { first, .. } = denoted.arena.get(denoted.certificate.term) else {
        panic!("the disjunction's inhabited branch pairs with a tag");
    };
    assert_eq!(denoted.arena.get(first), Term::TwoOne);
}

/// The open Boolean-equality expansion — `Equal(¬a, b)` denoting
/// `Id Two (not a) b` into its normalized valuation disjunction —
/// derives by `caseTwo` case analysis on the two atoms: each literal
/// valuation either inhabits the expanded conjunction or refutes the
/// premise through the `J` discriminant, so no rule-instance axiom
/// enters the signature.
#[test]
fn an_open_boolean_expansion_denotes_by_case_analysis() {
    let (a_id, a) = boolean_value(1);
    let (b_id, b) = boolean_value(2);
    let context = PropositionContext::from_value_types([
        (a_id, ScalarType::Boolean),
        (b_id, ScalarType::Boolean),
    ])
    .expect("context");
    let premise = Proposition::Equal(ScalarTerm::boolean_not(a.clone()).expect("not"), b.clone());
    // The normalized `Equal(¬a, b)` expansion: the two
    // same-valuation conjunctions, disjoined.
    let goal = Proposition::Disjunction(vec![
        Proposition::Conjunction(vec![
            Proposition::Equal(a.clone(), ScalarTerm::Boolean(false)),
            Proposition::Equal(b.clone(), ScalarTerm::Boolean(true)),
        ]),
        Proposition::Conjunction(vec![
            Proposition::Equal(a.clone(), ScalarTerm::Boolean(true)),
            Proposition::Equal(b, ScalarTerm::Boolean(false)),
        ]),
    ]);
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::PredicateDenotation {
            premise: Box::new(assumption_proof(premise.clone(), 0)),
        },
    };
    let machine_parameters = BTreeSet::new();
    let mut elaboration = Elaboration::new(
        &context,
        &goal,
        std::slice::from_ref(&premise),
        &[],
        &machine_parameters,
    )
    .expect("the Boolean expansion licenses the conversion");
    let term = elaboration
        .node(&proof)
        .expect("the open Boolean expansion denotes a case analysis");
    assert!(
        elaboration.denotation.rule_axioms.is_empty(),
        "the crossing derives without naming a rule-instance axiom",
    );
    let mut denoted = elaboration.finish(term);
    // The transport applies a `caseTwo` case-analysis function to the
    // cited premise; the atoms `a`/`b` are the whole signature.
    let Term::Apply { function, .. } = denoted.arena.get(denoted.certificate.term) else {
        panic!("the expansion denotes an applied case-analysis function");
    };
    assert!(
        matches!(denoted.arena.get(function), Term::CaseTwo { .. }),
        "the transport function eliminates a Boolean atom",
    );
    assert_eq!(denoted.certificate.signature.len(), 2);
    verify_mathematical_certificate(&mut denoted.arena, &denoted.certificate, &mut budget())
        .expect("the derived judgment re-decides in the kernel");
}

/// `connective()` sorting derives a permuted conjunction with real
/// projections: `(b ∧ a) → (a ∧ b)` is `⟨snd e, fst e⟩`, not a
/// rule-instance axiom.
#[test]
fn a_permuted_conjunction_derives_through_its_projections() {
    let premise = Proposition::Conjunction(vec![atom(2), atom(1)]);
    let goal = Proposition::Conjunction(vec![atom(1), atom(2)]);
    let denoted = predicate_denotation_elaboration(&premise, &goal)
        .expect("the permuted conjunction derives");
    let Term::Pair { first, second } = denoted.arena.get(denoted.certificate.term) else {
        panic!("the permuted conjunction re-pairs its projections");
    };
    assert!(matches!(denoted.arena.get(first), Term::Snd { .. }));
    assert!(matches!(denoted.arena.get(second), Term::Fst { .. }));
}

/// `connective()` deduplication and its single-child collapse: the
/// duplicated conjunction projects out the kept component, and the
/// reverse direction re-pairs the one premise.
#[test]
fn a_deduplicated_conjunction_projects_and_re_pairs() {
    let premise = Proposition::Conjunction(vec![atom(1), atom(1)]);
    predicate_denotation_elaboration(&premise, &atom(1))
        .expect("the duplicated conjunction projects");

    let goal = Proposition::Conjunction(vec![atom(1), atom(1)]);
    predicate_denotation_elaboration(&atom(1), &goal)
        .expect("the duplicated conclusion re-pairs the premise");
}

/// `connective()` flattening re-associates the nested `Σ`
/// denotation: `((a ∧ b) ∧ c) → (a ∧ (b ∧ c))` is a real projection
/// path, not an axiom.
#[test]
fn a_flattened_conjunction_re_associates_through_projections() {
    let premise = Proposition::Conjunction(vec![
        Proposition::Conjunction(vec![atom(1), atom(2)]),
        atom(3),
    ]);
    let goal = Proposition::Conjunction(vec![atom(1), atom(2), atom(3)]);
    let denoted = predicate_denotation_elaboration(&premise, &goal)
        .expect("the flattened conjunction derives");
    let Term::Pair { second, .. } = denoted.arena.get(denoted.certificate.term) else {
        panic!("the flattened conjunction re-pairs its components");
    };
    assert!(
        matches!(denoted.arena.get(second), Term::Pair { .. }),
        "the re-associated tail pairs the nested projections",
    );
}

/// A `Truth` conjunct drops: `(a ∧ ⊤) → a` is `fst e`, and the
/// reverse pairs the premise with `refl` at `Id Two zero zero`.
#[test]
fn a_truth_conjunct_drops_through_the_projection() {
    let premise = Proposition::Conjunction(vec![atom(1), Proposition::Truth]);
    let denoted =
        predicate_denotation_elaboration(&premise, &atom(1)).expect("the Truth conjunct drops");
    assert!(
        matches!(
            denoted.arena.get(denoted.certificate.term),
            Term::Fst { .. }
        ),
        "the evidence is the surviving component's projection",
    );

    let goal = Proposition::Conjunction(vec![atom(1), Proposition::Truth]);
    let denoted = predicate_denotation_elaboration(&atom(1), &goal)
        .expect("the Truth conjunct pairs with its closed inhabitant");
    let Term::Pair { second, .. } = denoted.arena.get(denoted.certificate.term) else {
        panic!("the Truth-normalized goal pairs its components");
    };
    assert!(
        matches!(denoted.arena.get(second), Term::Refl { .. }),
        "the dropped Truth is refl at Id Two zero zero",
    );
}

/// `connective()` sorting across the tagged-sum denotation: the
/// permuted disjunction eliminates `e` and re-tags each branch —
/// `caseTwo(M, λh.⟨one, h⟩, λh.⟨zero, h⟩, fst e) (snd e)` — rather
/// than a rule-instance axiom.
#[test]
fn a_permuted_disjunction_derives_through_retagging() {
    let premise = Proposition::Disjunction(vec![atom(2), atom(1)]);
    let goal = Proposition::Disjunction(vec![atom(1), atom(2)]);
    let denoted = predicate_denotation_elaboration(&premise, &goal)
        .expect("the permuted disjunction derives");
    let Term::Apply { function, .. } = denoted.arena.get(denoted.certificate.term) else {
        panic!("the disjunction eliminates the premise pair");
    };
    let Term::CaseTwo {
        zero_branch,
        one_branch,
        ..
    } = denoted.arena.get(function)
    else {
        panic!("the eliminator case-splits the tag");
    };
    let Term::Lambda { body, .. } = denoted.arena.get(zero_branch) else {
        panic!("the zero branch binds its payload");
    };
    let Term::Pair { first, .. } = denoted.arena.get(body) else {
        panic!("the zero branch re-tags into the goal");
    };
    assert_eq!(
        denoted.arena.get(first),
        Term::TwoOne,
        "the premise's first disjunct is the goal's rest",
    );
    let Term::Lambda { body, .. } = denoted.arena.get(one_branch) else {
        panic!("the one branch binds its payload");
    };
    let Term::Pair { first, .. } = denoted.arena.get(body) else {
        panic!("the one branch re-tags into the goal");
    };
    assert_eq!(
        denoted.arena.get(first),
        Term::TwoZero,
        "the premise's rest disjunct is the goal's first",
    );
}

/// A `Falsehood` disjunct drops: `(a ∨ ⊥) → a` case-splits the tag,
/// keeps the `a` payload, and refutes the `Id Two zero one` branch
/// through the `J` discriminant — and the reverse tags the premise
/// `zero` without ever inhabiting the dropped branch.
#[test]
fn a_falsehood_disjunct_drops_through_the_eliminator() {
    let premise = Proposition::Disjunction(vec![atom(1), Proposition::Falsehood]);
    let denoted =
        predicate_denotation_elaboration(&premise, &atom(1)).expect("the Falsehood disjunct drops");
    let Term::Apply { function, .. } = denoted.arena.get(denoted.certificate.term) else {
        panic!("the disjunction eliminates the premise pair");
    };
    let Term::CaseTwo { one_branch, .. } = denoted.arena.get(function) else {
        panic!("the eliminator case-splits the tag");
    };
    let Term::Lambda { body, .. } = denoted.arena.get(one_branch) else {
        panic!("the Falsehood branch binds its payload");
    };
    assert!(
        matches!(
            denoted.arena.get(body),
            Term::IdElim { .. } | Term::EmptyElim { .. }
        ),
        "the Falsehood payload refutes into the goal",
    );

    let goal = Proposition::Disjunction(vec![atom(1), Proposition::Falsehood]);
    let denoted = predicate_denotation_elaboration(&atom(1), &goal)
        .expect("the surviving disjunct tags zero");
    let Term::Pair { first, .. } = denoted.arena.get(denoted.certificate.term) else {
        panic!("the disjunction introduction pairs a tag");
    };
    assert_eq!(denoted.arena.get(first), Term::TwoZero);
}

/// Normalization under an implication premise derives
/// contravariantly: `((b ∧ a) → c) → ((a ∧ b) → c)` coerces the
/// domain through the same projection crossing inside the `λ`'s
/// applied body — the nested connective never becomes an axiom.
#[test]
fn an_implication_domain_derives_contravariantly() {
    let premise = Proposition::Implication {
        premise: Box::new(Proposition::Conjunction(vec![atom(2), atom(1)])),
        conclusion: Box::new(atom(3)),
    };
    let goal = Proposition::Implication {
        premise: Box::new(Proposition::Conjunction(vec![atom(1), atom(2)])),
        conclusion: Box::new(atom(3)),
    };
    let denoted = predicate_denotation_elaboration(&premise, &goal)
        .expect("the implication's normalized domain derives");
    assert!(
        matches!(
            denoted.arena.get(denoted.certificate.term),
            Term::Lambda { .. }
        ),
        "the crossing stays inside the function body",
    );
}
