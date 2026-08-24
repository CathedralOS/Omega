use super::*;
use omega_terminal_abstract_operations::{
    TerminalAbstractBlockEntry, TerminalAbstractFunction, TerminalAbstractOperation,
    TerminalAbstractOperationPlan, TerminalAbstractParameter, TerminalAbstractResult,
    TerminalAbstractSuccessor, TerminalValueBinding,
};
use omega_terminal_target_operations::MachineRegister;
use psi_core::{BlockId, EdgeId, PlaceId, StructuralFieldId, StructuralTypeId};
use psi_terminal::{
    BoundaryMachineDeclaration, SemanticFingerprint, StructuralAccess, StructuralArgument,
    StructuralFieldDeclaration, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape,
    TerminalAffineCleanupAction, TerminalPsiIdentity, VocabularyMarker,
};

fn structural_scalar_call_plan() -> TerminalAbstractOperationPlan {
    let caller = MachineId::new(70).unwrap();
    let callee = MachineId::new(71).unwrap();
    let structural_type = StructuralTypeId::new(70).unwrap();
    let caller_place = PlaceId::new(70).unwrap();
    let callee_place = PlaceId::new(71).unwrap();
    let caller_result = ValueId::new(70).unwrap();
    let callee_result = ValueId::new(71).unwrap();
    let callee_value = ValueId::new(72).unwrap();
    let block_entry =
        |machine: MachineId| omega_terminal_abstract_operations::TerminalAbstractBlockEntry {
            block: BlockId::new(machine.get()).unwrap(),
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
    TerminalAbstractOperationPlan {
        terminal_psi: identity(),
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
            TerminalAbstractFunction {
                machine: caller,
                attachment: None,
                entry: BlockId::new(caller.get()).unwrap(),
                parameters: Vec::new(),
                structural_parameters: vec![parameter(caller_place, 0)],
                result: TerminalAbstractFunctionResult::Scalar(TerminalAbstractResult {
                    value: caller_result,
                    scalar_type: ScalarType::Boolean,
                }),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![block_entry(caller)],
                operations: vec![
                    TerminalAbstractOperation::CallStructuralScalar {
                        psi_operation: OperationId::new(70).unwrap(),
                        result: TerminalAbstractResult {
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
                    },
                    TerminalAbstractOperation::Return {
                        psi_edge: EdgeId::new(70).unwrap(),
                        result: caller_result,
                        value: caller_result,
                        scalar_type: ScalarType::Boolean,
                        cleanup_actions: Vec::new(),
                    },
                ],
            },
            TerminalAbstractFunction {
                machine: callee,
                attachment: None,
                entry: BlockId::new(callee.get()).unwrap(),
                parameters: Vec::new(),
                structural_parameters: vec![parameter(callee_place, 0)],
                result: TerminalAbstractFunctionResult::Scalar(TerminalAbstractResult {
                    value: callee_result,
                    scalar_type: ScalarType::Boolean,
                }),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![block_entry(callee)],
                operations: vec![
                    TerminalAbstractOperation::BooleanConstant {
                        psi_operation: OperationId::new(71).unwrap(),
                        result: callee_value,
                        value: true,
                    },
                    TerminalAbstractOperation::Return {
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
        let TerminalTargetOperation::ReturnStructuralScalarCall {
            scalar_type,
            callee,
            structural_parameters,
            arguments,
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
    }
}

fn bounded_boolean_cleanup_plan() -> TerminalAbstractOperationPlan {
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
    let leaf_return = |edge, value| TerminalAbstractOperation::Return {
        psi_edge: EdgeId::new(edge).unwrap(),
        result,
        value,
        scalar_type: ScalarType::Boolean,
        cleanup_actions: cleanup_actions.clone(),
    };
    let block_entry =
        |block, operation_offset| omega_terminal_abstract_operations::TerminalAbstractBlockEntry {
            block: BlockId::new(block).unwrap(),
            operation_offset,
        };
    let return_unit = |edge| TerminalAbstractOperation::ReturnUnit {
        psi_edge: EdgeId::new(edge).unwrap(),
        cleanup_actions: Vec::new(),
    };
    let unit_function = |machine, attachment, operations| TerminalAbstractFunction {
        machine,
        attachment,
        entry: BlockId::new(machine.get()).unwrap(),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        result: TerminalAbstractFunctionResult::Unit,
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        block_entries: vec![block_entry(machine.get(), 0)],
        operations,
    };
    TerminalAbstractOperationPlan {
        terminal_psi: identity(),
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
            TerminalAbstractFunction {
                machine: caller,
                attachment: None,
                entry: BlockId::new(1).unwrap(),
                parameters: vec![
                    TerminalAbstractParameter {
                        value: left,
                        scalar_type: ScalarType::Boolean,
                    },
                    TerminalAbstractParameter {
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
                result: TerminalAbstractFunctionResult::Scalar(TerminalAbstractResult {
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
                    TerminalAbstractOperation::Conditional {
                        condition: left,
                        when_true: TerminalAbstractSuccessor {
                            psi_edge: EdgeId::new(1).unwrap(),
                            target: BlockId::new(2).unwrap(),
                            bindings: Vec::new(),
                        },
                        when_false: TerminalAbstractSuccessor {
                            psi_edge: EdgeId::new(2).unwrap(),
                            target: BlockId::new(3).unwrap(),
                            bindings: Vec::new(),
                        },
                    },
                    TerminalAbstractOperation::Conditional {
                        condition: right,
                        when_true: TerminalAbstractSuccessor {
                            psi_edge: EdgeId::new(3).unwrap(),
                            target: BlockId::new(4).unwrap(),
                            bindings: Vec::new(),
                        },
                        when_false: TerminalAbstractSuccessor {
                            psi_edge: EdgeId::new(4).unwrap(),
                            target: BlockId::new(5).unwrap(),
                            bindings: Vec::new(),
                        },
                    },
                    TerminalAbstractOperation::BooleanConstant {
                        psi_operation: OperationId::new(40).unwrap(),
                        result: false_value,
                        value: false,
                    },
                    leaf_return(5, false_value),
                    TerminalAbstractOperation::BooleanConstant {
                        psi_operation: OperationId::new(41).unwrap(),
                        result: true_value,
                        value: true,
                    },
                    leaf_return(6, true_value),
                    TerminalAbstractOperation::BooleanConstant {
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
                    TerminalAbstractOperation::CallUnit {
                        psi_operation: OperationId::new(43).unwrap(),
                        callee: helper,
                        structural_arguments: Vec::new(),
                        claim_transfers: Vec::new(),
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
        let TerminalTargetOperation::BooleanControlWithCleanup {
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
        let TerminalTargetBooleanControl::Conditional {
            when_true,
            when_false,
            ..
        } = control
        else {
            panic!("outer runtime input remains the root decision")
        };
        let TerminalTargetBooleanControl::Conditional {
            when_true: nested_true,
            when_false: nested_false,
            ..
        } = when_true.control.as_ref()
        else {
            panic!("true arm retains the second decision")
        };
        let leaf_edge = |control: &TerminalTargetBooleanControl| match control {
            TerminalTargetBooleanControl::ReturnImmediate {
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
    let TerminalAbstractOperation::Return {
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
    let TerminalAbstractOperation::Return {
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
    let return_unit = |edge| TerminalAbstractOperation::ReturnUnit {
        psi_edge: EdgeId::new(edge).unwrap(),
        cleanup_actions: Vec::new(),
    };
    let unit_function = |machine, attachment, operations| TerminalAbstractFunction {
        machine,
        attachment,
        entry: block(machine),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        result: TerminalAbstractFunctionResult::Unit,
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        block_entries: vec![
            omega_terminal_abstract_operations::TerminalAbstractBlockEntry {
                block: block(machine),
                operation_offset: 0,
            },
        ],
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
    let executable_call = TerminalAbstractOperation::CallUnit {
        psi_operation: OperationId::new(1).unwrap(),
        callee: helper,
        structural_arguments: Vec::new(),
        claim_transfers: Vec::new(),
    };
    let mut plan = TerminalAbstractOperationPlan {
        terminal_psi: identity(),
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
            TerminalAbstractFunction {
                structural_parameters: caller_parameters,
                ..unit_function(
                    caller,
                    None,
                    vec![TerminalAbstractOperation::ReturnUnit {
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
        TerminalAbstractOperation::CallUnit {
            psi_operation: OperationId::new(2).unwrap(),
            callee: helper,
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
        },
    );
    lower_to_target_operations(&plan, NativeTarget::linux_x64())
        .expect("two distinct executable cleanup bodies lower");

    let TerminalAbstractOperation::ReturnUnit {
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
    plan.functions[0].result = TerminalAbstractFunctionResult::Scalar(TerminalAbstractResult {
        value: scalar_result,
        scalar_type: ScalarType::Boolean,
    });
    plan.functions[0].operations = vec![
        TerminalAbstractOperation::BooleanConstant {
            psi_operation: OperationId::new(3).unwrap(),
            result: scalar_value,
            value: true,
        },
        TerminalAbstractOperation::Return {
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
        TerminalTargetOperation::ScalarReturnWithCleanup {
            scalar,
            structural_parameters,
            cleanup_actions: lowered_actions,
            ..
        } if matches!(scalar.as_ref(), TerminalTargetOperation::ReturnBooleanImmediate {
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
    let plan = TerminalAbstractOperationPlan {
        terminal_psi: identity(),
        entry: machine,
        structural_types: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![TerminalAbstractFunction {
            machine,
            attachment: None,
            entry: BlockId::new(1).expect("block"),
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: TerminalAbstractFunctionResult::Scalar(TerminalAbstractResult {
                value: result,
                scalar_type: ScalarType::Integer(i32_type),
            }),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: Vec::new(),
            operations: vec![TerminalAbstractOperation::Return {
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

#[test]
fn unit_fixed_array_call_selects_exact_forty_byte_native_placements() {
    let root = MachineId::new(1).unwrap();
    let callee = MachineId::new(2).unwrap();
    let element_type = StructuralTypeId::new(1).unwrap();
    let structural_type = StructuralTypeId::new(2).unwrap();
    let root_place = PlaceId::new(1).unwrap();
    let callee_place = PlaceId::new(2).unwrap();
    let u64_type =
        ScalarType::Integer(IntegerType::new(psi_core::IntegerSign::Unsigned, 64).unwrap());
    let structural_types = vec![
        StructuralTypeDeclaration {
            id: element_type,
            identity: "Acknowledgement".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![
                    StructuralFieldDeclaration {
                        id: StructuralFieldId::new(1).unwrap(),
                        identity: "value".into(),
                        relevance: psi_terminal::BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Scalar(u64_type),
                    },
                    StructuralFieldDeclaration {
                        id: StructuralFieldId::new(2).unwrap(),
                        identity: "proof".into(),
                        relevance: psi_terminal::BindingRelevance::Erased,
                        field_type: StructuralFieldType::Erased {
                            type_identity: "named(name(example::Evidence))".into(),
                        },
                    },
                ],
            },
        },
        StructuralTypeDeclaration {
            id: structural_type,
            identity: "[Acknowledgement; 5]".into(),
            shape: StructuralTypeShape::FixedArray {
                element: element_type,
                length: 5,
            },
        },
    ];
    let parameter = |place| StructuralParameterDeclaration {
        place,
        position: 0,
        is_self: false,
        structural_type,
        multiplicity: StructuralMultiplicity::Linear,
        access: StructuralAccess::Owned,
        qualifications: Vec::new(),
    };
    let unit_function = |machine, place, operations| TerminalAbstractFunction {
        machine,
        attachment: None,
        entry: BlockId::new(machine.get()).unwrap(),
        parameters: Vec::new(),
        structural_parameters: vec![parameter(place)],
        result: TerminalAbstractFunctionResult::Unit,
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        block_entries: vec![
            omega_terminal_abstract_operations::TerminalAbstractBlockEntry {
                block: BlockId::new(machine.get()).unwrap(),
                operation_offset: 0,
            },
        ],
        operations,
    };
    let plan = TerminalAbstractOperationPlan {
        terminal_psi: identity(),
        entry: root,
        structural_types,
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![
            unit_function(
                root,
                root_place,
                vec![
                    TerminalAbstractOperation::CallUnit {
                        psi_operation: OperationId::new(1).unwrap(),
                        callee,
                        structural_arguments: vec![psi_terminal::StructuralArgument {
                            place: root_place,
                            access: StructuralAccess::Owned,
                            path: Vec::new(),
                        }],
                        claim_transfers: Vec::new(),
                    },
                    TerminalAbstractOperation::ReturnUnit {
                        psi_edge: EdgeId::new(1).unwrap(),
                        cleanup_actions: Vec::new(),
                    },
                ],
            ),
            unit_function(
                callee,
                callee_place,
                vec![TerminalAbstractOperation::ReturnUnit {
                    psi_edge: EdgeId::new(2).unwrap(),
                    cleanup_actions: Vec::new(),
                }],
            ),
        ],
    };

    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let lowered = lower_to_target_operations(&plan, target).unwrap();
        let TerminalTargetOperation::UnitBody(root) = &lowered.functions[0].operation else {
            panic!("root must remain Unit")
        };
        assert_eq!(root.parameters[0].shape, ValueShape::integer(40, 8));
        let TerminalTargetUnitOperation::Call { arguments, .. } = &root.operations[0] else {
            panic!("root must call helper")
        };
        assert!(arguments[0].path.is_empty());
        assert_eq!(arguments[0].shape, ValueShape::integer(40, 8));
    }

    let linux = lower_to_target_operations(&plan, NativeTarget::linux_x64()).unwrap();
    let TerminalTargetOperation::UnitBody(linux_root) = &linux.functions[0].operation else {
        panic!("root must remain Unit")
    };
    assert_eq!(linux_root.parameters[0].shape, ValueShape::integer(40, 8));
    assert_eq!(linux_root.parameters[0].placement.locations.len(), 5);
    assert!(
        linux_root.parameters[0]
            .placement
            .locations
            .iter()
            .enumerate()
            .all(|(index, location)| matches!(
                location,
                ValueLocation::Stack {
                    stack_byte_offset,
                    value_byte_offset,
                    byte_size: 8,
                    alignment: 8,
                } if *stack_byte_offset == index as u32 * 8
                    && *value_byte_offset == index as u16 * 8
            ))
    );
    let TerminalTargetUnitOperation::Call { arguments, .. } = &linux_root.operations[0] else {
        panic!("root must call helper")
    };
    assert_eq!(arguments[0].source, arguments[0].destination);

    let windows = lower_to_target_operations(&plan, NativeTarget::windows_x64()).unwrap();
    let TerminalTargetOperation::UnitBody(windows_root) = &windows.functions[0].operation else {
        panic!("root must remain Unit")
    };
    assert!(matches!(
        windows_root.parameters[0].placement.locations.as_slice(),
        [ValueLocation::Indirect {
            pointer: omega_calling_conventions::IndirectPointerLocation::Register(
                MachineRegister::X86Rcx
            ),
            byte_size: 40,
            alignment: 8,
            ..
        }]
    ));
    let TerminalTargetUnitOperation::Call { arguments, .. } = &windows_root.operations[0] else {
        panic!("root must call helper")
    };
    assert_eq!(arguments[0].source, arguments[0].destination);
}

#[test]
fn fixed_array_layout_repeats_padded_nested_elements_and_rejects_overflow() {
    let element_type = StructuralTypeId::new(1).unwrap();
    let inner_array_type = StructuralTypeId::new(2).unwrap();
    let outer_array_type = StructuralTypeId::new(3).unwrap();
    let oversized_array_type = StructuralTypeId::new(4).unwrap();
    let u64_type =
        ScalarType::Integer(IntegerType::new(psi_core::IntegerSign::Unsigned, 64).unwrap());
    let declarations = vec![
        StructuralTypeDeclaration {
            id: element_type,
            identity: "PaddedElement".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![
                    StructuralFieldDeclaration {
                        id: StructuralFieldId::new(1).unwrap(),
                        identity: "tag".into(),
                        relevance: psi_terminal::BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
                    },
                    StructuralFieldDeclaration {
                        id: StructuralFieldId::new(2).unwrap(),
                        identity: "value".into(),
                        relevance: psi_terminal::BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Scalar(u64_type),
                    },
                ],
            },
        },
        StructuralTypeDeclaration {
            id: inner_array_type,
            identity: "[PaddedElement; 2]".into(),
            shape: StructuralTypeShape::FixedArray {
                element: element_type,
                length: 2,
            },
        },
        StructuralTypeDeclaration {
            id: outer_array_type,
            identity: "[[PaddedElement; 2]; 3]".into(),
            shape: StructuralTypeShape::FixedArray {
                element: inner_array_type,
                length: 3,
            },
        },
        StructuralTypeDeclaration {
            id: oversized_array_type,
            identity: "[PaddedElement; 4096]".into(),
            shape: StructuralTypeShape::FixedArray {
                element: element_type,
                length: 4096,
            },
        },
    ];
    let declarations = declarations
        .iter()
        .map(|declaration| (declaration.id, declaration))
        .collect::<BTreeMap<_, _>>();

    let shape = structural_shape(
        outer_array_type,
        &declarations,
        &mut BTreeMap::new(),
        &mut BTreeSet::new(),
    )
    .unwrap();
    assert_eq!(shape, ValueShape::integer(96, 8));
    assert_eq!(
        structural_shape(
            oversized_array_type,
            &declarations,
            &mut BTreeMap::new(),
            &mut BTreeSet::new(),
        ),
        Err(LoweringError::StructuralTypeTooLarge(oversized_array_type))
    );
}

#[test]
fn metadata_only_boundary_requires_the_exact_preceding_port_realization() {
    use omega_terminal_target_operations::{
        TerminalMetadataOnlyPortRealization, TerminalProviderExecutionBinding,
        TerminalProviderPlanIdentity,
    };

    let machine = MachineId::new(1).unwrap();
    let boundary = BoundaryMachineId::new(1).unwrap();
    let port_operation = OperationId::new(1).unwrap();
    let settlement_operation = OperationId::new(2).unwrap();
    let service = psi_core::ServiceId::new(1).unwrap();
    let element_type = StructuralTypeId::new(1).unwrap();
    let array_type = StructuralTypeId::new(2).unwrap();
    let argument_place = PlaceId::new(1).unwrap();
    let boundary_place = PlaceId::new(2).unwrap();
    let u64_type =
        ScalarType::Integer(IntegerType::new(psi_core::IntegerSign::Unsigned, 64).unwrap());
    let provider_execution = TerminalProviderExecutionBinding::from_execution_record(
        TerminalProviderPlanIdentity::new(7).unwrap(),
        8,
        9,
        10,
        11,
    )
    .unwrap();
    let realization = TerminalMetadataOnlyPortRealization {
        effect_operation: port_operation,
        service,
        port: 0x20,
        value: 0x20,
    };
    let plan = TerminalAbstractOperationPlan {
        terminal_psi: identity(),
        entry: machine,
        structural_types: vec![
            StructuralTypeDeclaration {
                id: element_type,
                identity: "Acknowledgement".into(),
                shape: StructuralTypeShape::Record {
                    fields: vec![StructuralFieldDeclaration {
                        id: StructuralFieldId::new(1).unwrap(),
                        identity: "value".into(),
                        relevance: psi_terminal::BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Scalar(u64_type),
                    }],
                },
            },
            StructuralTypeDeclaration {
                id: array_type,
                identity: "[Acknowledgement; 2]".into(),
                shape: StructuralTypeShape::FixedArray {
                    element: element_type,
                    length: 2,
                },
            },
        ],
        boundary_machines: vec![BoundaryMachineDeclaration {
            id: boundary,
            identity: "InterruptAcknowledgement::complete".into(),
            attachment: None,
            scalar_parameters: Vec::new(),
            structural_parameters: vec![StructuralParameterDeclaration {
                place: boundary_place,
                position: 0,
                is_self: false,
                structural_type: element_type,
                multiplicity: StructuralMultiplicity::Linear,
                access: StructuralAccess::Owned,
                qualifications: Vec::new(),
            }],
            result: None,
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: vec![service],
        }],
        provider_candidates: Vec::new(),
        functions: vec![TerminalAbstractFunction {
            machine,
            attachment: None,
            entry: BlockId::new(1).unwrap(),
            parameters: Vec::new(),
            structural_parameters: vec![StructuralParameterDeclaration {
                place: argument_place,
                position: 0,
                is_self: false,
                structural_type: array_type,
                multiplicity: StructuralMultiplicity::Affine,
                access: StructuralAccess::Owned,
                qualifications: Vec::new(),
            }],
            result: TerminalAbstractFunctionResult::Unit,
            entry_claims: Vec::new(),
            published_service_ceiling: vec![service],
            block_entries: vec![
                omega_terminal_abstract_operations::TerminalAbstractBlockEntry {
                    block: BlockId::new(1).unwrap(),
                    operation_offset: 0,
                },
            ],
            operations: vec![
                TerminalAbstractOperation::PortWrite {
                    psi_operation: port_operation,
                    service,
                    port: 0x20,
                    value: 0x20,
                },
                TerminalAbstractOperation::BoundaryCall {
                    psi_operation: settlement_operation,
                    result: None,
                    boundary,
                    arguments: Vec::new(),
                    structural_arguments: vec![psi_terminal::StructuralArgument {
                        place: argument_place,
                        access: StructuralAccess::Owned,
                        path: vec![StructuralPathSegment::FixedIndex(1)],
                    }],
                    completion_claim_sources: Vec::new(),
                    completion_receipts: Vec::new(),
                },
                TerminalAbstractOperation::ReturnUnit {
                    psi_edge: EdgeId::new(1).unwrap(),
                    cleanup_actions: vec![psi_terminal::TerminalAffineCleanupAction::DiscardRoot(
                        argument_place,
                    )],
                },
            ],
        }],
    };
    let binding = TerminalBoundarySettlementBinding {
        boundary,
        provider_execution,
        realization: realization.into(),
    };
    let lowered =
        lower_to_target_operations_with_settlements(&plan, NativeTarget::linux_x64(), &[binding])
            .expect("exact effect evidence");
    let TerminalTargetOperation::UnitBody(body) = &lowered.functions[0].operation else {
        panic!("Unit body")
    };
    let TerminalTargetUnitOperation::BoundarySettlement {
        provider_execution: actual,
        realization: actual_realization,
        arguments,
        ..
    } = &body.operations[1]
    else {
        panic!("boundary settlement")
    };
    assert_eq!(*actual, provider_execution);
    assert_eq!(*actual_realization, realization.into());
    assert_eq!(
        arguments,
        &[psi_terminal::StructuralArgument {
            place: argument_place,
            access: StructuralAccess::Owned,
            path: vec![StructuralPathSegment::FixedIndex(1)],
        }]
    );

    let mut scalar_argument = plan.clone();
    let argument = ValueId::new(1).unwrap();
    scalar_argument.boundary_machines[0]
        .scalar_parameters
        .push(ScalarType::Boolean);
    scalar_argument.functions[0]
        .parameters
        .push(TerminalAbstractParameter {
            value: argument,
            scalar_type: ScalarType::Boolean,
        });
    let TerminalAbstractOperation::BoundaryCall { arguments, .. } =
        &mut scalar_argument.functions[0].operations[1]
    else {
        unreachable!("fixture contains a boundary call")
    };
    arguments.push(argument);
    assert_eq!(
        lower_to_target_operations_with_settlements(
            &scalar_argument,
            NativeTarget::linux_x64(),
            &[binding],
        ),
        Err(
            LoweringError::ScalarBoundaryArgumentsRequireNativeRealization {
                machine,
                operation: settlement_operation,
                boundary,
            }
        )
    );

    let wrong = TerminalBoundarySettlementBinding {
        realization: TerminalMetadataOnlyPortRealization {
            value: 0x21,
            ..realization
        }
        .into(),
        ..binding
    };
    assert_eq!(
        lower_to_target_operations_with_settlements(&plan, NativeTarget::linux_x64(), &[wrong],),
        Err(LoweringError::BoundaryRealizationMismatch(boundary))
    );

    let mut result_bearing = plan.clone();
    let result = TerminalAbstractResult {
        value: ValueId::new(1).unwrap(),
        scalar_type: ScalarType::Boolean,
    };
    result_bearing.boundary_machines[0].result = Some(result.scalar_type);
    let TerminalAbstractOperation::BoundaryCall {
        result: operation_result,
        ..
    } = &mut result_bearing.functions[0].operations[1]
    else {
        unreachable!("fixture contains a boundary call")
    };
    *operation_result = Some(result);
    assert_eq!(
        lower_to_target_operations_with_settlements(
            &result_bearing,
            NativeTarget::linux_x64(),
            &[binding],
        ),
        Err(
            LoweringError::ResultBearingBoundarySettlementRequiresNativeRealization {
                machine,
                operation: settlement_operation,
                boundary,
            }
        )
    );
}

#[test]
fn selects_native_register_and_stack_locations_for_runtime_parameters() {
    let register_cases = [
        (
            NativeTarget::linux_x64(),
            TerminalScalarParameterLocation::Register(MachineRegister::X86Rdi),
        ),
        (
            NativeTarget::windows_x64(),
            TerminalScalarParameterLocation::Register(MachineRegister::X86Rcx),
        ),
        (
            NativeTarget::linux_arm64(),
            TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
        ),
    ];
    for (target, expected) in register_cases {
        let lowered = lower_to_target_operations(&parameter_return_plan(1), target).unwrap();
        assert!(matches!(
            lowered.functions[0].operation,
            TerminalTargetOperation::ReturnIntegerParameter {
                parameter_index: 0,
                location,
                ..
            } if location == expected
        ));
    }

    let stack_cases = [
        (
            NativeTarget::linux_x64(),
            TerminalScalarParameterLocation::IncomingStack { byte_offset: 16 },
        ),
        (
            NativeTarget::windows_x64(),
            TerminalScalarParameterLocation::IncomingStack { byte_offset: 64 },
        ),
        (
            NativeTarget::linux_arm64(),
            TerminalScalarParameterLocation::IncomingStack { byte_offset: 0 },
        ),
    ];
    for (target, expected) in stack_cases {
        let lowered = lower_to_target_operations(&parameter_return_plan(9), target).unwrap();
        assert!(matches!(
            lowered.functions[0].operation,
            TerminalTargetOperation::ReturnIntegerParameter {
                parameter_index: 8,
                location,
                ..
            } if location == expected
        ));
    }
}

#[test]
fn direct_calls_retain_stack_locations_from_the_callee_call_plan() {
    let stack_cases = [
        (
            NativeTarget::linux_x64(),
            TerminalScalarParameterLocation::IncomingStack { byte_offset: 16 },
        ),
        (
            NativeTarget::windows_x64(),
            TerminalScalarParameterLocation::IncomingStack { byte_offset: 64 },
        ),
        (
            NativeTarget::linux_arm64(),
            TerminalScalarParameterLocation::IncomingStack { byte_offset: 0 },
        ),
    ];
    for (target, expected) in stack_cases {
        let lowered = lower_to_target_operations(&direct_call_plan(9), target).unwrap();
        let TerminalTargetOperation::ReturnIntegerExpression { expression, .. } =
            &lowered.functions[0].operation
        else {
            panic!("caller must return its call result")
        };
        let TerminalTargetIntegerExpression::Call { arguments, .. } = expression else {
            panic!("caller result must remain a direct call")
        };
        assert_eq!(arguments[8].location, expected);
    }
}

#[test]
fn lowers_runtime_parameter_arithmetic_to_a_typed_target_expression() {
    let mut plan = parameter_return_plan(2);
    let function = &mut plan.functions[0];
    let sum = ValueId::new(50).expect("sum");
    let scalar_type = match scalar_result(function).scalar_type {
        ScalarType::Integer(integer) => integer,
        ScalarType::Boolean => unreachable!("fixture is integer"),
    };
    function.operations.insert(
        0,
        TerminalAbstractOperation::WrappingIntegerAdd {
            psi_operation: psi_core::OperationId::new(50).expect("operation"),
            result: sum,
            scalar_type,
            left: function.parameters[0].value,
            right: function.parameters[1].value,
        },
    );
    let TerminalAbstractOperation::Return { value, .. } = &mut function.operations[1] else {
        unreachable!("fixture ends in return")
    };
    *value = sum;

    let lowered = lower_to_target_operations(&plan, NativeTarget::host()).unwrap();
    assert!(matches!(
        &lowered.functions[0].operation,
        TerminalTargetOperation::ReturnIntegerExpression {
            source_value,
            scalar_type: result_type,
            expression: TerminalTargetIntegerExpression::WrappingAdd {
                psi_operation,
                left,
                right,
            },
            ..
        } if *source_value == sum
            && *result_type == scalar_type
            && *psi_operation == psi_core::OperationId::new(50).expect("operation")
            && matches!(
                left.as_ref(),
                TerminalTargetIntegerExpression::Parameter {
                    parameter_index: 0,
                    ..
                }
            )
            && matches!(
                right.as_ref(),
                TerminalTargetIntegerExpression::Parameter {
                    parameter_index: 1,
                    ..
                }
            )
    ));
}

#[test]
fn folds_closed_wrapping_subtraction_at_the_declared_width() {
    let mut plan = parameter_return_plan(1);
    let function = &mut plan.functions[0];
    let left = ValueId::new(50).expect("left");
    let right = ValueId::new(51).expect("right");
    let difference = ValueId::new(52).expect("difference");
    let scalar_type = match scalar_result(function).scalar_type {
        ScalarType::Integer(integer) => integer,
        ScalarType::Boolean => unreachable!("fixture is integer"),
    };
    function.operations.splice(
        0..0,
        [
            TerminalAbstractOperation::IntegerConstant {
                psi_operation: psi_core::OperationId::new(50).expect("left operation"),
                result: left,
                scalar_type: ScalarType::Integer(scalar_type),
                value: IntegerValue::Unsigned(5),
            },
            TerminalAbstractOperation::IntegerConstant {
                psi_operation: psi_core::OperationId::new(51).expect("right operation"),
                result: right,
                scalar_type: ScalarType::Integer(scalar_type),
                value: IntegerValue::Unsigned(10),
            },
            TerminalAbstractOperation::WrappingIntegerSubtract {
                psi_operation: psi_core::OperationId::new(52).expect("subtract operation"),
                result: difference,
                scalar_type,
                left,
                right,
            },
        ],
    );
    let TerminalAbstractOperation::Return { value, .. } =
        function.operations.last_mut().expect("return")
    else {
        unreachable!("fixture ends in return")
    };
    *value = difference;

    let lowered = lower_to_target_operations(&plan, NativeTarget::linux_x64()).unwrap();
    assert!(matches!(
        lowered.functions[0].operation,
        TerminalTargetOperation::ReturnIntegerImmediate {
            source_value,
            scalar_type: result_type,
            value: IntegerValue::Unsigned(251),
            ..
        } if source_value == difference && result_type == scalar_type
    ));
}

#[test]
fn folds_closed_saturating_subtraction_at_zero() {
    let mut plan = parameter_return_plan(1);
    let function = &mut plan.functions[0];
    let left = ValueId::new(50).expect("left");
    let right = ValueId::new(51).expect("right");
    let difference = ValueId::new(52).expect("difference");
    let scalar_type = match scalar_result(function).scalar_type {
        ScalarType::Integer(integer) => integer,
        ScalarType::Boolean => unreachable!("fixture is integer"),
    };
    function.operations.splice(
        0..0,
        [
            TerminalAbstractOperation::IntegerConstant {
                psi_operation: psi_core::OperationId::new(50).expect("left operation"),
                result: left,
                scalar_type: ScalarType::Integer(scalar_type),
                value: IntegerValue::Unsigned(5),
            },
            TerminalAbstractOperation::IntegerConstant {
                psi_operation: psi_core::OperationId::new(51).expect("right operation"),
                result: right,
                scalar_type: ScalarType::Integer(scalar_type),
                value: IntegerValue::Unsigned(10),
            },
            TerminalAbstractOperation::SaturatingIntegerSubtract {
                psi_operation: psi_core::OperationId::new(52).expect("subtract operation"),
                result: difference,
                scalar_type,
                left,
                right,
            },
        ],
    );
    let TerminalAbstractOperation::Return { value, .. } =
        function.operations.last_mut().expect("return")
    else {
        unreachable!("fixture ends in return")
    };
    *value = difference;

    let lowered = lower_to_target_operations(&plan, NativeTarget::linux_x64()).unwrap();
    assert!(matches!(
        lowered.functions[0].operation,
        TerminalTargetOperation::ReturnIntegerImmediate {
            source_value,
            scalar_type: result_type,
            value: IntegerValue::Unsigned(0),
            ..
        } if source_value == difference && result_type == scalar_type
    ));
}

#[test]
fn folds_closed_wrapping_multiplication_at_the_declared_width() {
    let mut plan = parameter_return_plan(1);
    let function = &mut plan.functions[0];
    let left = ValueId::new(50).expect("left");
    let right = ValueId::new(51).expect("right");
    let product = ValueId::new(52).expect("product");
    let scalar_type = match scalar_result(function).scalar_type {
        ScalarType::Integer(integer) => integer,
        ScalarType::Boolean => unreachable!("fixture is integer"),
    };
    function.operations.splice(
        0..0,
        [
            TerminalAbstractOperation::IntegerConstant {
                psi_operation: psi_core::OperationId::new(50).expect("left operation"),
                result: left,
                scalar_type: ScalarType::Integer(scalar_type),
                value: IntegerValue::Unsigned(20),
            },
            TerminalAbstractOperation::IntegerConstant {
                psi_operation: psi_core::OperationId::new(51).expect("right operation"),
                result: right,
                scalar_type: ScalarType::Integer(scalar_type),
                value: IntegerValue::Unsigned(13),
            },
            TerminalAbstractOperation::WrappingIntegerMultiply {
                psi_operation: psi_core::OperationId::new(52).expect("multiply operation"),
                result: product,
                scalar_type,
                left,
                right,
            },
        ],
    );
    let TerminalAbstractOperation::Return { value, .. } =
        function.operations.last_mut().expect("return")
    else {
        unreachable!("fixture ends in return")
    };
    *value = product;

    let lowered = lower_to_target_operations(&plan, NativeTarget::linux_x64()).unwrap();
    assert!(matches!(
        lowered.functions[0].operation,
        TerminalTargetOperation::ReturnIntegerImmediate {
            source_value,
            scalar_type: result_type,
            value: IntegerValue::Unsigned(4),
            ..
        } if source_value == product && result_type == scalar_type
    ));
}

#[test]
fn folds_closed_saturating_multiplication_at_the_declared_width() {
    let mut plan = parameter_return_plan(1);
    let function = &mut plan.functions[0];
    let left = ValueId::new(50).expect("left");
    let right = ValueId::new(51).expect("right");
    let product = ValueId::new(52).expect("product");
    let scalar_type = match scalar_result(function).scalar_type {
        ScalarType::Integer(integer) => integer,
        ScalarType::Boolean => unreachable!("fixture is integer"),
    };
    function.operations.splice(
        0..0,
        [
            TerminalAbstractOperation::IntegerConstant {
                psi_operation: psi_core::OperationId::new(50).expect("left operation"),
                result: left,
                scalar_type: ScalarType::Integer(scalar_type),
                value: IntegerValue::Unsigned(20),
            },
            TerminalAbstractOperation::IntegerConstant {
                psi_operation: psi_core::OperationId::new(51).expect("right operation"),
                result: right,
                scalar_type: ScalarType::Integer(scalar_type),
                value: IntegerValue::Unsigned(13),
            },
            TerminalAbstractOperation::SaturatingIntegerMultiply {
                psi_operation: psi_core::OperationId::new(52).expect("multiply operation"),
                result: product,
                scalar_type,
                left,
                right,
            },
        ],
    );
    let TerminalAbstractOperation::Return { value, .. } =
        function.operations.last_mut().expect("return")
    else {
        unreachable!("fixture ends in return")
    };
    *value = product;

    let lowered = lower_to_target_operations(&plan, NativeTarget::linux_x64()).unwrap();
    assert!(matches!(
        lowered.functions[0].operation,
        TerminalTargetOperation::ReturnIntegerImmediate {
            source_value,
            scalar_type: result_type,
            value: IntegerValue::Unsigned(255),
            ..
        } if source_value == product && result_type == scalar_type
    ));
}

#[test]
fn lowers_a_boolean_runtime_parameter_with_its_selected_abi_location() {
    let mut plan = parameter_return_plan(1);
    let function = &mut plan.functions[0];
    function.parameters[0].scalar_type = ScalarType::Boolean;
    scalar_result_mut(function).scalar_type = ScalarType::Boolean;
    let TerminalAbstractOperation::Return { scalar_type, .. } = &mut function.operations[0] else {
        unreachable!("fixture ends in return")
    };
    *scalar_type = ScalarType::Boolean;

    let lowered = lower_to_target_operations(&plan, NativeTarget::linux_x64()).unwrap();
    assert!(matches!(
        lowered.functions[0].operation,
        TerminalTargetOperation::ReturnBooleanParameter {
            parameter_index: 0,
            location: TerminalScalarParameterLocation::Register(MachineRegister::X86Rdi),
            ..
        }
    ));
}

#[test]
fn lowers_runtime_boolean_equality_to_a_target_expression() {
    let mut plan = parameter_return_plan(2);
    let function = &mut plan.functions[0];
    for parameter in &mut function.parameters {
        parameter.scalar_type = ScalarType::Boolean;
    }
    scalar_result_mut(function).scalar_type = ScalarType::Boolean;
    let result = ValueId::new(50).expect("equality result");
    function.operations.insert(
        0,
        TerminalAbstractOperation::BooleanEqual {
            psi_operation: OperationId::new(50).expect("equality operation"),
            result,
            left: function.parameters[0].value,
            right: function.parameters[1].value,
        },
    );
    let TerminalAbstractOperation::Return {
        value, scalar_type, ..
    } = &mut function.operations[1]
    else {
        unreachable!("fixture ends in return")
    };
    *value = result;
    *scalar_type = ScalarType::Boolean;

    let lowered = lower_to_target_operations(&plan, NativeTarget::linux_x64()).unwrap();
    assert!(matches!(
        &lowered.functions[0].operation,
        TerminalTargetOperation::ReturnBooleanExpression {
            source_value,
            expression: TerminalTargetBooleanExpression::Equal {
                psi_operation,
                left,
                right,
            },
            ..
        } if *source_value == result
            && *psi_operation == OperationId::new(50).expect("equality operation")
            && matches!(
                left.as_ref(),
                TerminalTargetBooleanExpression::Parameter { parameter_index: 0, .. }
            )
            && matches!(
                right.as_ref(),
                TerminalTargetBooleanExpression::Parameter { parameter_index: 1, .. }
            )
    ));
}

#[test]
fn lowers_runtime_integer_equality_to_a_typed_target_expression() {
    let mut plan = parameter_return_plan(2);
    let function = &mut plan.functions[0];
    let integer_type = match function.parameters[0].scalar_type {
        ScalarType::Integer(integer_type) => integer_type,
        ScalarType::Boolean => unreachable!("fixture has integer parameters"),
    };
    scalar_result_mut(function).scalar_type = ScalarType::Boolean;
    let result = ValueId::new(51).expect("integer-equality result");
    function.operations.insert(
        0,
        TerminalAbstractOperation::IntegerEqual {
            psi_operation: OperationId::new(51).expect("integer-equality operation"),
            result,
            left: function.parameters[0].value,
            right: function.parameters[1].value,
        },
    );
    let TerminalAbstractOperation::Return {
        value, scalar_type, ..
    } = &mut function.operations[1]
    else {
        unreachable!("fixture ends in return")
    };
    *value = result;
    *scalar_type = ScalarType::Boolean;

    let lowered = lower_to_target_operations(&plan, NativeTarget::linux_x64()).unwrap();
    assert!(matches!(
        &lowered.functions[0].operation,
        TerminalTargetOperation::ReturnBooleanExpression {
            source_value,
            expression: TerminalTargetBooleanExpression::IntegerEqual {
                psi_operation,
                scalar_type,
                left,
                right,
            },
            ..
        } if *source_value == result
            && *psi_operation == OperationId::new(51).expect("integer-equality operation")
            && *scalar_type == integer_type
            && matches!(
                left.as_ref(),
                TerminalTargetIntegerExpression::Parameter { parameter_index: 0, .. }
            )
            && matches!(
                right.as_ref(),
                TerminalTargetIntegerExpression::Parameter { parameter_index: 1, .. }
            )
    ));
}

#[test]
fn folds_a_compile_known_conditional_to_only_the_selected_arm() {
    let condition_operation = psi_core::OperationId::new(20).expect("condition operation");
    let true_operation = psi_core::OperationId::new(21).expect("true operation");
    let false_operation = psi_core::OperationId::new(22).expect("false operation");
    let true_edge = EdgeId::new(1).expect("true edge");
    let false_edge = EdgeId::new(2).expect("false edge");
    let true_return = EdgeId::new(3).expect("true return");
    let false_return = EdgeId::new(4).expect("false return");

    for (select_true, selected_operation, selected_edges) in [
        (true, true_operation, [true_edge, true_return]),
        (false, false_operation, [false_edge, false_return]),
    ] {
        let plan = constant_conditional_plan(select_true);
        let lowered = lower_to_target_operations(&plan, NativeTarget::linux_x64()).expect("lower");
        let function = &lowered.functions[0];
        assert_eq!(
            function.provenance.operations,
            [condition_operation, selected_operation]
        );
        assert_eq!(function.provenance.edges, selected_edges);
        assert!(
            matches!(
                &function.operation,
                TerminalTargetOperation::ReturnIntegerExpression {
                    psi_edge,
                    expression:
                        TerminalTargetIntegerExpression::WrappingAdd { psi_operation, .. },
                    ..
                } if select_true && *psi_edge == true_return && *psi_operation == true_operation
            ) || matches!(
                &function.operation,
                TerminalTargetOperation::ReturnIntegerExpression {
                    psi_edge,
                    expression:
                        TerminalTargetIntegerExpression::SaturatingMultiply {
                            psi_operation,
                            ..
                        },
                    ..
                } if !select_true && *psi_edge == false_return && *psi_operation == false_operation
            )
        );
    }
}

fn constant_conditional_plan(select_true: bool) -> TerminalAbstractOperationPlan {
    let machine = MachineId::new(20).expect("machine");
    let integer = IntegerType::new(psi_core::IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    let argument = ValueId::new(1).expect("argument");
    let condition = ValueId::new(2).expect("condition");
    let true_parameter = ValueId::new(3).expect("true parameter");
    let false_parameter = ValueId::new(4).expect("false parameter");
    let true_value = ValueId::new(5).expect("true value");
    let false_value = ValueId::new(6).expect("false value");
    let result = ValueId::new(7).expect("result");
    let true_edge = EdgeId::new(1).expect("true edge");
    let false_edge = EdgeId::new(2).expect("false edge");
    let true_return = EdgeId::new(3).expect("true return");
    let false_return = EdgeId::new(4).expect("false return");
    TerminalAbstractOperationPlan {
        terminal_psi: identity(),
        entry: machine,
        structural_types: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![TerminalAbstractFunction {
            machine,
            attachment: None,
            entry: BlockId::new(1).expect("entry block"),
            parameters: vec![TerminalAbstractParameter {
                value: argument,
                scalar_type,
            }],
            structural_parameters: Vec::new(),
            result: TerminalAbstractFunctionResult::Scalar(TerminalAbstractResult {
                value: result,
                scalar_type,
            }),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: vec![
                omega_terminal_abstract_operations::TerminalAbstractBlockEntry {
                    block: BlockId::new(1).expect("entry block"),
                    operation_offset: 0,
                },
                omega_terminal_abstract_operations::TerminalAbstractBlockEntry {
                    block: BlockId::new(2).expect("true block"),
                    operation_offset: 2,
                },
                omega_terminal_abstract_operations::TerminalAbstractBlockEntry {
                    block: BlockId::new(3).expect("false block"),
                    operation_offset: 4,
                },
            ],
            operations: vec![
                TerminalAbstractOperation::BooleanConstant {
                    psi_operation: psi_core::OperationId::new(20).expect("condition operation"),
                    result: condition,
                    value: select_true,
                },
                TerminalAbstractOperation::Conditional {
                    condition,
                    when_true: TerminalAbstractSuccessor {
                        psi_edge: true_edge,
                        target: BlockId::new(2).expect("true block"),
                        bindings: vec![TerminalValueBinding {
                            parameter: true_parameter,
                            argument,
                            scalar_type,
                        }],
                    },
                    when_false: TerminalAbstractSuccessor {
                        psi_edge: false_edge,
                        target: BlockId::new(3).expect("false block"),
                        bindings: vec![TerminalValueBinding {
                            parameter: false_parameter,
                            argument,
                            scalar_type,
                        }],
                    },
                },
                TerminalAbstractOperation::WrappingIntegerAdd {
                    psi_operation: psi_core::OperationId::new(21).expect("true operation"),
                    result: true_value,
                    scalar_type: integer,
                    left: true_parameter,
                    right: true_parameter,
                },
                TerminalAbstractOperation::Return {
                    psi_edge: true_return,
                    result,
                    value: true_value,
                    scalar_type,
                    cleanup_actions: Vec::new(),
                },
                TerminalAbstractOperation::SaturatingIntegerMultiply {
                    psi_operation: psi_core::OperationId::new(22).expect("false operation"),
                    result: false_value,
                    scalar_type: integer,
                    left: false_parameter,
                    right: false_parameter,
                },
                TerminalAbstractOperation::Return {
                    psi_edge: false_return,
                    result,
                    value: false_value,
                    scalar_type,
                    cleanup_actions: Vec::new(),
                },
            ],
        }],
    }
}

fn parameter_return_plan(parameter_count: usize) -> TerminalAbstractOperationPlan {
    let machine = MachineId::new(10).expect("machine");
    let result = ValueId::new(100).expect("result");
    let integer = IntegerType::new(psi_core::IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    let parameters = (0..parameter_count)
        .map(|index| TerminalAbstractParameter {
            value: ValueId::new(10 + index as u64).expect("parameter"),
            scalar_type,
        })
        .collect::<Vec<_>>();
    let returned = parameters.last().expect("fixture has parameters").value;
    TerminalAbstractOperationPlan {
        terminal_psi: identity(),
        entry: machine,
        structural_types: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![TerminalAbstractFunction {
            machine,
            attachment: None,
            entry: BlockId::new(10).expect("block"),
            parameters,
            structural_parameters: Vec::new(),
            result: TerminalAbstractFunctionResult::Scalar(TerminalAbstractResult {
                value: result,
                scalar_type,
            }),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: Vec::new(),
            operations: vec![TerminalAbstractOperation::Return {
                psi_edge: EdgeId::new(10).expect("edge"),
                result,
                value: returned,
                scalar_type,
                cleanup_actions: Vec::new(),
            }],
        }],
    }
}

fn direct_call_plan(parameter_count: usize) -> TerminalAbstractOperationPlan {
    let caller = MachineId::new(1).expect("caller");
    let callee = MachineId::new(2).expect("callee");
    let integer = IntegerType::new(psi_core::IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    let caller_parameters = (0..parameter_count)
        .map(|index| TerminalAbstractParameter {
            value: ValueId::new(10 + index as u64).expect("caller parameter"),
            scalar_type,
        })
        .collect::<Vec<_>>();
    let callee_parameters = (0..parameter_count)
        .map(|index| TerminalAbstractParameter {
            value: ValueId::new(30 + index as u64).expect("callee parameter"),
            scalar_type,
        })
        .collect::<Vec<_>>();
    let caller_result = ValueId::new(100).expect("caller result");
    let callee_result = ValueId::new(101).expect("callee result");
    TerminalAbstractOperationPlan {
        terminal_psi: identity(),
        entry: caller,
        structural_types: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![
            TerminalAbstractFunction {
                machine: caller,
                attachment: None,
                entry: BlockId::new(1).expect("caller block"),
                parameters: caller_parameters.clone(),
                structural_parameters: Vec::new(),
                result: TerminalAbstractFunctionResult::Scalar(TerminalAbstractResult {
                    value: caller_result,
                    scalar_type,
                }),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: Vec::new(),
                operations: vec![
                    TerminalAbstractOperation::Call {
                        psi_operation: OperationId::new(1).expect("call"),
                        result: caller_result,
                        scalar_type,
                        callee,
                        arguments: caller_parameters
                            .iter()
                            .map(|parameter| parameter.value)
                            .collect(),
                    },
                    TerminalAbstractOperation::Return {
                        psi_edge: EdgeId::new(1).expect("caller return"),
                        result: caller_result,
                        value: caller_result,
                        scalar_type,
                        cleanup_actions: Vec::new(),
                    },
                ],
            },
            TerminalAbstractFunction {
                machine: callee,
                attachment: None,
                entry: BlockId::new(2).expect("callee block"),
                parameters: callee_parameters.clone(),
                structural_parameters: Vec::new(),
                result: TerminalAbstractFunctionResult::Scalar(TerminalAbstractResult {
                    value: callee_result,
                    scalar_type,
                }),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: Vec::new(),
                operations: vec![TerminalAbstractOperation::Return {
                    psi_edge: EdgeId::new(2).expect("callee return"),
                    result: callee_result,
                    value: callee_parameters.last().expect("parameter").value,
                    scalar_type,
                    cleanup_actions: Vec::new(),
                }],
            },
        ],
    }
}

fn identity() -> TerminalPsiIdentity {
    TerminalPsiIdentity {
        vocabulary_marker: VocabularyMarker::CURRENT,
        program_fingerprint: SemanticFingerprint::from_bytes([7; 32]),
    }
}

#[test]
fn linux_exit_group_i32_requires_exact_literal_shape_and_stays_fail_closed_elsewhere() {
    let machine = MachineId::new(901).unwrap();
    let boundary = BoundaryMachineId::new(901).unwrap();
    let constant_operation = OperationId::new(901).unwrap();
    let settlement_operation = OperationId::new(902).unwrap();
    let return_edge = EdgeId::new(901).unwrap();
    let value = ValueId::new(901).unwrap();
    let block = BlockId::new(901).unwrap();
    let i32_type = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    let scalar_type = ScalarType::Integer(i32_type);
    let provider_execution =
        omega_terminal_target_operations::TerminalProviderExecutionBinding::from_execution_record(
            omega_terminal_target_operations::TerminalProviderPlanIdentity::new(901).unwrap(),
            902,
            903,
            904,
            905,
        )
        .unwrap();
    let plan = TerminalAbstractOperationPlan {
        terminal_psi: identity(),
        entry: machine,
        structural_types: Vec::new(),
        boundary_machines: vec![BoundaryMachineDeclaration {
            id: boundary,
            identity: "Console::exit_process(i32)->Unit".into(),
            attachment: None,
            scalar_parameters: vec![scalar_type],
            structural_parameters: Vec::new(),
            result: None,
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: Vec::new(),
        }],
        provider_candidates: Vec::new(),
        functions: vec![TerminalAbstractFunction {
            machine,
            attachment: None,
            entry: block,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: TerminalAbstractFunctionResult::Unit,
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: vec![
                omega_terminal_abstract_operations::TerminalAbstractBlockEntry {
                    block,
                    operation_offset: 0,
                },
            ],
            operations: vec![
                TerminalAbstractOperation::IntegerConstant {
                    psi_operation: constant_operation,
                    result: value,
                    scalar_type,
                    value: IntegerValue::Signed(37),
                },
                TerminalAbstractOperation::BoundaryCall {
                    psi_operation: settlement_operation,
                    result: None,
                    boundary,
                    arguments: vec![value],
                    structural_arguments: Vec::new(),
                    completion_claim_sources: Vec::new(),
                    completion_receipts: Vec::new(),
                },
                TerminalAbstractOperation::ReturnUnit {
                    psi_edge: return_edge,
                    cleanup_actions: Vec::new(),
                },
            ],
        }],
    };
    let binding = omega_terminal_target_operations::TerminalBoundarySettlementBinding {
        boundary,
        provider_execution,
        realization: omega_terminal_target_operations::TerminalLinuxExitGroupI32Realization.into(),
    };

    let x86 =
        lower_to_target_operations_with_settlements(&plan, NativeTarget::linux_x64(), &[binding])
            .expect("Linux x86-64 exit_group lowering");
    assert_eq!(
        x86,
        lower_to_target_operations_with_settlements(&plan, NativeTarget::linux_x64(), &[binding],)
            .expect("deterministic lowering")
    );
    assert!(matches!(
        &x86.functions[0].operation,
        TerminalTargetOperation::ExitProcessI32 { argument, nominal_return_edge, .. }
            if argument.source_value == value
                && argument.scalar_type == scalar_type
                && argument.immediate == IntegerValue::Signed(37)
                && argument.destination == MachineRegister::X86Rdi
                && *nominal_return_edge == return_edge
    ));
    let arm =
        lower_to_target_operations_with_settlements(&plan, NativeTarget::linux_arm64(), &[binding])
            .expect("Linux AArch64 exit_group lowering");
    assert!(matches!(
        &arm.functions[0].operation,
        TerminalTargetOperation::ExitProcessI32 { argument, .. }
            if argument.destination == MachineRegister::Aarch64X(0)
    ));
    assert!(matches!(
        lower_to_target_operations_with_settlements(&plan, NativeTarget::windows_x64(), &[binding],),
        Err(LoweringError::LinuxExitGroupUnsupportedTarget { .. })
    ));
    assert!(matches!(
        lower_to_target_operations_with_settlements(&plan, NativeTarget::macos_arm64(), &[binding],),
        Err(LoweringError::LinuxExitGroupUnsupportedTarget { .. })
    ));

    let mut wrong_signature = plan;
    wrong_signature.boundary_machines[0].scalar_parameters[0] = ScalarType::Boolean;
    assert_eq!(
        lower_to_target_operations_with_settlements(
            &wrong_signature,
            NativeTarget::linux_x64(),
            &[binding],
        ),
        Err(LoweringError::InvalidLinuxExitGroupShape(machine))
    );
}

#[test]
fn linux_write_line_and_exit_compose_in_one_shared_unit_body() {
    let machine = MachineId::new(920).unwrap();
    let block = BlockId::new(920).unwrap();
    let write_boundary = BoundaryMachineId::new(920).unwrap();
    let exit_boundary = BoundaryMachineId::new(921).unwrap();
    let byte_type = StructuralTypeId::new(920).unwrap();
    let literal_place = PlaceId::new(920).unwrap();
    let exit_value = ValueId::new(920).unwrap();
    let literal_operation = OperationId::new(920).unwrap();
    let write_operation = OperationId::new(921).unwrap();
    let constant_operation = OperationId::new(922).unwrap();
    let exit_operation = OperationId::new(923).unwrap();
    let return_edge = EdgeId::new(920).unwrap();
    let bytes = vec![0, 0x80, 0xff];
    let byte_declaration = StructuralTypeDeclaration {
        id: byte_type,
        identity: "test::BorrowedBytes".into(),
        shape: StructuralTypeShape::ByteSequence(psi_terminal::ByteSequenceCarrier::BorrowedView),
    };
    let literal_declaration = psi_terminal::StructuralPlaceDeclaration {
        id: literal_place,
        kind: psi_core::StructuralPlaceKind::ByteSequenceLiteral {
            declaration_ordinal: 0,
            structural_type: byte_type,
        },
    };
    let i32_type = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    let plan = TerminalAbstractOperationPlan {
        terminal_psi: identity(),
        entry: machine,
        structural_types: vec![byte_declaration.clone()],
        boundary_machines: vec![
            BoundaryMachineDeclaration {
                id: write_boundary,
                identity: "Console::write_line(&[u8])->Unit".into(),
                attachment: None,
                scalar_parameters: Vec::new(),
                structural_parameters: vec![StructuralParameterDeclaration {
                    place: PlaceId::new(921).unwrap(),
                    position: 0,
                    is_self: false,
                    structural_type: byte_type,
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    access: StructuralAccess::SharedBorrow,
                    qualifications: Vec::new(),
                }],
                result: None,
                requires: Vec::new(),
                program_local_root_introductions: Vec::new(),
                content_guarantees: Vec::new(),
                published_service_ceiling: Vec::new(),
            },
            BoundaryMachineDeclaration {
                id: exit_boundary,
                identity: "Console::exit_process(i32)->Unit".into(),
                attachment: None,
                scalar_parameters: vec![ScalarType::Integer(i32_type)],
                structural_parameters: Vec::new(),
                result: None,
                requires: Vec::new(),
                program_local_root_introductions: Vec::new(),
                content_guarantees: Vec::new(),
                published_service_ceiling: Vec::new(),
            },
        ],
        provider_candidates: Vec::new(),
        functions: vec![TerminalAbstractFunction {
            machine,
            attachment: None,
            entry: block,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: TerminalAbstractFunctionResult::Unit,
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: vec![TerminalAbstractBlockEntry {
                block,
                operation_offset: 0,
            }],
            operations: vec![
                TerminalAbstractOperation::EstablishByteSequenceLiteral {
                    psi_operation: literal_operation,
                    place: literal_declaration,
                    structural_type: byte_declaration,
                    bytes: bytes.clone(),
                },
                TerminalAbstractOperation::BoundaryCall {
                    psi_operation: write_operation,
                    result: None,
                    boundary: write_boundary,
                    arguments: Vec::new(),
                    structural_arguments: vec![StructuralArgument {
                        place: literal_place,
                        access: StructuralAccess::SharedBorrow,
                        path: Vec::new(),
                    }],
                    completion_claim_sources: Vec::new(),
                    completion_receipts: Vec::new(),
                },
                TerminalAbstractOperation::IntegerConstant {
                    psi_operation: constant_operation,
                    result: exit_value,
                    scalar_type: ScalarType::Integer(i32_type),
                    value: IntegerValue::Signed(37),
                },
                TerminalAbstractOperation::BoundaryCall {
                    psi_operation: exit_operation,
                    result: None,
                    boundary: exit_boundary,
                    arguments: vec![exit_value],
                    structural_arguments: Vec::new(),
                    completion_claim_sources: Vec::new(),
                    completion_receipts: Vec::new(),
                },
                TerminalAbstractOperation::ReturnUnit {
                    psi_edge: return_edge,
                    cleanup_actions: Vec::new(),
                },
            ],
        }],
    };
    let provider = |seed| {
        omega_terminal_target_operations::TerminalProviderExecutionBinding::from_execution_record(
            omega_terminal_target_operations::TerminalProviderPlanIdentity::new(seed).unwrap(),
            seed + 1,
            seed + 2,
            seed + 3,
            seed + 4,
        )
        .unwrap()
    };
    let settlements = [
        omega_terminal_target_operations::TerminalBoundarySettlementBinding {
            boundary: write_boundary,
            provider_execution: provider(920),
            realization: omega_terminal_target_operations::TerminalLinuxWriteLineRealization.into(),
        },
        omega_terminal_target_operations::TerminalBoundarySettlementBinding {
            boundary: exit_boundary,
            provider_execution: provider(930),
            realization: omega_terminal_target_operations::TerminalLinuxExitGroupI32Realization
                .into(),
        },
    ];

    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let lowered = lower_to_target_operations_with_settlements(&plan, target, &settlements)
            .expect("composed Linux effect body lowers");
        let TerminalTargetOperation::UnitBody(body) = &lowered.functions[0].operation else {
            panic!("write_line -> exit_process remains a shared Unit body")
        };
        assert!(matches!(
            &body.operations[0],
            TerminalTargetUnitOperation::EstablishByteSequenceLiteral { bytes: actual, .. }
                if actual == &bytes
        ));
        assert!(matches!(
            &body.operations[1],
            TerminalTargetUnitOperation::BoundarySettlement {
                realization: omega_terminal_target_operations::TerminalBoundaryRealization::LinuxWriteLine(_),
                byte_sequence_arguments,
                ..
            } if byte_sequence_arguments[0].bytes == bytes
        ));
        assert!(matches!(
            &body.operations[3],
            TerminalTargetUnitOperation::BoundarySettlement {
                realization: omega_terminal_target_operations::TerminalBoundaryRealization::LinuxExitGroupI32(_),
                scalar_arguments,
                ..
            } if scalar_arguments[0].immediate == IntegerValue::Signed(37)
        ));
    }
    assert!(matches!(
        lower_to_target_operations_with_settlements(
            &plan,
            NativeTarget::windows_x64(),
            &settlements,
        ),
        Err(LoweringError::LinuxWriteLineUnsupportedOrInvalid { .. })
            | Err(LoweringError::LinuxExitGroupUnsupportedTarget { .. })
    ));
    assert!(matches!(
        lower_to_target_operations_with_settlements(
            &plan,
            NativeTarget::macos_arm64(),
            &settlements,
        ),
        Err(LoweringError::LinuxWriteLineUnsupportedOrInvalid { .. })
            | Err(LoweringError::LinuxExitGroupUnsupportedTarget { .. })
    ));
}

fn scalar_result(function: &TerminalAbstractFunction) -> TerminalAbstractResult {
    function.result.scalar().expect("fixture is scalar")
}

fn scalar_result_mut(function: &mut TerminalAbstractFunction) -> &mut TerminalAbstractResult {
    let TerminalAbstractFunctionResult::Scalar(result) = &mut function.result else {
        panic!("fixture is scalar")
    };
    result
}
