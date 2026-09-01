use super::*;

#[test]
fn source_envelope_corruption_fails_closed() {
    assert_eq!(
        leaf_error(|function| function.parameters.push(AbstractParameter {
            value: ValueId::new(62_101).unwrap(),
            scalar_type: ScalarType::Boolean,
        })),
        StraightLineIntegerLiteralSequenceUnitReturnTranslationError::SourceParameters
    );
    assert_eq!(
        leaf_error(|function| function.structural_parameters.push(
            StructuralParameterDeclaration {
                place: PlaceId::new(62_102).unwrap(),
                position: 0,
                is_self: false,
                structural_type: StructuralTypeId::new(62_010).unwrap(),
                multiplicity: StructuralMultiplicity::Affine,
                access: StructuralAccess::Owned,
                qualifications: vec![StructuralDomainId::new(62_103).unwrap()],
                projected_qualifications: Vec::new(),
            }
        )),
        StraightLineIntegerLiteralSequenceUnitReturnTranslationError::SourceStructuralParameters
    );
    assert_eq!(
        leaf_error(
            |function| function.result = AbstractFunctionResult::Scalar(AbstractResult {
                value: ValueId::new(62_104).unwrap(),
                scalar_type: ScalarType::Boolean,
            })
        ),
        StraightLineIntegerLiteralSequenceUnitReturnTranslationError::SourceResult
    );
    assert_eq!(
        leaf_error(|function| function.entry_claims.push(EntryClaim {
            claim: ClaimId::new(62_105).unwrap(),
            input: PlaceId::new(62_106).unwrap(),
            path: Vec::new(),
        })),
        StraightLineIntegerLiteralSequenceUnitReturnTranslationError::SourceEntryClaims
    );
    assert_eq!(
        leaf_error(|function| function
            .published_service_ceiling
            .push(ServiceId::new(62_107).unwrap())),
        StraightLineIntegerLiteralSequenceUnitReturnTranslationError::SourcePublishedServices
    );
    for mutate in [
        |function: &mut AbstractFunction| function.block_entries.clear(),
        |function: &mut AbstractFunction| {
            function.block_entries[0].block = BlockId::new(62_108).unwrap()
        },
        |function: &mut AbstractFunction| function.block_entries[0].operation_offset = 1,
    ] {
        assert_eq!(
            leaf_error(mutate),
            StraightLineIntegerLiteralSequenceUnitReturnTranslationError::SourceBlockRoster
        );
    }
}

#[test]
fn source_roster_type_range_and_cleanup_corruption_fails_closed() {
    for mutate in [
        |function: &mut AbstractFunction| {
            function.operations.drain(1..literals().len());
        },
        |function: &mut AbstractFunction| {
            function.operations[1] = AbstractOperation::BooleanConstant {
                psi_operation: OperationId::new(62_110).unwrap(),
                result: ValueId::new(62_111).unwrap(),
                value: true,
            };
        },
        |function: &mut AbstractFunction| {
            function.operations.swap(0, literals().len());
        },
    ] {
        assert_eq!(
            leaf_error(mutate),
            StraightLineIntegerLiteralSequenceUnitReturnTranslationError::SourceOperationRoster
        );
    }
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::IntegerConstant { scalar_type, .. } =
                &mut function.operations[1]
            else {
                unreachable!()
            };
            *scalar_type = ScalarType::Boolean;
        }),
        StraightLineIntegerLiteralSequenceUnitReturnTranslationError::SourceConstantType
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::IntegerConstant {
                scalar_type, value, ..
            } = &mut function.operations[1]
            else {
                unreachable!()
            };
            *scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap());
            *value = IntegerValue::Unsigned(256);
        }),
        StraightLineIntegerLiteralSequenceUnitReturnTranslationError::SourceConstantOutsideType
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
                PlaceId::new(62_112).unwrap(),
            ));
        }),
        StraightLineIntegerLiteralSequenceUnitReturnTranslationError::SourceCleanupActions
    );
}

#[test]
fn source_identity_type_and_value_drift_rejects_original_target() {
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::IntegerConstant { psi_operation, .. } =
                &mut function.operations[1]
            else {
                unreachable!()
            };
            *psi_operation = OperationId::new(62_120).unwrap();
        }),
        StraightLineIntegerLiteralSequenceUnitReturnTranslationError::TargetProvenance
    );
    for mutate in [
        |operation: &mut AbstractOperation| {
            let AbstractOperation::IntegerConstant { result, .. } = operation else {
                unreachable!()
            };
            *result = ValueId::new(62_121).unwrap();
        },
        |operation: &mut AbstractOperation| {
            let AbstractOperation::IntegerConstant { scalar_type, .. } = operation else {
                unreachable!()
            };
            *scalar_type =
                ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).unwrap());
        },
        |operation: &mut AbstractOperation| {
            let AbstractOperation::IntegerConstant { value, .. } = operation else {
                unreachable!()
            };
            *value = IntegerValue::Unsigned(1);
        },
    ] {
        assert_eq!(
            leaf_error(|function| mutate(&mut function.operations[1])),
            StraightLineIntegerLiteralSequenceUnitReturnTranslationError::TargetConstant
        );
    }
}
