//! Compare the closed-form orbit decision with actual fixed-width execution.
use super::super::{Comparison, Operand};
use super::*;
use std::cmp::Ordering::{Equal, Less};

#[test]
fn every_three_bit_signed_and_unsigned_retry_agrees_with_execution() {
    for sign in [IntegerSign::Unsigned, IntegerSign::Signed] {
        let ty = IntegerType::new(sign, 3).unwrap();
        let word = |bits: u8| Scalar::Integer {
            scalar_type: ty,
            value: match sign {
                IntegerSign::Unsigned => IntegerValue::Unsigned(bits.into()),
                IntegerSign::Signed => IntegerValue::Signed(if bits < 4 {
                    i128::from(bits)
                } else {
                    i128::from(bits) - 8
                }),
            },
        };
        for operation in [Rmw::FetchAdd, Rmw::FetchSub, Rmw::FetchXor] {
            for step in 0..8 {
                let orbit = Orbit {
                    operation,
                    operand: word(step),
                };
                for pivot in 0..8 {
                    for mask in 0..8 {
                        for order in [Equal, Less, Less.reverse()] {
                            for positive in [false, true] {
                                let test = SpinCondition {
                                    comparison: Comparison {
                                        pivot: Operand::Literal(word(pivot)),
                                        order,
                                        mask: Some(Operand::Literal(word(mask))),
                                    },
                                    positive,
                                };
                                let mut outcomes = Vec::new();
                                for seed in 0..8 {
                                    let initial = word(seed);
                                    let mut value = initial;
                                    let actual = loop {
                                        let next = orbit.next(value).unwrap();
                                        if !test.matches(value).unwrap() {
                                            break Some(next);
                                        }
                                        if next == initial {
                                            break None;
                                        }
                                        value = next;
                                    };
                                    outcomes.push(actual);
                                    match orbit.trip(test, Some(initial)).unwrap() {
                                        Trip::Stuck => assert_eq!(actual, None),
                                        Trip::Returns(reported) => {
                                            assert!(actual.is_some());
                                            if let Some(value) = reported {
                                                assert_eq!(actual, Some(value));
                                            }
                                        }
                                    }
                                }
                                let returns =
                                    outcomes.iter().filter(|value| value.is_some()).count();
                                match orbit.trip(test, None) {
                                    Some(Trip::Stuck) => assert_eq!(returns, 0),
                                    Some(Trip::Returns(post)) => {
                                        assert_eq!(returns, outcomes.len());
                                        if post.is_some() {
                                            assert!(outcomes.iter().all(|item| *item == post));
                                        }
                                    }
                                    None => assert!(
                                        returns > 0 && returns < outcomes.len(),
                                        "uniform orbit omitted: {orbit:?}, {test:?}, {outcomes:?}"
                                    ),
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn an_odd_step_visits_the_full_128_bit_domain_without_enumeration() {
    let ty = IntegerType::new(IntegerSign::Unsigned, 128).unwrap();
    let word = |bits| Scalar::Integer {
        scalar_type: ty,
        value: IntegerValue::Unsigned(bits),
    };
    let orbit = Orbit {
        operation: Rmw::FetchAdd,
        operand: word(3),
    };
    let test = SpinCondition {
        comparison: Comparison::equal(word(u128::MAX)),
        positive: false,
    };
    let Some(Trip::Returns(Some(post))) = orbit.trip(test, None) else {
        panic!("an odd modular step must reach the exiting word")
    };
    assert_eq!(post, word(2));
}
