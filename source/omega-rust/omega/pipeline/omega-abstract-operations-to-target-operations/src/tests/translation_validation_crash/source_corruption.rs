use super::*;

#[test]
fn crash_source_envelope_corruption_fails_closed() {
    assert_eq!(
        leaf_error(|function| {
            function.parameters.push(AbstractParameter {
                value: ValueId::new(2_101).unwrap(),
                scalar_type: ScalarType::Boolean,
            });
        }),
        StraightLineScalarCrashTranslationError::SourceParameters
    );
    assert_eq!(
        leaf_error(|function| {
            function
                .structural_parameters
                .push(StructuralParameterDeclaration {
                    place: PlaceId::new(2_102).unwrap(),
                    position: 0,
                    is_self: false,
                    structural_type: StructuralTypeId::new(2_103).unwrap(),
                    multiplicity: StructuralMultiplicity::Affine,
                    access: StructuralAccess::Owned,
                    qualifications: vec![StructuralDomainId::new(2_104).unwrap()],
                    projected_qualifications: Vec::new(),
                });
        }),
        StraightLineScalarCrashTranslationError::SourceStructuralParameters
    );
    assert_eq!(
        leaf_error(|function| function.result = AbstractFunctionResult::Unit),
        StraightLineScalarCrashTranslationError::SourceResult
    );
    assert_eq!(
        leaf_error(|function| {
            function.entry_claims.push(EntryClaim {
                claim: ClaimId::new(2_105).unwrap(),
                input: PlaceId::new(2_106).unwrap(),
                path: Vec::new(),
            });
        }),
        StraightLineScalarCrashTranslationError::SourceEntryClaims
    );
    assert_eq!(
        leaf_error(|function| {
            function
                .published_service_ceiling
                .push(ServiceId::new(2_107).unwrap());
        }),
        StraightLineScalarCrashTranslationError::SourcePublishedServices
    );
    for mutate in [
        |function: &mut AbstractFunction| function.block_entries.clear(),
        |function: &mut AbstractFunction| {
            function.block_entries[0].block = BlockId::new(2_108).unwrap()
        },
        |function: &mut AbstractFunction| {
            function.block_entries[0]
                .parameters
                .push(AbstractParameter {
                    value: ValueId::new(2_109).unwrap(),
                    scalar_type: ScalarType::Boolean,
                })
        },
        |function: &mut AbstractFunction| function.block_entries[0].operation_offset = 1,
    ] {
        assert_eq!(
            leaf_error(mutate),
            StraightLineScalarCrashTranslationError::SourceBlockRoster
        );
    }
    assert_eq!(
        leaf_error(|function| function.operations.clear()),
        StraightLineScalarCrashTranslationError::SourceOperationRoster
    );
    assert_eq!(
        leaf_error(|function| {
            function.operations.insert(
                0,
                AbstractOperation::BooleanConstant {
                    psi_operation: OperationId::new(2_110).unwrap(),
                    result: ValueId::new(2_111).unwrap(),
                    value: true,
                },
            );
        }),
        StraightLineScalarCrashTranslationError::SourceOperationRoster
    );
    assert_eq!(
        leaf_error(|function| {
            function.operations[0] = AbstractOperation::Return {
                psi_edge: EdgeId::new(2_004).unwrap(),
                result: ValueId::new(2_003).unwrap(),
                value: ValueId::new(2_112).unwrap(),
                scalar_type: ScalarType::Boolean,
                cleanup_actions: Vec::new(),
            };
        }),
        StraightLineScalarCrashTranslationError::SourceOperationRoster
    );
}
