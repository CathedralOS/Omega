//! Catalog selection canaries for the exact constant-Boolean-not immediate family.

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
            descriptor.family == AbstractToTargetTranslationFamily::StraightLineBooleanNotImmediate
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
                first: AbstractToTargetTranslationFamily::StraightLineBooleanNotImmediate,
                second: AbstractToTargetTranslationFamily::StraightLineBooleanNotImmediate,
                ..
            }
        )
    ));
}

fn pair() -> (AbstractFunction, TargetFunction) {
    let machine = MachineId::new(68_301).unwrap();
    let entry = BlockId::new(68_302).unwrap();
    let constant_operation = OperationId::new(68_303).unwrap();
    let constant_result = ValueId::new(68_304).unwrap();
    let boolean_not_operation = OperationId::new(68_305).unwrap();
    let boolean_not_result = ValueId::new(68_306).unwrap();
    let edge = EdgeId::new(68_307).unwrap();
    let function_result = ValueId::new(68_308).unwrap();
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
                    psi_operation: constant_operation,
                    result: constant_result,
                    value: true,
                },
                AbstractOperation::BooleanNot {
                    psi_operation: boolean_not_operation,
                    result: boolean_not_result,
                    operand: constant_result,
                },
                AbstractOperation::Return {
                    psi_edge: edge,
                    result: function_result,
                    value: boolean_not_result,
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
                operations: vec![constant_operation, boolean_not_operation],
                edges: vec![edge],
            },
            operation: TargetOperation::ReturnBooleanImmediate {
                psi_edge: edge,
                source_value: boolean_not_result,
                value: false,
            },
        },
    )
}
