//! Catalog selection canaries for the exact constant-integer-wrapping integer multiplication family.

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
                == AbstractToTargetTranslationFamily::StraightLineWrappingIntegerMultiplyImmediate
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
                first:
                    AbstractToTargetTranslationFamily::StraightLineWrappingIntegerMultiplyImmediate,
                second:
                    AbstractToTargetTranslationFamily::StraightLineWrappingIntegerMultiplyImmediate,
                ..
            }
        )
    ));
}

fn pair() -> (AbstractFunction, TargetFunction) {
    let machine = MachineId::new(73_301).unwrap();
    let entry = BlockId::new(73_302).unwrap();
    let left_constant_operation = OperationId::new(73_303).unwrap();
    let left_constant_result = ValueId::new(73_304).unwrap();
    let right_constant_operation = OperationId::new(73_305).unwrap();
    let right_constant_result = ValueId::new(73_306).unwrap();
    let wrapping_multiply_operation = OperationId::new(73_307).unwrap();
    let wrapping_multiply_result = ValueId::new(73_308).unwrap();
    let edge = EdgeId::new(73_309).unwrap();
    let function_result = ValueId::new(73_310).unwrap();
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
                    psi_operation: left_constant_operation,
                    result: left_constant_result,
                    scalar_type: ScalarType::Integer(scalar_type),
                    value: IntegerValue::Unsigned(0x55),
                },
                AbstractOperation::IntegerConstant {
                    psi_operation: right_constant_operation,
                    result: right_constant_result,
                    scalar_type: ScalarType::Integer(scalar_type),
                    value: IntegerValue::Unsigned(0x0f),
                },
                AbstractOperation::WrappingIntegerMultiply {
                    psi_operation: wrapping_multiply_operation,
                    result: wrapping_multiply_result,
                    scalar_type,
                    left: left_constant_result,
                    right: right_constant_result,
                },
                AbstractOperation::Return {
                    psi_edge: edge,
                    result: function_result,
                    value: wrapping_multiply_result,
                    scalar_type: ScalarType::Integer(scalar_type),
                    cleanup_actions: Vec::new(),
                },
            ],
        },
        TargetFunction {
            machine,
            attachment: None,
            fixed_integer_scalar_abi: None,
            provenance: TerminalPsiProvenance {
                operations: vec![
                    left_constant_operation,
                    right_constant_operation,
                    wrapping_multiply_operation,
                ],
                edges: vec![edge],
            },
            operation: TargetOperation::ReturnIntegerImmediate {
                psi_edge: edge,
                source_value: wrapping_multiply_result,
                scalar_type,
                value: IntegerValue::Unsigned(0x4fb),
            },
        },
    )
}
