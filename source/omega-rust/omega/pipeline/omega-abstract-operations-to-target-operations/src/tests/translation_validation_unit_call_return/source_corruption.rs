use super::*;

#[test]
fn unit_call_source_envelope_corruption_fails_closed() {
    assert_eq!(
        leaf_error(|function| {
            function.parameters.push(AbstractParameter {
                value: ValueId::new(55_101).unwrap(),
                scalar_type: ScalarType::Boolean,
            });
        }),
        StraightLineUnitCallReturnTranslationError::SourceParameters
    );
    assert_eq!(
        leaf_error(|function| {
            function
                .structural_parameters
                .push(StructuralParameterDeclaration {
                    place: PlaceId::new(55_102).unwrap(),
                    position: 0,
                    is_self: false,
                    structural_type: StructuralTypeId::new(55_103).unwrap(),
                    multiplicity: StructuralMultiplicity::Affine,
                    access: StructuralAccess::Owned,
                    qualifications: vec![StructuralDomainId::new(55_104).unwrap()],
                    projected_qualifications: Vec::new(),
                });
        }),
        StraightLineUnitCallReturnTranslationError::SourceStructuralParameters
    );
    assert_eq!(
        leaf_error(|function| {
            function.result = AbstractFunctionResult::Scalar(AbstractResult {
                value: ValueId::new(55_105).unwrap(),
                scalar_type: ScalarType::Boolean,
            });
        }),
        StraightLineUnitCallReturnTranslationError::SourceResult
    );
    assert_eq!(
        leaf_error(|function| {
            function.entry_claims.push(EntryClaim {
                claim: ClaimId::new(55_106).unwrap(),
                input: PlaceId::new(55_107).unwrap(),
                path: Vec::new(),
            });
        }),
        StraightLineUnitCallReturnTranslationError::SourceEntryClaims
    );
    assert_eq!(
        leaf_error(|function| {
            function
                .published_service_ceiling
                .push(ServiceId::new(55_108).unwrap());
        }),
        StraightLineUnitCallReturnTranslationError::SourcePublishedServices
    );
    for mutate in [
        |function: &mut AbstractFunction| function.block_entries.clear(),
        |function: &mut AbstractFunction| {
            function.block_entries[0].block = BlockId::new(55_109).unwrap()
        },
        |function: &mut AbstractFunction| {
            function.block_entries[0]
                .parameters
                .push(AbstractParameter {
                    value: ValueId::new(55_110).unwrap(),
                    scalar_type: ScalarType::Boolean,
                })
        },
        |function: &mut AbstractFunction| function.block_entries[0].operation_offset = 1,
    ] {
        assert_eq!(
            leaf_error(mutate),
            StraightLineUnitCallReturnTranslationError::SourceBlockRoster
        );
    }
}

#[test]
fn unit_call_source_operation_corruption_fails_closed() {
    for mutate in [
        |function: &mut AbstractFunction| {
            function.operations.remove(0);
        },
        |function: &mut AbstractFunction| {
            function.operations.swap(0, 1);
        },
        |function: &mut AbstractFunction| {
            function.operations.push(AbstractOperation::ReturnUnit {
                psi_edge: EdgeId::new(55_111).unwrap(),
                cleanup_actions: Vec::new(),
            });
        },
    ] {
        assert_eq!(
            leaf_error(mutate),
            StraightLineUnitCallReturnTranslationError::SourceOperationRoster
        );
    }
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::CallUnit {
                structural_arguments,
                ..
            } = &mut function.operations[0]
            else {
                unreachable!()
            };
            structural_arguments.push(StructuralArgument {
                place: PlaceId::new(55_112).unwrap(),
                access: StructuralAccess::Owned,
                path: Vec::new(),
            });
        }),
        StraightLineUnitCallReturnTranslationError::SourceStructuralArguments
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::CallUnit {
                claim_transfers, ..
            } = &mut function.operations[0]
            else {
                unreachable!()
            };
            claim_transfers.push(ClaimTransfer {
                claim: ClaimId::new(55_113).unwrap(),
                argument_index: 0,
            });
        }),
        StraightLineUnitCallReturnTranslationError::SourceClaimTransfers
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
                PlaceId::new(55_114).unwrap(),
            ));
        }),
        StraightLineUnitCallReturnTranslationError::SourceCleanupActions
    );
}
