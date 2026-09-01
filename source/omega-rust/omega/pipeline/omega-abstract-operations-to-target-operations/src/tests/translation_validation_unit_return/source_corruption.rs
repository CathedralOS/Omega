use super::*;

#[test]
fn unit_return_source_envelope_corruption_fails_closed() {
    assert_eq!(
        leaf_error(|function| {
            function.parameters.push(AbstractParameter {
                value: ValueId::new(53_101).unwrap(),
                scalar_type: ScalarType::Boolean,
            });
        }),
        StraightLineUnitReturnTranslationError::SourceParameters
    );
    assert_eq!(
        leaf_error(|function| {
            function
                .structural_parameters
                .push(StructuralParameterDeclaration {
                    place: PlaceId::new(53_102).unwrap(),
                    position: 0,
                    is_self: false,
                    structural_type: StructuralTypeId::new(53_009).unwrap(),
                    multiplicity: StructuralMultiplicity::Affine,
                    access: StructuralAccess::Owned,
                    qualifications: vec![StructuralDomainId::new(53_103).unwrap()],
                    projected_qualifications: Vec::new(),
                });
        }),
        StraightLineUnitReturnTranslationError::SourceStructuralParameters
    );
    assert_eq!(
        leaf_error(|function| {
            function.result = AbstractFunctionResult::Scalar(AbstractResult {
                value: ValueId::new(53_104).unwrap(),
                scalar_type: ScalarType::Boolean,
            });
        }),
        StraightLineUnitReturnTranslationError::SourceResult
    );
    assert_eq!(
        leaf_error(|function| {
            function.entry_claims.push(EntryClaim {
                claim: ClaimId::new(53_105).unwrap(),
                input: PlaceId::new(53_106).unwrap(),
                path: Vec::new(),
            });
        }),
        StraightLineUnitReturnTranslationError::SourceEntryClaims
    );
    assert_eq!(
        leaf_error(|function| {
            function
                .published_service_ceiling
                .push(ServiceId::new(53_107).unwrap());
        }),
        StraightLineUnitReturnTranslationError::SourcePublishedServices
    );
    for mutate in [
        |function: &mut AbstractFunction| function.block_entries.clear(),
        |function: &mut AbstractFunction| {
            function.block_entries[0].block = BlockId::new(53_108).unwrap()
        },
        |function: &mut AbstractFunction| {
            function.block_entries[0]
                .parameters
                .push(AbstractParameter {
                    value: ValueId::new(53_109).unwrap(),
                    scalar_type: ScalarType::Boolean,
                })
        },
        |function: &mut AbstractFunction| function.block_entries[0].operation_offset = 1,
    ] {
        assert_eq!(
            leaf_error(mutate),
            StraightLineUnitReturnTranslationError::SourceBlockRoster
        );
    }
    assert_eq!(
        leaf_error(|function| function.operations.clear()),
        StraightLineUnitReturnTranslationError::SourceOperationRoster
    );
    assert_eq!(
        leaf_error(|function| {
            function.operations.insert(
                0,
                AbstractOperation::BooleanConstant {
                    psi_operation: OperationId::new(53_110).unwrap(),
                    result: ValueId::new(53_111).unwrap(),
                    value: true,
                },
            );
        }),
        StraightLineUnitReturnTranslationError::SourceOperationRoster
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::ReturnUnit {
                cleanup_actions, ..
            } = &mut function.operations[0]
            else {
                unreachable!()
            };
            cleanup_actions.push(TerminalAffineCleanupAction::DiscardRoot(
                PlaceId::new(53_112).unwrap(),
            ));
        }),
        StraightLineUnitReturnTranslationError::SourceCleanupActions
    );
}
