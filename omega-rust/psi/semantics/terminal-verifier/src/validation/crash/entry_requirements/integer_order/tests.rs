use proof_admission::ProofRule;
use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext, ScalarTerm,
    ScalarType, ValueId,
};

use super::super::{MAXIMUM_SEARCH_STEPS, establishes, prove};
use super::adjacent;

fn literal(integer_type: IntegerType, number: i128) -> ScalarTerm {
    let value = match integer_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(number),
        IntegerSign::Unsigned => IntegerValue::Unsigned(u128::try_from(number).unwrap()),
    };
    ScalarTerm::integer(integer_type, value).unwrap()
}

fn parameter(integer_type: IntegerType, ordinal: u64) -> ScalarTerm {
    ScalarTerm::value(
        ValueId::new(ordinal).unwrap(),
        ScalarType::Integer(integer_type),
    )
}

fn context(integer_type: IntegerType) -> PropositionContext {
    PropositionContext::from_value_types([1, 2].map(|ordinal| {
        (
            ValueId::new(ordinal).unwrap(),
            ScalarType::Integer(integer_type),
        )
    }))
    .unwrap()
}

#[test]
fn inclusive_entry_bound_covers_the_exact_strict_crash_denotation() {
    for sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
        for bits in [8, 16, 32, 64] {
            let integer_type = IntegerType::new(sign, bits).unwrap();
            let value = parameter(integer_type, 1);
            let context = context(integer_type);
            for lower in [true, false] {
                let (left, right, premise) = if lower {
                    (
                        literal(integer_type, 0),
                        value.clone(),
                        Proposition::LessOrEqual(literal(integer_type, 1), value.clone()),
                    )
                } else {
                    (
                        value.clone(),
                        literal(integer_type, 2),
                        Proposition::LessOrEqual(value.clone(), literal(integer_type, 1)),
                    )
                };
                let strict = Proposition::LessThan(left.clone(), right.clone());
                let predicate = Proposition::Equal(
                    ScalarTerm::integer_less_than(integer_type, left, right).unwrap(),
                    ScalarTerm::boolean(true),
                );
                let requirements = [premise];
                let mut remaining = MAXIMUM_SEARCH_STEPS;
                let proof = prove(&strict, &requirements, &[], &mut remaining, 0).unwrap();
                assert!(matches!(
                    proof.rule,
                    ProofRule::IntegerOrderDiscreteness { .. }
                ));
                assert!(establishes(&context, &predicate, &requirements, &[]));
            }
        }
    }
}

#[test]
fn inclusive_endpoints_and_foreign_values_do_not_prove_strict_bounds() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    let context = context(integer_type);
    let value = parameter(integer_type, 1);
    let other = parameter(integer_type, 2);
    let goal = Proposition::LessThan(literal(integer_type, 0), value.clone());
    for premise in [
        Proposition::LessOrEqual(literal(integer_type, 0), value.clone()),
        Proposition::LessOrEqual(literal(integer_type, 1), other),
        Proposition::LessOrEqual(value.clone(), literal(integer_type, 1)),
        Proposition::LessOrEqual(literal(integer_type, -1), value.clone()),
    ] {
        assert!(!establishes(&context, &goal, &[premise], &[]));
    }
    let requirement = Proposition::LessOrEqual(literal(integer_type, -1), value.clone());
    let negative = Proposition::LessThan(literal(integer_type, -2), value);
    assert!(establishes(&context, &negative, &[requirement], &[]));
}

#[test]
fn adjacent_bounds_preserve_every_ordered_branch_and_conjunction() {
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
    let context = context(integer_type);
    let value = parameter(integer_type, 1);
    let lower = Proposition::LessOrEqual(literal(integer_type, 1), value.clone());
    let upper = Proposition::LessOrEqual(value.clone(), literal(integer_type, 9));
    let strict_lower = Proposition::LessThan(literal(integer_type, 0), value.clone());
    let strict_upper = Proposition::LessThan(value, literal(integer_type, 10));
    let goal = Proposition::Conjunction(vec![strict_lower.clone(), strict_upper]);
    let both = Proposition::Conjunction(vec![lower.clone(), upper.clone()]);
    assert!(establishes(
        &context,
        &goal,
        std::slice::from_ref(&both),
        &[]
    ));
    let complete = Proposition::Disjunction(vec![
        both.clone(),
        Proposition::Conjunction(vec![Proposition::Truth, both.clone()]),
    ]);
    assert!(establishes(&context, &goal, &[complete], &[]));
    let missing = Proposition::Disjunction(vec![both, upper]);
    assert!(!establishes(&context, &strict_lower, &[missing], &[]));
    // A reconstructed body bound alone is not a new invocation-entry premise.
    assert!(!establishes(&context, &strict_lower, &[], &[lower]));
}

#[test]
fn adjacency_cannot_wrap_at_fixed_or_host_limits() {
    for sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
        for bits in [8, 64, 128] {
            let integer_type = IntegerType::new(sign, bits).unwrap();
            let minimum = ScalarTerm::integer(integer_type, integer_type.minimum_value()).unwrap();
            let maximum = ScalarTerm::integer(integer_type, integer_type.maximum_value()).unwrap();
            assert!(adjacent(&minimum, false).is_none());
            assert!(adjacent(&maximum, true).is_none());
            assert!(adjacent(&minimum, true).is_some());
            assert!(adjacent(&maximum, false).is_some());
            let value = parameter(integer_type, 1);
            let context = context(integer_type);
            assert!(!establishes(
                &context,
                &Proposition::LessThan(maximum.clone(), value.clone()),
                &[Proposition::LessOrEqual(maximum, value.clone())],
                &[],
            ));
            assert!(!establishes(
                &context,
                &Proposition::LessThan(value.clone(), minimum.clone()),
                &[Proposition::LessOrEqual(value, minimum)],
                &[],
            ));
        }
    }
}

#[test]
fn wrong_carriers_and_addresses_do_not_gain_discrete_proofs() {
    let unsigned = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
    let signed = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    let goal = Proposition::LessThan(literal(unsigned, 0), parameter(unsigned, 1));
    let wrong = Proposition::LessOrEqual(literal(signed, 1), parameter(signed, 1));
    assert!(!establishes(&context(unsigned), &goal, &[wrong], &[]));
    let address = IntegerType::address(64).unwrap();
    let one = literal(address, 1);
    assert!(adjacent(&one, true).is_none());
    assert!(adjacent(&one, false).is_none());
}

#[test]
fn integer_bridge_uses_the_existing_shared_search_and_depth_bounds() {
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
    let value = parameter(integer_type, 1);
    let goal = Proposition::LessThan(literal(integer_type, 0), value.clone());
    let requirements = [Proposition::LessOrEqual(literal(integer_type, 1), value)];
    let mut remaining = 1;
    assert!(prove(&goal, &requirements, &[], &mut remaining, 0).is_none());
    assert_eq!(remaining, 0);
    assert!(prove(&goal, &requirements, &[], &mut remaining, 0).is_none());
    let mut remaining = MAXIMUM_SEARCH_STEPS;
    assert!(
        prove(
            &goal,
            &requirements,
            &[],
            &mut remaining,
            super::super::MAXIMUM_PROOF_DEPTH
        )
        .is_none()
    );
}

#[test]
fn disequality_uses_each_exact_entry_branch_without_exposing_it_to_siblings() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
    let context = context(integer_type);
    let value = parameter(integer_type, 1);
    let negative = Proposition::LessOrEqual(value.clone(), literal(integer_type, -1));
    let positive = Proposition::LessOrEqual(literal(integer_type, 1), value.clone());
    let zero = Proposition::Equal(value.clone(), literal(integer_type, 0));
    let goal = Proposition::Equal(
        ScalarTerm::integer_equal(integer_type, value.clone(), literal(integer_type, 0)).unwrap(),
        ScalarTerm::boolean(false),
    );
    let complete = Proposition::Disjunction(vec![negative.clone(), positive.clone()]);
    assert!(establishes(
        &context,
        &goal,
        std::slice::from_ref(&complete),
        &[]
    ));
    for unrelated_branch in [zero.clone(), Proposition::Truth] {
        let missing = Proposition::Disjunction(vec![negative.clone(), unrelated_branch]);
        assert!(!establishes(&context, &goal, &[missing], &[]));
    }
    assert!(!establishes(
        &context,
        &Proposition::LessThan(literal(integer_type, 0), value),
        &[complete],
        &[],
    ));
    assert!(!establishes(&context, &zero, &[positive], &[]));
}

#[test]
fn differing_strict_union_branches_and_nested_orders_preserve_all_alternatives() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    let context = context(integer_type);
    let value = parameter(integer_type, 1);
    let below = Proposition::LessOrEqual(value.clone(), literal(integer_type, 1));
    let above = Proposition::LessOrEqual(literal(integer_type, 4), value.clone());
    let goal = Proposition::Disjunction(vec![
        Proposition::LessThan(value.clone(), literal(integer_type, 2)),
        Proposition::LessThan(literal(integer_type, 3), value.clone()),
    ]);
    let requirements = [Proposition::Disjunction(vec![
        Proposition::Conjunction(vec![Proposition::Truth, above.clone()]),
        Proposition::Disjunction(vec![below.clone(), above.clone()]),
    ])];
    assert!(establishes(&context, &goal, &requirements, &[]));
    let gap = Proposition::Equal(value, literal(integer_type, 2));
    assert!(!establishes(
        &context,
        &goal,
        &[Proposition::Disjunction(vec![below, gap])],
        &[],
    ));
    // Neither a different body namespace nor a reconstructed body-only union
    // acquires the entry-only numeric conversion rule.
    assert!(!establishes(&context, &goal, &[], &requirements));
    let mut remaining = 8;
    assert!(prove(&goal, &requirements, &[], &mut remaining, 0).is_none());
    assert_eq!(remaining, 0);
}
