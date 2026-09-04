//! Structural Unit control-flow regression families.

use super::*;

#[test]
fn lowers_conditional_unit_control_with_exact_boundary_effect_leaves() {
    let checked = checked_source(
        r#"
            boundary trait Host { machine exit(code: i32); }
            data Root {}
            machine Root::enter(flag: bool) {
                transition flag { true -> yes() _ -> no() }
                state yes() { Host::exit(1); }
                state no() { Host::exit(2); }
            }
        "#,
    );
    let lowered = lower_machine(&checked, "Root::enter")
        .expect("checked control and boundary effects lower atomically");
    let [machine] = lowered.semantic_module.machines.as_slice() else {
        panic!("composed route emits one machine")
    };
    assert_eq!(machine.blocks.len(), 3);
    assert!(matches!(
        machine.blocks[0].terminator,
        Terminator::Conditional { .. }
    ));
    for leaf in &machine.blocks[1..] {
        assert_eq!(
            leaf.operations
                .iter()
                .filter(|operation| matches!(operation.kind, OperationKind::BoundaryCall { .. }))
                .count(),
            1
        );
        assert!(matches!(leaf.terminator, Terminator::ReturnUnit { .. }));
    }
    assert_eq!(lowered.semantic_module.boundary_machines.len(), 1);
    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &psi_proof_admission::AdmissionProfile::default(),
    )
    .expect("composed Unit module verifies");
    let bytes = psi_terminal_codec::encode_module(&lowered.semantic_module).expect("encode");
    assert_eq!(
        psi_terminal_codec::decode_module(&bytes).expect("decode"),
        lowered.semantic_module
    );

    let mut without_boundary = checked;
    without_boundary
        .facts
        .flow
        .terminal_unit_effects
        .boundary_machines
        .clear();
    assert!(matches!(
        lower_machine(&without_boundary, "Root::enter"),
        Err(LoweringError::Unsupported(_))
    ));
}

#[test]
fn lowers_closed_guard_and_provider_attachment_as_one_composed_machine() {
    let checked = checked_source(
        r#"
            boundary trait Console {
                machine exit_process(return_code: i32) reaches Console;
            }
            const PAGE_SIZE: u32 = 64;
            data Main { console: Console; }
            machine Main::main(&mut self) reaches Console {
                transition PAGE_SIZE == 64 { true -> yes() _ -> no() }
                state yes(&mut self) { self.console.exit_process(70); }
                state no(&mut self) { self.console.exit_process(71); }
            }
        "#,
    );
    let lowered = lower_machine(&checked, "Main::main")
        .expect("closed guard and provider attachment lower atomically");
    let [machine] = lowered.semantic_module.machines.as_slice() else {
        panic!("composed provider route emits one machine")
    };
    assert!(machine.parameters.is_empty());
    assert!(machine.structural_parameters.is_empty());
    assert!(matches!(
        machine.structural_places.as_slice(),
        [psi_terminal::StructuralPlaceDeclaration {
            kind: StructuralPlaceKind::ProviderAttachment { .. },
            ..
        }]
    ));
    assert!(
        machine.blocks[0]
            .operations
            .iter()
            .any(|operation| matches!(
                operation.kind,
                OperationKind::BooleanConstant { value: true }
            ))
    );
    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &psi_proof_admission::AdmissionProfile::default(),
    )
    .expect("provider-backed composed Unit module verifies");

    let mut missing_provider = checked;
    missing_provider
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines[0]
        .provider_attachment_requirements
        .clear();
    assert!(matches!(
        lower_machine(&missing_provider, "Main::main"),
        Err(LoweringError::Unsupported(_))
    ));
}

#[test]
fn lowers_one_compile_known_u64_binding_and_rejects_checked_drift() {
    let source = r#"
        boundary trait Host { machine exit(code: i32); }
        data Root { values: [i32; 5]; }
        machine Root::enter(&mut self) {
            let length: u64 = (self.values[1..4]).len;
            transition length == 3 {
                true -> yes()
                false -> no()
            }
            state yes(&mut self) { Host::exit(1); }
            state no(&mut self) { Host::exit(2); }
        }
    "#;
    let checked = checked_source(source);
    let lowered = lower_machine(&checked, "Root::enter")
        .expect("one compile-known local lowers through composed Unit control");
    let [machine] = lowered.semantic_module.machines.as_slice() else {
        panic!("one composed Terminal machine")
    };
    assert!(matches!(
        machine.blocks[0].operations.first(),
        Some(Operation {
            kind: OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(3),
            },
            ..
        })
    ));

    let mut drifted_fact = checked.clone();
    let initializer = drifted_fact
        .facts
        .values
        .scalar_expressions
        .expressions
        .iter_mut()
        .find(|expression| {
            expression.statement_ordinal == 0
                && expression.role
                    == CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 0 }
        })
        .expect("checked initializer fact");
    initializer.expression = CheckedScalarExpression::IntegerLiteral {
        literal: psi_numerics::literals::IntegerLiteral::from_value(4).with_landing(
            psi_numerics::literals::IntegerLanding {
                landed_type: psi_numerics::literals::LandedIntegerType::U64,
                domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    assert!(matches!(
        lower_machine(&drifted_fact, "Root::enter"),
        Err(LoweringError::Unsupported(_))
    ));

    let mut drifted_retained_initializer = checked.clone();
    drifted_retained_initializer
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines[0]
        .states[0]
        .binding_initializers[0] = CheckedScalarExpression::IntegerLiteral {
        literal: psi_numerics::literals::IntegerLiteral::from_value(4).with_landing(
            psi_numerics::literals::IntegerLanding {
                landed_type: psi_numerics::literals::LandedIntegerType::U64,
                domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    assert!(matches!(
        lower_machine(&drifted_retained_initializer, "Root::enter"),
        Err(LoweringError::Unsupported(_))
    ));

    let mut drifted_binding = checked;
    drifted_binding
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines[0]
        .states[0]
        .bindings[0]
        .statement_ordinal = 1;
    assert!(matches!(
        lower_machine(&drifted_binding, "Root::enter"),
        Err(LoweringError::Unsupported(_))
    ));
}

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
        fused_service_erasure: None,
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
        fused_service_erasure: None,
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
        fused_service_erasure: None,
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
        fused_service_erasure: None,
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
fn static_requirement_evidence_does_not_preempt_exact_structural_unit_control() {
    let mut checked = hard_root_checked_fixture();
    install_structural_unit_control_fixture(&mut checked);
    let root = SymbolHandle::from_arena_index(1);
    checked
        .facts
        .proof
        .proof_output_calls
        .append(psi_checked_trees::ProofOutputCallFact {
            caller_machine_symbol: root,
            static_requirement_dispatch: Some(
                psi_checked_trees::StaticRequirementDispatchFact::default(),
            ),
            ..Default::default()
        });

    let selection = select_terminal_machine(&checked, "example::Root::enter")
        .expect("fixture has one selected root");
    let routed = lower_selected_machine(&checked, selection)
        .expect("retained structural control wins before attached-Unit fallback");

    assert_eq!(routed.route, SelectedMachineRoute::StructuralUnitControl);
    let [machine] = routed.terminal.semantic_module.machines.as_slice() else {
        panic!("the exact structural route lowers one machine")
    };
    assert_eq!(machine.blocks.len(), 2);
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

#[test]
fn ranked_countdown_lowers_to_verified_resumable_interpreter_execution() {
    let checked = checked_source(
        r#"
            data Token { value: i32; }
            data Root {}

            machine Root::countdown(token: Token, remaining: u32)
            terminates by remaining -> Nat::Descending;
            {
                transition remaining > 0 {
                    true -> countdown(token, remaining - 1)
                    _ -> done(token)
                }
                state done(token: Token) {}
            }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_structural_unit_controls
        .machines
        .iter()
        .find(|plan| plan.ranked_scc.is_some())
        .expect("checked countdown component");
    let lowered = lower_structural_unit_control_machine(&checked, plan)
        .expect("ranked representation should lower");
    let [machine] = lowered.semantic_module.machines.as_slice() else {
        panic!("one ranked machine")
    };
    let ranked = machine
        .ranked_scc
        .as_ref()
        .expect("ranked Terminal identity");
    assert_eq!(machine.entry, block_id(1));
    assert_eq!(ranked.header, block_id(2));
    assert_eq!(machine.blocks.len(), 4);
    assert!(matches!(
        machine.blocks[1].operations.as_slice(),
        [
            Operation {
                kind: OperationKind::IntegerConstant {
                    value: IntegerValue::Unsigned(0)
                },
                ..
            },
            Operation {
                kind: OperationKind::IntegerLessThan { .. },
                ..
            }
        ]
    ));
    assert!(matches!(
        machine.blocks[2].operations.as_slice(),
        [
            Operation {
                kind: OperationKind::IntegerConstant {
                    value: IntegerValue::Unsigned(1)
                },
                ..
            },
            Operation {
                kind: OperationKind::ExactIntegerSubtract { .. },
                ..
            }
        ]
    ));
    psi_terminal_verifier::validate_module_representation(&lowered.semantic_module)
        .expect("ranked representation policy");
    psi_terminal_verifier::verify_module_for_interpretation(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &psi_proof_admission::AdmissionProfile::default(),
    )
    .expect("ranked proof closes for interpreter admission");
    let fixed_fuel_verified = psi_terminal_verifier::verify_module_for_fixed_fuel(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &psi_proof_admission::AdmissionProfile::default(),
    )
    .expect("ranked proof closes for fixed-fuel admission");
    let fixed_fuel = psi_terminal_fixed_fuel::derive_ranked_countdown_entry_fuel(
        &fixed_fuel_verified,
        machine.id,
    )
    .expect("ranked countdown has an all-input ceiling");
    assert_eq!(fixed_fuel.ceiling_units(), 25_769_803_775);
    psi_terminal_fixed_fuel::validate_ranked_countdown_entry_fuel(
        &fixed_fuel_verified,
        &fixed_fuel,
    )
    .expect("ranked fixed-fuel theorem replays");
    let ranked_segments = psi_terminal_fixed_fuel::derive_ranked_countdown_safe_point_segments(
        &fixed_fuel_verified,
        machine.id,
    )
    .expect("ranked countdown has a complete safe-point partition");
    assert_eq!(ranked_segments.len(), 5);
    assert_eq!(
        ranked_segments
            .iter()
            .map(|segment| (
                segment.start_block(),
                segment.end_edge(),
                segment.ceiling_units(),
            ))
            .collect::<Vec<_>>(),
        vec![
            (block_id(1), edge_id(1), 1),
            (block_id(2), edge_id(2), 3),
            (block_id(2), edge_id(3), 3),
            (block_id(3), edge_id(4), 3),
            (block_id(4), edge_id(5), 1),
        ]
    );
    assert!(ranked_segments.iter().all(|segment| {
        segment.terminal_psi() == fixed_fuel.terminal_psi()
            && segment.schedule() == fixed_fuel.schedule()
            && segment.machine() == machine.id
            && segment.relevant_preconditions().is_empty()
    }));
    psi_terminal_fixed_fuel::validate_ranked_countdown_safe_point_segments(
        &fixed_fuel_verified,
        machine.id,
        &ranked_segments,
    )
    .expect("ranked safe-point partition replays as one canonical sequence");
    assert_eq!(
        psi_terminal_fixed_fuel::validate_ranked_countdown_safe_point_segments(
            &fixed_fuel_verified,
            machine.id,
            &ranked_segments[..4],
        ),
        Err(psi_terminal_fixed_fuel::FixedFuelError::CertificateMismatch),
        "an omitted ranked segment must reject"
    );
    let mut reordered_ranked_segments = ranked_segments.clone();
    reordered_ranked_segments.swap(1, 2);
    assert_eq!(
        psi_terminal_fixed_fuel::validate_ranked_countdown_safe_point_segments(
            &fixed_fuel_verified,
            machine.id,
            &reordered_ranked_segments,
        ),
        Err(psi_terminal_fixed_fuel::FixedFuelError::CertificateMismatch),
        "ranked conditional arms remain canonically ordered"
    );
    let mut duplicated_ranked_segments = ranked_segments.clone();
    duplicated_ranked_segments[4] = ranked_segments[3].clone();
    assert_eq!(
        psi_terminal_fixed_fuel::validate_ranked_countdown_safe_point_segments(
            &fixed_fuel_verified,
            machine.id,
            &duplicated_ranked_segments,
        ),
        Err(psi_terminal_fixed_fuel::FixedFuelError::CertificateMismatch),
        "a duplicated ranked endpoint cannot replace the return row"
    );
    let retained_ranked_segments =
        psi_terminal_fixed_fuel::retain_validated_ranked_countdown_safe_point_segments(
            &fixed_fuel_verified,
            machine.id,
            ranked_segments.clone(),
        )
        .expect("the exact ranked partition is retainable");
    assert_eq!(
        retained_ranked_segments.terminal_psi(),
        fixed_fuel.terminal_psi()
    );
    assert_eq!(retained_ranked_segments.schedule(), fixed_fuel.schedule());
    assert_eq!(retained_ranked_segments.machine(), machine.id);
    assert_eq!(
        retained_ranked_segments.certificates(),
        ranked_segments.as_slice()
    );
    psi_terminal_fixed_fuel::validate_retained_ranked_countdown_safe_point_segments(
        &fixed_fuel_verified,
        &retained_ranked_segments,
    )
    .expect("the retained ranked partition independently replays");
    let directly_retained_ranked_segments =
        psi_terminal_fixed_fuel::derive_validated_ranked_countdown_safe_point_segments(
            &fixed_fuel_verified,
            machine.id,
        )
        .expect("the complete ranked partition derives directly");
    assert_eq!(
        directly_retained_ranked_segments.certificates(),
        ranked_segments.as_slice()
    );
    psi_terminal_fixed_fuel::validate_retained_ranked_countdown_safe_point_segments(
        &fixed_fuel_verified,
        &directly_retained_ranked_segments,
    )
    .expect("the directly retained ranked partition independently replays");
    let native_verified = psi_terminal_verifier::verify_module_for_native_ranked_countdown(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &psi_proof_admission::AdmissionProfile::default(),
    )
    .expect("ranked proof closes for native authority");
    let synopsis =
        psi_terminal_codec::render_verified_native_ranked_countdown_synopsis(&native_verified)
            .expect("verified ranked countdown has a review synopsis");
    assert_eq!(
        synopsis,
        psi_terminal_codec::render_verified_native_ranked_countdown_synopsis(&native_verified)
            .expect("ranked synopsis is deterministic")
    );
    assert!(synopsis.starts_with("proof-bundle "));
    assert!(synopsis.contains("obligation 1 goal "));
    assert!(synopsis.contains(
        "ranked-countdown machine 1 header 2 rank 2 type Fixed-Unsigned-32 lower Unsigned(0) upper Unsigned(4294967295)"
    ));
    assert!(synopsis.contains("  ranking-rule closed-unsigned-countdown verifier-reconstructed"));
    assert!(synopsis.contains("  covered-edge 4 source 3 target 2"));
    assert!(
        synopsis.contains("    guard unsigned-positive block 2 edge 2 condition 4 parameter 2")
    );
    assert!(synopsis.contains(
        "    successor unsigned-minus-one argument-index 0 argument 6 source-parameter 2 target-parameter 2"
    ));
    assert!(synopsis.contains("trust-node implementation:rust-terminal-verifier"));
    assert!(!synopsis.contains("RecursiveComponentCertificate"));
    assert_eq!(lowered.proof_bundle.evidence.len(), 1);
    assert!(matches!(
        psi_terminal_verifier::validate_module(&lowered.semantic_module),
        Err(psi_terminal_verifier::ModuleError::NonExecutableRankedScc(
            _
        ))
    ));
    let bytes = psi_terminal_codec::encode_module(&lowered.semantic_module)
        .expect("ranked semantic identity should encode");
    let decoded = psi_terminal_codec::decode_module(&bytes).expect("ranked identity decodes");
    assert_eq!(decoded, lowered.semantic_module);
    let proof_bytes = psi_terminal_codec::encode_proof_bundle(&lowered.proof_bundle)
        .expect("ranked proof identity should encode");
    let decoded_proof = psi_terminal_codec::decode_proof_bundle(&proof_bytes)
        .expect("ranked proof identity decodes canonically");
    let decoded_fixed_fuel_verified = psi_terminal_verifier::verify_module_for_fixed_fuel(
        &decoded,
        &decoded_proof,
        &psi_proof_admission::AdmissionProfile::default(),
    )
    .expect("decoded ranked proof closes for fixed fuel");
    psi_terminal_fixed_fuel::validate_ranked_countdown_entry_fuel(
        &decoded_fixed_fuel_verified,
        &fixed_fuel,
    )
    .expect("ranked certificate binds the canonical round trip");
    psi_terminal_fixed_fuel::validate_retained_ranked_countdown_safe_point_segments(
        &decoded_fixed_fuel_verified,
        &retained_ranked_segments,
    )
    .expect("ranked segment catalog binds the canonical round trip");
    let mut drifted_identity = decoded.clone();
    drifted_identity.structural_types[0].identity = "test::OtherToken".to_owned();
    let drifted_fixed_fuel_verified = psi_terminal_verifier::verify_module_for_fixed_fuel(
        &drifted_identity,
        &decoded_proof,
        &psi_proof_admission::AdmissionProfile::default(),
    )
    .expect("identity-drifted ranked structure remains independently valid");
    assert_eq!(
        psi_terminal_fixed_fuel::validate_retained_ranked_countdown_safe_point_segments(
            &drifted_fixed_fuel_verified,
            &retained_ranked_segments,
        ),
        Err(psi_terminal_fixed_fuel::FixedFuelError::CertificateMismatch),
        "a different terminal semantic identity cannot replay ranked segments"
    );
    let lowered = lower_machine(&checked, "Root::countdown")
        .expect("public lowering admits the interpreter-only ranked slice");
    let semantic = psi_terminal_codec::encode_module(&lowered.semantic_module)
        .expect("ranked semantic section encodes");
    let proof = psi_terminal_codec::encode_proof_bundle(&lowered.proof_bundle)
        .expect("ranked proof section encodes");
    let decoded_native = psi_terminal_verifier::verify_module_for_native_ranked_countdown(
        &decoded,
        &decoded_proof,
        &psi_proof_admission::AdmissionProfile::default(),
    )
    .expect("decoded ranked proof closes for native authority");
    assert_eq!(
        synopsis,
        psi_terminal_codec::render_verified_native_ranked_countdown_synopsis(&decoded_native)
            .expect("round-tripped ranked synopsis")
    );
    let mut mutated_component = decoded.clone();
    let edge = &mut mutated_component.machines[0]
        .ranked_scc
        .as_mut()
        .expect("ranked component")
        .covered_cyclic_edges[0];
    let psi_terminal::TerminalRankedSuccessorArgument::UnsignedParameterMinusOne {
        argument_index,
        ..
    } = &mut edge.successor_argument;
    *argument_index = 1;
    assert!(
        psi_terminal_verifier::verify_module_for_native_ranked_countdown(
            &mutated_component,
            &decoded_proof,
            &psi_proof_admission::AdmissionProfile::default(),
        )
        .is_err(),
        "mutated ranked custody must reject before it can be rendered"
    );
    let mut missing_proof = decoded_proof.clone();
    missing_proof.evidence.clear();
    assert!(
        psi_terminal_verifier::verify_module_for_native_ranked_countdown(
            &decoded,
            &missing_proof,
            &psi_proof_admission::AdmissionProfile::default(),
        )
        .is_err(),
        "missing decrease evidence must reject before it can be rendered"
    );
    let machine = &lowered.semantic_module.machines[0];
    let structural_parameter = &machine.structural_parameters[0];
    let structural_argument = psi_terminal_interpreter::TerminalStructuralValue {
        opaque_identity: 0xc0de,
        structural_type: structural_parameter.structural_type,
        qualifications: structural_parameter.qualifications.clone(),
        path: Vec::new(),
    };
    let ScalarType::Integer(rank_type) = machine.parameters[0].scalar_type else {
        panic!("rank parameter is an integer")
    };
    let rank_argument = |remaining| psi_terminal_interpreter::TerminalScalarValue::Integer {
        scalar_type: rank_type,
        value: IntegerValue::Unsigned(remaining),
    };

    for (remaining, expected_units) in [(0, 5), (1, 11), (3, 23)] {
        let mut execution =
            psi_terminal_interpreter::TerminalExecution::start_artifact_with_structural_arguments(
                &semantic,
                &proof,
                &psi_proof_admission::AdmissionProfile::default(),
                &[rank_argument(remaining)],
                std::slice::from_ref(&structural_argument),
            )
            .expect("ranked artifact starts");
        let mut meter = psi_terminal_fuel::TerminalFuelMeter::unbounded();
        assert_eq!(
            execution
                .resume(&mut meter)
                .expect("ranked execution resumes"),
            psi_terminal_interpreter::TerminalExecutionStatus::Complete(
                psi_terminal_interpreter::TerminalExecutionResult::Unit
            )
        );
        assert_eq!(meter.usage().total_units(), expected_units);
        assert_eq!(execution.live_affine_frontier().count(), 0);
    }

    let mut execution =
        psi_terminal_interpreter::TerminalExecution::start_artifact_with_structural_arguments(
            &semantic,
            &proof,
            &psi_proof_admission::AdmissionProfile::default(),
            &[rank_argument(3)],
            std::slice::from_ref(&structural_argument),
        )
        .expect("ranked resumable artifact starts");
    let mut meter = psi_terminal_fuel::TerminalFuelMeter::with_allowance(8);
    assert!(matches!(
        execution
            .resume(&mut meter)
            .expect("ranked execution exhausts cleanly"),
        psi_terminal_interpreter::TerminalExecutionStatus::SponsorExhausted(_)
    ));
    assert_eq!(meter.usage().total_units(), 8);
    assert_eq!(execution.live_affine_frontier().count(), 1);
    meter.replenish(15).expect("remaining exact grant fits");
    assert_eq!(
        execution
            .resume(&mut meter)
            .expect("ranked execution completes after refill"),
        psi_terminal_interpreter::TerminalExecutionStatus::Complete(
            psi_terminal_interpreter::TerminalExecutionResult::Unit
        )
    );
    assert_eq!(meter.usage().total_units(), 23);
    assert_eq!(execution.live_affine_frontier().count(), 0);
}

#[test]
fn ranked_countdown_lowers_implicit_mutable_receiver_without_discarding_it() {
    let checked = checked_source(
        r#"
            data Root { value: i32; }

            machine Root::countdown(&mut self, remaining: u32)
            terminates by remaining -> Nat::Descending;
            {
                transition remaining > 0 {
                    true -> countdown(remaining - 1)
                    _ -> done()
                }
                state done(&mut self) {}
            }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_structural_unit_controls
        .machines
        .iter()
        .find(|plan| plan.ranked_scc.is_some())
        .expect("checked mutable-receiver countdown");
    let lowered = lower_structural_unit_control_machine(&checked, plan)
        .expect("mutable receiver should lower into ranked Terminal custody");
    let [machine] = lowered.semantic_module.machines.as_slice() else {
        panic!("one ranked machine")
    };
    let [receiver] = machine.structural_parameters.as_slice() else {
        panic!("one structural receiver")
    };
    assert!(receiver.is_self);
    assert_eq!(receiver.position, 0);
    assert_eq!(
        receiver.access,
        psi_terminal::StructuralAccess::MutableBorrow
    );
    assert_eq!(
        receiver.multiplicity,
        psi_terminal::StructuralMultiplicity::Affine
    );
    assert!(matches!(
        machine.structural_places.as_slice(),
        [psi_terminal::StructuralPlaceDeclaration {
            id,
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: true,
            },
        }] if *id == receiver.place
    ));
    let Terminator::ReturnUnit {
        trivial_affine_discards,
        ..
    } = &machine.blocks[3].terminator
    else {
        panic!("countdown exit returns Unit")
    };
    assert!(trivial_affine_discards.is_empty());
    psi_terminal_verifier::validate_module_representation(&lowered.semantic_module)
        .expect("mutable-receiver ranked representation is structurally valid");
    psi_terminal_verifier::verify_module_for_interpretation(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &psi_proof_admission::AdmissionProfile::default(),
    )
    .expect("mutable-receiver ranked proof closes for interpreter admission");
}

#[test]
fn ranked_u64_countdown_fails_closed_when_fixed_fuel_exceeds_u64() {
    let checked = checked_source(
        r#"
            data Token { value: i32; }
            data Root {}

            machine Root::countdown(token: Token, remaining: u64)
            terminates by remaining -> Nat::Descending;
            {
                transition remaining > 0 {
                    true -> countdown(token, remaining - 1)
                    _ -> done(token)
                }
                state done(token: Token) {}
            }
        "#,
    );
    let lowered =
        lower_machine(&checked, "Root::countdown").expect("u64 ranked representation should lower");
    let verified = psi_terminal_verifier::verify_module_for_fixed_fuel(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &psi_proof_admission::AdmissionProfile::default(),
    )
    .expect("u64 ranked proof closes for fixed-fuel admission");

    assert!(matches!(
        psi_terminal_fixed_fuel::derive_ranked_countdown_entry_fuel(
            &verified,
            lowered.semantic_module.entry,
        ),
        Err(psi_terminal_fixed_fuel::FixedFuelError::BoundOverflow)
    ));
    assert!(matches!(
        psi_terminal_fixed_fuel::derive_ranked_countdown_safe_point_segments(
            &verified,
            lowered.semantic_module.entry,
        ),
        Err(psi_terminal_fixed_fuel::FixedFuelError::BoundOverflow)
    ));
}
