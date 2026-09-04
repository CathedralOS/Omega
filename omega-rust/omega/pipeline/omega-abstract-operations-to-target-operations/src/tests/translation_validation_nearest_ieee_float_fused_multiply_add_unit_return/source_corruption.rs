use super::*;

#[test]
fn source_envelope_corruption_fails_closed() {
    assert_eq!(
        leaf_error(|function| function.parameters.push(AbstractParameter {
            value: ValueId::new(61_101).unwrap(),
            scalar_type: ScalarType::Boolean,
        })),
        StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturnTranslationError::SourceParameters
    );
    assert_eq!(
        leaf_error(|function| function.structural_parameters.push(StructuralParameterDeclaration {
            place: PlaceId::new(61_102).unwrap(),
            position: 0,
            is_self: false,
            structural_type: StructuralTypeId::new(61_012).unwrap(),
            multiplicity: StructuralMultiplicity::Affine,
            access: StructuralAccess::Owned,
            qualifications: vec![StructuralDomainId::new(61_103).unwrap()],
            projected_qualifications: Vec::new(),
        })),
        StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturnTranslationError::SourceStructuralParameters
    );
    assert_eq!(
        leaf_error(
            |function| function.result = AbstractFunctionResult::Scalar(AbstractResult {
                value: ValueId::new(61_104).unwrap(),
                scalar_type: ScalarType::Boolean,
            })
        ),
        StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturnTranslationError::SourceResult
    );
    assert_eq!(
        leaf_error(|function| function.entry_claims.push(EntryClaim {
            claim: ClaimId::new(61_105).unwrap(),
            input: PlaceId::new(61_106).unwrap(),
            path: Vec::new(),
        })),
        StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturnTranslationError::SourceEntryClaims
    );
    assert_eq!(
        leaf_error(|function| function.published_service_ceiling.push(ServiceId::new(61_107).unwrap())),
        StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturnTranslationError::SourcePublishedServices
    );
    for mutate in [
        |function: &mut AbstractFunction| function.block_entries.clear(),
        |function: &mut AbstractFunction| {
            function.block_entries[0].block = BlockId::new(61_108).unwrap()
        },
        |function: &mut AbstractFunction| function.block_entries[0].operation_offset = 1,
    ] {
        assert_eq!(
            leaf_error(mutate),
            StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturnTranslationError::SourceBlockRoster
        );
    }
}

#[test]
fn source_grammar_operand_and_return_corruption_fails_closed() {
    for mutate in [
        |function: &mut AbstractFunction| {
            function.operations.remove(0);
        },
        |function: &mut AbstractFunction| {
            function.operations.swap(0, 3);
        },
        |function: &mut AbstractFunction| {
            function.operations.push(AbstractOperation::ReturnUnit {
                psi_edge: EdgeId::new(61_109).unwrap(),
                cleanup_actions: Vec::new(),
            });
        },
    ] {
        assert_eq!(
            leaf_error(mutate),
            StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturnTranslationError::SourceOperationRoster
        );
    }
    for position in 0..3 {
        assert_eq!(
            leaf_error(|function| {
                let AbstractOperation::NearestIeeeFloatFusedMultiplyAdd {
                    left,
                    right,
                    addend,
                    ..
                } = &mut function.operations[3]
                else {
                    unreachable!()
                };
                *[left, right, addend][position] = ValueId::new(61_110 + position as u64).unwrap();
            }),
            StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturnTranslationError::SourceOperand
        );
    }
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::ReturnUnit { cleanup_actions, .. } = &mut function.operations[4] else { unreachable!() };
            cleanup_actions.push(TerminalAffineCleanupAction::DiscardRoot(PlaceId::new(61_115).unwrap()));
        }),
        StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturnTranslationError::SourceCleanupActions
    );
}

#[test]
fn source_identity_format_and_raw_bit_drift_rejects_original_target() {
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::IeeeFloatConstant { psi_operation, .. } =
                &mut function.operations[0]
            else {
                unreachable!()
            };
            *psi_operation = OperationId::new(61_120).unwrap();
        }),
        StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturnTranslationError::TargetProvenance
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::IeeeFloatConstant { value, .. } = &mut function.operations[1]
            else {
                unreachable!()
            };
            *value = IeeeFloatValue::Binary32(0x7fc1_2346);
        }),
        StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturnTranslationError::TargetConstant
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::NearestIeeeFloatFusedMultiplyAdd { format, .. } =
                &mut function.operations[3]
            else {
                unreachable!()
            };
            *format = IeeeFloatFormat::Binary64;
        }),
        StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturnTranslationError::SourceOperand
    );
}

#[test]
fn settlement_input_corruption_has_complete_typed_failures() {
    let format = IeeeFloatFormat::Binary32;
    let target = NativeTarget::linux_x64();
    let source = base_plan(format);
    let valid_plan = provider_plan(target, format);
    let valid = settlement(target, format, &valid_plan);
    let lowered = lowered(&source, target, &[valid]);
    let validate = |settlements: &[AdmittedIeeeFloatFmaSettlement<'_>]| {
        crate::validation::straight_line_nearest_ieee_float_fused_multiply_add_unit_return::validate(
            &source.functions[0], target, &lowered.functions[0], settlements,
        ).unwrap_err()
    };
    assert_eq!(
        validate(&[]),
        StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturnTranslationError::SettlementRoster
    );
    assert_eq!(
        validate(&[valid, valid]),
        StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturnTranslationError::SettlementRoster
    );
    assert_eq!(
        validate(&[AdmittedIeeeFloatFmaSettlement {
            format: IeeeFloatFormat::Binary64,
            ..valid
        }]),
        StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturnTranslationError::SettlementFormat
    );
    assert_eq!(
        validate(&[AdmittedIeeeFloatFmaSettlement {
            slot: X86ScalarFmaSlot::Binary64,
            ..valid
        }]),
        StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturnTranslationError::SettlementSlot
    );
    let windows_provider = omega_target::AdmittedX86ScalarFmaProvider::from_deployment_claim(
        TargetProfile::WindowsX64,
        &X86_SCALAR_FMA_REQUIRED_FEATURES,
    )
    .unwrap();
    assert_eq!(
        validate(&[AdmittedIeeeFloatFmaSettlement {
            provider: windows_provider,
            ..valid
        }]),
        StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturnTranslationError::SettlementProvider
    );

    let mut wrong_target_plan = valid_plan.clone();
    wrong_target_plan.target = TargetProfile::WindowsX64.target_name().into();
    assert_eq!(validate(&[AdmittedIeeeFloatFmaSettlement { provider_plan: &wrong_target_plan, ..valid }]), StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturnTranslationError::SettlementPlanTarget);
    let mut wrong_row_plan = valid_plan.clone();
    wrong_row_plan.rows[0].binding = ProviderBinding::Syscall { number: 1 };
    assert_eq!(
        validate(&[AdmittedIeeeFloatFmaSettlement {
            provider_plan: &wrong_row_plan,
            ..valid
        }]),
        StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturnTranslationError::SettlementPlanRow
    );
}
