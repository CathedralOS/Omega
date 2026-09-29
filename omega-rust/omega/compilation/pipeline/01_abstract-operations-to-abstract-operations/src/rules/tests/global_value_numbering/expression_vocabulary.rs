//! Exact proof-certified scalar expression vocabulary.

use crate::rules::global_value_numbering::proof_certified_scalar_expression;
use crate::rules::tests::fixtures::global_value_numbering::proof_certified_local_cse_unit;
use abstract_operations::{
    AbstractOperation as O, IntegerArithmetic, IntegerArithmeticOperator, IntegerOverflowPolicy,
    IntegerShiftDirection, IntegerShiftPolicy,
};

#[test]
fn proof_certified_cse_expression_vocabulary_is_closed_and_exact() {
    let seed = proof_certified_local_cse_unit();
    let O::IntegerArithmetic {
        psi_operation,
        result,
        scalar_type,
        left,
        right,
        arithmetic,
        ..
    } = seed.functions[0].blocks[0].nodes[0].operation
    else {
        unreachable!()
    };
    assert!(arithmetic.is(IntegerOverflowPolicy::Exact, IntegerArithmeticOperator::Add));
    let obligation = arithmetic
        .obligation()
        .expect("the pair carries its obligation");
    let arithmetic = |policy, operator| O::IntegerArithmetic {
        psi_operation,
        result,
        scalar_type,
        arithmetic: IntegerArithmetic::new(policy, operator, Some(obligation))
            .expect("admitted integer arithmetic"),
        left,
        right,
    };
    let exact_shift = |direction| O::IntegerShift {
        psi_operation,
        result,
        value_type: scalar_type,
        count_type: scalar_type,
        policy: IntegerShiftPolicy::Exact { obligation },
        direction,
        value: left,
        count: right,
    };
    let operations = [
        O::IntegerExactCast {
            psi_operation,
            obligation,
            result,
            source_type: scalar_type,
            target_type: scalar_type,
            operand: left,
        },
        exact_shift(IntegerShiftDirection::Left),
        exact_shift(IntegerShiftDirection::Right),
        arithmetic(IntegerOverflowPolicy::Exact, IntegerArithmeticOperator::Add),
        arithmetic(
            IntegerOverflowPolicy::Exact,
            IntegerArithmeticOperator::Subtract,
        ),
        arithmetic(
            IntegerOverflowPolicy::Exact,
            IntegerArithmeticOperator::Multiply,
        ),
        arithmetic(
            IntegerOverflowPolicy::Exact,
            IntegerArithmeticOperator::Divide,
        ),
        arithmetic(
            IntegerOverflowPolicy::Exact,
            IntegerArithmeticOperator::Remainder,
        ),
        arithmetic(
            IntegerOverflowPolicy::Wrapping,
            IntegerArithmeticOperator::Divide,
        ),
        arithmetic(
            IntegerOverflowPolicy::Wrapping,
            IntegerArithmeticOperator::Remainder,
        ),
        arithmetic(
            IntegerOverflowPolicy::Saturating,
            IntegerArithmeticOperator::Divide,
        ),
        arithmetic(
            IntegerOverflowPolicy::Saturating,
            IntegerArithmeticOperator::Remainder,
        ),
    ];
    for operation in &operations {
        assert!(
            proof_certified_scalar_expression(operation).is_some(),
            "closed proof-bearing shape must have an expression key: {operation:?}"
        );
    }
    assert!(
        proof_certified_scalar_expression(&O::IntegerArithmetic {
            psi_operation,
            result,
            scalar_type,
            arithmetic: IntegerArithmetic::new(
                IntegerOverflowPolicy::Wrapping,
                IntegerArithmeticOperator::Add,
                None
            )
            .expect("admitted integer arithmetic"),
            left,
            right
        })
        .is_none()
    );

    let exact_add = proof_certified_scalar_expression(&operations[3]).unwrap().0;
    let swapped_add = proof_certified_scalar_expression(&O::IntegerArithmetic {
        psi_operation,
        result,
        scalar_type,
        arithmetic: IntegerArithmetic::new(
            IntegerOverflowPolicy::Exact,
            IntegerArithmeticOperator::Add,
            Some(obligation),
        )
        .expect("admitted integer arithmetic"),
        left: right,
        right: left,
    })
    .unwrap()
    .0;
    assert_eq!(exact_add, swapped_add);
    let subtract = proof_certified_scalar_expression(&operations[4]).unwrap().0;
    let swapped_subtract = proof_certified_scalar_expression(&O::IntegerArithmetic {
        psi_operation,
        result,
        scalar_type,
        arithmetic: IntegerArithmetic::new(
            IntegerOverflowPolicy::Exact,
            IntegerArithmeticOperator::Subtract,
            Some(obligation),
        )
        .expect("admitted integer arithmetic"),
        left: right,
        right: left,
    })
    .unwrap()
    .0;
    assert_ne!(subtract, swapped_subtract);
}
