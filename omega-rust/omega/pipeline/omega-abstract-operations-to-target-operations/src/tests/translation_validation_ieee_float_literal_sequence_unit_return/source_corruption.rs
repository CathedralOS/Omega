use super::*;

#[test]
fn source_envelope_corruption_fails_closed() {
    assert_eq!(
        leaf_error(|function| {
            function.parameters.push(AbstractParameter {
                value: ValueId::new(60_101).unwrap(),
                scalar_type: ScalarType::Boolean,
            });
        }),
        StraightLineIeeeFloatLiteralSequenceUnitReturnTranslationError::SourceParameters
    );
    assert_eq!(
        leaf_error(|function| {
            function
                .structural_parameters
                .push(StructuralParameterDeclaration {
                    place: PlaceId::new(60_102).unwrap(),
                    position: 0,
                    is_self: false,
                    structural_type: StructuralTypeId::new(60_010).unwrap(),
                    multiplicity: StructuralMultiplicity::Affine,
                    access: StructuralAccess::Owned,
                    qualifications: vec![StructuralDomainId::new(60_103).unwrap()],
                    projected_qualifications: Vec::new(),
                });
        }),
        StraightLineIeeeFloatLiteralSequenceUnitReturnTranslationError::SourceStructuralParameters
    );
    assert_eq!(
        leaf_error(|function| {
            function.result = AbstractFunctionResult::Scalar(AbstractResult {
                value: ValueId::new(60_104).unwrap(),
                scalar_type: ScalarType::Boolean,
            });
        }),
        StraightLineIeeeFloatLiteralSequenceUnitReturnTranslationError::SourceResult
    );
    assert_eq!(
        leaf_error(|function| {
            function.entry_claims.push(EntryClaim {
                claim: ClaimId::new(60_105).unwrap(),
                input: PlaceId::new(60_106).unwrap(),
                path: Vec::new(),
            });
        }),
        StraightLineIeeeFloatLiteralSequenceUnitReturnTranslationError::SourceEntryClaims
    );
    assert_eq!(
        leaf_error(|function| {
            function
                .published_service_ceiling
                .push(ServiceId::new(60_107).unwrap());
        }),
        StraightLineIeeeFloatLiteralSequenceUnitReturnTranslationError::SourcePublishedServices
    );
    for mutate in [
        |function: &mut AbstractFunction| function.block_entries.clear(),
        |function: &mut AbstractFunction| {
            function.block_entries[0].block = BlockId::new(60_108).unwrap()
        },
        |function: &mut AbstractFunction| {
            function.block_entries[0]
                .parameters
                .push(AbstractParameter {
                    value: ValueId::new(60_109).unwrap(),
                    scalar_type: ScalarType::Boolean,
                })
        },
        |function: &mut AbstractFunction| function.block_entries[0].operation_offset = 1,
    ] {
        assert_eq!(
            leaf_error(mutate),
            StraightLineIeeeFloatLiteralSequenceUnitReturnTranslationError::SourceBlockRoster
        );
    }
}

#[test]
fn source_sequence_and_return_corruption_fails_closed() {
    for mutate in [
        |function: &mut AbstractFunction| {
            function.operations.drain(1..LITERALS.len());
        },
        |function: &mut AbstractFunction| {
            function.operations[1] = AbstractOperation::IntegerConstant {
                psi_operation: OperationId::new(60_110).unwrap(),
                result: ValueId::new(60_111).unwrap(),
                scalar_type: ScalarType::Integer(
                    IntegerType::new(IntegerSign::Signed, 32).unwrap(),
                ),
                value: psi_core::IntegerValue::Signed(1),
            };
        },
        |function: &mut AbstractFunction| {
            function.operations.swap(0, LITERALS.len());
        },
    ] {
        assert_eq!(
            leaf_error(mutate),
            StraightLineIeeeFloatLiteralSequenceUnitReturnTranslationError::SourceOperationRoster
        );
    }
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::ReturnUnit {
                cleanup_actions, ..
            } = function.operations.last_mut().unwrap()
            else {
                unreachable!()
            };
            cleanup_actions.push(TerminalAffineCleanupAction::DiscardRoot(
                PlaceId::new(60_112).unwrap(),
            ));
        }),
        StraightLineIeeeFloatLiteralSequenceUnitReturnTranslationError::SourceCleanupActions
    );
}

#[test]
fn source_identity_or_raw_bits_drift_rejects_against_the_original_target() {
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::IeeeFloatConstant { psi_operation, .. } =
                &mut function.operations[1]
            else {
                unreachable!()
            };
            *psi_operation = OperationId::new(60_120).unwrap();
        }),
        StraightLineIeeeFloatLiteralSequenceUnitReturnTranslationError::TargetProvenance
    );
    for mutate in [
        |operation: &mut AbstractOperation| {
            let AbstractOperation::IeeeFloatConstant { result, .. } = operation else {
                unreachable!()
            };
            *result = ValueId::new(60_121).unwrap();
        },
        |operation: &mut AbstractOperation| {
            let AbstractOperation::IeeeFloatConstant { value, .. } = operation else {
                unreachable!()
            };
            *value = IeeeFloatValue::Binary64(0xfff0_0000_0000_0000);
        },
    ] {
        assert_eq!(
            leaf_error(|function| mutate(&mut function.operations[1])),
            StraightLineIeeeFloatLiteralSequenceUnitReturnTranslationError::TargetConstant
        );
    }
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::ReturnUnit { psi_edge, .. } =
                function.operations.last_mut().unwrap()
            else {
                unreachable!()
            };
            *psi_edge = EdgeId::new(60_122).unwrap();
        }),
        StraightLineIeeeFloatLiteralSequenceUnitReturnTranslationError::TargetProvenance
    );
}
