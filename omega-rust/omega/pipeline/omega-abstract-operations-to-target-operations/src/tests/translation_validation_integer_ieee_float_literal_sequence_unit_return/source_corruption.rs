use super::*;

#[test]
fn source_envelope_corruption_fails_closed() {
    assert_eq!(
        leaf_error(|function| function.parameters.push(AbstractParameter {
            value: ValueId::new(63_101).unwrap(),
            scalar_type: ScalarType::Boolean,
        })),
        StraightLineIntegerIeeeFloatLiteralSequenceUnitReturnTranslationError::SourceParameters
    );
    assert_eq!(
        leaf_error(|function| function.structural_parameters.push(
            StructuralParameterDeclaration {
                place: PlaceId::new(63_102).unwrap(),
                position: 0,
                is_self: false,
                structural_type: StructuralTypeId::new(63_010).unwrap(),
                multiplicity: StructuralMultiplicity::Affine,
                access: StructuralAccess::Owned,
                qualifications: vec![StructuralDomainId::new(63_103).unwrap()],
                projected_qualifications: Vec::new(),
            }
        )),
        StraightLineIntegerIeeeFloatLiteralSequenceUnitReturnTranslationError::SourceStructuralParameters
    );
    assert_eq!(
        leaf_error(
            |function| function.result = AbstractFunctionResult::Scalar(AbstractResult {
                value: ValueId::new(63_104).unwrap(),
                scalar_type: ScalarType::Boolean,
            })
        ),
        StraightLineIntegerIeeeFloatLiteralSequenceUnitReturnTranslationError::SourceResult
    );
    assert_eq!(
        leaf_error(|function| function.entry_claims.push(EntryClaim {
            claim: ClaimId::new(63_105).unwrap(),
            input: PlaceId::new(63_106).unwrap(),
            path: Vec::new(),
        })),
        StraightLineIntegerIeeeFloatLiteralSequenceUnitReturnTranslationError::SourceEntryClaims
    );
    assert_eq!(
        leaf_error(|function| function
            .published_service_ceiling
            .push(ServiceId::new(63_107).unwrap())),
        StraightLineIntegerIeeeFloatLiteralSequenceUnitReturnTranslationError::SourcePublishedServices
    );
    for mutate in [
        |function: &mut AbstractFunction| function.block_entries.clear(),
        |function: &mut AbstractFunction| {
            function.block_entries[0].block = BlockId::new(63_108).unwrap()
        },
        |function: &mut AbstractFunction| function.block_entries[0].operation_offset = 1,
    ] {
        assert_eq!(
            leaf_error(mutate),
            StraightLineIntegerIeeeFloatLiteralSequenceUnitReturnTranslationError::SourceBlockRoster
        );
    }
}

#[test]
fn source_mixed_roster_type_range_and_cleanup_corruption_fails_closed() {
    for mutate in [
        |function: &mut AbstractFunction| {
            function.operations.remove(1);
        },
        |function: &mut AbstractFunction| {
            function.operations[1] = AbstractOperation::BooleanConstant {
                psi_operation: OperationId::new(63_110).unwrap(),
                result: ValueId::new(63_111).unwrap(),
                value: true,
            };
        },
        |function: &mut AbstractFunction| {
            function.operations.swap(0, 3);
        },
    ] {
        assert_eq!(
            leaf_error(mutate),
            StraightLineIntegerIeeeFloatLiteralSequenceUnitReturnTranslationError::SourceOperationRoster
        );
    }
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::IntegerConstant { scalar_type, .. } =
                &mut function.operations[0]
            else {
                unreachable!()
            };
            *scalar_type = ScalarType::Boolean;
        }),
        StraightLineIntegerIeeeFloatLiteralSequenceUnitReturnTranslationError::SourceConstantType
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::IntegerConstant {
                scalar_type, value, ..
            } = &mut function.operations[0]
            else {
                unreachable!()
            };
            *scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap());
            *value = IntegerValue::Unsigned(256);
        }),
        StraightLineIntegerIeeeFloatLiteralSequenceUnitReturnTranslationError::SourceConstantOutsideType
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::ReturnUnit {
                cleanup_actions, ..
            } = function.operations.last_mut().unwrap()
            else {
                unreachable!()
            };
            cleanup_actions.push(TerminalAffineCleanupAction::DiscardRoot(
                PlaceId::new(63_112).unwrap(),
            ));
        }),
        StraightLineIntegerIeeeFloatLiteralSequenceUnitReturnTranslationError::SourceCleanupActions
    );
}

#[test]
fn source_identity_and_literal_drift_rejects_the_original_target() {
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::IeeeFloatConstant { psi_operation, .. } =
                &mut function.operations[1]
            else {
                unreachable!()
            };
            *psi_operation = OperationId::new(63_120).unwrap();
        }),
        StraightLineIntegerIeeeFloatLiteralSequenceUnitReturnTranslationError::TargetProvenance
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::IeeeFloatConstant { result, .. } = &mut function.operations[1]
            else {
                unreachable!()
            };
            *result = ValueId::new(63_121).unwrap();
        }),
        StraightLineIntegerIeeeFloatLiteralSequenceUnitReturnTranslationError::TargetConstant
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::IeeeFloatConstant { value, .. } = &mut function.operations[1]
            else {
                unreachable!()
            };
            *value = IeeeFloatValue::Binary64(0x7ff8_1234_5678_9abc);
        }),
        StraightLineIntegerIeeeFloatLiteralSequenceUnitReturnTranslationError::TargetConstant
    );
}
