use super::{
    ConditionFact, Proposition, PropositionContext, ScalarTerm, ScalarType, ValueId, condition_fact,
};
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use std::collections::BTreeMap;

fn value(index: u64, scalar_type: ScalarType) -> ScalarTerm {
    ScalarTerm::value(ValueId::new(index).unwrap(), scalar_type)
}

fn integer(scalar_type: IntegerType, literal: i128) -> ScalarTerm {
    ScalarTerm::integer(
        scalar_type,
        match scalar_type.sign() {
            IntegerSign::Signed => IntegerValue::Signed(literal),
            IntegerSign::Unsigned => IntegerValue::Unsigned(u128::try_from(literal).unwrap()),
        },
    )
    .unwrap()
}

/// The machine-wide proposition context a certificate check would run
/// under: every `Value` leaf occurring in the roster keeps its declared
/// type.
fn context(axioms: &[Proposition]) -> PropositionContext {
    let mut types = BTreeMap::new();
    let mut pending: Vec<&ScalarTerm> = Vec::new();
    for axiom in axioms {
        pending.extend(proposition_terms(axiom));
    }
    while let Some(term) = pending.pop() {
        match term {
            ScalarTerm::Value { id, scalar_type } => {
                types.insert(*id, *scalar_type);
            }
            ScalarTerm::Boolean(_)
            | ScalarTerm::Integer { .. }
            | ScalarTerm::BooleanField { .. }
            | ScalarTerm::IntegerField { .. } => {}
            ScalarTerm::BooleanNot { operand }
            | ScalarTerm::IntegerBitwiseNot { operand, .. }
            | ScalarTerm::IntegerWiden { operand, .. }
            | ScalarTerm::IntegerExactCast { operand, .. } => pending.push(operand.as_ref()),
            ScalarTerm::WrappingIntegerShiftLeft {
                value: left,
                count: right,
                ..
            }
            | ScalarTerm::WrappingIntegerShiftRight {
                value: left,
                count: right,
                ..
            }
            | ScalarTerm::ExactIntegerShiftLeft {
                value: left,
                count: right,
                ..
            }
            | ScalarTerm::ExactIntegerShiftRight {
                value: left,
                count: right,
                ..
            }
            | ScalarTerm::BooleanEqual { left, right }
            | ScalarTerm::IntegerEqual { left, right, .. }
            | ScalarTerm::IntegerLessThan { left, right, .. }
            | ScalarTerm::IntegerLessOrEqual { left, right, .. }
            | ScalarTerm::IntegerBitwiseAnd { left, right, .. }
            | ScalarTerm::IntegerBitwiseOr { left, right, .. }
            | ScalarTerm::IntegerBitwiseXor { left, right, .. }
            | ScalarTerm::ExactIntegerAdd { left, right, .. }
            | ScalarTerm::ExactIntegerSubtract { left, right, .. }
            | ScalarTerm::ExactIntegerMultiply { left, right, .. }
            | ScalarTerm::ExactIntegerDivide { left, right, .. }
            | ScalarTerm::ExactIntegerRemainder { left, right, .. }
            | ScalarTerm::WrappingIntegerDivide { left, right, .. }
            | ScalarTerm::WrappingIntegerRemainder { left, right, .. }
            | ScalarTerm::SaturatingIntegerDivide { left, right, .. }
            | ScalarTerm::SaturatingIntegerRemainder { left, right, .. }
            | ScalarTerm::WrappingIntegerAdd { left, right, .. }
            | ScalarTerm::WrappingIntegerSubtract { left, right, .. }
            | ScalarTerm::WrappingIntegerMultiply { left, right, .. }
            | ScalarTerm::SaturatingIntegerAdd { left, right, .. }
            | ScalarTerm::SaturatingIntegerSubtract { left, right, .. }
            | ScalarTerm::SaturatingIntegerMultiply { left, right, .. } => {
                pending.push(left.as_ref());
                pending.push(right.as_ref());
            }
        }
    }
    PropositionContext::from_value_types(types).unwrap()
}

fn proposition_terms(proposition: &Proposition) -> Vec<&ScalarTerm> {
    match proposition {
        Proposition::Equal(left, right)
        | Proposition::LessThan(left, right)
        | Proposition::LessOrEqual(left, right) => vec![left, right],
        Proposition::Conjunction(children) | Proposition::Disjunction(children) => {
            let mut terms = Vec::new();
            for child in children {
                if let Proposition::Equal(left, right)
                | Proposition::LessThan(left, right)
                | Proposition::LessOrEqual(left, right) = child
                {
                    terms.push(left);
                    terms.push(right);
                }
            }
            terms
        }
        _ => Vec::new(),
    }
}

fn compare_alias_literal() -> ScalarTerm {
    ScalarTerm::Boolean(true)
}

fn selected(predicate: ScalarTerm, positive: bool) -> ConditionFact {
    let condition = value(1, ScalarType::Boolean);
    let axioms = [Proposition::Equal(condition, predicate)];
    condition_fact(
        ValueId::new(1).unwrap(),
        positive,
        &axioms,
        &|id| ScalarTerm::value(id, ScalarType::Boolean),
        &context(&axioms),
    )
    .unwrap()
}

#[test]
fn equality_complements_keep_full_signed_and_unsigned_nonzero_meaning() {
    for (sign, bits) in [
        (IntegerSign::Unsigned, 8),
        (IntegerSign::Signed, 8),
        (IntegerSign::Signed, 1),
    ] {
        let scalar_type = IntegerType::new(sign, bits).unwrap();
        let subject = value(2, ScalarType::Integer(scalar_type));
        let zero = integer(scalar_type, 0);
        let expected = match (sign, bits) {
            (IntegerSign::Unsigned, _) => {
                Proposition::LessOrEqual(integer(scalar_type, 1), subject.clone())
            }
            (IntegerSign::Signed, 1) => {
                Proposition::LessOrEqual(subject.clone(), integer(scalar_type, -1))
            }
            (IntegerSign::Signed, _) => Proposition::Disjunction(vec![
                Proposition::LessOrEqual(subject.clone(), integer(scalar_type, -1)),
                Proposition::LessOrEqual(integer(scalar_type, 1), subject.clone()),
            ]),
        };
        for (left, right) in [
            (subject.clone(), zero.clone()),
            (zero.clone(), subject.clone()),
        ] {
            let predicate = ScalarTerm::IntegerEqual {
                scalar_type,
                left: Box::new(left.clone()),
                right: Box::new(right.clone()),
            };
            assert_eq!(
                selected(predicate.clone(), true).proposition,
                Proposition::Equal(left, right)
            );
            assert_eq!(selected(predicate.clone(), false).proposition, expected);
            assert_eq!(
                selected(ScalarTerm::boolean_not(predicate).unwrap(), true).proposition,
                expected
            );
        }
    }
}

#[test]
fn order_complements_reverse_endpoints_without_changing_carriers() {
    for sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
        let scalar_type = IntegerType::new(sign, 16).unwrap();
        let left = value(2, ScalarType::Integer(scalar_type));
        let right = value(3, ScalarType::Integer(scalar_type));
        let less = ScalarTerm::IntegerLessThan {
            scalar_type,
            left: Box::new(left.clone()),
            right: Box::new(right.clone()),
        };
        let inclusive = ScalarTerm::IntegerLessOrEqual {
            scalar_type,
            left: Box::new(left.clone()),
            right: Box::new(right.clone()),
        };
        assert_eq!(
            selected(less.clone(), true).proposition,
            Proposition::LessThan(left.clone(), right.clone())
        );
        assert_eq!(
            selected(less, false).proposition,
            Proposition::LessOrEqual(right.clone(), left.clone())
        );
        assert_eq!(
            selected(inclusive.clone(), true).proposition,
            Proposition::LessOrEqual(left.clone(), right.clone())
        );
        assert_eq!(
            selected(inclusive, false).proposition,
            Proposition::LessThan(right.clone(), left.clone())
        );
        let equal = ScalarTerm::IntegerEqual {
            scalar_type,
            left: Box::new(left.clone()),
            right: Box::new(right.clone()),
        };
        assert_eq!(
            selected(equal, false).proposition,
            Proposition::Disjunction(vec![
                Proposition::LessThan(left.clone(), right.clone()),
                Proposition::LessThan(right, left)
            ])
        );
    }
}

#[test]
fn boolean_aliases_and_false_wrappers_preserve_selected_polarity() {
    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let subject = value(4, ScalarType::Integer(scalar_type));
    let comparison = ScalarTerm::IntegerEqual {
        scalar_type,
        left: Box::new(subject.clone()),
        right: Box::new(integer(scalar_type, 0)),
    };
    let condition = value(1, ScalarType::Boolean);
    let alias = value(2, ScalarType::Boolean);
    let comparison_value = value(3, ScalarType::Boolean);
    let axioms = [
        Proposition::Equal(comparison_value.clone(), comparison),
        Proposition::Equal(
            alias.clone(),
            ScalarTerm::BooleanEqual {
                left: Box::new(comparison_value),
                right: Box::new(ScalarTerm::Boolean(false)),
            },
        ),
        Proposition::Equal(condition.clone(), alias.clone()),
    ];
    assert_eq!(
        condition_fact(
            ValueId::new(1).unwrap(),
            true,
            &axioms,
            &|id| { ScalarTerm::value(id, ScalarType::Boolean) },
            &context(&axioms)
        )
        .map(|fact| fact.proposition),
        Some(Proposition::LessOrEqual(integer(scalar_type, 1), subject))
    );
    assert_eq!(
        condition_fact(
            ValueId::new(8).unwrap(),
            false,
            &axioms,
            &|id| { ScalarTerm::value(id, ScalarType::Boolean) },
            &context(&axioms)
        )
        .map(|fact| fact.proposition),
        Some(Proposition::Equal(
            value(8, ScalarType::Boolean),
            ScalarTerm::Boolean(false)
        ))
    );
    for positive in [false, true] {
        assert_eq!(
            selected(
                ScalarTerm::BooleanEqual {
                    left: Box::new(condition.clone()),
                    right: Box::new(alias.clone())
                },
                positive
            )
            .proposition,
            if positive {
                Proposition::Equal(condition.clone(), alias.clone())
            } else {
                Proposition::Equal(
                    ScalarTerm::boolean_not(condition.clone()).unwrap(),
                    alias.clone(),
                )
            }
        );
    }
}

#[test]
fn strict_literal_bounds_are_discrete_on_every_fixed_carrier() {
    use super::super::discrete::strict_bound;
    for sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
        let scalar_type = IntegerType::new(sign, 8).unwrap();
        let subject = value(2, ScalarType::Integer(scalar_type));
        assert_eq!(
            strict_bound(&Proposition::LessThan(
                integer(scalar_type, 0),
                subject.clone()
            )),
            Some(Proposition::LessOrEqual(
                integer(scalar_type, 1),
                subject.clone()
            ))
        );
        assert_eq!(
            strict_bound(&Proposition::LessThan(
                subject.clone(),
                integer(scalar_type, 3)
            )),
            Some(Proposition::LessOrEqual(
                subject.clone(),
                integer(scalar_type, 2)
            ))
        );
        assert_eq!(
            strict_bound(&Proposition::LessThan(
                ScalarTerm::integer(scalar_type, scalar_type.maximum_value()).unwrap(),
                subject.clone()
            )),
            Some(Proposition::Falsehood)
        );
        assert_eq!(
            strict_bound(&Proposition::LessThan(
                subject,
                ScalarTerm::integer(scalar_type, scalar_type.minimum_value()).unwrap()
            )),
            Some(Proposition::Falsehood)
        );
    }
    let address = IntegerType::address(64).unwrap();
    assert!(
        strict_bound(&Proposition::LessThan(
            integer(address, 0),
            value(2, ScalarType::Integer(address))
        ))
        .is_none()
    );
}

#[test]
fn strict_value_order_excludes_exact_fixed_carrier_endpoints_on_selected_branch() {
    use super::super::discrete::strict_carrier_bounds;
    for sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
        for bits in [1, 8, 64, 128] {
            let integer_type = IntegerType::new(sign, bits).unwrap();
            let minimum_next = match integer_type.minimum_value() {
                IntegerValue::Signed(minimum) => IntegerValue::Signed(minimum + 1),
                IntegerValue::Unsigned(minimum) => IntegerValue::Unsigned(minimum + 1),
            };
            let maximum_previous = match integer_type.maximum_value() {
                IntegerValue::Signed(maximum) => IntegerValue::Signed(maximum - 1),
                IntegerValue::Unsigned(maximum) => IntegerValue::Unsigned(maximum - 1),
            };
            for (left, right) in [(2, 3), (3, 2)] {
                let left = value(left, ScalarType::Integer(integer_type));
                let right = value(right, ScalarType::Integer(integer_type));
                let expected = Some([
                    Proposition::LessOrEqual(
                        left.clone(),
                        ScalarTerm::integer(integer_type, maximum_previous).unwrap(),
                    ),
                    Proposition::LessOrEqual(
                        ScalarTerm::integer(integer_type, minimum_next).unwrap(),
                        right.clone(),
                    ),
                ]);
                let strict = ScalarTerm::IntegerLessThan {
                    scalar_type: integer_type,
                    left: Box::new(left.clone()),
                    right: Box::new(right.clone()),
                };
                assert_eq!(
                    strict_carrier_bounds(&selected(strict.clone(), true).proposition),
                    expected
                );
                assert!(strict_carrier_bounds(&selected(strict, false).proposition).is_none());
                let opposite = ScalarTerm::IntegerLessOrEqual {
                    scalar_type: integer_type,
                    left: Box::new(right),
                    right: Box::new(left),
                };
                assert_eq!(
                    strict_carrier_bounds(&selected(opposite, false).proposition),
                    expected
                );
            }
        }
    }
}

#[test]
fn strict_carrier_endpoints_reject_mixed_address_and_nonscalar_value_shapes() {
    use super::super::discrete::strict_carrier_bounds;
    let unsigned = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let left = value(2, ScalarType::Integer(unsigned));
    for right in [
        value(
            3,
            ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 64).unwrap()),
        ),
        value(
            3,
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).unwrap()),
        ),
        value(3, ScalarType::Boolean),
        integer(unsigned, 3),
        ScalarTerm::exact_integer_add(unsigned, left.clone(), integer(unsigned, 1)).unwrap(),
    ] {
        assert!(strict_carrier_bounds(&Proposition::LessThan(left.clone(), right)).is_none());
    }
    let address = ScalarType::Integer(IntegerType::address(64).unwrap());
    assert!(
        strict_carrier_bounds(&Proposition::LessThan(value(2, address), value(3, address)))
            .is_none()
    );
    assert!(strict_carrier_bounds(&Proposition::LessOrEqual(left.clone(), left)).is_none());
}

#[test]
fn condition_alias_cycles_fail_closed_without_a_depth_limit() {
    let mut axioms = Vec::new();
    for index in 1..300 {
        axioms.push(Proposition::Equal(
            value(index, ScalarType::Boolean),
            value(index + 1, ScalarType::Boolean),
        ));
    }
    axioms.push(Proposition::Equal(
        value(300, ScalarType::Boolean),
        ScalarTerm::Boolean(true),
    ));
    let resolved = condition_fact(
        ValueId::new(1).unwrap(),
        true,
        &axioms,
        &|id| ScalarTerm::value(id, ScalarType::Boolean),
        &context(&axioms),
    )
    .unwrap();
    assert_eq!(resolved.proposition, Proposition::Truth);
    // The transport check is bounded: a chain this deep exhausts its work
    // budget, so the emission remains a licensed premise introduction even
    // though the fact itself is correct.
    assert!(!resolved.certified);
    axioms.pop();
    axioms.push(Proposition::Equal(
        value(300, ScalarType::Boolean),
        value(1, ScalarType::Boolean),
    ));
    assert!(
        condition_fact(
            ValueId::new(1).unwrap(),
            true,
            &axioms,
            &|id| { ScalarTerm::value(id, ScalarType::Boolean) },
            &context(&axioms)
        )
        .is_none()
    );
}

#[test]
fn reverse_edge_alias_keeps_the_entry_formals_selected_polarity() {
    let formal = value(1, ScalarType::Boolean);
    let alias = value(4, ScalarType::Boolean);
    let axioms = [Proposition::Equal(alias, formal.clone())];
    for positive in [false, true] {
        assert_eq!(
            condition_fact(
                ValueId::new(4).unwrap(),
                positive,
                &axioms,
                &|id| { ScalarTerm::value(id, ScalarType::Boolean) },
                &context(&axioms)
            )
            .map(|fact| fact.proposition),
            Some(Proposition::Equal(
                formal.clone(),
                ScalarTerm::Boolean(positive)
            )),
        );
    }
}

#[test]
fn disequality_at_carrier_extrema_does_not_wrap_adjacent_bounds() {
    for sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
        let scalar_type = IntegerType::new(sign, 128).unwrap();
        let subject = value(2, ScalarType::Integer(scalar_type));
        for maximum in [false, true] {
            let endpoint = if maximum {
                scalar_type.maximum_value()
            } else {
                scalar_type.minimum_value()
            };
            let adjacent = match (endpoint, maximum) {
                (IntegerValue::Signed(value), true) => IntegerValue::Signed(value - 1),
                (IntegerValue::Signed(value), false) => IntegerValue::Signed(value + 1),
                (IntegerValue::Unsigned(value), true) => IntegerValue::Unsigned(value - 1),
                (IntegerValue::Unsigned(value), false) => IntegerValue::Unsigned(value + 1),
            };
            let adjacent = ScalarTerm::integer(scalar_type, adjacent).unwrap();
            let predicate = ScalarTerm::IntegerEqual {
                scalar_type,
                left: Box::new(subject.clone()),
                right: Box::new(ScalarTerm::integer(scalar_type, endpoint).unwrap()),
            };
            let expected = if maximum {
                Proposition::LessOrEqual(subject.clone(), adjacent)
            } else {
                Proposition::LessOrEqual(adjacent, subject.clone())
            };
            let fact = selected(predicate, false);
            assert_eq!(fact.proposition, expected);
            // The literal-adjacency strengthening remains a licensed
            // premise introduction: no value-equation transport discharges
            // a bound the roster never stated.
            assert!(!fact.certified);
        }
    }
}

#[test]
fn checked_transport_certifies_denotation_facts_only() {
    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let subject = value(2, ScalarType::Integer(scalar_type));
    let zero = integer(scalar_type, 0);
    let condition = ValueId::new(1).unwrap();
    let compare = ScalarTerm::IntegerLessThan {
        scalar_type,
        left: Box::new(subject.clone()),
        right: Box::new(zero.clone()),
    };
    let axioms = [Proposition::Equal(value(1, ScalarType::Boolean), compare)];
    let proposition_context = context(&axioms);
    // A denotation-recognized arm fact carries a generation-time-checked
    // transport certificate: the arm's truth premise transports through the
    // roster's own equation to exactly the emitted proposition.
    for (positive, expected) in [
        (true, Proposition::LessThan(subject.clone(), zero.clone())),
        (false, Proposition::LessOrEqual(zero, subject.clone())),
    ] {
        let fact = condition_fact(
            condition,
            positive,
            &axioms,
            &|id| ScalarTerm::value(id, ScalarType::Boolean),
            &proposition_context,
        )
        .unwrap();
        assert_eq!(fact.proposition, expected);
        assert!(fact.certified);
    }
    // A short alias chain transports too: the premise follows the roster's
    // equations hop by hop before the denotation is compared.
    let alias = value(5, ScalarType::Boolean);
    let axioms = [
        Proposition::Equal(value(1, ScalarType::Boolean), alias.clone()),
        Proposition::Equal(alias, compare_alias_literal()),
    ];
    let fact = condition_fact(
        condition,
        true,
        &axioms,
        &|id| ScalarTerm::value(id, ScalarType::Boolean),
        &context(&axioms),
    )
    .unwrap();
    assert_eq!(fact.proposition, Proposition::Truth);
    assert!(fact.certified);
    // An unsatisfiable selected arm's falsehood is likewise the transport
    // of its closed inconsistent premise.
    let axioms = [Proposition::Equal(
        value(1, ScalarType::Boolean),
        ScalarTerm::Boolean(false),
    )];
    let fact = condition_fact(
        condition,
        true,
        &axioms,
        &|id| ScalarTerm::value(id, ScalarType::Boolean),
        &context(&axioms),
    )
    .unwrap();
    assert_eq!(fact.proposition, Proposition::Falsehood);
    assert!(fact.certified);
}

/// The transport cites only the roster equations its denotation can reach:
/// unrelated `Equal` rows before and after the selected arm's own equation
/// are not cloned into the certificate, and the certified classification is
/// unaffected.
#[test]
fn transport_roster_cites_only_reachable_equations() {
    let condition = ValueId::new(1).unwrap();
    let mut axioms = Vec::new();
    for index in 10..20u64 {
        axioms.push(Proposition::Equal(
            value(index, ScalarType::Boolean),
            value(index + 1, ScalarType::Boolean),
        ));
    }
    let relevant = axioms.len();
    axioms.push(Proposition::Equal(
        value(1, ScalarType::Boolean),
        ScalarTerm::Boolean(true),
    ));
    for index in 30..40u64 {
        axioms.push(Proposition::Equal(
            value(index, ScalarType::Boolean),
            value(index + 1, ScalarType::Boolean),
        ));
    }
    let premise = Proposition::Equal(value(1, ScalarType::Boolean), ScalarTerm::Boolean(true));
    assert_eq!(
        super::reachable_axiom_equalities(&axioms, [&premise, &Proposition::Truth])
            .iter()
            .map(|(index, _)| *index)
            .collect::<Vec<_>>(),
        [relevant]
    );
    let fact = condition_fact(
        condition,
        true,
        &axioms,
        &|id| ScalarTerm::value(id, ScalarType::Boolean),
        &context(&axioms),
    )
    .unwrap();
    assert_eq!(fact.proposition, Proposition::Truth);
    assert!(fact.certified);
}
