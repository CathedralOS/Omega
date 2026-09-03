//! Catalog canaries for the exact constant wrapping-integer-shift-right family.

use super::*;

#[test]
fn omission_and_duplicate_fail_closed() {
    let (source, target) = pair();
    assert_eq!(
        selection::validate(&source, NativeTarget::linux_x64(), &target, &[]).unwrap(),
        AbstractToTargetFunctionTranslationDisposition::Uncovered
    );
    let descriptor = super::super::dispatch::immediate::WRAPPING_INTEGER_SHIFT_RIGHT;
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
    let machine = MachineId::new(83_001).unwrap();
    let entry = BlockId::new(83_002).unwrap();
    let value_operation = OperationId::new(83_003).unwrap();
    let value_result = ValueId::new(83_004).unwrap();
    let count_operation = OperationId::new(83_005).unwrap();
    let count_result = ValueId::new(83_006).unwrap();
    let shift_operation = OperationId::new(83_007).unwrap();
    let shift_result = ValueId::new(83_008).unwrap();
    let edge = EdgeId::new(83_009).unwrap();
    let function_result = ValueId::new(83_010).unwrap();
    let value_type = IntegerType::new(IntegerSign::Unsigned, 16).unwrap();
    let count_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    (
        AbstractFunction {
            machine,
            attachment: None,
            entry,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: AbstractFunctionResult::Scalar(AbstractResult {
                value: function_result,
                scalar_type: ScalarType::Integer(value_type),
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
                    psi_operation: value_operation,
                    result: value_result,
                    scalar_type: ScalarType::Integer(value_type),
                    value: IntegerValue::Unsigned(65_535),
                },
                AbstractOperation::IntegerConstant {
                    psi_operation: count_operation,
                    result: count_result,
                    scalar_type: ScalarType::Integer(count_type),
                    value: IntegerValue::Unsigned(1),
                },
                AbstractOperation::WrappingIntegerShiftRight {
                    psi_operation: shift_operation,
                    result: shift_result,
                    value_type,
                    count_type,
                    value: value_result,
                    count: count_result,
                },
                AbstractOperation::Return {
                    psi_edge: edge,
                    result: function_result,
                    value: shift_result,
                    scalar_type: ScalarType::Integer(value_type),
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
                operations: vec![value_operation, count_operation, shift_operation],
                edges: vec![edge],
            },
            operation: TargetOperation::ReturnIntegerImmediate {
                psi_edge: edge,
                source_value: shift_result,
                scalar_type: value_type,
                value: IntegerValue::Unsigned(32_767),
            },
        },
    )
}
