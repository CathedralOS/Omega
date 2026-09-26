//! Tests for scalar expression plans.
use crate::values::scalar::boolean_lowering::lower_boolean_guard;
use crate::values::scalar::scalar_lowering::retag_exact_integer_literal;
use crate::values::scalar_expression_type;
use arena::Arena;
use checked_trees::CheckedBooleanExpression;
use checked_trees::CheckedOperatorFacts;
use checked_trees::CheckedOperatorResolutionStatus;
use checked_trees::CheckedScalarExpression;
use numerics::arithmetic::ArithmeticDomain;
use numerics::literals::IntegerLanding;
use numerics::literals::LandedIntegerType;
use typed_trees::TypedTrees;
use typed_trees::expression::BinaryOperator;
use typed_trees::expression::ExpressionNode;
use typed_trees::types::PrimitiveType;
use validation::integer_widen_is_total;

#[test]
fn landed_literal_guards_fold_both_polarities_only_with_builtin_meaning() {
    for (right_value, expected) in [(64, true), (65, false)] {
        let mut program = TypedTrees::default();
        let left = program.expression_table.insert(ExpressionNode::Integer(
            numerics::literals::IntegerLiteral::from_value(64).with_landing(IntegerLanding {
                landed_type: LandedIntegerType::U32,
                domain: ArithmeticDomain::Exact,
            }),
        ));
        let right = program.expression_table.insert(ExpressionNode::Integer(
            numerics::literals::IntegerLiteral::from_value(right_value),
        ));
        let expression = program.expression_table.insert(ExpressionNode::Binary(
            typed_trees::expression::TableBinaryExpression {
                left,
                operator: BinaryOperator::Equal,
                right,
            },
        ));
        for status in [
            CheckedOperatorResolutionStatus::BuiltinFallback,
            CheckedOperatorResolutionStatus::Resolved,
            CheckedOperatorResolutionStatus::Missing,
            CheckedOperatorResolutionStatus::Ambiguous,
        ] {
            let mut uses = Arena::new();
            uses.append(checked_trees::CheckedOperatorUseFact {
                expression,
                status,
                ..Default::default()
            });
            let operators = CheckedOperatorFacts::with_roots(uses, Arena::new(), Arena::new());
            let lowered =
                lower_boolean_guard(&program, &operators, expression, &[], &[], &[], &[], &[]);
            if status == CheckedOperatorResolutionStatus::BuiltinFallback {
                assert!(
                    matches!(lowered, Some(CheckedBooleanExpression::Constant(value)) if value == expected)
                );
            } else {
                assert!(
                    lowered.is_none(),
                    "{status:?} cannot acquire builtin comparison meaning"
                );
            }
        }
    }
}

#[test]
fn boolean_guard_selection_preserves_both_polarities_and_operator_meaning() {
    for value in [false, true] {
        let mut program = TypedTrees::default();
        let left = program
            .expression_table
            .insert(ExpressionNode::Boolean(true));
        let right = program
            .expression_table
            .insert(ExpressionNode::Boolean(value));
        let expression = program.expression_table.insert(ExpressionNode::Binary(
            typed_trees::expression::TableBinaryExpression {
                left,
                operator: BinaryOperator::Equal,
                right,
            },
        ));
        for (status, accepted) in [
            (CheckedOperatorResolutionStatus::BuiltinFallback, true),
            (CheckedOperatorResolutionStatus::Resolved, false),
            (CheckedOperatorResolutionStatus::Missing, false),
            (CheckedOperatorResolutionStatus::Ambiguous, false),
        ] {
            let mut uses = Arena::new();
            uses.append(checked_trees::CheckedOperatorUseFact {
                expression,
                status,
                ..Default::default()
            });
            let operators = CheckedOperatorFacts::with_roots(uses, Arena::new(), Arena::new());
            assert_eq!(
                lower_boolean_guard(&program, &operators, expression, &[], &[], &[], &[], &[])
                    .is_some(),
                accepted,
                "value={value}, status={status:?}",
            );
        }
    }
}

#[test]
fn retained_widening_requires_complete_fixed_integer_range_containment() {
    assert!(integer_widen_is_total(
        PrimitiveType::U8,
        PrimitiveType::U64
    ));
    assert!(integer_widen_is_total(
        PrimitiveType::I8,
        PrimitiveType::I64
    ));
    assert!(integer_widen_is_total(
        PrimitiveType::U8,
        PrimitiveType::I16
    ));
    assert!(!integer_widen_is_total(
        PrimitiveType::I8,
        PrimitiveType::U16
    ));
    assert!(!integer_widen_is_total(
        PrimitiveType::U16,
        PrimitiveType::U8
    ));
    assert!(!integer_widen_is_total(
        PrimitiveType::U32,
        PrimitiveType::Addr
    ));
}

#[test]
fn compile_known_exact_integer_conversion_relands_only_representable_fixed_values() {
    let source = CheckedScalarExpression::IntegerLiteral {
        literal: numerics::literals::IntegerLiteral::from_value(127).with_landing(IntegerLanding {
            landed_type: LandedIntegerType::I64,
            domain: ArithmeticDomain::Exact,
        }),
    };
    let narrowed = retag_exact_integer_literal(&source, PrimitiveType::I8)
        .expect("127 is exactly representable as i8");
    assert_eq!(scalar_expression_type(&narrowed), Some(PrimitiveType::I8));
    assert!(retag_exact_integer_literal(&source, PrimitiveType::Addr).is_none());

    let outside = CheckedScalarExpression::IntegerLiteral {
        literal: numerics::literals::IntegerLiteral::from_value(128).with_landing(IntegerLanding {
            landed_type: LandedIntegerType::I64,
            domain: ArithmeticDomain::Exact,
        }),
    };
    assert!(retag_exact_integer_literal(&outside, PrimitiveType::I8).is_none());
}

#[test]
fn compile_known_exact_integer_conversion_lands_untyped_literals() {
    let source = CheckedScalarExpression::IntegerLiteral {
        literal: numerics::literals::IntegerLiteral::from_value(70),
    };
    let landed = retag_exact_integer_literal(&source, PrimitiveType::I32)
        .expect("70 is exactly representable as i32");
    assert_eq!(scalar_expression_type(&landed), Some(PrimitiveType::I32));
    assert!(retag_exact_integer_literal(&source, PrimitiveType::Addr).is_none());
}
