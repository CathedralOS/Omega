use std::collections::BTreeSet;

use omega_abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation, AbstractResult,
};
use omega_target::NativeTarget;
use omega_target_operations::{TargetFunction, TargetOperation, TerminalPsiProvenance};
use psi_core::{BlockId, EdgeId, MachineId, OperationId, ScalarType, ValueId};

use super::*;
use crate::{
    AbstractToTargetFunctionTranslationDisposition, AbstractToTargetFunctionTranslationReceipt,
    AbstractToTargetTranslationFamily, AbstractToTargetTranslationValidationError,
};

fn boolean_literal_pair() -> (AbstractFunction, TargetFunction) {
    let machine = MachineId::new(51_001).unwrap();
    let entry = BlockId::new(51_002).unwrap();
    let constant = ValueId::new(51_003).unwrap();
    let result = ValueId::new(51_004).unwrap();
    let constant_operation = OperationId::new(51_005).unwrap();
    let return_edge = EdgeId::new(51_006).unwrap();
    (
        AbstractFunction {
            machine,
            attachment: None,
            entry,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: AbstractFunctionResult::Scalar(AbstractResult {
                value: result,
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
                    result: constant,
                    value: true,
                },
                AbstractOperation::Return {
                    psi_edge: return_edge,
                    result,
                    value: constant,
                    scalar_type: ScalarType::Boolean,
                    cleanup_actions: Vec::new(),
                },
            ],
        },
        TargetFunction {
            machine,
            attachment: None,
            fixed_integer_scalar_abi: None,
            provenance: TerminalPsiProvenance {
                operations: vec![constant_operation],
                edges: vec![return_edge],
            },
            operation: TargetOperation::ReturnBooleanImmediate {
                psi_edge: return_edge,
                source_value: constant,
                value: true,
            },
        },
    )
}

#[test]
fn enabled_family_identities_are_unique_and_dispatch_is_typed() {
    let ordered = ENABLED_TRANSLATION_FAMILIES
        .iter()
        .map(|descriptor| descriptor.family)
        .collect::<Vec<_>>();
    assert_eq!(
        ordered,
        vec![
            AbstractToTargetTranslationFamily::StraightLineIntegerImmediate,
            AbstractToTargetTranslationFamily::StraightLineBooleanImmediate,
            AbstractToTargetTranslationFamily::StraightLineScalarCrash,
            AbstractToTargetTranslationFamily::StraightLineIntegerParameter,
            AbstractToTargetTranslationFamily::StraightLineBooleanParameter,
            AbstractToTargetTranslationFamily::StraightLineBooleanNotParameter,
            AbstractToTargetTranslationFamily::StraightLineBooleanEqualParameters,
            AbstractToTargetTranslationFamily::StraightLineIntegerEqualParameters,
            AbstractToTargetTranslationFamily::StraightLineIntegerLessThanParameters,
            AbstractToTargetTranslationFamily::StraightLineIntegerLessOrEqualParameters,
            AbstractToTargetTranslationFamily::StraightLineIntegerBitwiseNotParameter,
            AbstractToTargetTranslationFamily::StraightLineIntegerWidenParameter,
            AbstractToTargetTranslationFamily::StraightLineIntegerExactCastParameter,
            AbstractToTargetTranslationFamily::StraightLineIntegerBitwiseAndParameters,
            AbstractToTargetTranslationFamily::StraightLineIntegerBitwiseOrParameters,
            AbstractToTargetTranslationFamily::StraightLineIntegerBitwiseXorParameters,
            AbstractToTargetTranslationFamily::StraightLineExactIntegerAddParameters,
            AbstractToTargetTranslationFamily::StraightLineSaturatingIntegerAddParameters,
            AbstractToTargetTranslationFamily::StraightLineWrappingIntegerAddParameters,
            AbstractToTargetTranslationFamily::StraightLineSaturatingIntegerSubtractParameters,
            AbstractToTargetTranslationFamily::StraightLineWrappingIntegerSubtractParameters,
            AbstractToTargetTranslationFamily::StraightLineWrappingIntegerMultiplyParameters,
        ]
    );
    let identities = ordered.iter().copied().collect::<BTreeSet<_>>();
    assert_eq!(identities.len(), ENABLED_TRANSLATION_FAMILIES.len());

    let (source, target) = boolean_literal_pair();
    let disposition = validate_function(&source, NativeTarget::linux_x64(), &target).unwrap();
    assert!(matches!(
        disposition,
        AbstractToTargetFunctionTranslationDisposition::Validated(
            AbstractToTargetFunctionTranslationReceipt::StraightLineBooleanImmediate(_)
        )
    ));
}

#[test]
fn omission_is_uncovered_while_duplicate_or_overlap_fails_closed() {
    let (source, target) = boolean_literal_pair();
    let source = &source;
    let target = &target;
    assert_eq!(
        validate_function_with_catalog(source, NativeTarget::linux_x64(), target, &[]).unwrap(),
        AbstractToTargetFunctionTranslationDisposition::Uncovered
    );

    let boolean = ENABLED_TRANSLATION_FAMILIES[1];
    assert!(matches!(
        validate_function_with_catalog(
            source,
            NativeTarget::linux_x64(),
            target,
            &[boolean, boolean]
        ),
        Err(
            AbstractToTargetTranslationValidationError::AmbiguousFunctionFamily {
                first: AbstractToTargetTranslationFamily::StraightLineBooleanImmediate,
                second: AbstractToTargetTranslationFamily::StraightLineBooleanImmediate,
                ..
            }
        )
    ));

    let overlapping_alias = TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineIntegerImmediate,
        boolean.is_candidate,
        boolean.validate,
    );
    assert!(matches!(
        validate_function_with_catalog(
            source,
            NativeTarget::linux_x64(),
            target,
            &[boolean, overlapping_alias]
        ),
        Err(
            AbstractToTargetTranslationValidationError::AmbiguousFunctionFamily {
                first: AbstractToTargetTranslationFamily::StraightLineBooleanImmediate,
                second: AbstractToTargetTranslationFamily::StraightLineIntegerImmediate,
                ..
            }
        )
    ));
}
