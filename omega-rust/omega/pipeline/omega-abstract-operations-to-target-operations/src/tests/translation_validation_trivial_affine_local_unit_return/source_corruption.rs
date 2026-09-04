use super::*;

#[test]
fn trivial_affine_local_source_envelope_corruption_fails_closed() {
    assert_eq!(
        leaf_error(|function| {
            function.parameters.push(AbstractParameter {
                value: ValueId::new(56_101).unwrap(),
                scalar_type: ScalarType::Boolean,
            });
        }),
        StraightLineTrivialAffineLocalUnitReturnTranslationError::SourceParameters
    );
    assert_eq!(
        leaf_error(|function| {
            function
                .structural_parameters
                .push(StructuralParameterDeclaration {
                    place: PlaceId::new(56_102).unwrap(),
                    position: 0,
                    is_self: false,
                    structural_type: StructuralTypeId::new(56_103).unwrap(),
                    multiplicity: StructuralMultiplicity::Affine,
                    access: StructuralAccess::Owned,
                    qualifications: vec![StructuralDomainId::new(56_104).unwrap()],
                    projected_qualifications: Vec::new(),
                });
        }),
        StraightLineTrivialAffineLocalUnitReturnTranslationError::SourceStructuralParameters
    );
    assert_eq!(
        leaf_error(|function| {
            function.result = AbstractFunctionResult::Scalar(AbstractResult {
                value: ValueId::new(56_105).unwrap(),
                scalar_type: ScalarType::Boolean,
            });
        }),
        StraightLineTrivialAffineLocalUnitReturnTranslationError::SourceResult
    );
    assert_eq!(
        leaf_error(|function| {
            function.entry_claims.push(EntryClaim {
                claim: ClaimId::new(56_106).unwrap(),
                input: PlaceId::new(56_107).unwrap(),
                path: Vec::new(),
            });
        }),
        StraightLineTrivialAffineLocalUnitReturnTranslationError::SourceEntryClaims
    );
    assert_eq!(
        leaf_error(|function| {
            function
                .published_service_ceiling
                .push(ServiceId::new(56_108).unwrap());
        }),
        StraightLineTrivialAffineLocalUnitReturnTranslationError::SourcePublishedServices
    );
    for mutate in [
        |function: &mut AbstractFunction| function.block_entries.clear(),
        |function: &mut AbstractFunction| {
            function.block_entries[0].block = BlockId::new(56_109).unwrap()
        },
        |function: &mut AbstractFunction| {
            function.block_entries[0]
                .parameters
                .push(AbstractParameter {
                    value: ValueId::new(56_110).unwrap(),
                    scalar_type: ScalarType::Boolean,
                })
        },
        |function: &mut AbstractFunction| function.block_entries[0].operation_offset = 1,
    ] {
        assert_eq!(
            leaf_error(mutate),
            StraightLineTrivialAffineLocalUnitReturnTranslationError::SourceBlockRoster
        );
    }
}

#[test]
fn trivial_affine_local_source_semantic_corruption_fails_closed() {
    for mutate in [
        |function: &mut AbstractFunction| {
            function.operations.remove(0);
        },
        |function: &mut AbstractFunction| {
            function.operations.swap(0, 1);
        },
        |function: &mut AbstractFunction| {
            function.operations.push(AbstractOperation::ReturnUnit {
                psi_edge: EdgeId::new(56_111).unwrap(),
                cleanup_actions: Vec::new(),
            });
        },
    ] {
        assert_eq!(
            leaf_error(mutate),
            StraightLineTrivialAffineLocalUnitReturnTranslationError::SourceOperationRoster
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
            let StructuralPlaceKind::TrivialAffineLocal {
                declaration_ordinal,
                ..
            } = &mut place.kind
            else {
                unreachable!()
            };
            *declaration_ordinal = 1;
        },
        |place: &mut StructuralPlaceDeclaration| {
            let StructuralPlaceKind::TrivialAffineLocal {
                structural_type, ..
            } = &mut place.kind
            else {
                unreachable!()
            };
            *structural_type = StructuralTypeId::new(56_112).unwrap();
        },
    ] {
        assert_eq!(
            leaf_error(|function| {
                let AbstractOperation::EstablishTrivialAffineLocal { place, .. } =
                    &mut function.operations[0]
                else {
                    unreachable!()
                };
                mutate(place);
            }),
            StraightLineTrivialAffineLocalUnitReturnTranslationError::SourcePlace
        );
    }
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::EstablishTrivialAffineLocal {
                structural_type, ..
            } = &mut function.operations[0]
            else {
                unreachable!()
            };
            structural_type.shape =
                StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView);
        }),
        StraightLineTrivialAffineLocalUnitReturnTranslationError::SourceStructuralType
    );
    for cleanup_actions in [
        Vec::new(),
        vec![TerminalAffineCleanupAction::DiscardRoot(
            PlaceId::new(56_113).unwrap(),
        )],
        vec![
            TerminalAffineCleanupAction::DiscardRoot(place_id()),
            TerminalAffineCleanupAction::DiscardRoot(place_id()),
        ],
    ] {
        assert_eq!(
            leaf_error(|function| {
                let AbstractOperation::ReturnUnit {
                    cleanup_actions: target,
                    ..
                } = &mut function.operations[1]
                else {
                    unreachable!()
                };
                *target = cleanup_actions;
            }),
            StraightLineTrivialAffineLocalUnitReturnTranslationError::SourceCleanupActions
        );
    }
}
