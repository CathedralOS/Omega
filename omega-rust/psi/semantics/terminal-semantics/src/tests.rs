//! Tests for the terminal operation semantic rows and scalar-leaf semantics.

use crate::{
    OperationSemanticCustody, OperationSemanticError, OperationSemanticRow, ScalarLeafDenotation,
    ScalarLeafFactShape, ScalarLeafLiteral, constant_goal_free_scalar_leaf,
    exact_operation_semantic_row_in, goal_free_scalar_leaf_semantics,
};
use std::collections::{BTreeMap, BTreeSet};

use crate::semantic_rows::OperationSemanticTag;
use semantic_vocabulary::IntegerValue;
use semantic_vocabulary::Proposition;
use semantic_vocabulary::ScalarTerm;
use semantic_vocabulary::{IntegerSign, IntegerType, OperationId, ScalarType, ValueId};
use terminal_psi::{Operation, OperationKind, OperationResult, ValueDeclaration};

fn i8_type() -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 8).expect("i8"))
}

#[test]
fn operation_inventory_is_exact_unique_and_closed() {
    assert_eq!(OperationSemanticTag::ALL.len(), 71);
    assert_eq!(OperationSemanticRow::ALL.len(), 71);
    assert_eq!(
        OperationSemanticRow::ALL
            .iter()
            .filter(|row| row.custody == OperationSemanticCustody::LeafDenotation)
            .count(),
        61,
    );
    assert_eq!(
        OperationSemanticRow::ALL
            .iter()
            .filter(|row| row.custody == OperationSemanticCustody::CallComposition)
            .count(),
        10,
    );
    assert_eq!(
        OperationSemanticRow::ALL
            .iter()
            .filter(|row| row.goal_free_scalar_leaf.is_some())
            .count(),
        20,
    );
    assert_eq!(
        OperationSemanticRow::ALL
            .iter()
            .map(|row| row.tag)
            .collect::<BTreeSet<_>>()
            .len(),
        71,
    );
    assert_eq!(
        OperationSemanticRow::ALL
            .iter()
            .map(|row| row.identity)
            .collect::<BTreeSet<_>>()
            .len(),
        71,
    );
    assert!(
        OperationSemanticRow::ALL
            .iter()
            .all(|row| !row.identity.is_empty()),
    );
}

#[test]
fn exact_row_lookup_rejects_missing_and_duplicate_rows() {
    let tag = OperationSemanticTag::WrappingIntegerAdd;
    let canonical = *exact_operation_semantic_row_in(tag, &OperationSemanticRow::ALL).unwrap();
    let missing = OperationSemanticRow::ALL
        .iter()
        .copied()
        .filter(|row| row.tag != tag)
        .collect::<Vec<_>>();
    assert_eq!(
        exact_operation_semantic_row_in(tag, &missing),
        Err(OperationSemanticError::MissingRow(tag)),
    );
    let mut duplicate = OperationSemanticRow::ALL.to_vec();
    duplicate.push(canonical);
    assert_eq!(
        exact_operation_semantic_row_in(tag, &duplicate),
        Err(OperationSemanticError::DuplicateRow(tag)),
    );
}

#[test]
fn goal_free_scalar_rows_emit_equations_through_one_interpreter() {
    let left = ValueId::new(1).unwrap();
    let right = ValueId::new(2).unwrap();
    let result = ValueId::new(3).unwrap();
    let operation = Operation {
        static_reach_binding: None,
        id: OperationId::new(1).unwrap(),
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: result,
            scalar_type: i8_type(),
        }),
        kind: OperationKind::WrappingIntegerAdd { left, right },
    };
    let value_types = BTreeMap::from([(left, i8_type()), (right, i8_type())]);
    let actual = goal_free_scalar_leaf_semantics(&operation, &value_types)
        .unwrap()
        .unwrap();
    let integer_type = match i8_type() {
        ScalarType::Integer(integer_type) => integer_type,
        ScalarType::Boolean | ScalarType::IeeeFloat(_) => unreachable!(),
    };
    assert_eq!(
        actual.result_equation(),
        &Proposition::Equal(
            ScalarTerm::value(result, i8_type()),
            ScalarTerm::wrapping_integer_add(
                integer_type,
                ScalarTerm::value(left, i8_type()),
                ScalarTerm::value(right, i8_type()),
            )
            .unwrap(),
        ),
    );
}

#[test]
fn nonliteral_boolean_rows_declare_polarity_fact_policy() {
    let mut polarity_rows = 0;
    for row in OperationSemanticRow::ALL {
        let Some(schema) = row.goal_free_scalar_leaf() else {
            continue;
        };
        let expected = match schema.denotation() {
            ScalarLeafDenotation::BooleanNot
            | ScalarLeafDenotation::BooleanEqual
            | ScalarLeafDenotation::IntegerEqual
            | ScalarLeafDenotation::IntegerLessThan
            | ScalarLeafDenotation::IntegerLessOrEqual => {
                polarity_rows += 1;
                ScalarLeafFactShape::BooleanResultEquationAndPolarityImplications
            }
            _ => ScalarLeafFactShape::ResultEquation,
        };
        assert_eq!(schema.fact(), expected);
    }
    assert_eq!(polarity_rows, 5);
}

#[test]
fn goal_free_scalar_rows_fail_closed_on_type_drift() {
    let left = ValueId::new(1).unwrap();
    let right = ValueId::new(2).unwrap();
    let result = ValueId::new(3).unwrap();
    let operation = Operation {
        static_reach_binding: None,
        id: OperationId::new(1).unwrap(),
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: result,
            scalar_type: i8_type(),
        }),
        kind: OperationKind::WrappingIntegerAdd { left, right },
    };
    let value_types = BTreeMap::from([(left, i8_type()), (right, ScalarType::Boolean)]);
    assert_eq!(
        goal_free_scalar_leaf_semantics(&operation, &value_types),
        Err(OperationSemanticError::OperandShapeMismatch(
            OperationSemanticTag::WrappingIntegerAdd,
        )),
    );
}

#[test]
fn constant_leaf_folds_only_when_every_operand_is_literal() {
    let left = ValueId::new(1).unwrap();
    let right = ValueId::new(2).unwrap();
    let result = ValueId::new(3).unwrap();
    let add = Operation {
        static_reach_binding: None,
        id: OperationId::new(1).unwrap(),
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: result,
            scalar_type: i8_type(),
        }),
        kind: OperationKind::WrappingIntegerAdd { left, right },
    };
    let value_types = BTreeMap::from([(left, i8_type()), (right, i8_type())]);
    // Partial knowledge is not a fold: the right operand is not yet literal.
    let partial = BTreeMap::from([(left, ScalarLeafLiteral::Integer(IntegerValue::Signed(126)))]);
    assert_eq!(
        constant_goal_free_scalar_leaf(&add, &partial, &value_types),
        None
    );
    let literals = BTreeMap::from([
        (left, ScalarLeafLiteral::Integer(IntegerValue::Signed(126))),
        (right, ScalarLeafLiteral::Integer(IntegerValue::Signed(5))),
    ]);
    // 126 + 5 wraps within signed 8 bits to -125.
    assert_eq!(
        constant_goal_free_scalar_leaf(&add, &literals, &value_types),
        Some(ScalarLeafLiteral::Integer(IntegerValue::Signed(-125))),
    );
}

#[test]
fn constant_leaf_evaluates_comparisons_and_rejects_nonleaf_rows() {
    let left = ValueId::new(1).unwrap();
    let right = ValueId::new(2).unwrap();
    let result = ValueId::new(3).unwrap();
    let less = Operation {
        static_reach_binding: None,
        id: OperationId::new(2).unwrap(),
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: result,
            scalar_type: ScalarType::Boolean,
        }),
        kind: OperationKind::IntegerLessThan { left, right },
    };
    let value_types = BTreeMap::from([(left, i8_type()), (right, i8_type())]);
    let literals = BTreeMap::from([
        (left, ScalarLeafLiteral::Integer(IntegerValue::Signed(-4))),
        (right, ScalarLeafLiteral::Integer(IntegerValue::Signed(-4))),
    ]);
    assert_eq!(
        constant_goal_free_scalar_leaf(&less, &literals, &value_types),
        Some(ScalarLeafLiteral::Boolean(false)),
    );
    // Literal rows are seeds, not candidates; non-leaf rows never fold.
    let literal = Operation {
        kind: OperationKind::IntegerConstant {
            value: IntegerValue::Signed(7),
        },
        ..less.clone()
    };
    assert_eq!(
        constant_goal_free_scalar_leaf(&literal, &literals, &value_types),
        None
    );
    let read = Operation {
        kind: OperationKind::PrimitiveScalarRead {
            source: semantic_vocabulary::PlaceId::new(1).unwrap(),
            path: Vec::new(),
        },
        ..less
    };
    assert_eq!(
        constant_goal_free_scalar_leaf(&read, &literals, &value_types),
        None
    );
}
