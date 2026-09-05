//! Catalog selection canaries for the proof-bearing exact-cast immediate-operand family.

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
                == AbstractToTargetTranslationFamily::StraightLineIntegerExactCastImmediateOperand
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
                    AbstractToTargetTranslationFamily::StraightLineIntegerExactCastImmediateOperand,
                second:
                    AbstractToTargetTranslationFamily::StraightLineIntegerExactCastImmediateOperand,
                ..
            }
        )
    ));
}

fn pair() -> (AbstractFunction, TargetFunction) {
    let machine = MachineId::new(66_301).unwrap();
    let entry = BlockId::new(66_302).unwrap();
    let constant_operation = OperationId::new(66_303).unwrap();
    let constant_result = ValueId::new(66_304).unwrap();
    let cast_operation = OperationId::new(66_305).unwrap();
    let cast_result = ValueId::new(66_306).unwrap();
    let obligation = ObligationId::new(66_309).unwrap();
    let edge = EdgeId::new(66_307).unwrap();
    let function_result = ValueId::new(66_308).unwrap();
    let source_type = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let target_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let source_value = IntegerValue::Unsigned(255);
    (
        AbstractFunction {
            machine,
            attachment: None,
            entry,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: AbstractFunctionResult::Scalar(AbstractResult {
                value: function_result,
                scalar_type: ScalarType::Integer(target_type),
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
                    scalar_type: ScalarType::Integer(source_type),
                    value: source_value,
                },
                AbstractOperation::IntegerExactCast {
                    psi_operation: cast_operation,
                    obligation,
                    result: cast_result,
                    source_type,
                    target_type,
                    operand: constant_result,
                },
                AbstractOperation::Return {
                    psi_edge: edge,
                    result: function_result,
                    value: cast_result,
                    scalar_type: ScalarType::Integer(target_type),
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
                operations: vec![constant_operation, cast_operation],
                edges: vec![edge],
            },
            operation: TargetOperation::ReturnIntegerExpression {
                psi_edge: edge,
                source_value: cast_result,
                scalar_type: target_type,
                expression: target_operations::TargetIntegerExpression::IntegerExactCast {
                    psi_operation: cast_operation,
                    obligation,
                    source_type,
                    operand: Box::new(target_operations::TargetIntegerExpression::Immediate {
                        source_value: constant_result,
                        value: source_value,
                    }),
                },
            },
        },
    )
}
