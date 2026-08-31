use super::scalar::constant_conditional_plan;
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

fn bounded_boolean_cleanup_plan() -> AbstractOperationPlan {
    let caller = MachineId::new(40).unwrap();
    let cleanup = MachineId::new(41).unwrap();
    let helper = MachineId::new(42).unwrap();
    let token_type = StructuralTypeId::new(40).unwrap();
    let plain_type = StructuralTypeId::new(41).unwrap();
    let helper_type = StructuralTypeId::new(42).unwrap();
    let token = PlaceId::new(40).unwrap();
    let plain = PlaceId::new(41).unwrap();
    let left = ValueId::new(40).unwrap();
    let right = ValueId::new(41).unwrap();
    let false_value = ValueId::new(42).unwrap();
    let true_value = ValueId::new(43).unwrap();
    let second_false_value = ValueId::new(44).unwrap();
    let result = ValueId::new(45).unwrap();
    let cleanup_actions = vec![
        TerminalAffineCleanupAction::DiscardRoot(plain),
        TerminalAffineCleanupAction::InvokeNominal(psi_terminal::NominalAffineCleanup {
            place: token,
            structural_type: token_type,
            cleanup_machine: cleanup,
            cleanup_receiver: None,
            requirement_obligations: Vec::new(),
        }),
    ];
    let leaf_return = |edge, value| AbstractOperation::Return {
        psi_edge: EdgeId::new(edge).unwrap(),
        result,
        value,
        scalar_type: ScalarType::Boolean,
        cleanup_actions: cleanup_actions.clone(),
    };
    let block_entry = |block, operation_offset| omega_abstract_operations::AbstractBlockEntry {
        block: BlockId::new(block).unwrap(),
        parameters: Vec::new(),
        operation_offset,
    };
    let return_unit = |edge| AbstractOperation::ReturnUnit {
        psi_edge: EdgeId::new(edge).unwrap(),
        cleanup_actions: Vec::new(),
    };
    let unit_function = |machine, attachment, operations| AbstractFunction {
        machine,
        attachment,
        entry: BlockId::new(machine.get()).unwrap(),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        result: AbstractFunctionResult::Unit,
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        block_entries: vec![block_entry(machine.get(), 0)],
        operations,
    };
    AbstractOperationPlan {
        psi: identity(),
        entry: caller,
        structural_types: vec![
            StructuralTypeDeclaration {
                id: token_type,
                identity: "Token".into(),
                shape: StructuralTypeShape::Record { fields: Vec::new() },
            },
            StructuralTypeDeclaration {
                id: plain_type,
                identity: "Plain".into(),
                shape: StructuralTypeShape::Record { fields: Vec::new() },
            },
            StructuralTypeDeclaration {
                id: helper_type,
                identity: "Helper".into(),
                shape: StructuralTypeShape::Record { fields: Vec::new() },
            },
        ],
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![
            AbstractFunction {
                machine: caller,
                attachment: None,
                entry: BlockId::new(1).unwrap(),
                parameters: vec![
                    AbstractParameter {
                        value: left,
                        scalar_type: ScalarType::Boolean,
                    },
                    AbstractParameter {
                        value: right,
                        scalar_type: ScalarType::Boolean,
                    },
                ],
                structural_parameters: vec![
                    StructuralParameterDeclaration {
                        place: token,
                        position: 0,
                        is_self: false,
                        structural_type: token_type,
                        multiplicity: StructuralMultiplicity::Affine,
                        access: StructuralAccess::Owned,
                        qualifications: Vec::new(),
                    },
                    StructuralParameterDeclaration {
                        place: plain,
                        position: 1,
                        is_self: false,
                        structural_type: plain_type,
                        multiplicity: StructuralMultiplicity::Affine,
                        access: StructuralAccess::Owned,
                        qualifications: Vec::new(),
                    },
                ],
                result: AbstractFunctionResult::Scalar(AbstractResult {
                    value: result,
                    scalar_type: ScalarType::Boolean,
                }),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![
                    block_entry(1, 0),
                    block_entry(2, 1),
                    block_entry(3, 2),
                    block_entry(4, 4),
                    block_entry(5, 6),
                ],
                operations: vec![
                    AbstractOperation::Conditional {
                        condition: left,
                        when_true: AbstractSuccessor {
                            psi_edge: EdgeId::new(1).unwrap(),
                            target: BlockId::new(2).unwrap(),
                            bindings: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                        when_false: AbstractSuccessor {
                            psi_edge: EdgeId::new(2).unwrap(),
                            target: BlockId::new(3).unwrap(),
                            bindings: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                    AbstractOperation::Conditional {
                        condition: right,
                        when_true: AbstractSuccessor {
                            psi_edge: EdgeId::new(3).unwrap(),
                            target: BlockId::new(4).unwrap(),
                            bindings: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                        when_false: AbstractSuccessor {
                            psi_edge: EdgeId::new(4).unwrap(),
                            target: BlockId::new(5).unwrap(),
                            bindings: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                    AbstractOperation::BooleanConstant {
                        psi_operation: OperationId::new(40).unwrap(),
                        result: false_value,
                        value: false,
                    },
                    leaf_return(5, false_value),
                    AbstractOperation::BooleanConstant {
                        psi_operation: OperationId::new(41).unwrap(),
                        result: true_value,
                        value: true,
                    },
                    leaf_return(6, true_value),
                    AbstractOperation::BooleanConstant {
                        psi_operation: OperationId::new(42).unwrap(),
                        result: second_false_value,
                        value: false,
                    },
                    leaf_return(7, second_false_value),
                ],
            },
            unit_function(
                cleanup,
                Some(token_type),
                vec![
                    AbstractOperation::CallUnit {
                        psi_operation: OperationId::new(43).unwrap(),
                        callee: helper,
                        structural_arguments: Vec::new(),
                        claim_transfers: Vec::new(),
                        requirement_obligations: Vec::new(),
                        crash_continuations: Vec::new(),
                    },
                    return_unit(8),
                ],
            ),
            unit_function(helper, Some(helper_type), vec![return_unit(9)]),
        ],
    }
}

#[test]
fn bounded_boolean_control_retains_one_uniform_mixed_cleanup_frontier() {
    let plan = bounded_boolean_cleanup_plan();
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let lowered = lower_to_target_operations(&plan, target)
            .expect("bounded Boolean control and mixed cleanup lower");
        let TargetOperation::BooleanControlWithCleanup {
            control,
            structural_parameters,
            cleanup_actions,
            ..
        } = &lowered.functions[0].operation
        else {
            panic!("bounded Boolean cleanup retains its target carrier")
        };
        assert_eq!(structural_parameters.len(), 2);
        assert!(matches!(
            cleanup_actions.as_slice(),
            [
                TerminalAffineCleanupAction::DiscardRoot(discarded),
                TerminalAffineCleanupAction::InvokeNominal(cleanup),
            ] if *discarded == PlaceId::new(41).unwrap()
                && cleanup.place == PlaceId::new(40).unwrap()
                && cleanup.cleanup_machine == MachineId::new(41).unwrap()
                && cleanup.cleanup_receiver.is_none()
                && cleanup.requirement_obligations.is_empty()
        ));
        let TargetBooleanControl::Conditional {
            when_true,
            when_false,
            ..
        } = control
        else {
            panic!("outer runtime input remains the root decision")
        };
        let TargetBooleanControl::Conditional {
            when_true: nested_true,
            when_false: nested_false,
            ..
        } = when_true.control.as_ref()
        else {
            panic!("true arm retains the second decision")
        };
        let leaf_edge = |control: &TargetBooleanControl| match control {
            TargetBooleanControl::ReturnImmediate {
                psi_return_edge, ..
            } => *psi_return_edge,
            _ => panic!("bounded decision leaf returns one immediate Boolean"),
        };
        assert_eq!(
            [
                leaf_edge(&nested_true.control),
                leaf_edge(&nested_false.control),
                leaf_edge(&when_false.control),
            ],
            [
                EdgeId::new(6).unwrap(),
                EdgeId::new(7).unwrap(),
                EdgeId::new(5).unwrap(),
            ],
        );
    }
}

#[test]
fn bounded_boolean_cleanup_rejects_nonuniform_or_hidden_frontiers() {
    let mut plan = bounded_boolean_cleanup_plan();
    let AbstractOperation::Return {
        cleanup_actions, ..
    } = &mut plan.functions[0].operations[3]
    else {
        unreachable!("first leaf returns")
    };
    cleanup_actions.clear();
    assert!(matches!(
        lower_to_target_operations(&plan, NativeTarget::linux_x64()),
        Err(LoweringError::UnsupportedOperationInScalarFunction(_))
    ));

    let mut ordinary = constant_conditional_plan(false);
    let place = PlaceId::new(90).unwrap();
    let AbstractOperation::Return {
        cleanup_actions, ..
    } = &mut ordinary.functions[0].operations[3]
    else {
        unreachable!("constant fixture true arm returns")
    };
    cleanup_actions.push(TerminalAffineCleanupAction::DiscardRoot(place));
    assert!(matches!(
        lower_to_target_operations(&ordinary, NativeTarget::linux_x64()),
        Err(LoweringError::UnsupportedOperationInScalarFunction(_))
    ));
}

#[test]
fn two_nominal_cleanups_admit_zero_one_distinct_or_shared_bounded_executable_bodies() {
    let caller = MachineId::new(1).unwrap();
    let executable_cleanup = MachineId::new(2).unwrap();
    let empty_cleanup = MachineId::new(3).unwrap();
    let helper = MachineId::new(4).unwrap();
    let receiver_type = StructuralTypeId::new(1).unwrap();
    let helper_type = StructuralTypeId::new(2).unwrap();
    let first_place = PlaceId::new(1).unwrap();
    let second_place = PlaceId::new(2).unwrap();
    let block = |machine: MachineId| BlockId::new(machine.get()).unwrap();
    let return_unit = |edge| AbstractOperation::ReturnUnit {
        psi_edge: EdgeId::new(edge).unwrap(),
        cleanup_actions: Vec::new(),
    };
    let unit_function = |machine, attachment, operations| AbstractFunction {
        machine,
        attachment,
        entry: block(machine),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        result: AbstractFunctionResult::Unit,
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        block_entries: vec![omega_abstract_operations::AbstractBlockEntry {
            block: block(machine),
            parameters: Vec::new(),
            operation_offset: 0,
        }],
        operations,
    };
    let cleanup = |place, cleanup_machine| psi_terminal::NominalAffineCleanup {
        place,
        structural_type: receiver_type,
        cleanup_machine,
        cleanup_receiver: None,
        requirement_obligations: Vec::new(),
    };
    let caller_parameters = [first_place, second_place]
        .into_iter()
        .enumerate()
        .map(|(position, place)| StructuralParameterDeclaration {
            place,
            position: u32::try_from(position).unwrap(),
            is_self: false,
            structural_type: receiver_type,
            multiplicity: StructuralMultiplicity::Affine,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
        })
        .collect::<Vec<_>>();
    let executable_call = AbstractOperation::CallUnit {
        psi_operation: OperationId::new(1).unwrap(),
        callee: helper,
        structural_arguments: Vec::new(),
        claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    };
    let mut plan = AbstractOperationPlan {
        psi: identity(),
        entry: caller,
        structural_types: vec![
            StructuralTypeDeclaration {
                id: receiver_type,
                identity: "Receiver".into(),
                shape: StructuralTypeShape::Record {
                    fields: vec![StructuralFieldDeclaration {
                        id: StructuralFieldId::new(1).unwrap(),
                        identity: "value".into(),
                        relevance: psi_terminal::BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                            IntegerType::new(psi_core::IntegerSign::Unsigned, 64).unwrap(),
                        )),
                    }],
                },
            },
            StructuralTypeDeclaration {
                id: helper_type,
                identity: "Helper".into(),
                shape: StructuralTypeShape::Record { fields: Vec::new() },
            },
        ],
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![
            AbstractFunction {
                structural_parameters: caller_parameters,
                ..unit_function(
                    caller,
                    None,
                    vec![AbstractOperation::ReturnUnit {
                        psi_edge: EdgeId::new(1).unwrap(),
                        cleanup_actions: vec![
                            TerminalAffineCleanupAction::InvokeNominal(cleanup(
                                second_place,
                                executable_cleanup,
                            )),
                            TerminalAffineCleanupAction::InvokeNominal(cleanup(
                                first_place,
                                empty_cleanup,
                            )),
                        ],
                    }],
                )
            },
            unit_function(
                executable_cleanup,
                Some(receiver_type),
                vec![executable_call.clone(), return_unit(2)],
            ),
            unit_function(empty_cleanup, Some(receiver_type), vec![return_unit(3)]),
            unit_function(helper, Some(helper_type), vec![return_unit(4)]),
        ],
    };

    lower_to_target_operations(&plan, NativeTarget::linux_x64())
        .expect("one executable and one empty cleanup lower");

    plan.functions[1].operations.remove(0);
    lower_to_target_operations(&plan, NativeTarget::linux_x64())
        .expect("two empty cleanup bodies remain accepted");

    plan.functions[1]
        .operations
        .insert(0, executable_call.clone());
    plan.functions[2].operations.insert(
        0,
        AbstractOperation::CallUnit {
            psi_operation: OperationId::new(2).unwrap(),
            callee: helper,
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    );
    lower_to_target_operations(&plan, NativeTarget::linux_x64())
        .expect("two distinct executable cleanup bodies lower");

    let AbstractOperation::ReturnUnit {
        cleanup_actions, ..
    } = &mut plan.functions[0].operations[0]
    else {
        unreachable!("caller remains a direct return")
    };
    let TerminalAffineCleanupAction::InvokeNominal(second) = &mut cleanup_actions[1] else {
        unreachable!("second action remains nominal")
    };
    second.cleanup_machine = executable_cleanup;
    let scalar_cleanup_actions = cleanup_actions.clone();
    lower_to_target_operations(&plan, NativeTarget::linux_x64())
        .expect("two actions sharing one executable cleanup body lower");

    let scalar_value = ValueId::new(1).unwrap();
    let scalar_result = ValueId::new(2).unwrap();
    plan.functions[0].result = AbstractFunctionResult::Scalar(AbstractResult {
        value: scalar_result,
        scalar_type: ScalarType::Boolean,
    });
    plan.functions[0].operations = vec![
        AbstractOperation::BooleanConstant {
            psi_operation: OperationId::new(3).unwrap(),
            result: scalar_value,
            value: true,
        },
        AbstractOperation::Return {
            psi_edge: EdgeId::new(1).unwrap(),
            result: scalar_result,
            value: scalar_value,
            scalar_type: ScalarType::Boolean,
            cleanup_actions: scalar_cleanup_actions.clone(),
        },
    ];
    let lowered = lower_to_target_operations(&plan, NativeTarget::linux_x64())
        .expect("scalar result composes the same ordered cleanup frontier");
    assert!(matches!(
        &lowered.functions[0].operation,
        TargetOperation::ScalarReturnWithCleanup {
            scalar,
            structural_parameters,
            cleanup_actions: lowered_actions,
            ..
        } if matches!(scalar.as_ref(), TargetOperation::ReturnBooleanImmediate {
            value: true,
            ..
        })
            && structural_parameters.len() == 2
            && lowered_actions == &scalar_cleanup_actions
    ));
}

#[test]
fn refuses_a_return_whose_value_was_never_materialized() {
    let machine = MachineId::new(1).expect("machine");
    let unknown = ValueId::new(1).expect("unknown value");
    let result = ValueId::new(2).expect("result");
    let i32_type = IntegerType::new(psi_core::IntegerSign::Signed, 32).expect("i32");
    let plan = AbstractOperationPlan {
        psi: identity(),
        entry: machine,
        structural_types: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![AbstractFunction {
            machine,
            attachment: None,
            entry: BlockId::new(1).expect("block"),
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: AbstractFunctionResult::Scalar(AbstractResult {
                value: result,
                scalar_type: ScalarType::Integer(i32_type),
            }),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: Vec::new(),
            operations: vec![AbstractOperation::Return {
                psi_edge: EdgeId::new(1).expect("edge"),
                result,
                value: unknown,
                scalar_type: ScalarType::Integer(i32_type),
                cleanup_actions: Vec::new(),
            }],
        }],
    };

    assert_eq!(
        lower_to_target_operations(&plan, NativeTarget::linux_x64()),
        Err(LoweringError::UnknownValue(unknown))
    );
}
