use omega_abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation, AbstractResult,
};
use omega_calling_conventions::{CallSignature, CallingPolicy, evaluate_call_plan};
use omega_target::NativeTarget;
use omega_target_operations::{
    TargetFunction, TargetOperation, TargetUnitBody, TargetUnitOperation, TerminalPsiProvenance,
};
use psi_core::{
    BlockId, EdgeId, IeeeFloatValue, IntegerSign, IntegerType, IntegerValue, MachineId,
    ObligationId, OperationId, PlaceId, ScalarType, StructuralPlaceKind, StructuralTypeId, ValueId,
};
use psi_terminal::{
    ByteSequenceCarrier, StructuralPlaceDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalAffineCleanupAction,
};

use super::model::TranslationFamilyValidator;
use super::*;
use crate::{
    AbstractToTargetFunctionTranslationDisposition, AbstractToTargetFunctionTranslationReceipt,
    AbstractToTargetTranslationFamily, AbstractToTargetTranslationValidationError,
};

mod boolean_equal_immediate;
mod boolean_not_immediate;
mod enabled_families;
mod integer_bitwise_not_immediate;
mod integer_exact_cast_immediate_operand;
mod integer_ieee_float_literal_sequence;
mod integer_widen_immediate;

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

fn unit_call_pair() -> (AbstractFunction, TargetFunction) {
    let machine = MachineId::new(52_001).unwrap();
    let entry = BlockId::new(52_002).unwrap();
    let callee = MachineId::new(52_003).unwrap();
    let call_operation = OperationId::new(52_004).unwrap();
    let return_edge = EdgeId::new(52_005).unwrap();
    (
        AbstractFunction {
            machine,
            attachment: None,
            entry,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: AbstractFunctionResult::Unit,
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: vec![AbstractBlockEntry {
                block: entry,
                parameters: Vec::new(),
                operation_offset: 0,
            }],
            operations: vec![
                AbstractOperation::CallUnit {
                    psi_operation: call_operation,
                    callee,
                    structural_arguments: Vec::new(),
                    claim_transfers: Vec::new(),
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                },
                AbstractOperation::ReturnUnit {
                    psi_edge: return_edge,
                    cleanup_actions: Vec::new(),
                },
            ],
        },
        TargetFunction {
            machine,
            attachment: None,
            fixed_integer_scalar_abi: None,
            provenance: TerminalPsiProvenance {
                operations: vec![call_operation],
                edges: vec![return_edge],
            },
            operation: TargetOperation::UnitBody(TargetUnitBody {
                structural_types: Vec::new(),
                call_plan: evaluate_call_plan(
                    CallingPolicy::native_for_target(NativeTarget::linux_x64()),
                    &CallSignature::default(),
                )
                .unwrap(),
                parameters: Vec::new(),
                operations: vec![
                    TargetUnitOperation::Call {
                        psi_operation: call_operation,
                        callee,
                        arguments: Vec::new(),
                        claim_transfers: Vec::new(),
                        requirement_obligations: Vec::new(),
                        crash_continuations: Vec::new(),
                    },
                    TargetUnitOperation::Return {
                        psi_edge: return_edge,
                        cleanup_actions: Vec::new(),
                    },
                ],
            }),
        },
    )
}

fn byte_sequence_literal_pair() -> (AbstractFunction, TargetFunction) {
    let machine = MachineId::new(54_001).unwrap();
    let entry = BlockId::new(54_002).unwrap();
    let structural_type = StructuralTypeDeclaration {
        id: StructuralTypeId::new(54_003).unwrap(),
        identity: "BorrowedBytes".into(),
        shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView),
    };
    let place = StructuralPlaceDeclaration {
        id: PlaceId::new(54_004).unwrap(),
        kind: StructuralPlaceKind::ByteSequenceLiteral {
            declaration_ordinal: 0,
            structural_type: structural_type.id,
        },
    };
    let operation = OperationId::new(54_005).unwrap();
    let edge = EdgeId::new(54_006).unwrap();
    let bytes = vec![0x4f, 0x6d, 0x65, 0x67, 0x61];
    (
        AbstractFunction {
            machine,
            attachment: None,
            entry,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: AbstractFunctionResult::Unit,
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: vec![AbstractBlockEntry {
                block: entry,
                parameters: Vec::new(),
                operation_offset: 0,
            }],
            operations: vec![
                AbstractOperation::EstablishByteSequenceLiteral {
                    psi_operation: operation,
                    place: place.clone(),
                    structural_type: structural_type.clone(),
                    bytes: bytes.clone(),
                },
                AbstractOperation::ReturnUnit {
                    psi_edge: edge,
                    cleanup_actions: Vec::new(),
                },
            ],
        },
        TargetFunction {
            machine,
            attachment: None,
            fixed_integer_scalar_abi: None,
            provenance: TerminalPsiProvenance {
                operations: vec![operation],
                edges: vec![edge],
            },
            operation: TargetOperation::UnitBody(TargetUnitBody {
                structural_types: vec![structural_type.clone()],
                call_plan: evaluate_call_plan(
                    CallingPolicy::native_for_target(NativeTarget::linux_x64()),
                    &CallSignature::default(),
                )
                .unwrap(),
                parameters: Vec::new(),
                operations: vec![
                    TargetUnitOperation::EstablishByteSequenceLiteral {
                        psi_operation: operation,
                        place,
                        structural_type,
                        bytes,
                    },
                    TargetUnitOperation::Return {
                        psi_edge: edge,
                        cleanup_actions: Vec::new(),
                    },
                ],
            }),
        },
    )
}

fn integer_literal_unit_return_pair() -> (AbstractFunction, TargetFunction) {
    let machine = MachineId::new(58_001).unwrap();
    let entry = BlockId::new(58_002).unwrap();
    let operation = OperationId::new(58_003).unwrap();
    let value_id = ValueId::new(58_004).unwrap();
    let edge = EdgeId::new(58_005).unwrap();
    let scalar_type = IntegerType::new(IntegerSign::Signed, 37).unwrap();
    let value = IntegerValue::Signed(-4_000_003);
    (
        AbstractFunction {
            machine,
            attachment: None,
            entry,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: AbstractFunctionResult::Unit,
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: vec![AbstractBlockEntry {
                block: entry,
                parameters: Vec::new(),
                operation_offset: 0,
            }],
            operations: vec![
                AbstractOperation::IntegerConstant {
                    psi_operation: operation,
                    result: value_id,
                    scalar_type: ScalarType::Integer(scalar_type),
                    value,
                },
                AbstractOperation::ReturnUnit {
                    psi_edge: edge,
                    cleanup_actions: Vec::new(),
                },
            ],
        },
        TargetFunction {
            machine,
            attachment: None,
            fixed_integer_scalar_abi: None,
            provenance: TerminalPsiProvenance {
                operations: vec![operation],
                edges: vec![edge],
            },
            operation: TargetOperation::UnitBody(TargetUnitBody {
                structural_types: Vec::new(),
                call_plan: evaluate_call_plan(
                    CallingPolicy::native_for_target(NativeTarget::linux_x64()),
                    &CallSignature::default(),
                )
                .unwrap(),
                parameters: Vec::new(),
                operations: vec![
                    TargetUnitOperation::IntegerConstant {
                        psi_operation: operation,
                        result: value_id,
                        scalar_type,
                        value,
                    },
                    TargetUnitOperation::Return {
                        psi_edge: edge,
                        cleanup_actions: Vec::new(),
                    },
                ],
            }),
        },
    )
}

fn ieee_float_literal_unit_return_pair() -> (AbstractFunction, TargetFunction) {
    let machine = MachineId::new(59_001).unwrap();
    let entry = BlockId::new(59_002).unwrap();
    let operation = OperationId::new(59_003).unwrap();
    let value_id = ValueId::new(59_004).unwrap();
    let edge = EdgeId::new(59_005).unwrap();
    let value = IeeeFloatValue::Binary64(0x7ff8_1234_5678_9abc);
    (
        AbstractFunction {
            machine,
            attachment: None,
            entry,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: AbstractFunctionResult::Unit,
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: vec![AbstractBlockEntry {
                block: entry,
                parameters: Vec::new(),
                operation_offset: 0,
            }],
            operations: vec![
                AbstractOperation::IeeeFloatConstant {
                    psi_operation: operation,
                    result: value_id,
                    value,
                },
                AbstractOperation::ReturnUnit {
                    psi_edge: edge,
                    cleanup_actions: Vec::new(),
                },
            ],
        },
        TargetFunction {
            machine,
            attachment: None,
            fixed_integer_scalar_abi: None,
            provenance: TerminalPsiProvenance {
                operations: vec![operation],
                edges: vec![edge],
            },
            operation: TargetOperation::UnitBody(TargetUnitBody {
                structural_types: Vec::new(),
                call_plan: evaluate_call_plan(
                    CallingPolicy::native_for_target(NativeTarget::linux_x64()),
                    &CallSignature::default(),
                )
                .unwrap(),
                parameters: Vec::new(),
                operations: vec![
                    TargetUnitOperation::IeeeFloatConstant {
                        psi_operation: operation,
                        result: value_id,
                        value,
                    },
                    TargetUnitOperation::Return {
                        psi_edge: edge,
                        cleanup_actions: Vec::new(),
                    },
                ],
            }),
        },
    )
}

fn ieee_float_literal_sequence_unit_return_pair() -> (AbstractFunction, TargetFunction) {
    let (mut source, mut target) = ieee_float_literal_unit_return_pair();
    let operation = OperationId::new(59_006).unwrap();
    let result = ValueId::new(59_007).unwrap();
    let value = IeeeFloatValue::Binary32(0x8000_0000);
    source.operations.insert(
        1,
        AbstractOperation::IeeeFloatConstant {
            psi_operation: operation,
            result,
            value,
        },
    );
    target.provenance.operations.push(operation);
    let TargetOperation::UnitBody(body) = &mut target.operation else {
        unreachable!()
    };
    body.operations.insert(
        1,
        TargetUnitOperation::IeeeFloatConstant {
            psi_operation: operation,
            result,
            value,
        },
    );
    (source, target)
}

fn trivial_affine_local_pair() -> (AbstractFunction, TargetFunction) {
    let machine = MachineId::new(53_001).unwrap();
    let entry = BlockId::new(53_002).unwrap();
    let structural_type = StructuralTypeDeclaration {
        id: StructuralTypeId::new(53_003).unwrap(),
        identity: "TrivialAffineToken".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    };
    let place = StructuralPlaceDeclaration {
        id: PlaceId::new(53_004).unwrap(),
        kind: StructuralPlaceKind::TrivialAffineLocal {
            declaration_ordinal: 0,
            structural_type: structural_type.id,
            construction: None,
        },
    };
    let operation = OperationId::new(53_005).unwrap();
    let edge = EdgeId::new(53_006).unwrap();
    let cleanup = vec![TerminalAffineCleanupAction::DiscardRoot(place.id)];
    (
        AbstractFunction {
            machine,
            attachment: None,
            entry,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: AbstractFunctionResult::Unit,
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: vec![AbstractBlockEntry {
                block: entry,
                parameters: Vec::new(),
                operation_offset: 0,
            }],
            operations: vec![
                AbstractOperation::EstablishTrivialAffineLocal {
                    psi_operation: operation,
                    place: place.clone(),
                    structural_type: structural_type.clone(),
                },
                AbstractOperation::ReturnUnit {
                    psi_edge: edge,
                    cleanup_actions: cleanup.clone(),
                },
            ],
        },
        TargetFunction {
            machine,
            attachment: None,
            fixed_integer_scalar_abi: None,
            provenance: TerminalPsiProvenance {
                operations: vec![operation],
                edges: vec![edge],
            },
            operation: TargetOperation::UnitBody(TargetUnitBody {
                structural_types: vec![structural_type.clone()],
                call_plan: evaluate_call_plan(
                    CallingPolicy::native_for_target(NativeTarget::linux_x64()),
                    &CallSignature::default(),
                )
                .unwrap(),
                parameters: Vec::new(),
                operations: vec![
                    TargetUnitOperation::EstablishTrivialAffineLocal {
                        psi_operation: operation,
                        place,
                        structural_type,
                    },
                    TargetUnitOperation::Return {
                        psi_edge: edge,
                        cleanup_actions: cleanup,
                    },
                ],
            }),
        },
    )
}

#[test]
fn omission_is_uncovered_while_duplicate_or_overlap_fails_closed() {
    let (source, target) = boolean_literal_pair();
    let source = &source;
    let target = &target;
    assert_eq!(
        selection::validate(source, NativeTarget::linux_x64(), target, &[]).unwrap(),
        AbstractToTargetFunctionTranslationDisposition::Uncovered
    );

    let boolean = ENABLED_TRANSLATION_FAMILIES
        .iter()
        .find(|descriptor| {
            descriptor.family == AbstractToTargetTranslationFamily::StraightLineBooleanImmediate
        })
        .copied()
        .unwrap();
    assert!(matches!(
        selection::validate(
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

    let TranslationFamilyValidator::Plain(boolean_validator) = boolean.validate else {
        panic!("the boolean family must use the plain validator contract");
    };
    let overlapping_alias = TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineIntegerImmediate,
        boolean.is_candidate,
        boolean_validator,
    );
    assert!(matches!(
        selection::validate(
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

#[test]
fn unit_call_catalog_omission_and_duplicate_fail_closed() {
    let (source, target) = unit_call_pair();
    assert_eq!(
        selection::validate(&source, NativeTarget::linux_x64(), &target, &[]).unwrap(),
        AbstractToTargetFunctionTranslationDisposition::Uncovered
    );

    let unit_call = ENABLED_TRANSLATION_FAMILIES
        .iter()
        .find(|descriptor| {
            descriptor.family == AbstractToTargetTranslationFamily::StraightLineUnitCallReturn
        })
        .copied()
        .unwrap();
    assert!(matches!(
        selection::validate(
            &source,
            NativeTarget::linux_x64(),
            &target,
            &[unit_call, unit_call]
        ),
        Err(
            AbstractToTargetTranslationValidationError::AmbiguousFunctionFamily {
                first: AbstractToTargetTranslationFamily::StraightLineUnitCallReturn,
                second: AbstractToTargetTranslationFamily::StraightLineUnitCallReturn,
                ..
            }
        )
    ));
}

#[test]
fn byte_sequence_literal_catalog_omission_and_duplicate_fail_closed() {
    let (source, target) = byte_sequence_literal_pair();
    assert_eq!(
        selection::validate(&source, NativeTarget::linux_x64(), &target, &[]).unwrap(),
        AbstractToTargetFunctionTranslationDisposition::Uncovered
    );

    let family = ENABLED_TRANSLATION_FAMILIES
        .iter()
        .find(|descriptor| {
            descriptor.family
                == AbstractToTargetTranslationFamily::StraightLineByteSequenceLiteralUnitReturn
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
                first: AbstractToTargetTranslationFamily::StraightLineByteSequenceLiteralUnitReturn,
                second:
                    AbstractToTargetTranslationFamily::StraightLineByteSequenceLiteralUnitReturn,
                ..
            }
        )
    ));
}

#[test]
fn integer_literal_unit_return_catalog_omission_and_duplicate_fail_closed() {
    let (source, target) = integer_literal_unit_return_pair();
    assert_eq!(
        selection::validate(&source, NativeTarget::linux_x64(), &target, &[]).unwrap(),
        AbstractToTargetFunctionTranslationDisposition::Uncovered
    );

    let family = ENABLED_TRANSLATION_FAMILIES
        .iter()
        .find(|descriptor| {
            descriptor.family
                == AbstractToTargetTranslationFamily::StraightLineIntegerLiteralUnitReturn
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
                first: AbstractToTargetTranslationFamily::StraightLineIntegerLiteralUnitReturn,
                second: AbstractToTargetTranslationFamily::StraightLineIntegerLiteralUnitReturn,
                ..
            }
        )
    ));
}

#[test]
fn ieee_float_literal_unit_return_catalog_omission_and_duplicate_fail_closed() {
    let (source, target) = ieee_float_literal_unit_return_pair();
    assert_eq!(
        selection::validate(&source, NativeTarget::linux_x64(), &target, &[]).unwrap(),
        AbstractToTargetFunctionTranslationDisposition::Uncovered
    );

    let family = ENABLED_TRANSLATION_FAMILIES
        .iter()
        .find(|descriptor| {
            descriptor.family
                == AbstractToTargetTranslationFamily::StraightLineIeeeFloatLiteralUnitReturn
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
                first: AbstractToTargetTranslationFamily::StraightLineIeeeFloatLiteralUnitReturn,
                second: AbstractToTargetTranslationFamily::StraightLineIeeeFloatLiteralUnitReturn,
                ..
            }
        )
    ));
}

#[test]
fn ieee_float_literal_sequence_catalog_omission_and_duplicate_fail_closed() {
    let (source, target) = ieee_float_literal_sequence_unit_return_pair();
    assert_eq!(
        selection::validate(&source, NativeTarget::linux_x64(), &target, &[]).unwrap(),
        AbstractToTargetFunctionTranslationDisposition::Uncovered
    );

    let family = ENABLED_TRANSLATION_FAMILIES
        .iter()
        .find(|descriptor| {
            descriptor.family
                == AbstractToTargetTranslationFamily::StraightLineIeeeFloatLiteralSequenceUnitReturn
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
                    AbstractToTargetTranslationFamily::StraightLineIeeeFloatLiteralSequenceUnitReturn,
                second:
                    AbstractToTargetTranslationFamily::StraightLineIeeeFloatLiteralSequenceUnitReturn,
                ..
            }
        )
    ));
}

#[test]
fn trivial_affine_local_catalog_omission_and_duplicate_fail_closed() {
    let (source, target) = trivial_affine_local_pair();
    assert_eq!(
        selection::validate(&source, NativeTarget::linux_x64(), &target, &[]).unwrap(),
        AbstractToTargetFunctionTranslationDisposition::Uncovered
    );

    let family = ENABLED_TRANSLATION_FAMILIES
        .iter()
        .find(|descriptor| {
            descriptor.family
                == AbstractToTargetTranslationFamily::StraightLineTrivialAffineLocalUnitReturn
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
                first: AbstractToTargetTranslationFamily::StraightLineTrivialAffineLocalUnitReturn,
                second: AbstractToTargetTranslationFamily::StraightLineTrivialAffineLocalUnitReturn,
                ..
            }
        )
    ));
}
