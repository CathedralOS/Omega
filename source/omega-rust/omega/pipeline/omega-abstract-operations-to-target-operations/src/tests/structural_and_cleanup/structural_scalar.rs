use super::*;

fn structural_scalar_call_plan() -> AbstractOperationPlan {
    let caller = MachineId::new(70).unwrap();
    let callee = MachineId::new(71).unwrap();
    let structural_type = StructuralTypeId::new(70).unwrap();
    let caller_place = PlaceId::new(70).unwrap();
    let callee_place = PlaceId::new(71).unwrap();
    let caller_result = ValueId::new(70).unwrap();
    let callee_result = ValueId::new(71).unwrap();
    let callee_value = ValueId::new(72).unwrap();
    let block_entry = |machine: MachineId| omega_abstract_operations::AbstractBlockEntry {
        block: BlockId::new(machine.get()).unwrap(),
        parameters: Vec::new(),
        operation_offset: 0,
    };
    let parameter = |place, position| StructuralParameterDeclaration {
        place,
        position,
        is_self: false,
        structural_type,
        multiplicity: StructuralMultiplicity::Affine,
        access: StructuralAccess::Owned,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    AbstractOperationPlan {
        psi: identity(),
        entry: caller,
        structural_types: vec![StructuralTypeDeclaration {
            id: structural_type,
            identity: "Token".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: StructuralFieldId::new(70).unwrap(),
                    identity: "live".into(),
                    relevance: psi_terminal::BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
                }],
            },
        }],
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![
            AbstractFunction {
                machine: caller,
                attachment: None,
                entry: BlockId::new(caller.get()).unwrap(),
                parameters: Vec::new(),
                structural_parameters: vec![parameter(caller_place, 0)],
                result: AbstractFunctionResult::Scalar(AbstractResult {
                    value: caller_result,
                    scalar_type: ScalarType::Boolean,
                }),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![block_entry(caller)],
                operations: vec![
                    AbstractOperation::CallStructuralScalar {
                        psi_operation: OperationId::new(70).unwrap(),
                        result: AbstractResult {
                            value: caller_result,
                            scalar_type: ScalarType::Boolean,
                        },
                        callee,
                        structural_arguments: vec![StructuralArgument {
                            place: caller_place,
                            access: StructuralAccess::Owned,
                            path: Vec::new(),
                        }],
                        claim_transfers: Vec::new(),
                        requirement_obligations: vec![ObligationId::new(70).unwrap()],
                        crash_continuations: vec![CrashRouteBucket {
                            cause: CrashCause::Trap,
                            alternatives: vec![CrashRouteGuard::Truth],
                        }],
                    },
                    AbstractOperation::Return {
                        psi_edge: EdgeId::new(70).unwrap(),
                        result: caller_result,
                        value: caller_result,
                        scalar_type: ScalarType::Boolean,
                        cleanup_actions: Vec::new(),
                    },
                ],
            },
            AbstractFunction {
                machine: callee,
                attachment: None,
                entry: BlockId::new(callee.get()).unwrap(),
                parameters: Vec::new(),
                structural_parameters: vec![parameter(callee_place, 0)],
                result: AbstractFunctionResult::Scalar(AbstractResult {
                    value: callee_result,
                    scalar_type: ScalarType::Boolean,
                }),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![block_entry(callee)],
                operations: vec![
                    AbstractOperation::BooleanConstant {
                        psi_operation: OperationId::new(71).unwrap(),
                        result: callee_value,
                        value: true,
                    },
                    AbstractOperation::Return {
                        psi_edge: EdgeId::new(71).unwrap(),
                        result: callee_result,
                        value: callee_value,
                        scalar_type: ScalarType::Boolean,
                        cleanup_actions: vec![TerminalAffineCleanupAction::DiscardRoot(
                            callee_place,
                        )],
                    },
                ],
            },
        ],
    }
}
fn unrestricted_shared_boolean_field_return_plan() -> AbstractOperationPlan {
    let realization = MachineId::new(72).unwrap();
    let structural_type = StructuralTypeId::new(72).unwrap();
    let source = PlaceId::new(72).unwrap();
    let field = StructuralFieldId::new(72).unwrap();
    let result = ValueId::new(72).unwrap();
    AbstractOperationPlan {
        psi: identity(),
        entry: realization,
        structural_types: vec![StructuralTypeDeclaration {
            id: structural_type,
            identity: "SharedCarrier".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: field,
                    identity: "ready".into(),
                    relevance: psi_terminal::BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
                }],
            },
        }],
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![AbstractFunction {
            machine: realization,
            attachment: Some(structural_type),
            entry: BlockId::new(realization.get()).unwrap(),
            parameters: Vec::new(),
            structural_parameters: vec![StructuralParameterDeclaration {
                place: source,
                position: 0,
                is_self: true,
                structural_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::SharedBorrow,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            }],
            result: AbstractFunctionResult::Scalar(AbstractResult {
                value: result,
                scalar_type: ScalarType::Boolean,
            }),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: vec![omega_abstract_operations::AbstractBlockEntry {
                block: BlockId::new(realization.get()).unwrap(),
                parameters: Vec::new(),
                operation_offset: 0,
            }],
            operations: vec![
                AbstractOperation::BooleanStructuralField {
                    psi_operation: OperationId::new(72).unwrap(),
                    result,
                    source,
                    field,
                },
                AbstractOperation::Return {
                    psi_edge: EdgeId::new(72).unwrap(),
                    result,
                    value: result,
                    scalar_type: ScalarType::Boolean,
                    cleanup_actions: Vec::new(),
                },
            ],
        }],
    }
}

fn unrestricted_shared_integer_field_return_plan() -> AbstractOperationPlan {
    let realization = MachineId::new(73).unwrap();
    let structural_type = StructuralTypeId::new(73).unwrap();
    let source = PlaceId::new(73).unwrap();
    let field = StructuralFieldId::new(73).unwrap();
    let result = ValueId::new(73).unwrap();
    let integer_type = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    let parameter = StructuralParameterDeclaration {
        place: source,
        position: 0,
        is_self: true,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::SharedBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    AbstractOperationPlan {
        psi: identity(),
        entry: realization,
        structural_types: vec![StructuralTypeDeclaration {
            id: structural_type,
            identity: "SharedIntegerCarrier".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: field,
                    identity: "value".into(),
                    relevance: psi_terminal::BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Scalar(ScalarType::Integer(integer_type)),
                }],
            },
        }],
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![AbstractFunction {
            machine: realization,
            attachment: Some(structural_type),
            entry: BlockId::new(realization.get()).unwrap(),
            parameters: Vec::new(),
            structural_parameters: vec![parameter.clone()],
            result: AbstractFunctionResult::Scalar(AbstractResult {
                value: result,
                scalar_type: ScalarType::Integer(integer_type),
            }),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: vec![omega_abstract_operations::AbstractBlockEntry {
                block: BlockId::new(realization.get()).unwrap(),
                parameters: Vec::new(),
                operation_offset: 0,
            }],
            operations: vec![
                AbstractOperation::IntegerStructuralField {
                    psi_operation: OperationId::new(73).unwrap(),
                    result: AbstractResult {
                        value: result,
                        scalar_type: ScalarType::Integer(integer_type),
                    },
                    source: parameter,
                    field,
                },
                AbstractOperation::Return {
                    psi_edge: EdgeId::new(73).unwrap(),
                    result,
                    value: result,
                    scalar_type: ScalarType::Integer(integer_type),
                    cleanup_actions: Vec::new(),
                },
            ],
        }],
    }
}

#[test]
fn unrestricted_shared_boolean_field_lowers_as_a_straight_line_return() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let lowered =
            lower_to_target_operations(&unrestricted_shared_boolean_field_return_plan(), target)
                .expect("unrestricted shared Boolean field return lowers");
        assert!(matches!(
            &lowered.functions[0].operation,
            TargetOperation::ReturnBooleanExpression {
                source_value,
                expression:
                    TargetBooleanExpression::StructuralField {
                        psi_operation,
                        source,
                        field,
                        field_byte_offset: 0,
                        ..
                    },
                ..
            } if *source_value == ValueId::new(72).unwrap()
                && *psi_operation == OperationId::new(72).unwrap()
                && *source == PlaceId::new(72).unwrap()
                && *field == StructuralFieldId::new(72).unwrap()
        ));
    }
}

#[test]
fn unrestricted_shared_integer_field_lowers_as_a_straight_line_return() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let lowered =
            lower_to_target_operations(&unrestricted_shared_integer_field_return_plan(), target)
                .expect("unrestricted shared integer field return lowers");
        assert!(matches!(
            &lowered.functions[0].operation,
            TargetOperation::ReturnIntegerExpression {
                source_value,
                scalar_type,
                expression:
                    TargetIntegerExpression::StructuralField {
                        psi_operation,
                        source,
                        field,
                        field_byte_offset: 0,
                        ..
                    },
                ..
            } if *source_value == ValueId::new(73).unwrap()
                && *scalar_type == IntegerType::new(IntegerSign::Signed, 32).unwrap()
                && *psi_operation == OperationId::new(73).unwrap()
                && *source == PlaceId::new(73).unwrap()
                && *field == StructuralFieldId::new(73).unwrap()
        ));
    }
}

#[test]
fn whole_root_structural_call_retains_direct_scalar_return_abi() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let lowered = lower_to_target_operations(&structural_scalar_call_plan(), target)
            .expect("bounded structural scalar call lowers");
        let TargetOperation::ReturnStructuralScalarCall {
            scalar_type,
            callee,
            structural_parameters,
            arguments,
            requirement_obligations,
            crash_continuations,
            ..
        } = &lowered.functions[0].operation
        else {
            panic!("structural scalar call retains its dedicated target carrier")
        };
        assert_eq!(*scalar_type, ScalarType::Boolean);
        assert_eq!(*callee, MachineId::new(71).unwrap());
        assert_eq!(structural_parameters.len(), 1);
        assert_eq!(arguments.len(), 1);
        assert!(arguments[0].path.is_empty());
        assert_eq!(arguments[0].source_byte_offset, 0);
        assert_eq!(requirement_obligations, &[ObligationId::new(70).unwrap()]);
        assert_eq!(
            crash_continuations,
            &[CrashRouteBucket {
                cause: CrashCause::Trap,
                alternatives: vec![CrashRouteGuard::Truth],
            }]
        );
    }
}
