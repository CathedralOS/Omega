//! Catalog selection canaries for the exact constant-widen immediate family.

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
                == AbstractToTargetTranslationFamily::StraightLineIntegerWidenImmediate
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
                first: AbstractToTargetTranslationFamily::StraightLineIntegerWidenImmediate,
                second: AbstractToTargetTranslationFamily::StraightLineIntegerWidenImmediate,
                ..
            }
        )
    ));
}

fn pair() -> (AbstractFunction, TargetFunction) {
    let machine = MachineId::new(64_301).unwrap();
    let entry = BlockId::new(64_302).unwrap();
    let constant_operation = OperationId::new(64_303).unwrap();
    let constant_result = ValueId::new(64_304).unwrap();
    let widen_operation = OperationId::new(64_305).unwrap();
    let widened_result = ValueId::new(64_306).unwrap();
    let edge = EdgeId::new(64_307).unwrap();
    let function_result = ValueId::new(64_308).unwrap();
    let source_type = IntegerType::new(IntegerSign::Unsigned, 16).unwrap();
    let target_type = IntegerType::new(IntegerSign::Signed, 64).unwrap();
    let source_value = IntegerValue::Unsigned(65_535);
    let materialized_value = IntegerValue::Signed(65_535);
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
                AbstractOperation::IntegerWiden {
                    psi_operation: widen_operation,
                    result: widened_result,
                    source_type,
                    target_type,
                    operand: constant_result,
                },
                AbstractOperation::Return {
                    psi_edge: edge,
                    result: function_result,
                    value: widened_result,
                    scalar_type: ScalarType::Integer(target_type),
                    cleanup_actions: Vec::new(),
                },
            ],
        },
        TargetFunction {
            machine,
            attachment: None,
            fixed_integer_scalar_abi: None,
            provenance: TerminalPsiProvenance {
                operations: vec![constant_operation, widen_operation],
                edges: vec![edge],
            },
            operation: TargetOperation::ReturnIntegerImmediate {
                psi_edge: edge,
                source_value: widened_result,
                scalar_type: target_type,
                value: materialized_value,
            },
        },
    )
}
