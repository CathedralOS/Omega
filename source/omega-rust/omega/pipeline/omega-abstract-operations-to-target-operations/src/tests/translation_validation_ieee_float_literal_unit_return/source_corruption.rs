use super::*;

#[test]
fn ieee_literal_source_envelope_corruption_fails_closed() {
    assert_eq!(
        leaf_error(|function| {
            function.parameters.push(AbstractParameter {
                value: ValueId::new(59_101).unwrap(),
                scalar_type: ScalarType::Boolean,
            });
        }),
        StraightLineIeeeFloatLiteralUnitReturnTranslationError::SourceParameters
    );
    assert_eq!(
        leaf_error(|function| {
            function
                .structural_parameters
                .push(StructuralParameterDeclaration {
                    place: PlaceId::new(59_102).unwrap(),
                    position: 0,
                    is_self: false,
                    structural_type: StructuralTypeId::new(59_009).unwrap(),
                    multiplicity: StructuralMultiplicity::Affine,
                    access: StructuralAccess::Owned,
                    qualifications: vec![StructuralDomainId::new(59_103).unwrap()],
                    projected_qualifications: Vec::new(),
                });
        }),
        StraightLineIeeeFloatLiteralUnitReturnTranslationError::SourceStructuralParameters
    );
    assert_eq!(
        leaf_error(|function| {
            function.result = AbstractFunctionResult::Scalar(AbstractResult {
                value: ValueId::new(59_104).unwrap(),
                scalar_type: ScalarType::Boolean,
            });
        }),
        StraightLineIeeeFloatLiteralUnitReturnTranslationError::SourceResult
    );
    assert_eq!(
        leaf_error(|function| {
            function.entry_claims.push(EntryClaim {
                claim: ClaimId::new(59_105).unwrap(),
                input: PlaceId::new(59_106).unwrap(),
                path: Vec::new(),
            });
        }),
        StraightLineIeeeFloatLiteralUnitReturnTranslationError::SourceEntryClaims
    );
    assert_eq!(
        leaf_error(|function| {
            function
                .published_service_ceiling
                .push(ServiceId::new(59_107).unwrap());
        }),
        StraightLineIeeeFloatLiteralUnitReturnTranslationError::SourcePublishedServices
    );
    for mutate in [
        |function: &mut AbstractFunction| function.block_entries.clear(),
        |function: &mut AbstractFunction| {
            function.block_entries[0].block = BlockId::new(59_108).unwrap()
        },
        |function: &mut AbstractFunction| {
            function.block_entries[0]
                .parameters
                .push(AbstractParameter {
                    value: ValueId::new(59_109).unwrap(),
                    scalar_type: ScalarType::Boolean,
                })
        },
        |function: &mut AbstractFunction| function.block_entries[0].operation_offset = 1,
    ] {
        assert_eq!(
            leaf_error(mutate),
            StraightLineIeeeFloatLiteralUnitReturnTranslationError::SourceBlockRoster
        );
    }
}

#[test]
fn ieee_literal_source_semantic_and_identity_corruption_fails_closed() {
    for mutate in [
        |function: &mut AbstractFunction| {
            function.operations.remove(0);
        },
        |function: &mut AbstractFunction| {
            function.operations.swap(0, 1);
        },
        |function: &mut AbstractFunction| {
            function.operations.push(AbstractOperation::ReturnUnit {
                psi_edge: EdgeId::new(59_110).unwrap(),
                cleanup_actions: Vec::new(),
            });
        },
    ] {
        assert_eq!(
            leaf_error(mutate),
            StraightLineIeeeFloatLiteralUnitReturnTranslationError::SourceOperationRoster
        );
    }
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::ReturnUnit {
                cleanup_actions, ..
            } = &mut function.operations[1]
            else {
                unreachable!()
            };
            cleanup_actions.push(TerminalAffineCleanupAction::DiscardRoot(
                PlaceId::new(59_111).unwrap(),
            ));
        }),
        StraightLineIeeeFloatLiteralUnitReturnTranslationError::SourceCleanupActions
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::IeeeFloatConstant { psi_operation, .. } =
                &mut function.operations[0]
            else {
                unreachable!()
            };
            *psi_operation = OperationId::new(59_112).unwrap();
        }),
        StraightLineIeeeFloatLiteralUnitReturnTranslationError::TargetProvenance
    );
    for mutate in [
        |operation: &mut AbstractOperation| {
            let AbstractOperation::IeeeFloatConstant { result, .. } = operation else {
                unreachable!()
            };
            *result = ValueId::new(59_113).unwrap();
        },
        |operation: &mut AbstractOperation| {
            let AbstractOperation::IeeeFloatConstant { value, .. } = operation else {
                unreachable!()
            };
            *value = IeeeFloatValue::Binary32(0x7fc1_2345);
        },
    ] {
        assert_eq!(
            leaf_error(|function| mutate(&mut function.operations[0])),
            StraightLineIeeeFloatLiteralUnitReturnTranslationError::TargetConstant
        );
    }
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::ReturnUnit { psi_edge, .. } = &mut function.operations[1] else {
                unreachable!()
            };
            *psi_edge = EdgeId::new(59_114).unwrap();
        }),
        StraightLineIeeeFloatLiteralUnitReturnTranslationError::TargetProvenance
    );
}
