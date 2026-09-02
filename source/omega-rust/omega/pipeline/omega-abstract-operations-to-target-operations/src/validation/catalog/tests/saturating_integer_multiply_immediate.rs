//! Catalog canaries for the exact constant saturating-integer-multiply family.

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
                == AbstractToTargetTranslationFamily::StraightLineSaturatingIntegerMultiplyImmediate
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
                first: AbstractToTargetTranslationFamily::StraightLineSaturatingIntegerMultiplyImmediate,
                second:
                    AbstractToTargetTranslationFamily::StraightLineSaturatingIntegerMultiplyImmediate,
                ..
            }
        )
    ));
}

fn pair() -> (AbstractFunction, TargetFunction) {
    let machine = MachineId::new(82_301).unwrap();
    let entry = BlockId::new(82_302).unwrap();
    let left_operation = OperationId::new(82_303).unwrap();
    let left_result = ValueId::new(82_304).unwrap();
    let right_operation = OperationId::new(82_305).unwrap();
    let right_result = ValueId::new(82_306).unwrap();
    let multiply_operation = OperationId::new(82_307).unwrap();
    let multiply_result = ValueId::new(82_308).unwrap();
    let edge = EdgeId::new(82_309).unwrap();
    let function_result = ValueId::new(82_310).unwrap();
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
                    psi_operation: left_operation,
                    result: left_result,
                    scalar_type: ScalarType::Integer(scalar_type),
                    value: IntegerValue::Unsigned(65_535),
                },
                AbstractOperation::IntegerConstant {
                    psi_operation: right_operation,
                    result: right_result,
                    scalar_type: ScalarType::Integer(scalar_type),
                    value: IntegerValue::Unsigned(2),
                },
                AbstractOperation::SaturatingIntegerMultiply {
                    psi_operation: multiply_operation,
                    result: multiply_result,
                    scalar_type,
                    left: left_result,
                    right: right_result,
                },
                AbstractOperation::Return {
                    psi_edge: edge,
                    result: function_result,
                    value: multiply_result,
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
                operations: vec![left_operation, right_operation, multiply_operation],
                edges: vec![edge],
            },
            operation: TargetOperation::ReturnIntegerImmediate {
                psi_edge: edge,
                source_value: multiply_result,
                scalar_type,
                value: IntegerValue::Unsigned(65_535),
            },
        },
    )
}
