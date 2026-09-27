use super::super::super::{CheckedIntegerComparisonKind, PrimitiveType};
use super::super::{
    CheckedBooleanExpression, CheckedScalarExpression, ClosedScalarContractValue,
    ContractViewNamespace, LoweringError, ValueDeclaration, clauses, proposition,
};
use super::{IntegerSign, IntegerType, ScalarType, ValueId};
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
        qualifications: Default::default(),
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
fn parameter_slot_carrier_mismatches_refuse() {
    let namespace = namespace();
    for position in [0, 2] {
        let predicate = compare(
            CheckedIntegerComparisonKind::Equal,
            parameter(position, PrimitiveType::U16),
            literal(7, LandedIntegerType::U16),
        );
        assert!(
            clauses(
                &[clause(predicate)],
                &namespace,
                &[],
                &ContractViewNamespace::EMPTY
            )
            .is_err(),
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
    assert!(
        clauses(
            &[clause(predicate.clone())],
            &namespace[..3],
            &[],
            &ContractViewNamespace::EMPTY
        )
        .is_err()
    );
    assert!(
        clauses(
            &[clause(predicate)],
            &namespace,
            &[],
            &ContractViewNamespace::EMPTY
        )
        .is_ok()
    );
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
            assert!(
                proposition(&predicate, &namespace, &[], &ContractViewNamespace::EMPTY).is_err()
            );
        }
    }
}

#[test]
fn nested_boolean_denotations_avoid_expansion_but_keep_resource_limits() {
    let namespace = (0..14)
        .map(|position| ValueDeclaration {
            qualifications: Default::default(),
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
    assert!(proposition(&predicate, &namespace, &[], &ContractViewNamespace::EMPTY).is_ok());
    for _ in 0..65 {
        predicate = CheckedBooleanExpression::Not(Box::new(predicate));
    }
    let deep = CheckedBooleanExpression::Equal {
        left: Box::new(predicate),
        right: Box::new(CheckedBooleanExpression::Parameter { position: 1 }),
    };
    assert!(matches!(
        proposition(&deep, &namespace, &[], &ContractViewNamespace::EMPTY),
        Err(LoweringError::Unsupported(
            "scalar contract Boolean denotation exceeds its depth limit"
        ))
    ));
    // Logical conjunctions still need polarity expansion. A small authored
    // tree must not manufacture an unbounded expanded proof proposition.
    let mut logical = CheckedBooleanExpression::And {
        left: Box::new(CheckedBooleanExpression::Parameter { position: 0 }),
        right: Box::new(CheckedBooleanExpression::Parameter { position: 1 }),
    };
    for position in 2..14 {
        logical = CheckedBooleanExpression::Equal {
            left: Box::new(logical),
            right: Box::new(CheckedBooleanExpression::Parameter { position }),
        };
    }
    assert!(matches!(
        proposition(&logical, &namespace, &[], &ContractViewNamespace::EMPTY),
        Err(LoweringError::Unsupported(
            "scalar contract Boolean expansion exceeds its lowering budget"
        ))
    ));
}

#[test]
fn absent_clauses_and_unsupported_clauses_remain_distinct() {
    let namespace = namespace();
    assert_eq!(
        clauses(&[], &namespace[..3], &[], &ContractViewNamespace::EMPTY).unwrap(),
        None
    );
    assert!(clauses(&[None], &namespace[..3], &[], &ContractViewNamespace::EMPTY).is_err());
    assert!(
        clauses(
            &[Some(ClosedScalarContractValue::Boolean(true)), None],
            &namespace[..3],
            &[],
            &ContractViewNamespace::EMPTY
        )
        .is_err()
    );
}

#[test]
fn covered_requires_delivers_float_range_clauses_only_through_the_roster() {
    use super::super::covered_requires;
    use checked_trees::{ClosedFloatRangeRequirement, ClosedScalarValueContractPlan};
    use semantic_vocabulary::IeeeFloatValue;

    let range = ClosedFloatRangeRequirement {
        position: 0,
        primitive_type: PrimitiveType::F32,
        minimum: IeeeFloatValue::Binary32(0.5f32.to_bits()),
        maximum: IeeeFloatValue::Binary32(2.25f32.to_bits()),
        maximum_inclusive: false,
    };
    let plan = |requires: Vec<Option<ClosedScalarContractValue>>,
                roster: Option<Vec<ClosedFloatRangeRequirement>>| {
        ClosedScalarValueContractPlan::new(requires, Vec::new(), false, false)
            .with_float_entry_ranges(roster)
    };
    // A `FloatRange` clause is covered by its identical retained row; the
    // clause never enters the proposition tail.
    assert!(
        covered_requires(&plan(
            vec![Some(ClosedScalarContractValue::FloatRange(range))],
            Some(vec![range]),
        ))
        .unwrap()
        .is_empty()
    );
    // Drifted evidence, a missing roster, an empty roster, a leftover roster
    // row, and a clause that lost its payload all fail closed.
    let mut drifted = range;
    drifted.maximum = IeeeFloatValue::Binary32(2.5f32.to_bits());
    for (requires, roster) in [
        (
            vec![Some(ClosedScalarContractValue::FloatRange(range))],
            Some(vec![drifted]),
        ),
        (
            vec![Some(ClosedScalarContractValue::FloatRange(range))],
            None,
        ),
        (
            vec![Some(ClosedScalarContractValue::FloatRange(range))],
            Some(Vec::new()),
        ),
        (Vec::new(), Some(vec![range])),
        (vec![None], Some(vec![range])),
    ] {
        assert!(
            covered_requires(&plan(requires, roster)).is_err(),
            "roster and requires tail must carry the same rows"
        );
    }
    // The clause still cannot become a proposition.
    assert!(
        clauses(
            &[Some(ClosedScalarContractValue::FloatRange(range))],
            &namespace(),
            &[],
            &ContractViewNamespace::EMPTY,
        )
        .is_err()
    );
}
