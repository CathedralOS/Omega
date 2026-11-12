// A guard that cannot be taken, where the facts that rule it out relate two
// values and no literal: after `requires low <= high`, the arm of
// `high < low` is unreachable, so the arithmetic on it owes nothing.
//
// The prover closes it without a kernel rule of its own: the arm knows
// `c == true` for the guard's value `c`, and `c == false` denotes
// `low <= high`, which is known, so `true == false` follows and the kernel's
// denotation calls that `Falsehood`. This checks that step against arithmetic
// on every pair of 8-bit values; `ranges` runs it on programs
// (`an_order_between_two_values_rules_out_the_guard_that_contradicts_it`).

use proof_admission::{PrimitiveJudgment, check_value_equality_denotation, decide_primitive};
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, ValueId};

use super::*;

#[test]
fn a_guard_and_its_negation_agree_with_arithmetic_on_every_pair_of_small_values() {
    const CONDITION: u64 = 1;
    const LOW: u64 = 2;
    const HIGH: u64 = 3;
    for sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
        let integer = IntegerType::new(sign, 8).unwrap();
        let scalar = ScalarType::Integer(integer);
        let context = PropositionContext::from_value_types([
            (ValueId::new(CONDITION).unwrap(), ScalarType::Boolean),
            (ValueId::new(LOW).unwrap(), scalar),
            (ValueId::new(HIGH).unwrap(), scalar),
        ])
        .unwrap();
        let condition = ScalarTerm::value(id(CONDITION), ScalarType::Boolean);
        let is = |value| Proposition::Equal(condition.clone(), ScalarTerm::Boolean(value));
        // `c == (high < low)`, the guard's result equation.
        let defined = |high: &ScalarTerm, low: &ScalarTerm| {
            Proposition::Equal(
                condition.clone(),
                ScalarTerm::IntegerLessThan {
                    scalar_type: integer,
                    left: Box::new(high.clone()),
                    right: Box::new(low.clone()),
                },
            )
        };
        let denotes = |goal: &Proposition, definition: &Proposition| {
            check_value_equality_denotation(&context, goal, [definition]).unwrap()
        };

        // Over values: the taken guard is the strict order, and its negation
        // is exactly the opposite non-strict order. This is the one step the
        // prover's contradiction rests on.
        let (low, high) = (formal(LOW, scalar), formal(HIGH, scalar));
        let definition = defined(&high, &low);
        assert_eq!(
            denotes(&is(true), &definition),
            Proposition::LessThan(high.clone(), low.clone())
        );
        assert_eq!(
            denotes(&is(false), &definition),
            Proposition::LessOrEqual(low.clone(), high.clone())
        );

        // Over every pair of values of the type: those two orders, decided by
        // the kernel on literals, are what the machine computes, and exactly
        // one of them holds. So `low <= high` and a taken `high < low` never
        // hold together, for either sign, including both ends of the range.
        let (minimum, maximum): (i128, i128) = match sign {
            IntegerSign::Signed => (i128::from(i8::MIN), i128::from(i8::MAX)),
            IntegerSign::Unsigned => (0, i128::from(u8::MAX)),
        };
        let literal = |number: i128| {
            ScalarTerm::integer(integer, IntegerValue::for_sign(sign, number).unwrap()).unwrap()
        };
        let holds = |proposition: &Proposition| {
            decide_primitive(
                &context,
                proposition,
                PrimitiveJudgment::ClosedIntegerRelation,
            )
            .is_ok()
        };
        let truth = |holds: bool| {
            if holds {
                Proposition::Truth
            } else {
                Proposition::Falsehood
            }
        };
        let mut pairs = 0u32;
        for low_number in minimum..=maximum {
            for high_number in minimum..=maximum {
                let (low, high) = (literal(low_number), literal(high_number));
                let taken = high_number < low_number;
                let known = low_number <= high_number;
                assert_ne!(taken, known, "{sign:?} low {low_number} high {high_number}");
                assert_eq!(
                    holds(&Proposition::LessThan(high.clone(), low.clone())),
                    taken
                );
                assert_eq!(
                    holds(&Proposition::LessOrEqual(low.clone(), high.clone())),
                    known
                );
                let definition = defined(&high, &low);
                assert_eq!(denotes(&is(true), &definition), truth(taken));
                assert_eq!(denotes(&is(false), &definition), truth(known));
                pairs += 1;
            }
        }
        assert_eq!(pairs, 256 * 256);
    }
}
