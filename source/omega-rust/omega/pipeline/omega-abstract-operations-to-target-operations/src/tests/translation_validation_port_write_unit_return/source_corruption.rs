use super::*;

#[test]
fn port_write_unit_return_source_envelope_corruption_fails_closed() {
    assert_eq!(
        leaf_error(|function| {
            function.parameters.push(AbstractParameter {
                value: ValueId::new(54_101).unwrap(),
                scalar_type: ScalarType::Boolean,
            });
        }),
        StraightLinePortWriteUnitReturnTranslationError::SourceParameters
    );
    assert_eq!(
        leaf_error(|function| {
            function
                .structural_parameters
                .push(StructuralParameterDeclaration {
                    place: PlaceId::new(54_102).unwrap(),
                    position: 0,
                    is_self: false,
                    structural_type: StructuralTypeId::new(54_103).unwrap(),
                    multiplicity: StructuralMultiplicity::Affine,
                    access: StructuralAccess::Owned,
                    qualifications: vec![StructuralDomainId::new(54_104).unwrap()],
                    projected_qualifications: Vec::new(),
                });
        }),
        StraightLinePortWriteUnitReturnTranslationError::SourceStructuralParameters
    );
    assert_eq!(
        leaf_error(|function| {
            function.result = AbstractFunctionResult::Scalar(AbstractResult {
                value: ValueId::new(54_105).unwrap(),
                scalar_type: ScalarType::Boolean,
            });
        }),
        StraightLinePortWriteUnitReturnTranslationError::SourceResult
    );
    assert_eq!(
        leaf_error(|function| {
            function.entry_claims.push(EntryClaim {
                claim: ClaimId::new(54_106).unwrap(),
                input: PlaceId::new(54_107).unwrap(),
                path: Vec::new(),
            });
        }),
        StraightLinePortWriteUnitReturnTranslationError::SourceEntryClaims
    );
    assert_eq!(
        leaf_error(|function| function.published_service_ceiling.clear()),
        StraightLinePortWriteUnitReturnTranslationError::SourcePublishedServices
    );
    for mutate in [
        |function: &mut AbstractFunction| function.block_entries.clear(),
        |function: &mut AbstractFunction| {
            function.block_entries[0].block = BlockId::new(54_108).unwrap()
        },
        |function: &mut AbstractFunction| {
            function.block_entries[0]
                .parameters
                .push(AbstractParameter {
                    value: ValueId::new(54_109).unwrap(),
                    scalar_type: ScalarType::Boolean,
                })
        },
        |function: &mut AbstractFunction| function.block_entries[0].operation_offset = 1,
    ] {
        assert_eq!(
            leaf_error(mutate),
            StraightLinePortWriteUnitReturnTranslationError::SourceBlockRoster
        );
    }
    assert_eq!(
        leaf_error(|function| {
            function.operations.remove(0);
        }),
        StraightLinePortWriteUnitReturnTranslationError::SourceOperationRoster
    );
    assert_eq!(
        leaf_error(|function| {
            function.operations.swap(0, 1);
        }),
        StraightLinePortWriteUnitReturnTranslationError::SourceOperationRoster
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
                PlaceId::new(54_110).unwrap(),
            ));
        }),
        StraightLinePortWriteUnitReturnTranslationError::SourceCleanupActions
    );
}
