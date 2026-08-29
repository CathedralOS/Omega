use omega_abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan, AbstractParameter, AbstractResult,
};
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use omega_target_operations::{TargetOperation, TerminalPsiProvenance};
use psi_core::{
    BlockId, ClaimId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId,
    PlaceId, ScalarType, ServiceId, StructuralDomainId, StructuralTypeId, ValueId,
};
use psi_terminal::{
    EntryClaim, SemanticFingerprint, StructuralAccess, StructuralMultiplicity,
    StructuralParameterDeclaration, TerminalAffineCleanupAction, TerminalPsiIdentity,
    VocabularyMarker,
};

use crate::{
    AbstractToTargetFunctionTranslationDisposition, AbstractToTargetFunctionTranslationReceipt,
    AbstractToTargetTranslationFamily, AbstractToTargetTranslationFamilyError,
    AbstractToTargetTranslationValidationError, StraightLineIntegerImmediateTranslationError,
    lower_to_target_operations, validate_abstract_to_target_translation,
};

fn integer_type(sign: IntegerSign, bits: u16) -> IntegerType {
    IntegerType::new(sign, bits).expect("valid fixture type")
}

fn literal_function(base: u64, scalar_type: IntegerType, value: IntegerValue) -> AbstractFunction {
    let machine = MachineId::new(base + 1).unwrap();
    let entry = BlockId::new(base + 2).unwrap();
    let constant_operation = OperationId::new(base + 3).unwrap();
    let constant_value = ValueId::new(base + 4).unwrap();
    let function_result = ValueId::new(base + 5).unwrap();
    let return_edge = EdgeId::new(base + 6).unwrap();
    let scalar_type = ScalarType::Integer(scalar_type);
    AbstractFunction {
        machine,
        attachment: None,
        entry,
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        result: AbstractFunctionResult::Scalar(AbstractResult {
            value: function_result,
            scalar_type,
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
                result: constant_value,
                scalar_type,
                value,
            },
            AbstractOperation::Return {
                psi_edge: return_edge,
                result: function_result,
                value: constant_value,
                scalar_type,
                cleanup_actions: Vec::new(),
            },
        ],
    }
}

fn literal_plan(functions: Vec<AbstractFunction>) -> AbstractOperationPlan {
    AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([0x5a; 32]),
        },
        entry: functions[0].machine,
        structural_types: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions,
    }
}

fn base_plan() -> AbstractOperationPlan {
    literal_plan(vec![literal_function(
        100,
        integer_type(IntegerSign::Unsigned, 64),
        IntegerValue::Unsigned(37),
    )])
}

fn leaf_error(
    mutate: impl FnOnce(&mut AbstractFunction),
) -> StraightLineIntegerImmediateTranslationError {
    let mut source = base_plan();
    let target = lower_to_target_operations(&source, NativeTarget::linux_x64()).unwrap();
    mutate(&mut source.functions[0]);
    crate::validation::straight_line_integer_immediate::validate(
        &source.functions[0],
        &target.functions[0],
    )
    .unwrap_err()
}

fn candidate_error(
    mutate: impl FnOnce(&mut omega_target_operations::TargetOperationPlan),
) -> AbstractToTargetTranslationValidationError {
    let source = base_plan();
    let target_profile = NativeTarget::linux_x64();
    let mut candidate = lower_to_target_operations(&source, target_profile).unwrap();
    mutate(&mut candidate);
    validate_abstract_to_target_translation(&source, target_profile, &candidate).unwrap_err()
}

#[test]
fn validates_exact_integer_identity_on_every_native_target() {
    let cases = [
        (
            integer_type(IntegerSign::Unsigned, 8),
            IntegerValue::Unsigned(u8::MAX.into()),
        ),
        (
            integer_type(IntegerSign::Signed, 16),
            IntegerValue::Signed(i16::MIN.into()),
        ),
        (
            integer_type(IntegerSign::Unsigned, 64),
            IntegerValue::Unsigned(37),
        ),
    ];
    for target_profile in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        for (scalar_type, value) in cases {
            let source = literal_plan(vec![literal_function(100, scalar_type, value)]);
            let target = lower_to_target_operations(&source, target_profile).unwrap();
            let receipt =
                validate_abstract_to_target_translation(&source, target_profile, &target).unwrap();
            assert_eq!(receipt.psi(), source.psi);
            assert_eq!(receipt.target(), target_profile);
            assert_eq!(receipt.entry(), source.entry);
            assert_eq!(receipt.function_count(), 1);
            assert_eq!(
                receipt.function_roster()[0].machine(),
                source.functions[0].machine
            );
            assert_eq!(receipt.function_roster()[0].attachment(), None);
            let AbstractToTargetFunctionTranslationDisposition::Validated(
                AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerImmediate(row),
            ) = receipt.function_roster()[0].translation()
            else {
                panic!("exact literal return must publish one validated family row")
            };
            assert_eq!(row.machine(), source.functions[0].machine);
            assert_eq!(row.constant_operation(), OperationId::new(103).unwrap());
            assert_eq!(row.return_edge(), EdgeId::new(106).unwrap());
            assert_eq!(row.source_value(), ValueId::new(104).unwrap());
            assert_eq!(row.scalar_type(), scalar_type);
            assert_eq!(row.value(), value);
        }
    }
}

#[test]
fn receipt_does_not_claim_unimplemented_parameterized_literal_family() {
    let mut source = base_plan();
    source.functions[0].parameters.push(AbstractParameter {
        value: ValueId::new(920).unwrap(),
        scalar_type: ScalarType::Integer(integer_type(IntegerSign::Unsigned, 64)),
    });
    let target_profile = NativeTarget::linux_x64();
    let target = lower_to_target_operations(&source, target_profile).unwrap();
    let receipt =
        validate_abstract_to_target_translation(&source, target_profile, &target).unwrap();
    assert_eq!(
        receipt.function_roster()[0].translation(),
        &AbstractToTargetFunctionTranslationDisposition::Uncovered
    );
    assert_eq!(receipt.function_count(), 1);
}

#[test]
fn receipt_retains_an_exact_attached_literal_function_roster() {
    let mut source = base_plan();
    let attachment = StructuralTypeId::new(921).unwrap();
    source.functions[0].attachment = Some(attachment);
    let target_profile = NativeTarget::linux_x64();
    let target = lower_to_target_operations(&source, target_profile).unwrap();
    let receipt =
        validate_abstract_to_target_translation(&source, target_profile, &target).unwrap();
    assert_eq!(receipt.function_roster()[0].attachment(), Some(attachment));
    assert!(matches!(
        receipt.function_roster()[0].translation(),
        AbstractToTargetFunctionTranslationDisposition::Validated(
            AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerImmediate(_)
        )
    ));
}

#[test]
fn root_and_function_roster_corruption_fails_closed() {
    assert_eq!(
        candidate_error(|candidate| {
            candidate.psi.program_fingerprint = SemanticFingerprint::from_bytes([0x6b; 32]);
        }),
        AbstractToTargetTranslationValidationError::PsiMismatch
    );
    for mutate in [
        |target: &mut NativeTarget| target.architecture = Architecture::Aarch64,
        |target: &mut NativeTarget| target.object_format = ObjectFormat::MachO,
        |target: &mut NativeTarget| target.pointer_size = 4,
        |target: &mut NativeTarget| target.pointer_alignment = 4,
    ] {
        assert_eq!(
            candidate_error(|candidate| mutate(&mut candidate.target)),
            AbstractToTargetTranslationValidationError::TargetMismatch
        );
    }
    assert_eq!(
        candidate_error(|candidate| candidate.entry = MachineId::new(999).unwrap()),
        AbstractToTargetTranslationValidationError::EntryMismatch
    );
    assert_eq!(
        candidate_error(|candidate| candidate.functions.clear()),
        AbstractToTargetTranslationValidationError::FunctionCountMismatch
    );
    assert_eq!(
        candidate_error(|candidate| candidate.functions.push(candidate.functions[0].clone())),
        AbstractToTargetTranslationValidationError::FunctionCountMismatch
    );
    assert_eq!(
        candidate_error(|candidate| {
            candidate.functions[0].machine = MachineId::new(998).unwrap();
        }),
        AbstractToTargetTranslationValidationError::FunctionMachineMismatch { position: 0 }
    );
    assert!(matches!(
        candidate_error(|candidate| {
            candidate.functions[0].attachment = Some(StructuralTypeId::new(997).unwrap());
        }),
        AbstractToTargetTranslationValidationError::FunctionAttachmentMismatch { .. }
    ));

    let source = literal_plan(vec![
        literal_function(
            100,
            integer_type(IntegerSign::Unsigned, 32),
            IntegerValue::Unsigned(1),
        ),
        literal_function(
            200,
            integer_type(IntegerSign::Signed, 32),
            IntegerValue::Signed(-1),
        ),
    ]);
    let target_profile = NativeTarget::linux_x64();
    let mut target = lower_to_target_operations(&source, target_profile).unwrap();
    target.functions.swap(0, 1);
    assert_eq!(
        validate_abstract_to_target_translation(&source, target_profile, &target).unwrap_err(),
        AbstractToTargetTranslationValidationError::FunctionMachineMismatch { position: 0 }
    );
}

#[test]
fn source_family_shape_and_semantic_corruption_fails_closed() {
    assert_eq!(
        leaf_error(|function| {
            function.parameters.push(AbstractParameter {
                value: ValueId::new(901).unwrap(),
                scalar_type: ScalarType::Integer(integer_type(IntegerSign::Unsigned, 64)),
            });
        }),
        StraightLineIntegerImmediateTranslationError::SourceParameters
    );
    assert_eq!(
        leaf_error(|function| {
            function
                .structural_parameters
                .push(StructuralParameterDeclaration {
                    place: PlaceId::new(902).unwrap(),
                    position: 0,
                    is_self: false,
                    structural_type: StructuralTypeId::new(903).unwrap(),
                    multiplicity: StructuralMultiplicity::Affine,
                    access: StructuralAccess::Owned,
                    qualifications: vec![StructuralDomainId::new(904).unwrap()],
                });
        }),
        StraightLineIntegerImmediateTranslationError::SourceStructuralParameters
    );
    assert_eq!(
        leaf_error(|function| function.result = AbstractFunctionResult::Unit),
        StraightLineIntegerImmediateTranslationError::SourceResult
    );
    assert_eq!(
        leaf_error(|function| {
            function.entry_claims.push(EntryClaim {
                claim: ClaimId::new(905).unwrap(),
                input: PlaceId::new(906).unwrap(),
                path: Vec::new(),
            });
        }),
        StraightLineIntegerImmediateTranslationError::SourceEntryClaims
    );
    assert_eq!(
        leaf_error(|function| {
            function
                .published_service_ceiling
                .push(ServiceId::new(907).unwrap());
        }),
        StraightLineIntegerImmediateTranslationError::SourcePublishedServices
    );
    for mutate in [
        |function: &mut AbstractFunction| function.block_entries.clear(),
        |function: &mut AbstractFunction| {
            function.block_entries[0].block = BlockId::new(908).unwrap()
        },
        |function: &mut AbstractFunction| {
            function.block_entries[0]
                .parameters
                .push(AbstractParameter {
                    value: ValueId::new(909).unwrap(),
                    scalar_type: ScalarType::Integer(integer_type(IntegerSign::Unsigned, 64)),
                })
        },
        |function: &mut AbstractFunction| function.block_entries[0].operation_offset = 1,
    ] {
        assert_eq!(
            leaf_error(mutate),
            StraightLineIntegerImmediateTranslationError::SourceBlockRoster
        );
    }
    assert_eq!(
        leaf_error(|function| function.operations.swap(0, 1)),
        StraightLineIntegerImmediateTranslationError::SourceOperationRoster
    );
    assert_eq!(
        leaf_error(|function| {
            function.operations.pop();
        }),
        StraightLineIntegerImmediateTranslationError::SourceOperationRoster
    );
    assert_eq!(
        leaf_error(|function| {
            function
                .operations
                .push(AbstractOperation::BooleanConstant {
                    psi_operation: OperationId::new(915).unwrap(),
                    result: ValueId::new(916).unwrap(),
                    value: false,
                });
        }),
        StraightLineIntegerImmediateTranslationError::SourceOperationRoster
    );
    assert_eq!(
        leaf_error(|function| {
            function.operations[0] = AbstractOperation::BooleanConstant {
                psi_operation: OperationId::new(910).unwrap(),
                result: ValueId::new(911).unwrap(),
                value: true,
            };
        }),
        StraightLineIntegerImmediateTranslationError::SourceOperationRoster
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::IntegerConstant { scalar_type, .. } =
                &mut function.operations[0]
            else {
                unreachable!()
            };
            *scalar_type = ScalarType::Boolean;
        }),
        StraightLineIntegerImmediateTranslationError::SourceConstantType
    );
    assert_eq!(
        leaf_error(|function| {
            let narrowed = integer_type(IntegerSign::Unsigned, 8);
            let AbstractFunctionResult::Scalar(result) = &mut function.result else {
                unreachable!()
            };
            result.scalar_type = ScalarType::Integer(narrowed);
            let AbstractOperation::IntegerConstant {
                scalar_type, value, ..
            } = &mut function.operations[0]
            else {
                unreachable!()
            };
            *scalar_type = ScalarType::Integer(narrowed);
            *value = IntegerValue::Unsigned(256);
            let AbstractOperation::Return { scalar_type, .. } = &mut function.operations[1] else {
                unreachable!()
            };
            *scalar_type = ScalarType::Integer(narrowed);
        }),
        StraightLineIntegerImmediateTranslationError::SourceConstantOutsideType
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::IntegerConstant { value, .. } = &mut function.operations[0]
            else {
                unreachable!()
            };
            *value = IntegerValue::Signed(-1);
        }),
        StraightLineIntegerImmediateTranslationError::SourceConstantOutsideType
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractFunctionResult::Scalar(result) = &mut function.result else {
                unreachable!()
            };
            result.value = ValueId::new(912).unwrap();
        }),
        StraightLineIntegerImmediateTranslationError::SourceResultLink
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::IntegerConstant { result, .. } = &mut function.operations[0]
            else {
                unreachable!()
            };
            *result = ValueId::new(917).unwrap();
        }),
        StraightLineIntegerImmediateTranslationError::SourceResultLink
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::Return { scalar_type, .. } = &mut function.operations[1] else {
                unreachable!()
            };
            *scalar_type = ScalarType::Boolean;
        }),
        StraightLineIntegerImmediateTranslationError::SourceResultLink
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::Return { value, .. } = &mut function.operations[1] else {
                unreachable!()
            };
            *value = ValueId::new(913).unwrap();
        }),
        StraightLineIntegerImmediateTranslationError::SourceResultLink
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
                PlaceId::new(914).unwrap(),
            ));
        }),
        StraightLineIntegerImmediateTranslationError::SourceCleanup
    );
}

#[test]
fn candidate_operation_and_provenance_corruption_fails_closed() {
    for provenance in [
        TerminalPsiProvenance::default(),
        TerminalPsiProvenance {
            operations: vec![OperationId::new(800).unwrap()],
            edges: vec![EdgeId::new(106).unwrap()],
        },
        TerminalPsiProvenance {
            operations: vec![
                OperationId::new(103).unwrap(),
                OperationId::new(801).unwrap(),
            ],
            edges: vec![EdgeId::new(106).unwrap()],
        },
        TerminalPsiProvenance {
            operations: vec![OperationId::new(103).unwrap()],
            edges: vec![EdgeId::new(802).unwrap()],
        },
        TerminalPsiProvenance {
            operations: vec![OperationId::new(103).unwrap()],
            edges: vec![EdgeId::new(106).unwrap(), EdgeId::new(803).unwrap()],
        },
    ] {
        assert!(matches!(
            candidate_error(|candidate| candidate.functions[0].provenance = provenance),
            AbstractToTargetTranslationValidationError::FunctionFamily {
                family: AbstractToTargetTranslationFamily::StraightLineIntegerImmediate,
                error: AbstractToTargetTranslationFamilyError::StraightLineIntegerImmediate(
                    StraightLineIntegerImmediateTranslationError::TargetProvenance
                ),
                ..
            }
        ));
    }
    assert!(matches!(
        candidate_error(|candidate| {
            candidate.functions[0].operation = TargetOperation::ReturnBooleanImmediate {
                psi_edge: EdgeId::new(106).unwrap(),
                source_value: ValueId::new(104).unwrap(),
                value: true,
            };
        }),
        AbstractToTargetTranslationValidationError::FunctionFamily {
            family: AbstractToTargetTranslationFamily::StraightLineIntegerImmediate,
            error: AbstractToTargetTranslationFamilyError::StraightLineIntegerImmediate(
                StraightLineIntegerImmediateTranslationError::TargetOperation
            ),
            ..
        }
    ));
    for mutate in [
        |operation: &mut TargetOperation| {
            let TargetOperation::ReturnIntegerImmediate { psi_edge, .. } = operation else {
                unreachable!()
            };
            *psi_edge = EdgeId::new(804).unwrap();
        },
        |operation: &mut TargetOperation| {
            let TargetOperation::ReturnIntegerImmediate { source_value, .. } = operation else {
                unreachable!()
            };
            *source_value = ValueId::new(805).unwrap();
        },
        |operation: &mut TargetOperation| {
            let TargetOperation::ReturnIntegerImmediate { scalar_type, .. } = operation else {
                unreachable!()
            };
            *scalar_type = integer_type(IntegerSign::Signed, 64);
        },
        |operation: &mut TargetOperation| {
            let TargetOperation::ReturnIntegerImmediate { value, .. } = operation else {
                unreachable!()
            };
            *value = IntegerValue::Unsigned(38);
        },
    ] {
        assert!(matches!(
            candidate_error(|candidate| mutate(&mut candidate.functions[0].operation)),
            AbstractToTargetTranslationValidationError::FunctionFamily {
                family: AbstractToTargetTranslationFamily::StraightLineIntegerImmediate,
                error: AbstractToTargetTranslationFamilyError::StraightLineIntegerImmediate(
                    StraightLineIntegerImmediateTranslationError::TargetOperation
                ),
                ..
            }
        ));
    }
}
