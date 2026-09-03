//! Catalog selection canaries for the exact constant-bitwise-not immediate family.

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
                == AbstractToTargetTranslationFamily::StraightLineIntegerBitwiseNotImmediate
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
                first: AbstractToTargetTranslationFamily::StraightLineIntegerBitwiseNotImmediate,
                second: AbstractToTargetTranslationFamily::StraightLineIntegerBitwiseNotImmediate,
                ..
            }
        )
    ));
}

fn pair() -> (AbstractFunction, TargetFunction) {
    let machine = MachineId::new(67_301).unwrap();
    let entry = BlockId::new(67_302).unwrap();
    let constant_operation = OperationId::new(67_303).unwrap();
    let constant_result = ValueId::new(67_304).unwrap();
    let bitwise_not_operation = OperationId::new(67_305).unwrap();
    let bitwise_not_result = ValueId::new(67_306).unwrap();
    let edge = EdgeId::new(67_307).unwrap();
    let function_result = ValueId::new(67_308).unwrap();
    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 16).unwrap();
    let source_value = IntegerValue::Unsigned(255);
    let materialized_value = IntegerValue::Unsigned(65_280);
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
                    psi_operation: constant_operation,
                    result: constant_result,
                    scalar_type: ScalarType::Integer(scalar_type),
                    value: source_value,
                },
                AbstractOperation::IntegerBitwiseNot {
                    psi_operation: bitwise_not_operation,
                    result: bitwise_not_result,
                    scalar_type,
                    operand: constant_result,
                },
                AbstractOperation::Return {
                    psi_edge: edge,
                    result: function_result,
                    value: bitwise_not_result,
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
                operations: vec![constant_operation, bitwise_not_operation],
                edges: vec![edge],
            },
            operation: TargetOperation::ReturnIntegerImmediate {
                psi_edge: edge,
                source_value: bitwise_not_result,
                scalar_type,
                value: materialized_value,
            },
        },
    )
}
