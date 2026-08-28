//! Structural Unit control-flow regression families.

use super::*;

fn install_structural_unit_control_fixture(checked: &mut CheckedTrees) {
    let root = SymbolHandle::from_arena_index(1);
    let entry = SymbolHandle::from_arena_index(11);
    let leaf = SymbolHandle::from_arena_index(14);
    let affine_parameter = |position| psi_checked_trees::CheckedUnitStructuralParameterPlan {
        position,
        is_self: false,
        type_identity: "example::Acknowledgement".to_owned(),
        multiplicity: Multiplicity::Affine,
        access: psi_checked_trees::CheckedStructuralAccess::Owned,
        qualifications: Vec::new(),
    };
    checked.facts.flow.terminal_structural_unit_controls =
        psi_checked_trees::CheckedStructuralUnitControlPlans {
            structural_types: checked
                .facts
                .flow
                .terminal_unit_effects
                .structural_types
                .clone(),
            machines: vec![CheckedStructuralUnitControlMachinePlan {
                machine: root,
                attachment_type_identity: "example::Root".to_owned(),
                ranked_scc: None,
                states: vec![
                    psi_checked_trees::CheckedStructuralUnitControlStatePlan {
                        state: entry,
                        structural_parameters: vec![affine_parameter(0), affine_parameter(1)],
                        scalar_parameters: vec![
                            psi_checked_trees::CheckedStructuralScalarParameterPlan {
                                source_position: 2,
                                primitive_type: PrimitiveType::I32,
                            },
                        ],
                        terminator: CheckedStructuralUnitControlTerminatorPlan::Jump {
                            statement_ordinal: 0,
                            target_state: leaf,
                            transfers: vec![
                                psi_checked_trees::CheckedStructuralControlTransferPlan {
                                    source_parameter_index: 1,
                                    target_parameter_index: 0,
                                },
                            ],
                            scalar_arguments: vec![
                                psi_checked_trees::CheckedStructuralScalarArgumentPlan {
                                    argument_ordinal: 1,
                                    source_scalar_parameter_index: 0,
                                    target_scalar_parameter_index: 0,
                                    primitive_type: PrimitiveType::I32,
                                },
                            ],
                            trivial_affine_discard_parameter_positions: vec![0],
                        },
                    },
                    psi_checked_trees::CheckedStructuralUnitControlStatePlan {
                        state: leaf,
                        structural_parameters: vec![affine_parameter(0)],
                        scalar_parameters: vec![
                            psi_checked_trees::CheckedStructuralScalarParameterPlan {
                                source_position: 1,
                                primitive_type: PrimitiveType::I32,
                            },
                        ],
                        terminator: CheckedStructuralUnitControlTerminatorPlan::ReturnUnit {
                            trivial_affine_discard_parameter_positions: vec![0],
                        },
                    },
                ],
            }],
        };
}

fn install_structural_unit_conditional_fixture(checked: &mut CheckedTrees) {
    let root = SymbolHandle::from_arena_index(1);
    let entry = SymbolHandle::from_arena_index(11);
    let true_leaf = SymbolHandle::from_arena_index(12);
    let false_leaf = SymbolHandle::from_arena_index(13);
    let affine_parameter = |position| psi_checked_trees::CheckedUnitStructuralParameterPlan {
        position,
        is_self: false,
        type_identity: "example::Acknowledgement".to_owned(),
        multiplicity: Multiplicity::Affine,
        access: psi_checked_trees::CheckedStructuralAccess::Owned,
        qualifications: Vec::new(),
    };
    let leaf = |state| psi_checked_trees::CheckedStructuralUnitControlStatePlan {
        state,
        structural_parameters: vec![affine_parameter(0)],
        scalar_parameters: vec![psi_checked_trees::CheckedStructuralScalarParameterPlan {
            source_position: 1,
            primitive_type: PrimitiveType::I32,
        }],
        terminator: CheckedStructuralUnitControlTerminatorPlan::ReturnUnit {
            trivial_affine_discard_parameter_positions: vec![0],
        },
    };
    checked.facts.flow.terminal_structural_unit_controls =
        psi_checked_trees::CheckedStructuralUnitControlPlans {
            structural_types: checked
                .facts
                .flow
                .terminal_unit_effects
                .structural_types
                .clone(),
            machines: vec![CheckedStructuralUnitControlMachinePlan {
                machine: root,
                attachment_type_identity: "example::Root".to_owned(),
                ranked_scc: None,
                states: vec![
                    psi_checked_trees::CheckedStructuralUnitControlStatePlan {
                        state: entry,
                        structural_parameters: vec![affine_parameter(0), affine_parameter(1)],
                        scalar_parameters: vec![
                            psi_checked_trees::CheckedStructuralScalarParameterPlan {
                                source_position: 2,
                                primitive_type: PrimitiveType::Bool,
                            },
                            psi_checked_trees::CheckedStructuralScalarParameterPlan {
                                source_position: 3,
                                primitive_type: PrimitiveType::I32,
                            },
                        ],
                        terminator: CheckedStructuralUnitControlTerminatorPlan::Conditional {
                            guard_scalar_parameter_index: 0,
                            when_true: psi_checked_trees::CheckedStructuralControlSuccessorPlan {
                                statement_ordinal: 0,
                                target_state: true_leaf,
                                transfers: vec![
                                    psi_checked_trees::CheckedStructuralControlTransferPlan {
                                        source_parameter_index: 0,
                                        target_parameter_index: 0,
                                    },
                                ],
                                scalar_arguments: vec![
                                    psi_checked_trees::CheckedStructuralScalarArgumentPlan {
                                        argument_ordinal: 1,
                                        source_scalar_parameter_index: 1,
                                        target_scalar_parameter_index: 0,
                                        primitive_type: PrimitiveType::I32,
                                    },
                                ],
                                trivial_affine_discard_parameter_positions: vec![1],
                            },
                            when_false: psi_checked_trees::CheckedStructuralControlSuccessorPlan {
                                statement_ordinal: 1,
                                target_state: false_leaf,
                                transfers: vec![
                                    psi_checked_trees::CheckedStructuralControlTransferPlan {
                                        source_parameter_index: 1,
                                        target_parameter_index: 0,
                                    },
                                ],
                                scalar_arguments: vec![
                                    psi_checked_trees::CheckedStructuralScalarArgumentPlan {
                                        argument_ordinal: 1,
                                        source_scalar_parameter_index: 1,
                                        target_scalar_parameter_index: 0,
                                        primitive_type: PrimitiveType::I32,
                                    },
                                ],
                                trivial_affine_discard_parameter_positions: vec![0],
                            },
                        },
                    },
                    leaf(true_leaf),
                    leaf(false_leaf),
                ],
            }],
        };
}

fn install_structural_unit_nonentry_conditional_fixture(checked: &mut CheckedTrees) {
    install_structural_unit_conditional_fixture(checked);
    let plan = &mut checked
        .facts
        .flow
        .terminal_structural_unit_controls
        .machines[0];
    let conditional_state = plan.states[0].state;
    let structural_parameters = plan.states[0].structural_parameters.clone();
    let scalar_parameters = plan.states[0].scalar_parameters.clone();
    plan.states.insert(
        0,
        psi_checked_trees::CheckedStructuralUnitControlStatePlan {
            state: SymbolHandle::from_arena_index(14),
            structural_parameters,
            scalar_parameters,
            terminator: CheckedStructuralUnitControlTerminatorPlan::Jump {
                statement_ordinal: 0,
                target_state: conditional_state,
                transfers: vec![
                    psi_checked_trees::CheckedStructuralControlTransferPlan {
                        source_parameter_index: 0,
                        target_parameter_index: 0,
                    },
                    psi_checked_trees::CheckedStructuralControlTransferPlan {
                        source_parameter_index: 1,
                        target_parameter_index: 1,
                    },
                ],
                scalar_arguments: vec![
                    psi_checked_trees::CheckedStructuralScalarArgumentPlan {
                        argument_ordinal: 2,
                        source_scalar_parameter_index: 0,
                        target_scalar_parameter_index: 0,
                        primitive_type: PrimitiveType::Bool,
                    },
                    psi_checked_trees::CheckedStructuralScalarArgumentPlan {
                        argument_ordinal: 3,
                        source_scalar_parameter_index: 1,
                        target_scalar_parameter_index: 1,
                        primitive_type: PrimitiveType::I32,
                    },
                ],
                trivial_affine_discard_parameter_positions: Vec::new(),
            },
        },
    );
}

fn install_structural_unit_two_conditional_fixture(checked: &mut CheckedTrees) {
    install_structural_unit_conditional_fixture(checked);
    let plan = &mut checked
        .facts
        .flow
        .terminal_structural_unit_controls
        .machines[0];
    let nested_state = plan.states[1].state;
    let affine_parameter = |position| psi_checked_trees::CheckedUnitStructuralParameterPlan {
        position,
        is_self: false,
        type_identity: "example::Acknowledgement".to_owned(),
        multiplicity: Multiplicity::Affine,
        access: psi_checked_trees::CheckedStructuralAccess::Owned,
        qualifications: Vec::new(),
    };
    let CheckedStructuralUnitControlTerminatorPlan::Conditional { when_true, .. } =
        &mut plan.states[0].terminator
    else {
        unreachable!()
    };
    when_true.scalar_arguments = vec![
        psi_checked_trees::CheckedStructuralScalarArgumentPlan {
            argument_ordinal: 1,
            source_scalar_parameter_index: 0,
            target_scalar_parameter_index: 0,
            primitive_type: PrimitiveType::Bool,
        },
        psi_checked_trees::CheckedStructuralScalarArgumentPlan {
            argument_ordinal: 2,
            source_scalar_parameter_index: 1,
            target_scalar_parameter_index: 1,
            primitive_type: PrimitiveType::I32,
        },
    ];
    let nested_true = SymbolHandle::from_arena_index(14);
    let nested_false = SymbolHandle::from_arena_index(15);
    plan.states[1].scalar_parameters = vec![
        psi_checked_trees::CheckedStructuralScalarParameterPlan {
            source_position: 1,
            primitive_type: PrimitiveType::Bool,
        },
        psi_checked_trees::CheckedStructuralScalarParameterPlan {
            source_position: 2,
            primitive_type: PrimitiveType::I32,
        },
    ];
    let nested_successor = |statement_ordinal, target_state| {
        psi_checked_trees::CheckedStructuralControlSuccessorPlan {
            statement_ordinal,
            target_state,
            transfers: vec![psi_checked_trees::CheckedStructuralControlTransferPlan {
                source_parameter_index: 0,
                target_parameter_index: 0,
            }],
            scalar_arguments: vec![psi_checked_trees::CheckedStructuralScalarArgumentPlan {
                argument_ordinal: 1,
                source_scalar_parameter_index: 1,
                target_scalar_parameter_index: 0,
                primitive_type: PrimitiveType::I32,
            }],
            trivial_affine_discard_parameter_positions: Vec::new(),
        }
    };
    plan.states[1].terminator = CheckedStructuralUnitControlTerminatorPlan::Conditional {
        guard_scalar_parameter_index: 0,
        when_true: nested_successor(0, nested_true),
        when_false: nested_successor(1, nested_false),
    };
    let leaf = |state| psi_checked_trees::CheckedStructuralUnitControlStatePlan {
        state,
        structural_parameters: vec![affine_parameter(0)],
        scalar_parameters: vec![psi_checked_trees::CheckedStructuralScalarParameterPlan {
            source_position: 1,
            primitive_type: PrimitiveType::I32,
        }],
        terminator: CheckedStructuralUnitControlTerminatorPlan::ReturnUnit {
            trivial_affine_discard_parameter_positions: vec![0],
        },
    };
    plan.states.push(leaf(nested_true));
    plan.states.push(leaf(nested_false));
    assert_eq!(plan.states[1].state, nested_state);
}

fn install_structural_unit_join_fixture(checked: &mut CheckedTrees) {
    install_structural_unit_conditional_fixture(checked);
    let plan = &mut checked
        .facts
        .flow
        .terminal_structural_unit_controls
        .machines[0];
    let affine_parameter = |position| psi_checked_trees::CheckedUnitStructuralParameterPlan {
        position,
        is_self: false,
        type_identity: "example::Acknowledgement".to_owned(),
        multiplicity: Multiplicity::Affine,
        access: psi_checked_trees::CheckedStructuralAccess::Owned,
        qualifications: Vec::new(),
    };
    plan.states[0].scalar_parameters.push(
        psi_checked_trees::CheckedStructuralScalarParameterPlan {
            source_position: 4,
            primitive_type: PrimitiveType::I32,
        },
    );
    let CheckedStructuralUnitControlTerminatorPlan::Conditional { when_false, .. } =
        &mut plan.states[0].terminator
    else {
        unreachable!()
    };
    when_false.transfers[0].source_parameter_index = 0;
    when_false.scalar_arguments[0].source_scalar_parameter_index = 2;
    when_false.trivial_affine_discard_parameter_positions = vec![1];

    let join = SymbolHandle::from_arena_index(14);
    for state in &mut plan.states[1..3] {
        state.terminator = CheckedStructuralUnitControlTerminatorPlan::Jump {
            statement_ordinal: 0,
            target_state: join,
            transfers: vec![psi_checked_trees::CheckedStructuralControlTransferPlan {
                source_parameter_index: 0,
                target_parameter_index: 0,
            }],
            scalar_arguments: vec![psi_checked_trees::CheckedStructuralScalarArgumentPlan {
                argument_ordinal: 1,
                source_scalar_parameter_index: 0,
                target_scalar_parameter_index: 0,
                primitive_type: PrimitiveType::I32,
            }],
            trivial_affine_discard_parameter_positions: Vec::new(),
        };
    }
    plan.states
        .push(psi_checked_trees::CheckedStructuralUnitControlStatePlan {
            state: join,
            structural_parameters: vec![affine_parameter(0)],
            scalar_parameters: vec![psi_checked_trees::CheckedStructuralScalarParameterPlan {
                source_position: 1,
                primitive_type: PrimitiveType::I32,
            }],
            terminator: CheckedStructuralUnitControlTerminatorPlan::ReturnUnit {
                trivial_affine_discard_parameter_positions: vec![0],
            },
        });
}

#[test]
fn structural_unit_control_lowers_exact_transfer_and_edge_cleanup() {
    let mut checked = hard_root_checked_fixture();
    install_structural_unit_control_fixture(&mut checked);

    let lowered = lower_machine(&checked, "example::Root::enter")
        .expect("exact structural custody chain should lower");
    let [machine] = lowered.semantic_module.machines.as_slice() else {
        panic!("structural control slice lowers one attached machine")
    };
    assert_eq!(machine.structural_parameters.len(), 2);
    assert!(matches!(
        machine.parameters.as_slice(),
        [ValueDeclaration {
            id,
            scalar_type: ScalarType::Integer(_),
        }] if *id == value_id(1)
    ));
    assert_eq!(machine.blocks.len(), 2);
    assert!(
        machine
            .blocks
            .iter()
            .all(|block| block.operations.is_empty())
    );
    assert!(matches!(
        &machine.blocks[0].terminator,
        Terminator::Jump {
            target,
            arguments,
            trivial_affine_discards,
            ..
        } if *target == block_id(2)
            && arguments == &[value_id(1)]
            && trivial_affine_discards == &[place_id(1)]
    ));
    assert!(matches!(
        machine.blocks[1].parameters.as_slice(),
        [ValueDeclaration {
            id,
            scalar_type: ScalarType::Integer(_),
        }] if *id == value_id(2)
    ));
    assert!(matches!(
        &machine.blocks[1].terminator,
        Terminator::ReturnUnit {
            trivial_affine_discards,
            ..
        } if trivial_affine_discards == &[place_id(2)]
    ));
    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &psi_proof_admission::AdmissionProfile::default(),
    )
    .expect("structural jump scalar binding and cleanup should verify independently");
    let bytes = psi_terminal_codec::encode_module(&lowered.semantic_module)
        .expect("structural control slice should encode canonically");
    assert_eq!(
        psi_terminal_codec::decode_module(&bytes)
            .expect("canonical structural control bytes should decode"),
        lowered.semantic_module
    );
}

#[test]
fn structural_unit_conditional_lowers_independent_transfer_cleanup_frontiers() {
    let mut checked = hard_root_checked_fixture();
    install_structural_unit_conditional_fixture(&mut checked);

    let lowered = lower_machine(&checked, "example::Root::enter")
        .expect("exact structural conditional frontiers should lower");
    let [machine] = lowered.semantic_module.machines.as_slice() else {
        panic!("structural conditional slice lowers one attached machine")
    };
    assert!(matches!(
        machine.parameters.as_slice(),
        [
            ValueDeclaration {
                id: guard,
                scalar_type: ScalarType::Boolean,
            },
            ValueDeclaration {
                id: value,
                scalar_type: ScalarType::Integer(_),
            },
        ] if *guard == value_id(1) && *value == value_id(2)
    ));
    assert_eq!(machine.blocks.len(), 3);
    assert!(matches!(
        &machine.blocks[0].terminator,
        Terminator::Conditional {
            condition,
            when_true: SuccessorEdge {
                target: true_target,
                arguments: true_arguments,
                trivial_affine_discards: true_discards,
                ..
            },
            when_false: SuccessorEdge {
                target: false_target,
                arguments: false_arguments,
                trivial_affine_discards: false_discards,
                ..
            },
        } if *condition == value_id(1)
            && *true_target == block_id(2)
            && true_arguments == &[value_id(2)]
            && true_discards == &[place_id(2)]
            && *false_target == block_id(3)
            && false_arguments == &[value_id(2)]
            && false_discards == &[place_id(1)]
    ));
    assert!(matches!(
        machine.blocks[1].parameters.as_slice(),
        [ValueDeclaration {
            id,
            scalar_type: ScalarType::Integer(_),
        }] if *id == value_id(3)
    ));
    assert!(matches!(
        machine.blocks[2].parameters.as_slice(),
        [ValueDeclaration {
            id,
            scalar_type: ScalarType::Integer(_),
        }] if *id == value_id(4)
    ));
    assert!(matches!(
        &machine.blocks[1].terminator,
        Terminator::ReturnUnit {
            trivial_affine_discards,
            ..
        } if trivial_affine_discards == &[place_id(1)]
    ));
    assert!(matches!(
        &machine.blocks[2].terminator,
        Terminator::ReturnUnit {
            trivial_affine_discards,
            ..
        } if trivial_affine_discards == &[place_id(2)]
    ));
    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &psi_proof_admission::AdmissionProfile::default(),
    )
    .expect("structural conditional cleanup should verify independently");
    let bytes = psi_terminal_codec::encode_module(&lowered.semantic_module)
        .expect("structural conditional should encode canonically");
    assert_eq!(
        psi_terminal_codec::decode_module(&bytes)
            .expect("structural conditional should decode canonically"),
        lowered.semantic_module
    );

    let CheckedStructuralUnitControlTerminatorPlan::Conditional { when_true, .. } = &mut checked
        .facts
        .flow
        .terminal_structural_unit_controls
        .machines[0]
        .states[0]
        .terminator
    else {
        unreachable!()
    };
    when_true.scalar_arguments[0].source_scalar_parameter_index = 0;
    assert!(matches!(
        lower_machine(&checked, "example::Root::enter"),
        Err(LoweringError::Unsupported(
            "structural Unit scalar successor map changes its checked signature"
        ))
    ));

    install_structural_unit_conditional_fixture(&mut checked);

    let CheckedStructuralUnitControlTerminatorPlan::Conditional {
        when_true,
        when_false,
        ..
    } = &mut checked
        .facts
        .flow
        .terminal_structural_unit_controls
        .machines[0]
        .states[0]
        .terminator
    else {
        unreachable!()
    };
    std::mem::swap(
        &mut when_true.statement_ordinal,
        &mut when_false.statement_ordinal,
    );
    assert!(matches!(
        lower_machine(&checked, "example::Root::enter"),
        Err(LoweringError::Unsupported(
            "structural Unit conditional successors are not in canonical order"
        ))
    ));
}

#[test]
fn structural_unit_conditional_lowers_after_an_unconditional_prefix() {
    let mut checked = hard_root_checked_fixture();
    install_structural_unit_nonentry_conditional_fixture(&mut checked);

    let lowered = lower_machine(&checked, "example::Root::enter")
        .expect("one structural conditional may follow an unconditional prefix");
    let [machine] = lowered.semantic_module.machines.as_slice() else {
        panic!("prefixed structural conditional lowers one attached machine")
    };
    assert_eq!(machine.blocks.len(), 4);
    assert!(matches!(
        &machine.blocks[0].terminator,
        Terminator::Jump {
            target,
            arguments,
            trivial_affine_discards,
            ..
        } if *target == block_id(2)
            && arguments == &[value_id(1), value_id(2)]
            && trivial_affine_discards.is_empty()
    ));
    assert!(matches!(
        machine.blocks[1].parameters.as_slice(),
        [
            ValueDeclaration {
                id: guard,
                scalar_type: ScalarType::Boolean,
            },
            ValueDeclaration {
                id: value,
                scalar_type: ScalarType::Integer(_),
            },
        ] if *guard == value_id(3) && *value == value_id(4)
    ));
    assert!(matches!(
        &machine.blocks[1].terminator,
        Terminator::Conditional {
            condition,
            when_true: SuccessorEdge {
                target: true_target,
                arguments: true_arguments,
                trivial_affine_discards: true_discards,
                ..
            },
            when_false: SuccessorEdge {
                target: false_target,
                arguments: false_arguments,
                trivial_affine_discards: false_discards,
                ..
            },
        } if *condition == value_id(3)
            && *true_target == block_id(3)
            && true_arguments == &[value_id(4)]
            && true_discards == &[place_id(2)]
            && *false_target == block_id(4)
            && false_arguments == &[value_id(4)]
            && false_discards == &[place_id(1)]
    ));
    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &psi_proof_admission::AdmissionProfile::default(),
    )
    .expect("prefixed conditional maps should verify independently");
    let bytes = psi_terminal_codec::encode_module(&lowered.semantic_module)
        .expect("prefixed structural conditional should encode canonically");
    assert_eq!(
        psi_terminal_codec::decode_module(&bytes)
            .expect("prefixed structural conditional should decode canonically"),
        lowered.semantic_module
    );

    let second_conditional = checked
        .facts
        .flow
        .terminal_structural_unit_controls
        .machines[0]
        .states[1]
        .terminator
        .clone();
    checked
        .facts
        .flow
        .terminal_structural_unit_controls
        .machines[0]
        .states[2]
        .terminator = second_conditional.clone();
    checked
        .facts
        .flow
        .terminal_structural_unit_controls
        .machines[0]
        .states[3]
        .terminator = second_conditional;
    assert!(matches!(
        lower_machine(&checked, "example::Root::enter"),
        Err(LoweringError::Unsupported(
            "structural Unit control supports at most two checked conditional states"
        ))
    ));
}

#[test]
fn structural_unit_two_conditional_tree_lowers_exact_edge_maps() {
    let mut checked = hard_root_checked_fixture();
    install_structural_unit_two_conditional_fixture(&mut checked);

    let lowered = lower_machine(&checked, "example::Root::enter")
        .expect("two checked structural conditionals should lower");
    let [machine] = lowered.semantic_module.machines.as_slice() else {
        panic!("two-decision structural tree lowers one attached machine")
    };
    assert_eq!(machine.blocks.len(), 5);
    assert!(matches!(
        &machine.blocks[0].terminator,
        Terminator::Conditional {
            condition,
            when_true: SuccessorEdge {
                target: true_target,
                arguments: true_arguments,
                trivial_affine_discards: true_discards,
                ..
            },
            when_false: SuccessorEdge {
                target: false_target,
                arguments: false_arguments,
                trivial_affine_discards: false_discards,
                ..
            },
        } if *condition == value_id(1)
            && *true_target == block_id(2)
            && true_arguments == &[value_id(1), value_id(2)]
            && true_discards == &[place_id(2)]
            && *false_target == block_id(3)
            && false_arguments == &[value_id(2)]
            && false_discards == &[place_id(1)]
    ));
    assert!(matches!(
        machine.blocks[1].parameters.as_slice(),
        [
            ValueDeclaration {
                id: guard,
                scalar_type: ScalarType::Boolean,
            },
            ValueDeclaration {
                id: value,
                scalar_type: ScalarType::Integer(_),
            },
        ] if *guard == value_id(3) && *value == value_id(4)
    ));
    assert!(matches!(
        &machine.blocks[1].terminator,
        Terminator::Conditional {
            condition,
            when_true: SuccessorEdge {
                target: true_target,
                arguments: true_arguments,
                trivial_affine_discards: true_discards,
                ..
            },
            when_false: SuccessorEdge {
                target: false_target,
                arguments: false_arguments,
                trivial_affine_discards: false_discards,
                ..
            },
        } if *condition == value_id(3)
            && *true_target == block_id(4)
            && true_arguments == &[value_id(4)]
            && true_discards.is_empty()
            && *false_target == block_id(5)
            && false_arguments == &[value_id(4)]
            && false_discards.is_empty()
    ));
    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &psi_proof_admission::AdmissionProfile::default(),
    )
    .expect("two-decision structural maps should verify independently");
    let bytes = psi_terminal_codec::encode_module(&lowered.semantic_module)
        .expect("two-decision structural tree should encode canonically");
    assert_eq!(
        psi_terminal_codec::decode_module(&bytes)
            .expect("two-decision structural tree should decode canonically"),
        lowered.semantic_module
    );
}

#[test]
fn structural_unit_diamond_requires_one_exact_join_frontier() {
    let mut checked = hard_root_checked_fixture();
    install_structural_unit_join_fixture(&mut checked);

    let lowered = lower_machine(&checked, "example::Root::enter")
        .expect("one exact structural diamond should lower");
    let [machine] = lowered.semantic_module.machines.as_slice() else {
        panic!("structural diamond lowers one attached machine")
    };
    assert_eq!(machine.blocks.len(), 4);
    assert!(matches!(
        &machine.blocks[0].terminator,
        Terminator::Conditional {
            condition,
            when_true: SuccessorEdge {
                target: true_target,
                arguments: true_arguments,
                trivial_affine_discards: true_discards,
                ..
            },
            when_false: SuccessorEdge {
                target: false_target,
                arguments: false_arguments,
                trivial_affine_discards: false_discards,
                ..
            },
        } if *condition == value_id(1)
            && *true_target == block_id(2)
            && true_arguments == &[value_id(2)]
            && true_discards == &[place_id(2)]
            && *false_target == block_id(3)
            && false_arguments == &[value_id(3)]
            && false_discards == &[place_id(2)]
    ));
    assert!(matches!(
        &machine.blocks[1].terminator,
        Terminator::Jump {
            target,
            arguments,
            trivial_affine_discards,
            ..
        } if *target == block_id(4)
            && arguments == &[value_id(4)]
            && trivial_affine_discards.is_empty()
    ));
    assert!(matches!(
        &machine.blocks[2].terminator,
        Terminator::Jump {
            target,
            arguments,
            trivial_affine_discards,
            ..
        } if *target == block_id(4)
            && arguments == &[value_id(5)]
            && trivial_affine_discards.is_empty()
    ));
    assert!(matches!(
        machine.blocks[3].parameters.as_slice(),
        [ValueDeclaration {
            id,
            scalar_type: ScalarType::Integer(_),
        }] if *id == value_id(6)
    ));
    assert!(matches!(
        &machine.blocks[3].terminator,
        Terminator::ReturnUnit {
            trivial_affine_discards,
            ..
        } if trivial_affine_discards == &[place_id(1)]
    ));
    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &psi_proof_admission::AdmissionProfile::default(),
    )
    .expect("the independent verifier should reconstruct one identical join frontier");
    let bytes = psi_terminal_codec::encode_module(&lowered.semantic_module)
        .expect("structural diamond should encode canonically");
    assert_eq!(
        psi_terminal_codec::decode_module(&bytes)
            .expect("structural diamond should decode canonically"),
        lowered.semantic_module
    );

    let CheckedStructuralUnitControlTerminatorPlan::Conditional { when_false, .. } = &mut checked
        .facts
        .flow
        .terminal_structural_unit_controls
        .machines[0]
        .states[0]
        .terminator
    else {
        unreachable!()
    };
    when_false.transfers[0].source_parameter_index = 1;
    when_false.trivial_affine_discard_parameter_positions = vec![0];
    assert!(matches!(
        lower_machine(&checked, "example::Root::enter"),
        Err(LoweringError::Unsupported(
            "structural Unit join predecessors reconstruct different custody frontiers"
        ))
    ));

    install_structural_unit_join_fixture(&mut checked);
    let entry = checked
        .facts
        .flow
        .terminal_structural_unit_controls
        .machines[0]
        .states[0]
        .state;
    checked
        .facts
        .flow
        .terminal_structural_unit_controls
        .machines[0]
        .states[3]
        .terminator = CheckedStructuralUnitControlTerminatorPlan::Jump {
        statement_ordinal: 0,
        target_state: entry,
        transfers: vec![psi_checked_trees::CheckedStructuralControlTransferPlan {
            source_parameter_index: 0,
            target_parameter_index: 0,
        }],
        scalar_arguments: Vec::new(),
        trivial_affine_discard_parameter_positions: Vec::new(),
    };
    assert!(matches!(
        lower_machine(&checked, "example::Root::enter"),
        Err(LoweringError::Unsupported(
            "structural Unit control entry has an incoming edge"
        ))
    ));
}

#[test]
fn structural_unit_control_fails_closed_on_stale_cleanup_or_signature() {
    let mut checked = hard_root_checked_fixture();
    install_structural_unit_control_fixture(&mut checked);
    let plan = &mut checked
        .facts
        .flow
        .terminal_structural_unit_controls
        .machines[0];
    let CheckedStructuralUnitControlTerminatorPlan::Jump {
        trivial_affine_discard_parameter_positions,
        ..
    } = &mut plan.states[0].terminator
    else {
        unreachable!()
    };
    trivial_affine_discard_parameter_positions.clear();
    assert!(matches!(
        lower_machine(&checked, "example::Root::enter"),
        Err(LoweringError::Unsupported(
            "structural Unit jump transfer and cleanup do not partition its exact frontier"
        ))
    ));

    install_structural_unit_control_fixture(&mut checked);
    let CheckedStructuralUnitControlTerminatorPlan::Jump {
        scalar_arguments, ..
    } = &mut checked
        .facts
        .flow
        .terminal_structural_unit_controls
        .machines[0]
        .states[0]
        .terminator
    else {
        unreachable!()
    };
    scalar_arguments[0].source_scalar_parameter_index = 1;
    assert!(matches!(
        lower_machine(&checked, "example::Root::enter"),
        Err(LoweringError::Unsupported(
            "structural Unit scalar successor map changes its checked signature"
        ))
    ));

    install_structural_unit_control_fixture(&mut checked);
    checked
        .facts
        .flow
        .terminal_structural_unit_controls
        .machines[0]
        .states[1]
        .structural_parameters[0]
        .type_identity = "example::Root".to_owned();
    let stale_signature = lower_machine(&checked, "example::Root::enter");
    assert!(
        matches!(
            &stale_signature,
            Err(LoweringError::Unsupported(
                "structural Unit transfer changes its checked structural signature"
            ))
        ),
        "unexpected stale-signature result: {stale_signature:?}"
    );
}
