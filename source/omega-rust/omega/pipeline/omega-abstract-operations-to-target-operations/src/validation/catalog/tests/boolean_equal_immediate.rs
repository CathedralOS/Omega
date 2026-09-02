//! Catalog selection canaries for the exact constant-Boolean-equality family.

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
                == AbstractToTargetTranslationFamily::StraightLineBooleanEqualImmediate
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
                first: AbstractToTargetTranslationFamily::StraightLineBooleanEqualImmediate,
                second: AbstractToTargetTranslationFamily::StraightLineBooleanEqualImmediate,
                ..
            }
        )
    ));
}

fn pair() -> (AbstractFunction, TargetFunction) {
    let machine = MachineId::new(69_301).unwrap();
    let entry = BlockId::new(69_302).unwrap();
    let left_constant_operation = OperationId::new(69_303).unwrap();
    let left_constant_result = ValueId::new(69_304).unwrap();
    let right_constant_operation = OperationId::new(69_305).unwrap();
    let right_constant_result = ValueId::new(69_306).unwrap();
    let equal_operation = OperationId::new(69_307).unwrap();
    let equal_result = ValueId::new(69_308).unwrap();
    let edge = EdgeId::new(69_309).unwrap();
    let function_result = ValueId::new(69_310).unwrap();
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
                AbstractOperation::BooleanConstant {
                    psi_operation: left_constant_operation,
                    result: left_constant_result,
                    value: true,
                },
                AbstractOperation::BooleanConstant {
                    psi_operation: right_constant_operation,
                    result: right_constant_result,
                    value: false,
                },
                AbstractOperation::BooleanEqual {
                    psi_operation: equal_operation,
                    result: equal_result,
                    left: left_constant_result,
                    right: right_constant_result,
                },
                AbstractOperation::Return {
                    psi_edge: edge,
                    result: function_result,
                    value: equal_result,
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
                    equal_operation,
                ],
                edges: vec![edge],
            },
            operation: TargetOperation::ReturnBooleanImmediate {
                psi_edge: edge,
                source_value: equal_result,
                value: false,
            },
        },
    )
}
