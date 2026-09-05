use super::*;

#[test]
fn integer_literal_source_envelope_corruption_fails_closed() {
    assert_eq!(
        leaf_error(|function| {
            function.parameters.push(AbstractParameter {
                value: ValueId::new(58_101).unwrap(),
                scalar_type: ScalarType::Boolean,
            });
        }),
        StraightLineIntegerLiteralUnitReturnTranslationError::SourceParameters
    );
    assert_eq!(
        leaf_error(|function| {
            function
                .structural_parameters
                .push(StructuralParameterDeclaration {
                    place: PlaceId::new(58_102).unwrap(),
                    position: 0,
                    is_self: false,
                    structural_type: StructuralTypeId::new(58_009).unwrap(),
                    multiplicity: StructuralMultiplicity::Affine,
                    access: StructuralAccess::Owned,
                    qualifications: vec![StructuralDomainId::new(58_103).unwrap()],
                    projected_qualifications: Vec::new(),
                });
        }),
        StraightLineIntegerLiteralUnitReturnTranslationError::SourceStructuralParameters
    );
    assert_eq!(
        leaf_error(|function| {
            function.result = AbstractFunctionResult::Scalar(AbstractResult {
                value: ValueId::new(58_104).unwrap(),
                scalar_type: ScalarType::Boolean,
            });
        }),
        StraightLineIntegerLiteralUnitReturnTranslationError::SourceResult
    );
    assert_eq!(
        leaf_error(|function| {
            function.entry_claims.push(EntryClaim {
                claim: ClaimId::new(58_105).unwrap(),
                input: PlaceId::new(58_106).unwrap(),
                path: Vec::new(),
            });
        }),
        StraightLineIntegerLiteralUnitReturnTranslationError::SourceEntryClaims
    );
    assert_eq!(
        leaf_error(|function| {
            function
                .published_service_ceiling
                .push(ServiceId::new(58_107).unwrap());
        }),
        StraightLineIntegerLiteralUnitReturnTranslationError::SourcePublishedServices
    );
    for mutate in [
        |function: &mut AbstractFunction| function.block_entries.clear(),
        |function: &mut AbstractFunction| {
            function.block_entries[0].block = BlockId::new(58_108).unwrap()
        },
        |function: &mut AbstractFunction| {
            function.block_entries[0]
                .parameters
                .push(AbstractParameter {
                    value: ValueId::new(58_109).unwrap(),
                    scalar_type: ScalarType::Boolean,
                })
        },
        |function: &mut AbstractFunction| function.block_entries[0].operation_offset = 1,
    ] {
        assert_eq!(
            leaf_error(mutate),
            StraightLineIntegerLiteralUnitReturnTranslationError::SourceBlockRoster
        );
    }
}

#[test]
fn integer_literal_source_semantic_corruption_fails_closed() {
    for mutate in [
        |function: &mut AbstractFunction| {
            function.operations.remove(0);
        },
        |function: &mut AbstractFunction| {
            function.operations.swap(0, 1);
        },
        |function: &mut AbstractFunction| {
            function.operations.push(AbstractOperation::ReturnUnit {
                psi_edge: EdgeId::new(58_110).unwrap(),
                cleanup_actions: Vec::new(),
            });
        },
    ] {
        assert_eq!(
            leaf_error(mutate),
            StraightLineIntegerLiteralUnitReturnTranslationError::SourceOperationRoster
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
        StraightLineIntegerLiteralUnitReturnTranslationError::SourceConstantType
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::IntegerConstant {
                scalar_type, value, ..
            } = &mut function.operations[0]
            else {
                unreachable!()
            };
            *scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 3).unwrap());
            *value = IntegerValue::Signed(7);
        }),
        StraightLineIntegerLiteralUnitReturnTranslationError::SourceConstantOutsideType
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::ReturnUnit {
                cleanup_actions, ..
            } = &mut function.operations[1]
            else {
                unreachable!()
            };
            cleanup_actions.push(TerminalAffineCleanupAction::DiscardRoot(
                PlaceId::new(58_111).unwrap(),
            ));
        }),
        StraightLineIntegerLiteralUnitReturnTranslationError::SourceCleanupActions
    );
}

#[test]
fn integer_literal_source_identity_drift_fails_against_the_fixed_target() {
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::IntegerConstant { psi_operation, .. } =
                &mut function.operations[0]
            else {
                unreachable!()
            };
            *psi_operation = OperationId::new(58_112).unwrap();
        }),
        StraightLineIntegerLiteralUnitReturnTranslationError::TargetProvenance
    );
    for mutate in [
        |operation: &mut AbstractOperation| {
            let AbstractOperation::IntegerConstant { result, .. } = operation else {
                unreachable!()
            };
            *result = ValueId::new(58_113).unwrap();
        },
        |operation: &mut AbstractOperation| {
            let AbstractOperation::IntegerConstant { scalar_type, .. } = operation else {
                unreachable!()
            };
            *scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 38).unwrap());
        },
        |operation: &mut AbstractOperation| {
            let AbstractOperation::IntegerConstant { value, .. } = operation else {
                unreachable!()
            };
            *value = IntegerValue::Signed(-4_000_002);
        },
    ] {
        assert_eq!(
            leaf_error(|function| mutate(&mut function.operations[0])),
            StraightLineIntegerLiteralUnitReturnTranslationError::TargetConstant
        );
    }
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::ReturnUnit { psi_edge, .. } = &mut function.operations[1] else {
                unreachable!()
            };
            *psi_edge = EdgeId::new(58_114).unwrap();
        }),
        StraightLineIntegerLiteralUnitReturnTranslationError::TargetProvenance
    );
}
