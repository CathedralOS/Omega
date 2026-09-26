//! Focused correlated-forbidden-root denotation tests: the elaborated
//! evidence is a checked four-way `Two` split over chain inversions and
//! numeral equations, never a per-instance rule axiom.

use std::collections::BTreeSet;

use super::super::Elaboration;
use super::Law;
use crate::{
    Budget, CorrelatedAffineBranchWitness, CorrelatedAffineStepWitness,
    IntegerCorrelatedForbiddenRootWitness, ProofError, ProofNode, ProofRule,
    verify_bounded_certificate_with_machine_parameters,
};
use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext, ScalarTerm,
    ScalarType, ValueId,
};

fn i8() -> IntegerType {
    IntegerType::new(IntegerSign::Signed, 8).expect("i8")
}

fn value(index: u64, integer_type: IntegerType) -> ScalarTerm {
    ScalarTerm::value(
        ValueId::new(index).expect("value id"),
        ScalarType::Integer(integer_type),
    )
}

fn integer(integer_type: IntegerType, value: i128) -> ScalarTerm {
    ScalarTerm::integer(integer_type, IntegerValue::Signed(value)).expect("integer")
}

fn equal(target: ScalarTerm, expression: ScalarTerm) -> Proposition {
    Proposition::Equal(target, expression)
}

fn add(
    target: ScalarTerm,
    integer_type: IntegerType,
    left: ScalarTerm,
    right: ScalarTerm,
) -> Proposition {
    equal(
        target,
        ScalarTerm::exact_integer_add(integer_type, left, right).expect("add"),
    )
}

fn subtract(
    target: ScalarTerm,
    integer_type: IntegerType,
    left: ScalarTerm,
    right: ScalarTerm,
) -> Proposition {
    equal(
        target,
        ScalarTerm::exact_integer_subtract(integer_type, left, right).expect("subtract"),
    )
}

fn multiply(
    target: ScalarTerm,
    integer_type: IntegerType,
    left: ScalarTerm,
    right: ScalarTerm,
) -> Proposition {
    equal(
        target,
        ScalarTerm::exact_integer_multiply(integer_type, left, right).expect("multiply"),
    )
}

/// The exact-division definedness conclusion the conversion binds:
/// `d ≤ −2 ∨ 1 ≤ d ∨ (d ≤ −1 ∧ min+1 ≤ n)`.
fn definedness(
    integer_type: IntegerType,
    dividend: &ScalarTerm,
    divisor: &ScalarTerm,
) -> Proposition {
    let minimum_plus_one = match integer_type.minimum_value() {
        IntegerValue::Signed(minimum) => minimum + 1,
        _ => unreachable!("signed fixed carrier"),
    };
    Proposition::Disjunction(vec![
        Proposition::LessOrEqual(divisor.clone(), integer(integer_type, -2)),
        Proposition::LessOrEqual(integer(integer_type, 1), divisor.clone()),
        Proposition::Conjunction(vec![
            Proposition::LessOrEqual(divisor.clone(), integer(integer_type, -1)),
            Proposition::LessOrEqual(integer(integer_type, minimum_plus_one), dividend.clone()),
        ]),
    ])
}

/// One checked correlated forbidden-root certificate.
struct Fixture {
    context: PropositionContext,
    axioms: Vec<Proposition>,
    premises: Vec<Proposition>,
    parameters: BTreeSet<ValueId>,
    goal: Proposition,
    proof: ProofNode,
}

/// `n = (r + 64)·(−2)`, `d = 2r + 1`, `−1 ≤ r ≤ 0` over `i8`: `d = 0`
/// has no integer root — `r = −1/2` is non-divisible — and `d = −1`
/// solves to `r = −1` where `n = −126 ≥ −127`. Both literal-axiom
/// indirections and the positive-quotient squeeze participate.
fn safe_fixture() -> Fixture {
    let integer_type = i8();
    let root = value(1, integer_type);
    let sixty_four = value(2, integer_type);
    let left_offset = value(3, integer_type);
    let negative_two = value(4, integer_type);
    let dividend = value(5, integer_type);
    let two = value(6, integer_type);
    let right_product = value(7, integer_type);
    let divisor = value(8, integer_type);
    let lower = Proposition::LessOrEqual(integer(integer_type, -1), root.clone());
    let upper = Proposition::LessOrEqual(root.clone(), integer(integer_type, 0));
    let axioms = vec![
        equal(sixty_four.clone(), integer(integer_type, 64)),
        add(left_offset.clone(), integer_type, root.clone(), sixty_four),
        equal(negative_two.clone(), integer(integer_type, -2)),
        multiply(dividend.clone(), integer_type, left_offset, negative_two),
        equal(two.clone(), integer(integer_type, 2)),
        multiply(right_product.clone(), integer_type, root.clone(), two),
        add(
            divisor.clone(),
            integer_type,
            right_product,
            integer(integer_type, 1),
        ),
    ];
    let context = PropositionContext::from_value_types((1..=8).map(|index| {
        (
            ValueId::new(index).unwrap(),
            ScalarType::Integer(integer_type),
        )
    }))
    .expect("context");
    let witness = IntegerCorrelatedForbiddenRootWitness {
        dividend: CorrelatedAffineBranchWitness {
            root: root.clone(),
            target: dividend.clone(),
            steps: vec![
                CorrelatedAffineStepWitness {
                    definition_axiom: 1,
                    literal_axiom: Some(0),
                },
                CorrelatedAffineStepWitness {
                    definition_axiom: 3,
                    literal_axiom: Some(2),
                },
            ],
        },
        divisor: CorrelatedAffineBranchWitness {
            root,
            target: divisor.clone(),
            steps: vec![
                CorrelatedAffineStepWitness {
                    definition_axiom: 5,
                    literal_axiom: Some(4),
                },
                CorrelatedAffineStepWitness {
                    definition_axiom: 6,
                    literal_axiom: None,
                },
            ],
        },
        definition_axiom_count: 7,
        lower_bound_axiom: 7,
        upper_bound_axiom: 8,
        conclusion: Proposition::Conjunction(vec![lower.clone(), upper.clone()]),
    };
    let goal = definedness(integer_type, &dividend, &divisor);
    Fixture {
        context,
        axioms,
        premises: vec![lower, upper],
        parameters: BTreeSet::from([ValueId::new(1).expect("root")]),
        goal: goal.clone(),
        proof: ProofNode {
            conclusion: goal,
            rule: ProofRule::IntegerCorrelatedForbiddenRoots { witness },
        },
    }
}

/// `n = r − 100`, `d = r·(−2) − 5`, `r = −2` tight over `i8`: `d = −1`
/// has the in-interval root `r = −2` where the forward dividend
/// evaluation lands `n = −102 ≥ −127`; `d = 0` needs `r = −5/2` and
/// refutes on the non-divisible product. The subtract steps exercise
/// `SubtractRightInverse`; the `−2` multiplier the negative-quotient
/// laws.
fn negative_multiplier_fixture() -> Fixture {
    let integer_type = i8();
    let root = value(1, integer_type);
    let dividend = value(2, integer_type);
    let negative_two = value(3, integer_type);
    let right_product = value(4, integer_type);
    let divisor = value(5, integer_type);
    let lower = Proposition::LessOrEqual(integer(integer_type, -2), root.clone());
    let upper = Proposition::LessOrEqual(root.clone(), integer(integer_type, -2));
    let axioms = vec![
        subtract(
            dividend.clone(),
            integer_type,
            root.clone(),
            integer(integer_type, 100),
        ),
        equal(negative_two.clone(), integer(integer_type, -2)),
        multiply(
            right_product.clone(),
            integer_type,
            root.clone(),
            negative_two,
        ),
        subtract(
            divisor.clone(),
            integer_type,
            right_product,
            integer(integer_type, 5),
        ),
    ];
    let context = PropositionContext::from_value_types((1..=5).map(|index| {
        (
            ValueId::new(index).unwrap(),
            ScalarType::Integer(integer_type),
        )
    }))
    .expect("context");
    let witness = IntegerCorrelatedForbiddenRootWitness {
        dividend: CorrelatedAffineBranchWitness {
            root: root.clone(),
            target: dividend.clone(),
            steps: vec![CorrelatedAffineStepWitness {
                definition_axiom: 0,
                literal_axiom: None,
            }],
        },
        divisor: CorrelatedAffineBranchWitness {
            root,
            target: divisor.clone(),
            steps: vec![
                CorrelatedAffineStepWitness {
                    definition_axiom: 2,
                    literal_axiom: Some(1),
                },
                CorrelatedAffineStepWitness {
                    definition_axiom: 3,
                    literal_axiom: None,
                },
            ],
        },
        definition_axiom_count: 4,
        lower_bound_axiom: 4,
        upper_bound_axiom: 5,
        conclusion: Proposition::Conjunction(vec![lower.clone(), upper.clone()]),
    };
    let goal = definedness(integer_type, &dividend, &divisor);
    Fixture {
        context,
        axioms,
        premises: vec![lower, upper],
        parameters: BTreeSet::from([ValueId::new(1).expect("root")]),
        goal: goal.clone(),
        proof: ProofNode {
            conclusion: goal,
            rule: ProofRule::IntegerCorrelatedForbiddenRoots { witness },
        },
    }
}

/// The admission checker accepts the certificate, then the elaboration
/// produces real evidence: no per-instance rule axiom, no decisions —
/// only the fixed law roster, interned numeral equations and the cited
/// premises/axioms close the assumption set.
fn check(fixture: &Fixture, laws: &[Law]) {
    verify_bounded_certificate_with_machine_parameters(
        &fixture.context,
        &fixture.goal,
        &fixture.premises,
        &fixture.axioms,
        &fixture.parameters,
        &fixture.proof,
        &mut Budget::default(),
    )
    .unwrap();
    let mut elaboration = Elaboration::new(
        &fixture.context,
        &fixture.goal,
        &fixture.premises,
        &fixture.axioms,
        &fixture.parameters,
    )
    .unwrap();
    let evidence = elaboration.node(&fixture.proof).unwrap();
    assert!(elaboration.denotation.rule_axioms.is_empty());
    assert!(elaboration.denotation.decisions.is_empty());
    for law in laws {
        assert!(
            elaboration
                .denotation
                .forbidden_roots
                .laws
                .contains_key(law),
            "missing {law:?}"
        );
    }
    // The evidence the elaboration produced is a real derivation, not a
    // constant naming the conclusion.
    let term = elaboration.denotation.arena.get(evidence);
    assert!(!matches!(
        term,
        crate::mathematical_core::Term::Constant { .. }
    ));
}

/// `n = (r + 64)·(−2)`, `d = 2r + 1`, `−1 ≤ r ≤ 0`: the `d = 0` case
/// refutes on the non-divisible product `2r = −1`, and the `d = −1`
/// case lands `n = −126` through the forward dividend evaluation.
#[test]
fn forbidden_roots_positive_multiplier_in_interval_root() {
    let integer = i8();
    let fixture = safe_fixture();
    check(
        &fixture,
        &[
            Law::AddRightInverse,
            Law::QuotientUpperPositive(integer),
            Law::QuotientLowerPositive(integer),
            Law::LessOrEqualAntisymmetry,
            Law::DivisorCaseSplit,
        ],
    );
}

/// `n = r − 100`, `d = −2r − 5`, `r = −2`: the `d = −1` solve lands the
/// in-interval root `r = −2`, and the dividend's `subtract` step
/// evaluates `n = −102 ≥ −127` directly — the bound is real evidence,
/// not a vacuous branch.
#[test]
fn forbidden_roots_negative_multiplier_in_interval_root() {
    let integer = i8();
    let fixture = negative_multiplier_fixture();
    check(
        &fixture,
        &[
            Law::SubtractRightInverse,
            Law::QuotientUpperNegative(integer),
            Law::QuotientLowerNegative(integer),
            Law::LessOrEqualAntisymmetry,
            Law::DivisorCaseSplit,
        ],
    );
}

/// A root outside the cited interval keeps the `d = −1` bound vacuous:
/// `d = −2r + 1`, `2 ≤ r ≤ 2` gives `d = −1` only at `r = 1` — the
/// solve lands `r = 1` and the tight bounds refute the case by
/// `IntLt 2 2` before the dividend chain ever runs.
#[test]
fn forbidden_roots_out_of_interval_root_refutes_case() {
    let integer_type = i8();
    let root = value(1, integer_type);
    let dividend = value(2, integer_type);
    let negative_two = value(3, integer_type);
    let right_product = value(4, integer_type);
    let divisor = value(5, integer_type);
    let lower = Proposition::LessOrEqual(integer(integer_type, 2), root.clone());
    let upper = Proposition::LessOrEqual(root.clone(), integer(integer_type, 2));
    let axioms = vec![
        add(
            dividend.clone(),
            integer_type,
            root.clone(),
            integer(integer_type, 1),
        ),
        equal(negative_two.clone(), integer(integer_type, -2)),
        multiply(
            right_product.clone(),
            integer_type,
            root.clone(),
            negative_two,
        ),
        add(
            divisor.clone(),
            integer_type,
            right_product,
            integer(integer_type, 1),
        ),
    ];
    let context = PropositionContext::from_value_types((1..=5).map(|index| {
        (
            ValueId::new(index).unwrap(),
            ScalarType::Integer(integer_type),
        )
    }))
    .expect("context");
    let witness = IntegerCorrelatedForbiddenRootWitness {
        dividend: CorrelatedAffineBranchWitness {
            root: root.clone(),
            target: dividend.clone(),
            steps: vec![CorrelatedAffineStepWitness {
                definition_axiom: 0,
                literal_axiom: None,
            }],
        },
        divisor: CorrelatedAffineBranchWitness {
            root,
            target: divisor.clone(),
            steps: vec![
                CorrelatedAffineStepWitness {
                    definition_axiom: 2,
                    literal_axiom: Some(1),
                },
                CorrelatedAffineStepWitness {
                    definition_axiom: 3,
                    literal_axiom: None,
                },
            ],
        },
        definition_axiom_count: 4,
        lower_bound_axiom: 4,
        upper_bound_axiom: 5,
        conclusion: Proposition::Conjunction(vec![lower.clone(), upper.clone()]),
    };
    let goal = definedness(integer_type, &dividend, &divisor);
    let fixture = Fixture {
        context,
        axioms,
        premises: vec![lower, upper],
        parameters: BTreeSet::from([ValueId::new(1).expect("root")]),
        goal: goal.clone(),
        proof: ProofNode {
            conclusion: goal,
            rule: ProofRule::IntegerCorrelatedForbiddenRoots { witness },
        },
    };
    check(
        &fixture,
        &[
            Law::AddRightInverse,
            Law::QuotientUpperNegative(integer_type),
            Law::QuotientLowerNegative(integer_type),
            Law::LessOrEqualAntisymmetry,
            Law::DivisorCaseSplit,
        ],
    );
}

/// A mutated conclusion — the disjunction's conjunct bound weakened to
/// `min+2` — rejects at the conversion before denotation; the witness's
/// own conclusion mutation rejects at the shared witness checker.
#[test]
fn forbidden_roots_rejects_mutated_conclusion() {
    let fixture = safe_fixture();
    let integer_type = i8();
    let dividend = value(5, integer_type);
    let divisor = value(8, integer_type);
    let drifted = Proposition::Disjunction(vec![
        Proposition::LessOrEqual(divisor.clone(), integer(integer_type, -2)),
        Proposition::LessOrEqual(integer(integer_type, 1), divisor.clone()),
        Proposition::Conjunction(vec![
            Proposition::LessOrEqual(divisor, integer(integer_type, -1)),
            Proposition::LessOrEqual(integer(integer_type, -126), dividend),
        ]),
    ]);
    let ProofRule::IntegerCorrelatedForbiddenRoots { witness } = &fixture.proof.rule else {
        unreachable!()
    };
    let proof = ProofNode {
        conclusion: drifted,
        rule: ProofRule::IntegerCorrelatedForbiddenRoots {
            witness: witness.clone(),
        },
    };
    assert!(matches!(
        verify_bounded_certificate_with_machine_parameters(
            &fixture.context,
            &proof.conclusion,
            &fixture.premises,
            &fixture.axioms,
            &fixture.parameters,
            &proof,
            &mut Budget::default(),
        ),
        Err(crate::BoundedDenotationError::Certificate(
            ProofError::IntegerCorrelatedForbiddenRootConversion(
                crate::IntegerCorrelatedForbiddenRootConversionError::ConclusionMismatch,
            )
        )),
    ));
}

/// A witness pointing one definition citation at the wrong axiom index
/// rejects at the witness checker — the denotation never sees it.
#[test]
fn forbidden_roots_rejects_mutated_witness() {
    let fixture = safe_fixture();
    let ProofRule::IntegerCorrelatedForbiddenRoots { witness } = &fixture.proof.rule else {
        unreachable!()
    };
    let mut drifted = witness.clone();
    drifted.dividend.steps[0].definition_axiom = 3;
    let proof = ProofNode {
        conclusion: fixture.goal.clone(),
        rule: ProofRule::IntegerCorrelatedForbiddenRoots { witness: drifted },
    };
    assert!(
        verify_bounded_certificate_with_machine_parameters(
            &fixture.context,
            &fixture.goal,
            &fixture.premises,
            &fixture.axioms,
            &fixture.parameters,
            &proof,
            &mut Budget::default(),
        )
        .is_err()
    );
}
