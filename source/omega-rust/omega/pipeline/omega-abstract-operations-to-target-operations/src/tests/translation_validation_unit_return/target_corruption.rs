use super::*;

#[test]
fn unit_return_function_envelope_and_provenance_corruption_fails_closed() {
    assert_eq!(
        candidate_error(|candidate| {
            candidate.functions[0].fixed_integer_scalar_abi =
                Some(fixed_integer_scalar_abi(NativeTarget::linux_x64()));
        }),
        StraightLineUnitReturnTranslationError::TargetFixedIntegerScalarAbi
    );
    for provenance in [
        TerminalPsiProvenance::default(),
        TerminalPsiProvenance {
            operations: vec![OperationId::new(53_120).unwrap()],
            edges: vec![return_edge()],
        },
        TerminalPsiProvenance {
            operations: Vec::new(),
            edges: vec![EdgeId::new(53_121).unwrap()],
        },
        TerminalPsiProvenance {
            operations: Vec::new(),
            edges: vec![return_edge(), EdgeId::new(53_122).unwrap()],
        },
    ] {
        assert_eq!(
            candidate_error(|candidate| candidate.functions[0].provenance = provenance),
            StraightLineUnitReturnTranslationError::TargetProvenance
        );
    }
    assert_eq!(
        candidate_error(|candidate| {
            candidate.functions[0].operation = TargetOperation::ReturnBooleanImmediate {
                psi_edge: return_edge(),
                source_value: ValueId::new(53_123).unwrap(),
                value: false,
            };
        }),
        StraightLineUnitReturnTranslationError::TargetOperation
    );
}

#[test]
fn unit_return_call_plan_parameters_and_return_corruption_fails_closed() {
    assert_eq!(
        candidate_error(|candidate| {
            let TargetOperation::UnitBody(body) = &mut candidate.functions[0].operation else {
                unreachable!()
            };
            body.call_plan = evaluate_call_plan(
                CallingPolicy::native_for_target(NativeTarget::windows_x64()),
                &CallSignature::default(),
            )
            .unwrap();
        }),
        StraightLineUnitReturnTranslationError::TargetCallPlan
    );
    assert_eq!(
        candidate_error(|candidate| {
            let TargetOperation::UnitBody(body) = &mut candidate.functions[0].operation else {
                unreachable!()
            };
            let shape = ValueShape::integer(4, 4);
            let placement = evaluate_call_plan(
                CallingPolicy::native_for_target(NativeTarget::linux_x64()),
                &CallSignature {
                    parameters: vec![shape],
                    result: None,
                },
            )
            .unwrap()
            .parameters
            .remove(0);
            body.parameters.push(TargetStructuralParameter {
                place: PlaceId::new(53_124).unwrap(),
                structural_type: StructuralTypeId::new(53_010).unwrap(),
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::Owned,
                projected_qualifications: Vec::new(),
                shape,
                placement,
            });
        }),
        StraightLineUnitReturnTranslationError::TargetParameters
    );
    for operations in [
        Vec::new(),
        vec![
            TargetUnitOperation::Return {
                psi_edge: return_edge(),
                cleanup_actions: Vec::new(),
            },
            TargetUnitOperation::Return {
                psi_edge: return_edge(),
                cleanup_actions: Vec::new(),
            },
        ],
    ] {
        assert_eq!(
            candidate_error(|candidate| {
                let TargetOperation::UnitBody(body) = &mut candidate.functions[0].operation else {
                    unreachable!()
                };
                body.operations = operations;
            }),
            StraightLineUnitReturnTranslationError::TargetOperationRoster
        );
    }
    for operation in [
        TargetUnitOperation::Return {
            psi_edge: EdgeId::new(53_125).unwrap(),
            cleanup_actions: Vec::new(),
        },
        TargetUnitOperation::Return {
            psi_edge: return_edge(),
            cleanup_actions: vec![TerminalAffineCleanupAction::DiscardRoot(
                PlaceId::new(53_126).unwrap(),
            )],
        },
    ] {
        assert_eq!(
            candidate_error(|candidate| {
                let TargetOperation::UnitBody(body) = &mut candidate.functions[0].operation else {
                    unreachable!()
                };
                body.operations = vec![operation];
            }),
            StraightLineUnitReturnTranslationError::TargetReturn
        );
    }
}

#[test]
fn whole_plan_custody_rejects_unit_body_structural_type_roster_corruption() {
    let source = base_plan();
    let target_profile = NativeTarget::linux_x64();

    for mutate in [
        |body: &mut omega_target_operations::TargetUnitBody| {
            body.structural_types.pop();
        },
        |body: &mut omega_target_operations::TargetUnitBody| {
            body.structural_types.swap(0, 1);
        },
        |body: &mut omega_target_operations::TargetUnitBody| {
            body.structural_types[0].identity = "test::substituted".into();
        },
        |body: &mut omega_target_operations::TargetUnitBody| {
            body.structural_types.push(StructuralTypeDeclaration {
                id: StructuralTypeId::new(53_127).unwrap(),
                identity: "test::injected".into(),
                shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
            });
        },
    ] {
        let mut candidate = lower_to_target_operations(&source, target_profile).unwrap();
        let TargetOperation::UnitBody(body) = &mut candidate.functions[0].operation else {
            unreachable!()
        };
        mutate(body);
        assert_eq!(
            validate_abstract_to_target_translation(&source, target_profile, &candidate),
            Err(
                AbstractToTargetTranslationValidationError::FunctionStructuralTypeRosterMismatch {
                    machine: source.entry,
                }
            )
        );
    }
}
