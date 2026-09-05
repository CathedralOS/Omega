//! Catalog canaries for proof-bearing saturating divide over constant operands.

use super::*;
use target_operations::TargetIntegerExpression;

#[test]
fn omission_and_duplicate_fail_closed() {
    let (source, target) = pair();
    assert_eq!(
        selection::validate(&source, NativeTarget::linux_x64(), &target, &[]).unwrap(),
        AbstractToTargetFunctionTranslationDisposition::Uncovered
    );
    let descriptor = super::super::dispatch::immediate::SATURATING_INTEGER_DIVIDE_OPERANDS;
    assert!(matches!(
        selection::validate(
            &source,
            NativeTarget::linux_x64(),
            &target,
            &[descriptor, descriptor],
        ),
        Err(AbstractToTargetTranslationValidationError::AmbiguousFunctionFamily { .. })
    ));
}

fn pair() -> (AbstractFunction, TargetFunction) {
    let machine = MachineId::new(85_001).unwrap();
    let entry = BlockId::new(85_002).unwrap();
    let left_operation = OperationId::new(85_003).unwrap();
    let left_result = ValueId::new(85_004).unwrap();
    let right_operation = OperationId::new(85_005).unwrap();
    let right_result = ValueId::new(85_006).unwrap();
    let divide_operation = OperationId::new(85_007).unwrap();
    let divide_result = ValueId::new(85_008).unwrap();
    let edge = EdgeId::new(85_009).unwrap();
    let function_result = ValueId::new(85_010).unwrap();
    let obligation = ObligationId::new(85_011).unwrap();
    let scalar_type = IntegerType::new(IntegerSign::Signed, 16).unwrap();
    let left_value = IntegerValue::Signed(-32_768);
    let right_value = IntegerValue::Signed(-1);
    (
        AbstractFunction {
            machine,
            attachment: None,
            entry,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: AbstractFunctionResult::Scalar(AbstractResult {
                value: function_result,
                scalar_type: ScalarType::Integer(scalar_type),
            }),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: vec![AbstractBlockEntry {
                block: entry,
                parameters: Vec::new(),
                operation_offset: 0,
            }],
            operations: vec![
                AbstractOperation::IntegerConstant {
                    psi_operation: left_operation,
                    result: left_result,
                    scalar_type: ScalarType::Integer(scalar_type),
                    value: left_value,
                },
                AbstractOperation::IntegerConstant {
                    psi_operation: right_operation,
                    result: right_result,
                    scalar_type: ScalarType::Integer(scalar_type),
                    value: right_value,
                },
                AbstractOperation::SaturatingIntegerDivide {
                    psi_operation: divide_operation,
                    obligation,
                    result: divide_result,
                    scalar_type,
                    left: left_result,
                    right: right_result,
                },
                AbstractOperation::Return {
                    psi_edge: edge,
                    result: function_result,
                    value: divide_result,
                    scalar_type: ScalarType::Integer(scalar_type),
                    cleanup_actions: Vec::new(),
                },
            ],
        },
        TargetFunction {
            machine,
            attachment: None,
            fixed_integer_scalar_abi: None,
            mixed_structural_scalar_abi: None,
            provenance: TerminalPsiProvenance {
                operations: vec![left_operation, right_operation, divide_operation],
                edges: vec![edge],
            },
            operation: TargetOperation::ReturnIntegerExpression {
                psi_edge: edge,
                source_value: divide_result,
                scalar_type,
                expression: TargetIntegerExpression::SaturatingDivide {
                    psi_operation: divide_operation,
                    obligation,
                    left: Box::new(TargetIntegerExpression::Immediate {
                        source_value: left_result,
                        value: left_value,
                    }),
                    right: Box::new(TargetIntegerExpression::Immediate {
                        source_value: right_result,
                        value: right_value,
                    }),
                },
            },
        },
    )
}
