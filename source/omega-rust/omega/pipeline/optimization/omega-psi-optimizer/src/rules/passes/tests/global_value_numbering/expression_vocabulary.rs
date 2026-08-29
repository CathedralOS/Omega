//! Exact proof-certified scalar expression vocabulary.

use super::*;

#[test]
fn proof_certified_cse_expression_vocabulary_is_closed_and_exact() {
    let seed = proof_certified_local_cse_unit();
    let O::ExactIntegerAdd {
        psi_operation,
        obligation,
        result,
        scalar_type,
        left,
        right,
    } = seed.functions[0].blocks[0].nodes[0].operation
    else {
        unreachable!()
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
        O::ExactIntegerShiftLeft {
            psi_operation,
            obligation,
            result,
            value_type: scalar_type,
            count_type: scalar_type,
            value: left,
            count: right,
        },
        O::ExactIntegerShiftRight {
            psi_operation,
            obligation,
            result,
            value_type: scalar_type,
            count_type: scalar_type,
            value: left,
            count: right,
        },
        O::ExactIntegerAdd {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        },
        O::ExactIntegerSubtract {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        },
        O::ExactIntegerMultiply {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        },
        O::ExactIntegerDivide {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        },
        O::ExactIntegerRemainder {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        },
        O::WrappingIntegerDivide {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        },
        O::WrappingIntegerRemainder {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        },
        O::SaturatingIntegerDivide {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        },
        O::SaturatingIntegerRemainder {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        },
    ];
    for operation in &operations {
        assert!(
            proof_certified_scalar_expression(operation).is_some(),
            "closed proof-bearing shape must have an expression key: {operation:?}"
        );
    }
    assert!(
        proof_certified_scalar_expression(&O::WrappingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        })
        .is_none()
    );

    let exact_add = proof_certified_scalar_expression(&operations[3]).unwrap().0;
    let swapped_add = proof_certified_scalar_expression(&O::ExactIntegerAdd {
        psi_operation,
        obligation,
        result,
        scalar_type,
        left: right,
        right: left,
    })
    .unwrap()
    .0;
    assert_eq!(exact_add, swapped_add);
    let subtract = proof_certified_scalar_expression(&operations[4]).unwrap().0;
    let swapped_subtract = proof_certified_scalar_expression(&O::ExactIntegerSubtract {
        psi_operation,
        obligation,
        result,
        scalar_type,
        left: right,
        right: left,
    })
    .unwrap()
    .0;
    assert_ne!(subtract, swapped_subtract);
}
