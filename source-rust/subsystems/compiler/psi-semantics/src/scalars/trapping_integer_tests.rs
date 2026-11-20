use crate::*;
use arena::IdTable;
use semantic_vocabulary::{
    IntegerSign, IntegerType, ObligationId, OperationId, ScalarType, ValueId,
};
use terminal_psi::{
    Operation, OperationKind, OperationResult, TrappingIntegerOperation as Trap,
    ValueDeclaration,
};

#[test]
fn trapping_predicates_and_results_share_exact_primitive_semantics() {
    let left = ValueId::new(1).unwrap();
    let right = ValueId::new(2).unwrap();
    let obligation = ObligationId::new(1).unwrap();
    for sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
        // Exercise the closed carriers the current Terminal representation
        // admits, including a non-roster width and its widest carrier.
        for width in [1, 13, 128] {
            let scalar = ScalarType::Integer(IntegerType::new(sign, width).unwrap());
            let mut types = IdTable::default();
            assert_eq!(types.push(scalar), left);
            assert_eq!(types.push(scalar), right);
            for (trapping, exact) in [
                (
                    Trap::Add { left, right },
                    OperationKind::ExactIntegerAdd {
                        left,
                        right,
                        obligation,
                    },
                ),
                (
                    Trap::Subtract { left, right },
                    OperationKind::ExactIntegerSubtract {
                        left,
                        right,
                        obligation,
                    },
                ),
                (
                    Trap::Multiply { left, right },
                    OperationKind::ExactIntegerMultiply {
                        left,
                        right,
                        obligation,
                    },
                ),
                (
                    Trap::Divide { left, right },
                    OperationKind::ExactIntegerDivide {
                        left,
                        right,
                        obligation,
                    },
                ),
                (
                    Trap::Remainder { left, right },
                    OperationKind::ExactIntegerRemainder {
                        left,
                        right,
                        obligation,
                    },
                ),
                (
                    Trap::ShiftLeft {
                        value: left,
                        count: right,
                    },
                    OperationKind::ExactIntegerShiftLeft {
                        value: left,
                        count: right,
                        obligation,
                    },
                ),
                (
                    Trap::ShiftRight {
                        value: left,
                        count: right,
                    },
                    OperationKind::ExactIntegerShiftRight {
                        value: left,
                        count: right,
                        obligation,
                    },
                ),
                (
                    Trap::Convert { operand: left },
                    OperationKind::IntegerExactCast {
                        operand: left,
                        obligation,
                    },
                ),
            ] {
                let mut operation = Operation::unsuspended(
                    OperationId::new(1).unwrap(),
                    OperationResult::Scalar(ValueDeclaration {
                        id: ValueId::new(3).unwrap(),
                        scalar_type: scalar,
                        qualifications: Default::default(),
                    }),
                    exact,
                );
                let exact = proof_bearing_scalar_leaf_semantics(&operation, &types)
                    .unwrap()
                    .unwrap();
                operation.kind = OperationKind::TrappingInteger {
                    operation: trapping,
                };
                let trapped = trapping_integer_semantics(&operation, &types)
                    .unwrap()
                    .unwrap();
                assert_eq!(trapped.completes(), exact.canonical_goal());
                assert_eq!(trapped.result_equation(), exact.result_equation());
                // A foreign or unresolved input is never treated as a safe
                // operation, even when its result type is well formed.
                assert!(
                    trapping_integer_semantics(
                        &operation,
                        &IdTable::<ValueId, ScalarType>::default()
                    )
                    .is_err()
                );
                operation.result = OperationResult::Unit;
                assert!(trapping_integer_semantics(&operation, &types).is_err());
            }
        }
    }
}
