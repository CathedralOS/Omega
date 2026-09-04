use super::*;

#[test]
fn byte_sequence_literal_source_envelope_corruption_fails_closed() {
    assert_eq!(
        leaf_error(|function| {
            function.parameters.push(AbstractParameter {
                value: ValueId::new(57_101).unwrap(),
                scalar_type: ScalarType::Boolean,
            });
        }),
        StraightLineByteSequenceLiteralUnitReturnTranslationError::SourceParameters
    );
    assert_eq!(
        leaf_error(|function| {
            function
                .structural_parameters
                .push(StructuralParameterDeclaration {
                    place: PlaceId::new(57_102).unwrap(),
                    position: 0,
                    is_self: false,
                    structural_type: StructuralTypeId::new(57_103).unwrap(),
                    multiplicity: StructuralMultiplicity::Affine,
                    access: StructuralAccess::Owned,
                    qualifications: vec![StructuralDomainId::new(57_104).unwrap()],
                    projected_qualifications: Vec::new(),
                });
        }),
        StraightLineByteSequenceLiteralUnitReturnTranslationError::SourceStructuralParameters
    );
    assert_eq!(
        leaf_error(|function| {
            function.result = AbstractFunctionResult::Scalar(AbstractResult {
                value: ValueId::new(57_105).unwrap(),
                scalar_type: ScalarType::Boolean,
            });
        }),
        StraightLineByteSequenceLiteralUnitReturnTranslationError::SourceResult
    );
    assert_eq!(
        leaf_error(|function| {
            function.entry_claims.push(EntryClaim {
                claim: ClaimId::new(57_106).unwrap(),
                input: PlaceId::new(57_107).unwrap(),
                path: Vec::new(),
            });
        }),
        StraightLineByteSequenceLiteralUnitReturnTranslationError::SourceEntryClaims
    );
    assert_eq!(
        leaf_error(|function| {
            function
                .published_service_ceiling
                .push(ServiceId::new(57_108).unwrap());
        }),
        StraightLineByteSequenceLiteralUnitReturnTranslationError::SourcePublishedServices
    );
    for mutate in [
        |function: &mut AbstractFunction| function.block_entries.clear(),
        |function: &mut AbstractFunction| {
            function.block_entries[0].block = BlockId::new(57_109).unwrap()
        },
        |function: &mut AbstractFunction| {
            function.block_entries[0]
                .parameters
                .push(AbstractParameter {
                    value: ValueId::new(57_110).unwrap(),
                    scalar_type: ScalarType::Boolean,
                })
        },
        |function: &mut AbstractFunction| function.block_entries[0].operation_offset = 1,
    ] {
        assert_eq!(
            leaf_error(mutate),
            StraightLineByteSequenceLiteralUnitReturnTranslationError::SourceBlockRoster
        );
    }
}

#[test]
fn byte_sequence_literal_source_semantic_corruption_fails_closed() {
    for mutate in [
        |function: &mut AbstractFunction| {
            function.operations.remove(0);
        },
        |function: &mut AbstractFunction| {
            function.operations.swap(0, 1);
        },
        |function: &mut AbstractFunction| {
            function.operations.push(AbstractOperation::ReturnUnit {
                psi_edge: EdgeId::new(57_111).unwrap(),
                cleanup_actions: Vec::new(),
            });
        },
    ] {
        assert_eq!(
            leaf_error(mutate),
            StraightLineByteSequenceLiteralUnitReturnTranslationError::SourceOperationRoster
        );
    }
    for mutate in [
        |place: &mut StructuralPlaceDeclaration| {
            place.kind = StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            };
        },
        |place: &mut StructuralPlaceDeclaration| {
            let StructuralPlaceKind::ByteSequenceLiteral {
                declaration_ordinal,
                ..
            } = &mut place.kind
            else {
                unreachable!()
            };
            *declaration_ordinal = 1;
        },
        |place: &mut StructuralPlaceDeclaration| {
            let StructuralPlaceKind::ByteSequenceLiteral {
                structural_type, ..
            } = &mut place.kind
            else {
                unreachable!()
            };
            *structural_type = StructuralTypeId::new(57_112).unwrap();
        },
    ] {
        assert_eq!(
            leaf_error(|function| {
                let AbstractOperation::EstablishByteSequenceLiteral { place, .. } =
                    &mut function.operations[0]
                else {
                    unreachable!()
                };
                mutate(place);
            }),
            StraightLineByteSequenceLiteralUnitReturnTranslationError::SourcePlace
        );
    }
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::EstablishByteSequenceLiteral {
                structural_type, ..
            } = &mut function.operations[0]
            else {
                unreachable!()
            };
            structural_type.shape = StructuralTypeShape::Record { fields: Vec::new() };
        }),
        StraightLineByteSequenceLiteralUnitReturnTranslationError::SourceStructuralType
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::ReturnUnit {
                cleanup_actions, ..
            } = &mut function.operations[1]
            else {
                unreachable!()
            };
            cleanup_actions.push(TerminalAffineCleanupAction::DiscardRoot(place_id()));
        }),
        StraightLineByteSequenceLiteralUnitReturnTranslationError::SourceCleanupActions
    );
}
