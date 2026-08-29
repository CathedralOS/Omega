use omega_abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan, AbstractParameter, AbstractResult,
};
use omega_target::NativeTarget;
use omega_target_operations::{TargetOperation, TerminalPsiProvenance};
use psi_core::{
    BlockId, ClaimId, EdgeId, IntegerSign, IntegerType, MachineId, OperationId, PlaceId,
    ScalarType, ServiceId, StructuralDomainId, StructuralTypeId, ValueId,
};
use psi_terminal::{
    EntryClaim, SemanticFingerprint, StructuralAccess, StructuralMultiplicity,
    StructuralParameterDeclaration, TerminalAffineCleanupAction, TerminalPsiIdentity,
    VocabularyMarker,
};

use crate::{
    AbstractToTargetTranslationValidationError, StraightLineBooleanImmediateTranslationError,
    lower_to_target_operations, validate_abstract_to_target_translation,
};

fn boolean_plan(value: bool) -> AbstractOperationPlan {
    let machine = MachineId::new(1_001).unwrap();
    let entry = BlockId::new(1_002).unwrap();
    let constant_operation = OperationId::new(1_003).unwrap();
    let constant_value = ValueId::new(1_004).unwrap();
    let function_result = ValueId::new(1_005).unwrap();
    let return_edge = EdgeId::new(1_006).unwrap();
    AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([0xb0; 32]),
        },
        entry: machine,
        structural_types: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![AbstractFunction {
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
                    result: constant_value,
                    value,
                },
                AbstractOperation::Return {
                    psi_edge: return_edge,
                    result: function_result,
                    value: constant_value,
                    scalar_type: ScalarType::Boolean,
                    cleanup_actions: Vec::new(),
                },
            ],
        }],
    }
}

fn leaf_error(
    mutate: impl FnOnce(&mut AbstractFunction),
) -> StraightLineBooleanImmediateTranslationError {
    let mut source = boolean_plan(true);
    let target = lower_to_target_operations(&source, NativeTarget::linux_x64()).unwrap();
    mutate(&mut source.functions[0]);
    crate::validation::straight_line_boolean_immediate::validate(
        &source.functions[0],
        &target.functions[0],
    )
    .unwrap_err()
}

fn candidate_error(
    mutate: impl FnOnce(&mut omega_target_operations::TargetOperationPlan),
) -> StraightLineBooleanImmediateTranslationError {
    let source = boolean_plan(true);
    let target_profile = NativeTarget::linux_x64();
    let mut candidate = lower_to_target_operations(&source, target_profile).unwrap();
    mutate(&mut candidate);
    let AbstractToTargetTranslationValidationError::StraightLineBooleanImmediate { error, .. } =
        validate_abstract_to_target_translation(&source, target_profile, &candidate).unwrap_err()
    else {
        panic!("Boolean-family corruption must fail at its independent validator")
    };
    error
}

#[test]
fn validates_exact_boolean_identity_on_every_native_target() {
    for target_profile in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        for value in [false, true] {
            let source = boolean_plan(value);
            let target = lower_to_target_operations(&source, target_profile).unwrap();
            let receipt =
                validate_abstract_to_target_translation(&source, target_profile, &target).unwrap();
            let [row] = receipt.straight_line_boolean_immediates() else {
                panic!("exact Boolean return must publish one validated family row")
            };
            assert_eq!(row.machine(), MachineId::new(1_001).unwrap());
            assert_eq!(row.constant_operation(), OperationId::new(1_003).unwrap());
            assert_eq!(row.return_edge(), EdgeId::new(1_006).unwrap());
            assert_eq!(row.source_value(), ValueId::new(1_004).unwrap());
            assert_eq!(row.value(), value);
            assert!(receipt.straight_line_integer_immediates().is_empty());
        }
    }
}

#[test]
fn receipt_does_not_claim_unimplemented_parameterized_boolean_family() {
    let mut source = boolean_plan(true);
    source.functions[0].parameters.push(AbstractParameter {
        value: ValueId::new(1_100).unwrap(),
        scalar_type: ScalarType::Boolean,
    });
    let target_profile = NativeTarget::linux_x64();
    let target = lower_to_target_operations(&source, target_profile).unwrap();
    let receipt =
        validate_abstract_to_target_translation(&source, target_profile, &target).unwrap();
    assert!(receipt.straight_line_boolean_immediates().is_empty());
}

#[test]
fn boolean_source_shape_and_result_corruption_fails_closed() {
    assert_eq!(
        leaf_error(|function| {
            function.parameters.push(AbstractParameter {
                value: ValueId::new(1_101).unwrap(),
                scalar_type: ScalarType::Boolean,
            });
        }),
        StraightLineBooleanImmediateTranslationError::SourceParameters
    );
    assert_eq!(
        leaf_error(|function| {
            function
                .structural_parameters
                .push(StructuralParameterDeclaration {
                    place: PlaceId::new(1_102).unwrap(),
                    position: 0,
                    is_self: false,
                    structural_type: StructuralTypeId::new(1_103).unwrap(),
                    multiplicity: StructuralMultiplicity::Affine,
                    access: StructuralAccess::Owned,
                    qualifications: vec![StructuralDomainId::new(1_104).unwrap()],
                });
        }),
        StraightLineBooleanImmediateTranslationError::SourceStructuralParameters
    );
    assert_eq!(
        leaf_error(|function| {
            function.result = AbstractFunctionResult::Scalar(AbstractResult {
                value: ValueId::new(1_005).unwrap(),
                scalar_type: ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
                ),
            });
        }),
        StraightLineBooleanImmediateTranslationError::SourceResult
    );
    assert_eq!(
        leaf_error(|function| {
            function.entry_claims.push(EntryClaim {
                claim: ClaimId::new(1_105).unwrap(),
                input: PlaceId::new(1_106).unwrap(),
                path: Vec::new(),
            });
        }),
        StraightLineBooleanImmediateTranslationError::SourceEntryClaims
    );
    assert_eq!(
        leaf_error(|function| {
            function
                .published_service_ceiling
                .push(ServiceId::new(1_107).unwrap());
        }),
        StraightLineBooleanImmediateTranslationError::SourcePublishedServices
    );
    for mutate in [
        |function: &mut AbstractFunction| function.block_entries.clear(),
        |function: &mut AbstractFunction| {
            function.block_entries[0].block = BlockId::new(1_114).unwrap()
        },
        |function: &mut AbstractFunction| {
            function.block_entries[0]
                .parameters
                .push(AbstractParameter {
                    value: ValueId::new(1_115).unwrap(),
                    scalar_type: ScalarType::Boolean,
                })
        },
        |function: &mut AbstractFunction| function.block_entries[0].operation_offset = 1,
    ] {
        assert_eq!(
            leaf_error(mutate),
            StraightLineBooleanImmediateTranslationError::SourceBlockRoster
        );
    }
    assert_eq!(
        leaf_error(|function| function.operations.swap(0, 1)),
        StraightLineBooleanImmediateTranslationError::SourceOperationRoster
    );
    assert_eq!(
        leaf_error(|function| {
            function.operations.pop();
        }),
        StraightLineBooleanImmediateTranslationError::SourceOperationRoster
    );
    assert_eq!(
        leaf_error(|function| {
            function
                .operations
                .push(AbstractOperation::BooleanConstant {
                    psi_operation: OperationId::new(1_116).unwrap(),
                    result: ValueId::new(1_117).unwrap(),
                    value: false,
                });
        }),
        StraightLineBooleanImmediateTranslationError::SourceOperationRoster
    );
    assert_eq!(
        leaf_error(|function| {
            function.operations[0] = AbstractOperation::IntegerConstant {
                psi_operation: OperationId::new(1_118).unwrap(),
                result: ValueId::new(1_119).unwrap(),
                scalar_type: ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
                ),
                value: psi_core::IntegerValue::Unsigned(1),
            };
        }),
        StraightLineBooleanImmediateTranslationError::SourceOperationRoster
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractFunctionResult::Scalar(result) = &mut function.result else {
                unreachable!()
            };
            result.value = ValueId::new(1_120).unwrap();
        }),
        StraightLineBooleanImmediateTranslationError::SourceResultLink
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::BooleanConstant { result, .. } = &mut function.operations[0]
            else {
                unreachable!()
            };
            *result = ValueId::new(1_121).unwrap();
        }),
        StraightLineBooleanImmediateTranslationError::SourceResultLink
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::Return { value, .. } = &mut function.operations[1] else {
                unreachable!()
            };
            *value = ValueId::new(1_108).unwrap();
        }),
        StraightLineBooleanImmediateTranslationError::SourceResultLink
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::Return { scalar_type, .. } = &mut function.operations[1] else {
                unreachable!()
            };
            *scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap());
        }),
        StraightLineBooleanImmediateTranslationError::SourceResultLink
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::Return {
                cleanup_actions, ..
            } = &mut function.operations[1]
            else {
                unreachable!()
            };
            cleanup_actions.push(TerminalAffineCleanupAction::DiscardRoot(
                PlaceId::new(1_109).unwrap(),
            ));
        }),
        StraightLineBooleanImmediateTranslationError::SourceCleanup
    );
}

#[test]
fn boolean_candidate_and_provenance_corruption_fails_closed() {
    for provenance in [
        TerminalPsiProvenance::default(),
        TerminalPsiProvenance {
            operations: vec![OperationId::new(1_110).unwrap()],
            edges: vec![EdgeId::new(1_006).unwrap()],
        },
        TerminalPsiProvenance {
            operations: vec![
                OperationId::new(1_003).unwrap(),
                OperationId::new(1_122).unwrap(),
            ],
            edges: vec![EdgeId::new(1_006).unwrap()],
        },
        TerminalPsiProvenance {
            operations: vec![OperationId::new(1_003).unwrap()],
            edges: vec![EdgeId::new(1_111).unwrap()],
        },
        TerminalPsiProvenance {
            operations: vec![OperationId::new(1_003).unwrap()],
            edges: vec![EdgeId::new(1_006).unwrap(), EdgeId::new(1_123).unwrap()],
        },
    ] {
        assert_eq!(
            candidate_error(|candidate| candidate.functions[0].provenance = provenance),
            StraightLineBooleanImmediateTranslationError::TargetProvenance
        );
    }
    assert_eq!(
        candidate_error(|candidate| {
            candidate.functions[0].operation = TargetOperation::ReturnIntegerImmediate {
                psi_edge: EdgeId::new(1_006).unwrap(),
                source_value: ValueId::new(1_004).unwrap(),
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
                value: psi_core::IntegerValue::Unsigned(1),
            };
        }),
        StraightLineBooleanImmediateTranslationError::TargetOperation
    );
    for mutate in [
        |operation: &mut TargetOperation| {
            let TargetOperation::ReturnBooleanImmediate { psi_edge, .. } = operation else {
                unreachable!()
            };
            *psi_edge = EdgeId::new(1_112).unwrap();
        },
        |operation: &mut TargetOperation| {
            let TargetOperation::ReturnBooleanImmediate { source_value, .. } = operation else {
                unreachable!()
            };
            *source_value = ValueId::new(1_113).unwrap();
        },
        |operation: &mut TargetOperation| {
            let TargetOperation::ReturnBooleanImmediate { value, .. } = operation else {
                unreachable!()
            };
            *value = false;
        },
    ] {
        assert_eq!(
            candidate_error(|candidate| mutate(&mut candidate.functions[0].operation)),
            StraightLineBooleanImmediateTranslationError::TargetOperation
        );
    }
}
