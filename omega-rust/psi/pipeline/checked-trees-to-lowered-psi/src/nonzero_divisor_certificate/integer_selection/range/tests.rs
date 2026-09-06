use super::*;
use proof_admission::check_certificate;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, ScalarType, ValueId};

fn integer_type() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 16).unwrap()
}

fn value(identity: u64) -> ScalarTerm {
    ScalarTerm::value(
        ValueId::new(identity).unwrap(),
        ScalarType::Integer(integer_type()),
    )
}

fn literal(number: u128) -> ScalarTerm {
    ScalarTerm::integer(integer_type(), IntegerValue::Unsigned(number)).unwrap()
}

fn context() -> PropositionContext {
    PropositionContext::from_value_types((1..=6).map(|identity| {
        (
            ValueId::new(identity).unwrap(),
            ScalarType::Integer(integer_type()),
        )
    }))
    .unwrap()
}

fn operation_axioms(remainder: bool) -> Vec<Proposition> {
    let operation = if remainder {
        ScalarTerm::ExactIntegerRemainder {
            scalar_type: integer_type(),
            left: Box::new(value(1)),
            right: Box::new(value(2)),
        }
    } else {
        ScalarTerm::ExactIntegerDivide {
            scalar_type: integer_type(),
            left: Box::new(value(1)),
            right: Box::new(value(2)),
        }
    };
    vec![
        Proposition::Equal(value(2), literal(256)),
        Proposition::Equal(value(3), operation),
        Proposition::Equal(value(4), value(3)),
    ]
}

fn upper_bound() -> Proposition {
    Proposition::LessOrEqual(value(4), literal(255))
}

#[test]
fn call_result_alias_preserves_divide_and_remainder_bounds() {
    for remainder in [false, true] {
        let semantic_axioms = operation_axioms(remainder);
        for goal in [
            upper_bound(),
            Proposition::LessOrEqual(literal(0), value(4)),
        ] {
            let proof = prove(&context(), &goal, &[], &semantic_axioms)
                .expect("computed argument bound follows the cited result alias");
            assert!(matches!(
                proof.rule,
                ProofRule::IntegerOrderSubstitution { .. }
            ));
            check_certificate(&context(), &goal, &[], &semantic_axioms, &proof).unwrap();
        }
    }
}

#[test]
fn operation_bounds_cross_each_explicitly_cited_alias() {
    let mut semantic_axioms = operation_axioms(true);
    semantic_axioms[2] = Proposition::Equal(value(4), value(5));
    semantic_axioms.push(Proposition::Equal(value(5), value(3)));
    let goal = upper_bound();
    let proof = prove(&context(), &goal, &[], &semantic_axioms)
        .expect("two directed aliases retain the operation bound");
    check_certificate(&context(), &goal, &[], &semantic_axioms, &proof).unwrap();
    for omitted in [2, 3] {
        let mut missing = semantic_axioms.clone();
        missing[omitted] = Proposition::Truth;
        assert!(prove(&context(), &goal, &[], &missing).is_none());
        assert!(check_certificate(&context(), &goal, &[], &missing, &proof).is_err());
    }
}

#[test]
fn missing_or_changed_alias_cannot_replace_the_cited_edge() {
    let semantic_axioms = operation_axioms(true);
    let goal = upper_bound();
    let proof = prove(&context(), &goal, &[], &semantic_axioms).unwrap();
    for replacement in [Proposition::Truth, Proposition::Equal(value(4), value(6))] {
        let mut changed = semantic_axioms.clone();
        changed[2] = replacement;
        assert!(prove(&context(), &goal, &[], &changed).is_none());
        assert!(check_certificate(&context(), &goal, &[], &changed, &proof).is_err());
    }
}

#[test]
fn canonical_reversed_alias_requires_a_new_symmetry_proof() {
    let semantic_axioms = operation_axioms(true);
    let goal = upper_bound();
    let original = prove(&context(), &goal, &[], &semantic_axioms).unwrap();
    let mut canonical = semantic_axioms.clone();
    canonical[2] = Proposition::Equal(value(3), value(4));
    assert!(check_certificate(&context(), &goal, &[], &canonical, &original).is_err());
    let proof = prove(&context(), &goal, &[], &canonical).expect("explicit equality symmetry");
    check_certificate(&context(), &goal, &[], &canonical, &proof).unwrap();
    let ProofRule::IntegerOrderSubstitution { equality, .. } = &proof.rule else {
        panic!("range transport retains equality evidence");
    };
    assert!(matches!(equality.rule, ProofRule::EqualitySymmetry { .. }));
}

#[test]
fn computed_argument_bounds_cross_only_the_proved_order_direction() {
    for strict in [false, true] {
        let mut axioms = operation_axioms(true);
        axioms[2] = if strict {
            Proposition::LessThan(value(4), value(3))
        } else {
            Proposition::LessOrEqual(value(4), value(3))
        };
        let goal = upper_bound();
        let proof = prove(&context(), &goal, &[], &axioms).expect("ordered result bound");
        check_certificate(&context(), &goal, &[], &axioms, &proof).unwrap();
        axioms[2] = Proposition::LessOrEqual(value(3), value(4));
        assert!(prove(&context(), &goal, &[], &axioms).is_none());
        assert!(check_certificate(&context(), &goal, &[], &axioms, &proof).is_err());
    }
}

#[test]
fn operation_definition_and_divisor_are_required_for_the_transported_bound() {
    for remainder in [false, true] {
        let semantic_axioms = operation_axioms(remainder);
        let goal = upper_bound();
        let proof = prove(&context(), &goal, &[], &semantic_axioms).unwrap();
        let mut changed_divisor = semantic_axioms.clone();
        changed_divisor[0] =
            Proposition::Equal(value(2), literal(if remainder { 512 } else { 128 }));
        let mut missing_definition = semantic_axioms.clone();
        missing_definition[1] = Proposition::Truth;
        let mut missing_divisor = semantic_axioms.clone();
        missing_divisor[0] = Proposition::Truth;
        for changed in [changed_divisor, missing_definition, missing_divisor] {
            assert!(prove(&context(), &goal, &[], &changed).is_none());
            assert!(check_certificate(&context(), &goal, &[], &changed, &proof).is_err());
        }
    }
}

#[test]
fn mathematical_exact_cast_bounds_replay_the_operation_and_result_alias() {
    for remainder in [false, true] {
        let semantic_axioms = operation_axioms(remainder);
        // The upper bound is narrower than the u16 carrier. Its proof must
        // cite the operation and alias; the lower bound zero needs neither.
        let goal = proof_admission::lift_fixed_integer_relation(&upper_bound()).unwrap();
        let proof = super::super::build(&context(), &goal, &[], &semantic_axioms)
            .expect("mathematical narrowing bound retains the scalar operation proof");
        check_certificate(&context(), &goal, &[], &semantic_axioms, &proof).unwrap();
        let mut missing_alias = semantic_axioms.clone();
        missing_alias[2] = Proposition::Truth;
        assert!(check_certificate(&context(), &goal, &[], &missing_alias, &proof).is_err());
    }
}
