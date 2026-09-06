use super::*;
use numerics::{
    arithmetic::ArithmeticDomain,
    literals::{IntegerLanding, IntegerLiteral, LandedIntegerType},
};

fn namespace() -> Vec<ValueDeclaration> {
    [
        (101, ScalarType::Boolean),
        (
            203,
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 16).unwrap()),
        ),
        (
            307,
            ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap()),
        ),
        (
            409,
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 16).unwrap()),
        ),
    ]
    .into_iter()
    .map(|(identity, scalar_type)| ValueDeclaration {
        id: ValueId::new(identity).unwrap(),
        scalar_type,
    })
    .collect()
}

fn parameter(position: usize, primitive_type: PrimitiveType) -> CheckedScalarExpression {
    CheckedScalarExpression::Parameter {
        position,
        primitive_type,
    }
}

fn literal(value: i64, landed_type: LandedIntegerType) -> CheckedScalarExpression {
    CheckedScalarExpression::IntegerLiteral {
        literal: IntegerLiteral::from_value(value).with_landing(IntegerLanding {
            landed_type,
            domain: ArithmeticDomain::Exact,
        }),
    }
}

fn compare(
    kind: CheckedIntegerComparisonKind,
    left: CheckedScalarExpression,
    right: CheckedScalarExpression,
) -> CheckedBooleanExpression {
    CheckedBooleanExpression::IntegerComparison {
        kind,
        left: Box::new(left),
        right: Box::new(right),
    }
}

fn clause(predicate: CheckedBooleanExpression) -> Option<ClosedScalarContractValue> {
    Some(ClosedScalarContractValue::Predicate(predicate))
}

#[test]
fn mixed_parameter_positions_preserve_exact_value_ids_and_carriers() {
    let namespace = namespace();
    for (position, primitive, landed, value) in [
        (1, PrimitiveType::U16, LandedIntegerType::U16, 7),
        (2, PrimitiveType::I32, LandedIntegerType::I32, -7),
        (3, PrimitiveType::U16, LandedIntegerType::U16, 9),
    ] {
        let predicate = compare(
            CheckedIntegerComparisonKind::Equal,
            parameter(position, primitive),
            literal(value, landed),
        );
        let Proposition::Equal(subject, endpoint) = proposition(&predicate, &namespace).unwrap()
        else {
            panic!("integer equality");
        };
        assert_eq!(
            subject,
            ScalarTerm::value(namespace[position].id, namespace[position].scalar_type)
        );
        let ScalarType::Integer(scalar_type) = namespace[position].scalar_type else {
            unreachable!();
        };
        assert_eq!(
            endpoint,
            ScalarTerm::integer(
                scalar_type,
                if value < 0 {
                    IntegerValue::Signed(value.into())
                } else {
                    IntegerValue::Unsigned(value as u128)
                }
            )
            .unwrap()
        );
    }
}

#[test]
fn parameter_slot_carrier_mismatches_refuse() {
    let namespace = namespace();
    for position in [0, 2] {
        let predicate = compare(
            CheckedIntegerComparisonKind::Equal,
            parameter(position, PrimitiveType::U16),
            literal(7, LandedIntegerType::U16),
        );
        assert!(
            clauses(&[clause(predicate)], &namespace).is_err(),
            "position={position}"
        );
    }
}

#[test]
fn requires_namespace_excludes_the_result_slot() {
    let namespace = namespace();
    let predicate = compare(
        CheckedIntegerComparisonKind::Equal,
        parameter(3, PrimitiveType::U16),
        parameter(1, PrimitiveType::U16),
    );
    assert!(clauses(&[clause(predicate.clone())], &namespace[..3]).is_err());
    assert!(clauses(&[clause(predicate)], &namespace).is_ok());
}

#[test]
fn boolean_requirements_retain_exact_entry_values_and_reject_wrong_carriers() {
    let namespace = namespace();
    let predicate = CheckedBooleanExpression::Parameter { position: 0 };
    assert_eq!(
        proposition(&predicate, &namespace).unwrap(),
        canonical_equality(
            ScalarTerm::value(namespace[0].id, ScalarType::Boolean),
            ScalarTerm::boolean(true),
        )
        .unwrap(),
    );
    for position in [1, 4] {
        assert!(
            proposition(
                &CheckedBooleanExpression::Parameter { position },
                &namespace
            )
            .is_err()
        );
    }
    assert!(proposition(&predicate, &[]).is_err());
}

#[test]
fn boolean_literal_wrappers_and_negation_keep_one_canonical_predicate() {
    let namespace = namespace();
    let parameter = CheckedBooleanExpression::Parameter { position: 0 };
    for positive in [false, true] {
        let expected = canonical_equality(
            ScalarTerm::value(namespace[0].id, ScalarType::Boolean),
            ScalarTerm::boolean(positive),
        )
        .unwrap();
        for predicate in [
            CheckedBooleanExpression::Equal {
                left: Box::new(parameter.clone()),
                right: Box::new(CheckedBooleanExpression::Constant(positive)),
            },
            CheckedBooleanExpression::Equal {
                left: Box::new(CheckedBooleanExpression::Constant(positive)),
                right: Box::new(parameter.clone()),
            },
            CheckedBooleanExpression::Not(Box::new(CheckedBooleanExpression::Equal {
                left: Box::new(parameter.clone()),
                right: Box::new(CheckedBooleanExpression::Constant(!positive)),
            })),
        ] {
            assert_eq!(proposition(&predicate, &namespace).unwrap(), expected);
        }
    }
}

#[test]
fn nested_boolean_body_locals_cannot_alias_entry_slots() {
    let namespace = namespace();
    for local in [
        CheckedBooleanExpression::Local { position: 0 },
        CheckedBooleanExpression::StorageRead {
            symbol: symbols::SymbolHandle::invalid(),
        },
    ] {
        for predicate in [
            CheckedBooleanExpression::Not(Box::new(local.clone())),
            CheckedBooleanExpression::Equal {
                left: Box::new(local),
                right: Box::new(CheckedBooleanExpression::Constant(true)),
            },
        ] {
            assert!(proposition(&predicate, &namespace).is_err());
        }
    }
}

#[test]
fn nested_boolean_equality_has_a_bounded_expansion() {
    let namespace = (0..14)
        .map(|position| ValueDeclaration {
            id: ValueId::new(position + 1).unwrap(),
            scalar_type: ScalarType::Boolean,
        })
        .collect::<Vec<_>>();
    let mut predicate = CheckedBooleanExpression::Parameter { position: 0 };
    for position in 1..14 {
        predicate = CheckedBooleanExpression::Equal {
            left: Box::new(predicate),
            right: Box::new(CheckedBooleanExpression::Parameter { position }),
        };
    }
    assert!(matches!(
        proposition(&predicate, &namespace),
        Err(LoweringError::Unsupported(
            "scalar contract Boolean expansion exceeds its lowering budget"
        ))
    ));
}

#[test]
fn reversed_parameter_result_equality_uses_canonical_serialized_term_order() {
    // Above 255, the codec's little-endian ID bytes do not sort numerically.
    // Either the input or the result may be first; source orientation is irrelevant.
    for (input_identity, result_identity, canonical_first) in
        [(1, 2, 1), (2, 1, 1), (511, 512, 512), (512, 511, 512)]
    {
        let mut namespace = namespace();
        namespace[1].id = ValueId::new(input_identity).unwrap();
        namespace[3].id = ValueId::new(result_identity).unwrap();
        let input = parameter(1, PrimitiveType::U16);
        let result = parameter(3, PrimitiveType::U16);
        let forward = compare(
            CheckedIntegerComparisonKind::Equal,
            result.clone(),
            input.clone(),
        );
        let reversed = compare(CheckedIntegerComparisonKind::Equal, input, result);
        let (first, second) = if input_identity == canonical_first {
            (1, 3)
        } else {
            (3, 1)
        };
        let expected = Proposition::Equal(
            ScalarTerm::value(namespace[first].id, namespace[first].scalar_type),
            ScalarTerm::value(namespace[second].id, namespace[second].scalar_type),
        );
        for predicate in [forward, reversed] {
            assert_eq!(proposition(&predicate, &namespace).unwrap(), expected);
        }
    }
}

#[test]
fn duplicate_and_conjoined_requirements_have_one_stable_proposition() {
    let namespace = namespace();
    let low = compare(
        CheckedIntegerComparisonKind::LessOrEqual,
        literal(0, LandedIntegerType::U16),
        parameter(1, PrimitiveType::U16),
    );
    let high = compare(
        CheckedIntegerComparisonKind::LessOrEqual,
        parameter(1, PrimitiveType::U16),
        literal(255, LandedIntegerType::U16),
    );
    let expected = clauses(
        &[clause(low.clone()), clause(high.clone())],
        &namespace[..3],
    )
    .unwrap();
    let conjunction = CheckedBooleanExpression::And {
        left: Box::new(high.clone()),
        right: Box::new(low.clone()),
    };
    let repeated = [
        Some(ClosedScalarContractValue::Boolean(true)),
        clause(conjunction),
        clause(low),
        Some(ClosedScalarContractValue::Integer(
            IntegerLiteral::from_value(7),
        )),
        clause(high),
        Some(ClosedScalarContractValue::Boolean(false)),
    ];
    assert_eq!(clauses(&repeated, &namespace[..3]).unwrap(), expected);
    assert!(matches!(expected, Some(Proposition::Conjunction(parts)) if parts.len() == 2));
    assert_eq!(
        clauses(
            &[
                Some(ClosedScalarContractValue::Boolean(false)),
                Some(ClosedScalarContractValue::Boolean(true))
            ],
            &namespace[..3],
        )
        .unwrap(),
        Some(Proposition::Truth)
    );
}

#[test]
fn absent_clauses_and_unsupported_clauses_remain_distinct() {
    let namespace = namespace();
    assert_eq!(clauses(&[], &namespace[..3]).unwrap(), None);
    assert!(clauses(&[None], &namespace[..3]).is_err());
    assert!(
        clauses(
            &[Some(ClosedScalarContractValue::Boolean(true)), None],
            &namespace[..3],
        )
        .is_err()
    );
}
