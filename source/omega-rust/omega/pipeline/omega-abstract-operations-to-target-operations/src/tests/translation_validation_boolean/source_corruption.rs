use super::*;

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
                    projected_qualifications: Vec::new(),
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
