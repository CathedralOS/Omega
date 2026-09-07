//! Widened exact-subtract authored order, theorem temporaries, and exact custody.

use crate::tests::*;

#[test]
fn widened_u8_exact_subtract_legalization_preserves_authored_order_and_exact_custody() {
    let u8_integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let u64_integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let expected_function_operations = vec![
        OperationId::new(5_121).unwrap(),
        OperationId::new(5_122).unwrap(),
        OperationId::new(5_123).unwrap(),
        OperationId::new(5_124).unwrap(),
        OperationId::new(5_125).unwrap(),
        OperationId::new(5_126).unwrap(),
        OperationId::new(5_127).unwrap(),
        OperationId::new(5_128).unwrap(),
    ];
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let staged = staged_widened_u8_exact_subtract_conditional(target);
        let target_function = &staged.optimized_target().target_operations().functions[0];
        let TargetOperation::ReturnIntegerConditionalControl {
            scalar_type,
            when_true,
            when_false,
            ..
        } = &target_function.operation
        else {
            panic!("fixture must lower to bounded integer conditional control")
        };
        assert_eq!(*scalar_type, u64_integer);
        assert_eq!(
            target_function.provenance.operations,
            expected_function_operations
        );
        for (
            arm,
            expected_wide,
            expected_widen_operation,
            expected_subtract_operation,
            expected_obligation,
            expected_left,
            expected_left_value,
            expected_right,
            expected_right_value,
        ) in [
            (
                when_true,
                ValueId::new(5_109).unwrap(),
                OperationId::new(5_124).unwrap(),
                OperationId::new(5_123).unwrap(),
                ObligationId::new(5_131).unwrap(),
                ValueId::new(5_106).unwrap(),
                IntegerValue::Unsigned(255),
                ValueId::new(5_107).unwrap(),
                IntegerValue::Unsigned(0),
            ),
            (
                when_false,
                ValueId::new(5_113).unwrap(),
                OperationId::new(5_128).unwrap(),
                OperationId::new(5_127).unwrap(),
                ObligationId::new(5_132).unwrap(),
                ValueId::new(5_110).unwrap(),
                IntegerValue::Unsigned(200),
                ValueId::new(5_111).unwrap(),
                IntegerValue::Unsigned(55),
            ),
        ] {
            let TargetIntegerControl::Return {
                source_value,
                expression,
                ..
            } = arm.control.as_ref()
            else {
                panic!("conditional arm must return its widened value")
            };
            assert_eq!(*source_value, expected_wide);
            let TargetIntegerExpression::IntegerWiden {
                psi_operation: widen_operation,
                source_type,
                operand,
            } = expression
            else {
                panic!("exact u8 subtraction must remain nested under its widening")
            };
            assert_eq!(*widen_operation, expected_widen_operation);
            assert_eq!(*source_type, u8_integer);
            let TargetIntegerExpression::ExactSubtract {
                psi_operation: subtract_operation,
                obligation,
                left,
                right,
            } = operand.as_ref()
            else {
                panic!("proof-bearing exact subtraction must remain explicit")
            };
            assert_eq!(*subtract_operation, expected_subtract_operation);
            assert_eq!(*obligation, expected_obligation);
            assert_eq!(
                left.as_ref(),
                &TargetIntegerExpression::Immediate {
                    source_value: expected_left,
                    value: expected_left_value,
                }
            );
            assert_eq!(
                right.as_ref(),
                &TargetIntegerExpression::Immediate {
                    source_value: expected_right,
                    value: expected_right_value,
                }
            );
        }

        let function = &staged.legalized().plan().scalar_functions[0];
        assert_eq!(function.provenance.operations, expected_function_operations);
        let nodes = function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .collect::<Vec<_>>();
        assert_eq!(
            nodes
                .iter()
                .filter(|row| matches!(
                    row.kind,
                    legalized_operations::LegalizedScalarInstructionKind::IntegerWiden { .. }
                ))
                .count(),
            2
        );
        for row in nodes {
            if let legalized_operations::LegalizedScalarInstructionKind::IntegerWiden {
                source_type,
                ..
            } = row.kind
            {
                assert_eq!(source_type, u8_integer);
                assert_eq!(
                    row.result.as_ref().unwrap().scalar_type,
                    semantic_vocabulary::ScalarType::Integer(u64_integer)
                );
            }
        }
        assert_ordinary_graph_custody(&staged);
    }
}
