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
    let block_entry = |machine: MachineId| abstract_operations::AbstractBlockEntry {
        structural_parameters: Vec::new(),
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
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
                }],
            },
        }]
        .into(),
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
                        arguments: Vec::new(),
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

fn free_whole_affine_unit_call_plan() -> AbstractOperationPlan {
    let mut plan = structural_scalar_call_plan();
    let caller = &mut plan.functions[0];
    caller.result = AbstractFunctionResult::Unit;
    let AbstractOperation::CallStructuralScalar {
        requirement_obligations,
        crash_continuations,
        ..
    } = &mut caller.operations[0]
    else {
        unreachable!()
    };
    requirement_obligations.clear();
    crash_continuations.clear();
    caller.operations[1] = AbstractOperation::ReturnUnit {
        psi_edge: EdgeId::new(70).unwrap(),
        cleanup_actions: Vec::new(),
    };
    plan
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
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
                }],
            },
        }]
        .into(),
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
            block_entries: vec![abstract_operations::AbstractBlockEntry {
                structural_parameters: Vec::new(),
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
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Scalar(ScalarType::Integer(integer_type)),
                }],
            },
        }]
        .into(),
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
            block_entries: vec![abstract_operations::AbstractBlockEntry {
                structural_parameters: Vec::new(),
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

fn unrestricted_mutable_integer_field_stores_return_plan() -> AbstractOperationPlan {
    let mut plan = unrestricted_shared_integer_field_return_plan();
    let StructuralTypeShape::Record { fields } = &mut plan.structural_types.make_mut()[0].shape
    else {
        unreachable!()
    };
    fields.push(StructuralFieldDeclaration {
        id: StructuralFieldId::new(76).unwrap(),
        identity: "other".into(),
        relevance: terminal_psi::BindingRelevance::Relevant,
        field_type: StructuralFieldType::Scalar(ScalarType::Integer(
            IntegerType::new(IntegerSign::Signed, 32).unwrap(),
        )),
    });
    fields.push(StructuralFieldDeclaration {
        id: StructuralFieldId::new(78).unwrap(),
        identity: "third".into(),
        relevance: terminal_psi::BindingRelevance::Relevant,
        field_type: StructuralFieldType::Scalar(ScalarType::Integer(
            IntegerType::new(IntegerSign::Signed, 32).unwrap(),
        )),
    });
    let function = &mut plan.functions[0];
    let parameter = &mut function.structural_parameters[0];
    parameter.access = StructuralAccess::MutableBorrow;
    let parameter = parameter.clone();
    let AbstractOperation::IntegerStructuralField { source, .. } = &mut function.operations[0]
    else {
        unreachable!()
    };
    *source = parameter.clone();
    let result = function.result.scalar().unwrap();
    let scalar_type = result.scalar_type;
    let ScalarType::Integer(integer_type) = scalar_type else {
        unreachable!()
    };
    let literal = AbstractResult {
        value: ValueId::new(74).unwrap(),
        scalar_type,
    };
    function.operations.insert(
        0,
        AbstractOperation::IntegerConstant {
            psi_operation: OperationId::new(74).unwrap(),
            result: literal.value,
            scalar_type,
            value: IntegerValue::Signed(23),
        },
    );
    function.operations.insert(
        1,
        AbstractOperation::StructuralScalarFieldStore {
            psi_operation: OperationId::new(75).unwrap(),
            destination: parameter,
            path: Vec::new(),
            field: StructuralFieldId::new(73).unwrap(),
            value: literal,
        },
    );
    let second_literal = AbstractResult {
        value: ValueId::new(75).unwrap(),
        scalar_type,
    };
    function.operations.insert(
        2,
        AbstractOperation::IntegerConstant {
            psi_operation: OperationId::new(76).unwrap(),
            result: second_literal.value,
            scalar_type,
            value: IntegerValue::Signed(31),
        },
    );
    function.operations.insert(
        3,
        AbstractOperation::StructuralScalarFieldStore {
            psi_operation: OperationId::new(77).unwrap(),
            destination: function.structural_parameters[0].clone(),
            path: Vec::new(),
            field: StructuralFieldId::new(76).unwrap(),
            value: second_literal,
        },
    );
    let third_literal = AbstractResult {
        value: ValueId::new(76).unwrap(),
        scalar_type,
    };
    function.operations.insert(
        4,
        AbstractOperation::IntegerConstant {
            psi_operation: OperationId::new(78).unwrap(),
            result: third_literal.value,
            scalar_type,
            value: IntegerValue::Signed(47),
        },
    );
    function.operations.insert(
        5,
        AbstractOperation::StructuralScalarFieldStore {
            psi_operation: OperationId::new(79).unwrap(),
            destination: function.structural_parameters[0].clone(),
            path: Vec::new(),
            field: StructuralFieldId::new(78).unwrap(),
            value: third_literal,
        },
    );
    assert_eq!(integer_type.bits(), 32);
    plan
}

#[test]
fn scalar_graph_rejects_unimplemented_boolean_field_observation() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        assert!(
            lower_to_target_operations(&unrestricted_shared_boolean_field_return_plan(), target)
                .is_err()
        );
    }
}

#[test]
fn scalar_graph_rejects_unimplemented_integer_field_observation() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        assert!(
            lower_to_target_operations(&unrestricted_shared_integer_field_return_plan(), target)
                .is_err()
        );
    }
}

#[test]
fn scalar_graph_does_not_hide_unimplemented_field_reads_behind_stores() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        assert!(
            lower_to_target_operations(
                &unrestricted_mutable_integer_field_stores_return_plan(),
                target
            )
            .is_err()
        );
    }
}

#[test]
fn scalar_graph_rejects_unimplemented_claim_bearing_structural_calls() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        assert!(lower_to_target_operations(&structural_scalar_call_plan(), target).is_err());
    }
}

#[test]
fn free_whole_affine_unit_call_requires_the_exact_callee_contract() {
    let target = NativeTarget::linux_x64();
    let lowered = lower_to_target_operations(&free_whole_affine_unit_call_plan(), target)
        .expect("exact free whole-affine Unit call lowers");
    assert!(matches!(
        &lowered.functions[0].operation,
        TargetOperation::UnitBody(body)
            if matches!(
                body.operations.as_slice(),
                [TargetUnitOperation::StructuralScalarCall { .. }, TargetUnitOperation::Return { .. }]
            )
    ));

    let mutations: [fn(&mut StructuralParameterDeclaration); 3] = [
        |parameter: &mut StructuralParameterDeclaration| {
            parameter.access = StructuralAccess::SharedBorrow;
        },
        |parameter: &mut StructuralParameterDeclaration| {
            parameter.multiplicity = StructuralMultiplicity::Unrestricted;
        },
        |parameter: &mut StructuralParameterDeclaration| {
            parameter.qualifications.push(
                semantic_vocabulary::StructuralDomainId::new(1).expect("forged qualification"),
            );
        },
    ];
    for mutate in mutations {
        let mut drifted = free_whole_affine_unit_call_plan();
        mutate(&mut drifted.functions[1].structural_parameters[0]);
        assert!(matches!(
            lower_to_target_operations(&drifted, target),
            Err(crate::LoweringError::UnsupportedOperationInUnitFunction(machine))
                if machine == MachineId::new(70).unwrap()
        ));
    }
}
