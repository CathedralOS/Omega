use super::*;

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
                    projected_qualifications: Vec::new(),
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
