//! Catalog selection canaries for the exact constant-integer-less-or-equal family.

use super::*;

#[test]
fn omission_and_duplicate_fail_closed() {
    let (source, target) = pair();
    assert_eq!(
        selection::validate(&source, NativeTarget::linux_x64(), &target, &[]).unwrap(),
        AbstractToTargetFunctionTranslationDisposition::Uncovered
    );
    let family = ENABLED_TRANSLATION_FAMILIES
        .iter()
        .find(|descriptor| {
            descriptor.family
                == AbstractToTargetTranslationFamily::StraightLineIntegerLessOrEqualImmediate
        })
        .copied()
        .unwrap();
    assert!(matches!(
        selection::validate(
            &source,
            NativeTarget::linux_x64(),
            &target,
            &[family, family]
        ),
        Err(
            AbstractToTargetTranslationValidationError::AmbiguousFunctionFamily {
                first: AbstractToTargetTranslationFamily::StraightLineIntegerLessOrEqualImmediate,
                second: AbstractToTargetTranslationFamily::StraightLineIntegerLessOrEqualImmediate,
                ..
            }
        )
    ));
}

fn pair() -> (AbstractFunction, TargetFunction) {
    let machine = MachineId::new(72_301).unwrap();
    let entry = BlockId::new(72_302).unwrap();
    let left_constant_operation = OperationId::new(72_303).unwrap();
    let left_constant_result = ValueId::new(72_304).unwrap();
    let right_constant_operation = OperationId::new(72_305).unwrap();
    let right_constant_result = ValueId::new(72_306).unwrap();
    let less_or_equal_operation = OperationId::new(72_307).unwrap();
    let less_or_equal_result = ValueId::new(72_308).unwrap();
    let edge = EdgeId::new(72_309).unwrap();
    let function_result = ValueId::new(72_310).unwrap();
    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 16).unwrap();
    (
        AbstractFunction {
            machine,
            attachment: None,
            entry,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: AbstractFunctionResult::Scalar(AbstractResult {
                value: function_result,
                scalar_type: ScalarType::Boolean,
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
                    psi_operation: left_constant_operation,
                    result: left_constant_result,
                    scalar_type: ScalarType::Integer(scalar_type),
                    value: IntegerValue::Unsigned(255),
                },
                AbstractOperation::IntegerConstant {
                    psi_operation: right_constant_operation,
                    result: right_constant_result,
                    scalar_type: ScalarType::Integer(scalar_type),
                    value: IntegerValue::Unsigned(256),
                },
                AbstractOperation::IntegerLessOrEqual {
                    psi_operation: less_or_equal_operation,
                    result: less_or_equal_result,
                    left: left_constant_result,
                    right: right_constant_result,
                },
                AbstractOperation::Return {
                    psi_edge: edge,
                    result: function_result,
                    value: less_or_equal_result,
                    scalar_type: ScalarType::Boolean,
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
                operations: vec![
                    left_constant_operation,
                    right_constant_operation,
                    less_or_equal_operation,
                ],
                edges: vec![edge],
            },
            operation: TargetOperation::ReturnBooleanImmediate {
                psi_edge: edge,
                source_value: less_or_equal_result,
                value: true,
            },
        },
    )
}
