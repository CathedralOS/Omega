//! Root-level checked-to-terminal producer regressions.

use super::*;
use psi_language_semantics::{
    PermissionEventSource, SemanticDomainId,
    content::{
        ContentCaseSegment, ContentConservationEquation, ContentConservationOwnerKind,
        ContentFieldSegment,
    },
};
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_symbols::SymbolHandle;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_tokens_to_syntax_trees::parse_syntax_trees;
use psi_typed_trees_to_checked_trees::lower_typed_trees;

mod content_conservation;
mod scalar_graph;
mod unit_cleanup;

#[test]
fn shared_boolean_comparison_normalization_rejects_two_runtime_sides() {
    let comparison = LoweredBooleanReturnExpression::Equal {
        left: Box::new(LoweredBooleanReturnExpression::Parameter { position: 0 }),
        right: Box::new(LoweredBooleanReturnExpression::Parameter { position: 1 }),
    };
    assert!(normalize_shared_boolean_comparison_leaves(&comparison).is_none());

    let local_comparison = LoweredBooleanReturnExpression::Equal {
        left: Box::new(LoweredBooleanReturnExpression::Local { position: 1 }),
        right: Box::new(LoweredBooleanReturnExpression::Constant { value: false }),
    };
    assert!(normalize_shared_boolean_comparison_leaves(&local_comparison).is_none());
}

#[test]
fn scalar_crash_disjunction_lowers_to_canonical_terminal_propositions() {
    let values = vec![
        ValueDeclaration {
            id: value_id(2),
            scalar_type: ScalarType::Boolean,
        },
        ValueDeclaration {
            id: value_id(1),
            scalar_type: ScalarType::Boolean,
        },
    ];
    let proposition = checked_boolean_proposition(
        &CheckedBooleanExpression::Or {
            left: Box::new(CheckedBooleanExpression::Parameter { position: 0 }),
            right: Box::new(CheckedBooleanExpression::Parameter { position: 1 }),
        },
        &values,
    )
    .expect("scalar disjunction lowers");
    let Proposition::Disjunction(disjuncts) = &proposition else {
        panic!("scalar disjunction retains proposition structure")
    };
    assert_eq!(disjuncts.len(), 2);
    let keys = disjuncts
        .iter()
        .map(|disjunct| psi_terminal_codec::canonical_proposition_order_key(disjunct).unwrap())
        .collect::<Vec<_>>();
    assert!(keys[0] < keys[1]);
    PropositionContext::from_value_types(values.iter().map(|value| (value.id, value.scalar_type)))
        .unwrap()
        .validate(&proposition)
        .expect("scalar disjunction is well typed");
}

fn unit_claim_at(
    machine: SymbolHandle,
    state: SymbolHandle,
    ordinal: u32,
) -> PermissionClaimIdentity {
    PermissionClaimIdentity::Established {
        machine_symbol: machine,
        state_symbol: state,
        source: PermissionEventSource::StateEntry,
        ordinal,
    }
}

fn unit_claim(machine: SymbolHandle, state: SymbolHandle) -> PermissionClaimIdentity {
    unit_claim_at(machine, state, 0)
}

fn hard_root_checked_fixture() -> CheckedTrees {
    let root = SymbolHandle::from_arena_index(1);
    let helper = SymbolHandle::from_arena_index(2);
    let boundary = SymbolHandle::from_arena_index(3);
    let root_state = SymbolHandle::from_arena_index(11);
    let helper_state = SymbolHandle::from_arena_index(12);
    let boundary_state = SymbolHandle::from_arena_index(13);
    let port_service_symbol = SymbolHandle::from_arena_index(20);
    let domain = SemanticDomainId(9);

    let mut checked = CheckedTrees::default();
    let port_service = checked
        .facts
        .service_reaches
        .services
        .intern(port_service_symbol, "PortIo");
    let empty_reach = checked.facts.service_reaches.rows.intern(Vec::new());
    assert_eq!(
        empty_reach,
        psi_language_semantics::ServiceReachRowTable::EMPTY_ROW
    );
    let port_reach = checked
        .facts
        .service_reaches
        .rows
        .intern(vec![port_service]);
    let reach = ServiceReachSummary {
        direct: port_reach,
        transitive: port_reach,
    };
    let contract_reach = ServiceReachPlan {
        interface: ServiceReachInterface::PublishedCeiling(port_reach),
        checked_inferred: port_reach,
    };
    checked.facts.flow.terminal_machines = psi_checked_trees::CheckedTerminalMachineSelections {
        machines: vec![
            CheckedTerminalMachineSelection {
                machine: root,
                name: "example::Root::enter".to_owned(),
                signature: CheckedTerminalSignatureEligibility::Attached,
            },
            CheckedTerminalMachineSelection {
                machine: helper,
                name: "example::Helper::run".to_owned(),
                signature: CheckedTerminalSignatureEligibility::Attached,
            },
            CheckedTerminalMachineSelection {
                machine: boundary,
                name: "example::Acknowledgement::settle".to_owned(),
                signature: CheckedTerminalSignatureEligibility::Attached,
            },
        ],
    };
    let structural_parameter = |position| psi_checked_trees::CheckedUnitStructuralParameterPlan {
        position,
        is_self: false,
        type_identity: "example::Acknowledgement".to_owned(),
        multiplicity: Multiplicity::Linear,
        qualifications: vec![domain],
    };
    let entry_claim = |machine, state| psi_checked_trees::CheckedUnitEntryClaimPlan {
        claim_identity: unit_claim(machine, state),
        parameter_index: 0,
        path: Vec::new(),
        carry: CarryPolicy::STRICT,
    };
    checked.facts.flow.terminal_unit_effects = psi_checked_trees::CheckedUnitEffectPlans {
        structural_types: vec![
            psi_checked_trees::CheckedUnitStructuralTypePlan {
                identity: "example::Acknowledgement".to_owned(),
                shape: CheckedUnitStructuralTypeShape::Record {
                    fields: vec![
                        psi_checked_trees::CheckedUnitStructuralFieldPlan {
                            identity: "sequence".to_owned(),
                            relevance: psi_terminal::BindingRelevance::Relevant,
                            field_type: CheckedUnitStructuralFieldType::Scalar(PrimitiveType::U64),
                        },
                        psi_checked_trees::CheckedUnitStructuralFieldPlan {
                            identity: "proof".to_owned(),
                            relevance: psi_terminal::BindingRelevance::Erased,
                            field_type: CheckedUnitStructuralFieldType::Erased {
                                type_identity: "named(name(example::Evidence))".to_owned(),
                            },
                        },
                    ],
                },
            },
            psi_checked_trees::CheckedUnitStructuralTypePlan {
                identity: "example::Helper".to_owned(),
                shape: CheckedUnitStructuralTypeShape::Record { fields: Vec::new() },
            },
            psi_checked_trees::CheckedUnitStructuralTypePlan {
                identity: "example::Root".to_owned(),
                shape: CheckedUnitStructuralTypeShape::Record { fields: Vec::new() },
            },
        ],
        structural_domains: vec![psi_checked_trees::CheckedUnitStructuralDomainPlan {
            domain,
            identity: "example::Acknowledgement::Pending".to_owned(),
            carrier_type_identity: "example::Acknowledgement".to_owned(),
        }],
        boundary_machines: vec![CheckedBoundaryMachinePlan {
            machine: boundary,
            state: boundary_state,
            attachment_type_identity: Some("example::Acknowledgement".to_owned()),
            structural_parameters: vec![psi_checked_trees::CheckedUnitStructuralParameterPlan {
                is_self: true,
                ..structural_parameter(0)
            }],
            result_type: None,
            domain_requirements: vec![
                psi_checked_trees::CheckedUnitStructuralDomainRequirementPlan {
                    argument_index: 0,
                    domain,
                },
            ],
            contract_fingerprint: 0x303,
            contract_service_reach: contract_reach,
            service_reach: reach,
        }],
        machines: vec![
            CheckedUnitEffectMachinePlan {
                machine: root,
                state: root_state,
                attachment_type_identity: "example::Root".to_owned(),
                structural_parameters: vec![structural_parameter(7)],
                trivial_affine_locals: Vec::new(),
                entry_claims: vec![entry_claim(root, root_state)],
                body_qualifications: vec![domain],
                contract_fingerprint: 0x101,
                contract_service_reach: contract_reach,
                service_reach: reach,
                operations: vec![
                    CheckedUnitEffectOperationPlan::CallUnit {
                        coordinate: psi_checked_trees::CheckedUnitCallCoordinate {
                            statement_index: 0,
                            call_ordinal: 0,
                        },
                        target_machine: helper,
                        target_state: helper_state,
                        target_contract_fingerprint: 0x202,
                        service_reach: reach,
                        structural_arguments: vec![
                            psi_checked_trees::CheckedUnitStructuralArgumentPlan {
                                source_parameter_index: 0,
                                type_identity: "example::Acknowledgement".to_owned(),
                                path: Vec::new(),
                            },
                        ],
                        claim_transfers: vec![psi_checked_trees::CheckedUnitClaimTransferPlan {
                            claim_identity: unit_claim(root, root_state),
                            argument_index: 0,
                        }],
                    },
                    CheckedUnitEffectOperationPlan::ReturnUnit {
                        statement_index: 1,
                        trivial_affine_local_discard_ordinals: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                ],
            },
            CheckedUnitEffectMachinePlan {
                machine: helper,
                state: helper_state,
                attachment_type_identity: "example::Helper".to_owned(),
                structural_parameters: vec![structural_parameter(3)],
                trivial_affine_locals: Vec::new(),
                entry_claims: vec![entry_claim(helper, helper_state)],
                body_qualifications: vec![domain],
                contract_fingerprint: 0x202,
                contract_service_reach: contract_reach,
                service_reach: reach,
                operations: vec![
                    CheckedUnitEffectOperationPlan::PortWrite {
                        coordinate: psi_checked_trees::CheckedUnitCallCoordinate {
                            statement_index: 0,
                            call_ordinal: 0,
                        },
                        port: 0x3f8,
                        value: 0x5a,
                        service_reach: reach,
                    },
                    CheckedUnitEffectOperationPlan::BoundaryCall {
                        coordinate: psi_checked_trees::CheckedUnitCallCoordinate {
                            statement_index: 1,
                            call_ordinal: 0,
                        },
                        target_machine: boundary,
                        target_state: boundary_state,
                        target_contract_fingerprint: 0x303,
                        service_reach: reach,
                        structural_arguments: vec![
                            psi_checked_trees::CheckedUnitStructuralArgumentPlan {
                                source_parameter_index: 0,
                                type_identity: "example::Acknowledgement".to_owned(),
                                path: Vec::new(),
                            },
                        ],
                        completion_receipts: vec![
                            psi_checked_trees::CheckedUnitClaimTransferPlan {
                                claim_identity: unit_claim(helper, helper_state),
                                argument_index: 0,
                            },
                        ],
                    },
                    CheckedUnitEffectOperationPlan::ReturnUnit {
                        statement_index: 2,
                        trivial_affine_local_discard_ordinals: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                ],
            },
        ],
    };
    checked
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

fn install_structural_scalar_return_fixture(checked: &mut CheckedTrees) {
    let root = SymbolHandle::from_arena_index(1);
    let entry = SymbolHandle::from_arena_index(11);
    let affine_parameter = |position| psi_checked_trees::CheckedUnitStructuralParameterPlan {
        position,
        is_self: false,
        type_identity: "example::Acknowledgement".to_owned(),
        multiplicity: Multiplicity::Affine,
        qualifications: Vec::new(),
    };
    checked.facts.flow.terminal_structural_scalar_returns =
        psi_checked_trees::CheckedStructuralScalarReturnPlans {
            structural_types: checked
                .facts
                .flow
                .terminal_unit_effects
                .structural_types
                .clone(),
            machines: vec![CheckedStructuralScalarReturnMachinePlan {
                machine: root,
                state: entry,
                attachment_type_identity: "example::Root".to_owned(),
                structural_parameters: vec![affine_parameter(0), affine_parameter(1)],
                scalar_parameters: Vec::new(),
                bindings: Vec::new(),
                result_type: PrimitiveType::I32,
                return_statement_ordinal: 0,
                shared_boolean_convergence: None,
                caller_requirements: Vec::new(),
                scalar_requirements: Vec::new(),
                cleanup_actions: vec![
                    CheckedStructuralScalarReturnCleanupAction::DiscardRoot(1),
                    CheckedStructuralScalarReturnCleanupAction::DiscardRoot(0),
                ],
            }],
        };
    checked.facts.values.scalar_expressions.expressions.push(
        psi_checked_trees::CheckedLocatedScalarExpression {
            state: entry,
            statement_ordinal: 0,
            role: CheckedScalarExpressionRole::Return,
            expression: CheckedScalarExpression::IntegerLiteral {
                literal: psi_numerics::literals::IntegerLiteral::from_value(7).with_landing(
                    psi_numerics::literals::IntegerLanding {
                        landed_type: psi_numerics::literals::LandedIntegerType::I32,
                        domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
                    },
                ),
            },
        },
    );
}

#[test]
fn structural_scalar_return_lowers_value_before_exact_affine_cleanup() {
    let mut checked = hard_root_checked_fixture();
    install_structural_scalar_return_fixture(&mut checked);

    let lowered = lower_machine(&checked, "example::Root::enter")
        .expect("closed scalar return and exact affine cleanup should lower");
    let [machine] = lowered.semantic_module.machines.as_slice() else {
        panic!("structural scalar return lowers one attached machine")
    };
    assert_eq!(machine.structural_parameters.len(), 2);
    assert!(machine.parameters.is_empty());
    assert!(matches!(machine.result, TerminalMachineResult::Scalar(_)));
    let [block] = machine.blocks.as_slice() else {
        panic!("closed structural scalar return lowers one block")
    };
    assert!(matches!(
        &block.terminator,
        Terminator::Return {
            cleanup_actions,
            ..
        } if cleanup_actions == &[
            TerminalAffineCleanupAction::DiscardRoot(place_id(2)),
            TerminalAffineCleanupAction::DiscardRoot(place_id(1)),
        ]
    ));
    assert!(matches!(
        block.operations.as_slice(),
        [Operation {
            kind: OperationKind::IntegerConstant { .. },
            ..
        }]
    ));
    let bytes = psi_terminal_codec::encode_module(&lowered.semantic_module)
        .expect("structural scalar return should encode canonically");
    assert_eq!(
        psi_terminal_codec::decode_module(&bytes)
            .expect("canonical structural scalar return bytes should decode"),
        lowered.semantic_module
    );
}

#[test]
fn structural_scalar_return_fails_closed_on_stale_cleanup() {
    let mut checked = hard_root_checked_fixture();
    install_structural_scalar_return_fixture(&mut checked);
    checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .machines[0]
        .cleanup_actions = vec![
        CheckedStructuralScalarReturnCleanupAction::DiscardRoot(0),
        CheckedStructuralScalarReturnCleanupAction::DiscardRoot(1),
    ];

    assert!(matches!(
        lower_machine(&checked, "example::Root::enter"),
        Err(LoweringError::Unsupported(
            "structural scalar return cleanup does not consume its exact frontier"
        ))
    ));
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
        &psi_proof_kernel::AdmissionProfile::default(),
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
        &psi_proof_kernel::AdmissionProfile::default(),
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
        &psi_proof_kernel::AdmissionProfile::default(),
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
        &psi_proof_kernel::AdmissionProfile::default(),
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
        &psi_proof_kernel::AdmissionProfile::default(),
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
fn attached_unit_hard_root_lowers_exact_checked_closure_with_dense_identities() {
    let checked = hard_root_checked_fixture();
    let lowered = lower_machine(&checked, "example::Root::enter")
        .expect("complete attached Unit closure should lower");
    let module = &lowered.semantic_module;

    assert_eq!(module.entry, machine_id(1));
    assert_eq!(module.structural_types.len(), 3);
    assert_eq!(
        module
            .structural_types
            .iter()
            .map(|declaration| declaration.id.get())
            .collect::<Vec<_>>(),
        [1, 2, 3]
    );
    assert_eq!(module.structural_domains[0].id, structural_domain_id(1));
    let acknowledgement = module
        .structural_types
        .iter()
        .find(|declaration| declaration.identity == "example::Acknowledgement")
        .expect("acknowledgement structural type");
    let StructuralTypeShape::Record { fields } = &acknowledgement.shape else {
        panic!("acknowledgement is a record")
    };
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[1].relevance, psi_terminal::BindingRelevance::Erased);
    assert!(matches!(
        &fields[1].field_type,
        StructuralFieldType::Erased { type_identity }
            if type_identity == "named(name(example::Evidence))"
    ));
    assert_eq!(module.services[0].id, service_id(1));
    assert_eq!(module.services[0].identity, "PortIo");
    assert_eq!(module.boundary_machines[0].id, boundary_machine_id(1));
    assert_eq!(module.boundary_machines[0].requires.len(), 1);
    assert_eq!(module.machines.len(), 2);
    assert_eq!(module.machines[0].id, machine_id(1));
    assert_eq!(module.machines[1].id, machine_id(2));
    assert_eq!(module.machines[0].structural_parameters[0].position, 0);
    assert_eq!(module.machines[1].structural_parameters[0].position, 0);
    assert_eq!(module.machines[0].entry_claims[0].claim, claim_id(1));
    assert_eq!(module.machines[1].entry_claims[0].claim, claim_id(1));

    let [root_call] = module.machines[0].blocks[0].operations.as_slice() else {
        panic!("root emits one call before its Unit return")
    };
    let OperationKind::CallUnit {
        callee,
        structural_arguments,
        claim_transfers,
        requirement_obligations,
        ..
    } = &root_call.kind
    else {
        panic!("root operation should be CallUnit")
    };
    assert_eq!(*callee, machine_id(2));
    assert_eq!(structural_arguments[0].place, place_id(2));
    assert_eq!(claim_transfers[0].claim, claim_id(1));
    assert!(requirement_obligations.is_empty());

    let [port, settlement] = module.machines[1].blocks[0].operations.as_slice() else {
        panic!("helper emits port output and boundary settlement")
    };
    assert!(matches!(
        port.kind,
        OperationKind::PortWrite {
            service,
            port: 0x3f8,
            value: 0x5a,
        } if service == service_id(1)
    ));
    let OperationKind::BoundaryCall {
        boundary,
        structural_arguments,
        completion_receipts,
        requirement_obligations,
    } = &settlement.kind
    else {
        panic!("helper settlement should be BoundaryCall")
    };
    assert_eq!(*boundary, boundary_machine_id(1));
    assert_eq!(structural_arguments[0].place, place_id(3));
    assert_eq!(completion_receipts[0].claim, claim_id(1));
    assert!(requirement_obligations.is_empty());
    assert!(matches!(
        module.machines[0].blocks[0].terminator,
        Terminator::ReturnUnit { edge, .. } if edge == edge_id(1)
    ));
    assert!(matches!(
        module.machines[1].blocks[0].terminator,
        Terminator::ReturnUnit { edge, .. } if edge == edge_id(2)
    ));
    assert!(lowered.proof_bundle.evidence.is_empty());
    assert_eq!(
        lower_machine(&checked, "example::Root::enter")
            .expect("repeat lowering")
            .semantic_module,
        *module,
        "canonical identities must be deterministic"
    );
}

#[test]
fn attached_unit_record_field_custody_crosses_call_and_boundary_settlement() {
    let mut checked = hard_root_checked_fixture();
    let plans = &mut checked.facts.flow.terminal_unit_effects;
    plans
        .structural_types
        .push(psi_checked_trees::CheckedUnitStructuralTypePlan {
            identity: "example::Token".to_owned(),
            shape: CheckedUnitStructuralTypeShape::Record { fields: Vec::new() },
        });
    let acknowledgement = plans
        .structural_types
        .iter_mut()
        .find(|shape| shape.identity == "example::Acknowledgement")
        .expect("acknowledgement shape");
    let CheckedUnitStructuralTypeShape::Record { fields } = &mut acknowledgement.shape else {
        panic!("acknowledgement is a record")
    };
    fields[0].identity = "#7".to_owned();
    fields[0].field_type = CheckedUnitStructuralFieldType::Structural {
        type_identity: "example::Token".to_owned(),
    };
    for machine in &mut plans.machines {
        machine.entry_claims[0].path =
            vec![CheckedUnitStructuralPathSegment::Field("#7".to_owned())];
    }

    let lowered = lower_machine(&checked, "example::Root::enter")
        .expect("record-field custody should cross the complete Unit closure");
    assert_eq!(
        lowered.semantic_module.machines[0].entry_claims[0].path,
        [StructuralPathSegment::Field("#7".to_owned())]
    );
    assert_eq!(
        lowered.semantic_module.machines[1].entry_claims[0].path,
        [StructuralPathSegment::Field("#7".to_owned())]
    );
    let bytes = psi_terminal_codec::encode_module(&lowered.semantic_module)
        .expect("aggregate custody must have a canonical terminal encoding");
    assert_eq!(
        psi_terminal_codec::decode_module(&bytes).expect("canonical aggregate custody bytes"),
        lowered.semantic_module
    );
}

#[test]
fn attached_unit_nested_record_claim_lowers_through_complete_closure() {
    let mut checked = hard_root_checked_fixture();
    let plans = &mut checked.facts.flow.terminal_unit_effects;
    plans.structural_types.extend([
        psi_checked_trees::CheckedUnitStructuralTypePlan {
            identity: "example::Pocket".to_owned(),
            shape: CheckedUnitStructuralTypeShape::Record {
                fields: vec![psi_checked_trees::CheckedUnitStructuralFieldPlan {
                    identity: "#9".to_owned(),
                    relevance: psi_terminal::BindingRelevance::Relevant,
                    field_type: CheckedUnitStructuralFieldType::Structural {
                        type_identity: "example::Token".to_owned(),
                    },
                }],
            },
        },
        psi_checked_trees::CheckedUnitStructuralTypePlan {
            identity: "example::Token".to_owned(),
            shape: CheckedUnitStructuralTypeShape::Record { fields: Vec::new() },
        },
    ]);
    let acknowledgement = plans
        .structural_types
        .iter_mut()
        .find(|shape| shape.identity == "example::Acknowledgement")
        .expect("acknowledgement shape");
    let CheckedUnitStructuralTypeShape::Record { fields } = &mut acknowledgement.shape else {
        panic!("acknowledgement is a record")
    };
    fields[0].identity = "#7".to_owned();
    fields[0].field_type = CheckedUnitStructuralFieldType::Structural {
        type_identity: "example::Pocket".to_owned(),
    };
    for boundary in &mut plans.boundary_machines {
        boundary.structural_parameters[0].multiplicity = Multiplicity::Affine;
    }
    for machine in &mut plans.machines {
        machine.structural_parameters[0].multiplicity = Multiplicity::Affine;
        machine.entry_claims[0].path = vec![
            CheckedUnitStructuralPathSegment::Field("#7".to_owned()),
            CheckedUnitStructuralPathSegment::Field("#9".to_owned()),
        ];
    }

    let lowered = lower_machine(&checked, "example::Root::enter")
        .expect("nested record custody should cross the complete Unit closure");
    for machine in &lowered.semantic_module.machines {
        assert_eq!(
            machine.structural_parameters[0].multiplicity,
            StructuralMultiplicity::Affine
        );
        assert_eq!(machine.entry_claims.len(), 1);
        assert_eq!(
            machine.entry_claims[0].path,
            [
                StructuralPathSegment::Field("#7".to_owned()),
                StructuralPathSegment::Field("#9".to_owned()),
            ]
        );
    }
    let acknowledgement = lowered
        .semantic_module
        .structural_types
        .iter()
        .find(|shape| shape.identity == "example::Acknowledgement")
        .expect("lowered acknowledgement shape");
    let StructuralTypeShape::Record { fields } = &acknowledgement.shape else {
        panic!("acknowledgement is a record")
    };
    assert!(matches!(
        &fields[0].field_type,
        StructuralFieldType::Structural(structural_type)
            if lowered.semantic_module.structural_types.iter().any(|shape| {
                shape.id == *structural_type && shape.identity == "example::Pocket"
            })
    ));
}

#[test]
fn attached_unit_disjoint_sibling_claims_lower_as_one_aggregate_transfer() {
    let mut checked = hard_root_checked_fixture();
    let plans = &mut checked.facts.flow.terminal_unit_effects;
    plans
        .structural_types
        .push(psi_checked_trees::CheckedUnitStructuralTypePlan {
            identity: "example::Token".to_owned(),
            shape: CheckedUnitStructuralTypeShape::Record { fields: Vec::new() },
        });
    let acknowledgement = plans
        .structural_types
        .iter_mut()
        .find(|shape| shape.identity == "example::Acknowledgement")
        .expect("acknowledgement shape");
    let CheckedUnitStructuralTypeShape::Record { fields } = &mut acknowledgement.shape else {
        panic!("acknowledgement is a record")
    };
    fields[0].identity = "#7".to_owned();
    fields[0].field_type = CheckedUnitStructuralFieldType::Structural {
        type_identity: "example::Token".to_owned(),
    };
    fields.insert(
        1,
        psi_checked_trees::CheckedUnitStructuralFieldPlan {
            identity: "#9".to_owned(),
            relevance: psi_terminal::BindingRelevance::Relevant,
            field_type: CheckedUnitStructuralFieldType::Structural {
                type_identity: "example::Token".to_owned(),
            },
        },
    );
    for boundary in &mut plans.boundary_machines {
        boundary.structural_parameters[0].multiplicity = Multiplicity::Affine;
    }
    for machine in &mut plans.machines {
        machine.structural_parameters[0].multiplicity = Multiplicity::Affine;
        machine.entry_claims[0].path =
            vec![CheckedUnitStructuralPathSegment::Field("#7".to_owned())];
        let mut sibling = machine.entry_claims[0].clone();
        sibling.claim_identity = unit_claim_at(machine.machine, machine.state, 1);
        sibling.path = vec![CheckedUnitStructuralPathSegment::Field("#9".to_owned())];
        machine.entry_claims.push(sibling);
    }
    let root = plans.machines[0].machine;
    let root_state = plans.machines[0].state;
    let CheckedUnitEffectOperationPlan::CallUnit {
        claim_transfers, ..
    } = &mut plans.machines[0].operations[0]
    else {
        unreachable!()
    };
    claim_transfers.push(psi_checked_trees::CheckedUnitClaimTransferPlan {
        claim_identity: unit_claim_at(root, root_state, 1),
        argument_index: 0,
    });
    let helper = plans.machines[1].machine;
    let helper_state = plans.machines[1].state;
    let CheckedUnitEffectOperationPlan::BoundaryCall {
        completion_receipts,
        ..
    } = &mut plans.machines[1].operations[1]
    else {
        unreachable!()
    };
    completion_receipts.push(psi_checked_trees::CheckedUnitClaimTransferPlan {
        claim_identity: unit_claim_at(helper, helper_state, 1),
        argument_index: 0,
    });

    let lowered = lower_machine(&checked, "example::Root::enter")
        .expect("both sibling resources should cross the complete Unit closure");
    for machine in &lowered.semantic_module.machines {
        assert_eq!(
            machine.structural_parameters[0].multiplicity,
            StructuralMultiplicity::Affine
        );
        assert_eq!(machine.entry_claims.len(), 2);
        assert_eq!(machine.entry_claims[0].claim, claim_id(1));
        assert_eq!(
            machine.entry_claims[0].path,
            [StructuralPathSegment::Field("#7".to_owned())]
        );
        assert_eq!(machine.entry_claims[1].claim, claim_id(2));
        assert_eq!(
            machine.entry_claims[1].path,
            [StructuralPathSegment::Field("#9".to_owned())]
        );
    }
    let bytes = psi_terminal_codec::encode_module(&lowered.semantic_module)
        .expect("multi-field custody must have a canonical terminal encoding");
    assert_eq!(
        psi_terminal_codec::decode_module(&bytes).expect("canonical aggregate custody bytes"),
        lowered.semantic_module
    );
}

#[test]
fn attached_unit_affine_argument_lowers_as_an_owned_transfer_without_a_claim_row() {
    let mut checked = hard_root_checked_fixture();
    let plans = &mut checked.facts.flow.terminal_unit_effects.machines;
    for plan in plans.iter_mut() {
        plan.structural_parameters[0].multiplicity = Multiplicity::Affine;
        plan.entry_claims.clear();
    }
    let CheckedUnitEffectOperationPlan::CallUnit {
        claim_transfers, ..
    } = &mut plans[0].operations[0]
    else {
        unreachable!()
    };
    claim_transfers.clear();
    plans[1].operations.retain(|operation| {
        !matches!(
            operation,
            CheckedUnitEffectOperationPlan::BoundaryCall { .. }
        )
    });
    let CheckedUnitEffectOperationPlan::ReturnUnit {
        trivial_affine_discards,
        ..
    } = plans[1].operations.last_mut().unwrap()
    else {
        unreachable!()
    };
    *trivial_affine_discards = vec![0];

    let lowered = lower_machine(&checked, "example::Root::enter")
        .expect("the checked affine Unit transfer should lower and verify");
    assert_eq!(
        lowered.semantic_module.machines[0].structural_parameters[0].multiplicity,
        StructuralMultiplicity::Affine
    );
    let OperationKind::CallUnit {
        claim_transfers, ..
    } = &lowered.semantic_module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    assert!(claim_transfers.is_empty());
}

#[test]
fn attached_unit_affine_return_lowers_exact_no_code_discard() {
    let mut checked = hard_root_checked_fixture();
    let root = &mut checked.facts.flow.terminal_unit_effects.machines[0];
    root.structural_parameters[0].multiplicity = Multiplicity::Affine;
    root.entry_claims.clear();
    root.operations = vec![CheckedUnitEffectOperationPlan::ReturnUnit {
        statement_index: 0,
        trivial_affine_local_discard_ordinals: Vec::new(),
        trivial_affine_discards: vec![0],
    }];

    let lowered = lower_machine(&checked, "example::Root::enter")
        .expect("checked affine discard should lower as explicit return-edge cleanup");
    let [machine] = lowered.semantic_module.machines.as_slice() else {
        panic!("the no-call closure should contain only its root")
    };
    let [block] = machine.blocks.as_slice() else {
        panic!("the no-call root should contain one block")
    };
    let Terminator::ReturnUnit {
        trivial_affine_discards,
        ..
    } = &block.terminator
    else {
        panic!("affine cleanup should remain attached to the Unit return")
    };
    assert_eq!(
        trivial_affine_discards,
        &[machine.structural_parameters[0].place]
    );
    let bytes = psi_terminal_codec::encode_module(&lowered.semantic_module)
        .expect("affine discard must have a canonical terminal encoding");
    assert_eq!(
        psi_terminal_codec::decode_module(&bytes).expect("canonical affine discard bytes"),
        lowered.semantic_module
    );
}

#[test]
fn attached_unit_hard_root_fails_closed_on_missing_transitive_member() {
    let mut checked = hard_root_checked_fixture();
    checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .retain(|machine| machine.contract_fingerprint != 0x202);

    assert!(matches!(
        lower_machine(&checked, "example::Root::enter"),
        Err(LoweringError::Unsupported(
            "attached Unit closure is missing a checked transitive machine plan"
        ))
    ));
}

#[test]
fn attached_unit_port_write_requires_exact_direct_checked_port_service() {
    let mut checked = hard_root_checked_fixture();
    let empty = psi_language_semantics::ServiceReachRowTable::EMPTY_ROW;
    let helper = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter_mut()
        .find(|machine| machine.contract_fingerprint == 0x202)
        .expect("helper plan");
    let CheckedUnitEffectOperationPlan::PortWrite { service_reach, .. } = &mut helper.operations[0]
    else {
        panic!("fixture begins helper with port output")
    };
    service_reach.direct = empty;

    assert!(matches!(
        lower_machine(&checked, "example::Root::enter"),
        Err(LoweringError::Unsupported(
            "port output does not carry the unique exact checked PortIo service"
        ))
    ));
}

fn source_projection(
    version: CheckedContentPlaceVersion,
    root: CheckedContentPlaceRoot,
    fields: &[(&str, u32)],
    semantic_domain: SemanticDomainId,
) -> CheckedContentConservationTerm {
    CheckedContentConservationTerm::Projection {
        domain: SymbolHandle::from_arena_index(70),
        semantic_domain,
        projection_machine: SymbolHandle::from_arena_index(71),
        projection_fingerprint: 0xfeed,
        subject: CheckedContentStructuralPlace {
            version,
            root,
            segments: fields
                .iter()
                .map(|(name, symbol)| {
                    CheckedContentPlaceSegment::Field(ContentFieldSegment {
                        symbol: SymbolHandle::from_arena_index(*symbol),
                        name: (*name).to_owned(),
                    })
                })
                .collect(),
        },
    }
}

#[test]
fn structural_scalar_return_reconstructs_closed_exact_expression_proof() {
    let mut checked = hard_root_checked_fixture();
    install_structural_scalar_return_fixture(&mut checked);
    let landed = |value| {
        psi_numerics::literals::IntegerLiteral::from_value(value).with_landing(
            psi_numerics::literals::IntegerLanding {
                landed_type: psi_numerics::literals::LandedIntegerType::I32,
                domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
            },
        )
    };
    checked.facts.values.scalar_expressions.expressions[0].expression =
        CheckedScalarExpression::IntegerBinary {
            kind: CheckedIntegerBinaryKind::ExactAdd,
            primitive_type: PrimitiveType::I32,
            left: Box::new(CheckedScalarExpression::IntegerLiteral { literal: landed(3) }),
            right: Box::new(CheckedScalarExpression::IntegerLiteral { literal: landed(4) }),
        };

    let lowered = lower_machine(&checked, "example::Root::enter")
        .expect("closed exact expression should lower with reconstructed proof");
    let operations = &lowered.semantic_module.machines[0].blocks[0].operations;
    assert!(matches!(
        operations.as_slice(),
        [
            Operation {
                kind: OperationKind::IntegerConstant { .. },
                ..
            },
            Operation {
                kind: OperationKind::IntegerConstant { .. },
                ..
            },
            Operation {
                kind: OperationKind::ExactIntegerAdd { .. },
                ..
            }
        ]
    ));
    assert_eq!(lowered.proof_bundle.evidence.len(), 1);
    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &psi_proof_kernel::AdmissionProfile::default(),
    )
    .expect("the reconstructed exact-operation proof should verify canonically");
    let module_bytes = psi_terminal_codec::encode_module(&lowered.semantic_module)
        .expect("closed structural expression module should encode canonically");
    assert_eq!(
        psi_terminal_codec::decode_module(&module_bytes)
            .expect("closed structural expression module should decode canonically"),
        lowered.semantic_module
    );
    assert!(matches!(
        &lowered.semantic_module.machines[0].blocks[0].terminator,
        Terminator::Return {
            cleanup_actions,
            ..
        } if cleanup_actions == &[
            TerminalAffineCleanupAction::DiscardRoot(place_id(2)),
            TerminalAffineCleanupAction::DiscardRoot(place_id(1)),
        ]
    ));

    checked.facts.values.scalar_expressions.expressions[0].expression =
        CheckedScalarExpression::Parameter {
            position: 0,
            primitive_type: PrimitiveType::I32,
        };
    assert!(matches!(
        lower_machine(&checked, "example::Root::enter"),
        Err(LoweringError::Unsupported(
            "structural scalar return is outside its checked value/control slice"
        ))
    ));
}

#[test]
fn structural_scalar_return_materializes_branch_free_local_prefix_before_cleanup() {
    let mut checked = hard_root_checked_fixture();
    install_structural_scalar_return_fixture(&mut checked);
    let landed = |value| {
        psi_numerics::literals::IntegerLiteral::from_value(value).with_landing(
            psi_numerics::literals::IntegerLanding {
                landed_type: psi_numerics::literals::LandedIntegerType::I32,
                domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
            },
        )
    };
    let plan = &mut checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .machines[0];
    plan.bindings = vec![psi_checked_trees::CheckedScalarBinding {
        statement_ordinal: 0,
        primitive_type: PrimitiveType::I32,
        value: CheckedScalarBindingValue::Expression,
    }];
    plan.return_statement_ordinal = 1;
    checked.facts.values.scalar_expressions.expressions = vec![
        psi_checked_trees::CheckedLocatedScalarExpression {
            state: plan.state,
            statement_ordinal: 0,
            role: CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 0 },
            expression: CheckedScalarExpression::IntegerBinary {
                kind: CheckedIntegerBinaryKind::ExactAdd,
                primitive_type: PrimitiveType::I32,
                left: Box::new(CheckedScalarExpression::IntegerLiteral { literal: landed(3) }),
                right: Box::new(CheckedScalarExpression::IntegerLiteral { literal: landed(4) }),
            },
        },
        psi_checked_trees::CheckedLocatedScalarExpression {
            state: plan.state,
            statement_ordinal: 1,
            role: CheckedScalarExpressionRole::Return,
            expression: CheckedScalarExpression::Local {
                position: 0,
                primitive_type: PrimitiveType::I32,
            },
        },
    ];

    let lowered = lower_machine(&checked, "example::Root::enter")
        .expect("checked local prefix should lower before exact affine cleanup");
    let block = &lowered.semantic_module.machines[0].blocks[0];
    assert!(matches!(
        block.operations.as_slice(),
        [
            Operation {
                kind: OperationKind::IntegerConstant { .. },
                ..
            },
            Operation {
                kind: OperationKind::IntegerConstant { .. },
                ..
            },
            Operation {
                kind: OperationKind::ExactIntegerAdd { .. },
                ..
            }
        ]
    ));
    assert!(matches!(
        &block.terminator,
        Terminator::Return {
            value,
            cleanup_actions,
            ..
        } if *value == value_id(3)
            && cleanup_actions == &[
                TerminalAffineCleanupAction::DiscardRoot(place_id(2)),
                TerminalAffineCleanupAction::DiscardRoot(place_id(1)),
            ]
    ));
    assert_eq!(lowered.proof_bundle.evidence.len(), 1);
    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &psi_proof_kernel::AdmissionProfile::default(),
    )
    .expect("local-prefix cleanup module should verify");

    checked.facts.values.scalar_expressions.expressions[0].expression =
        CheckedScalarExpression::Local {
            position: 0,
            primitive_type: PrimitiveType::I32,
        };
    assert!(matches!(
        lower_machine(&checked, "example::Root::enter"),
        Err(LoweringError::Unsupported(
            "structural scalar binding is not one branch-free local expression"
        ))
    ));
}

#[test]
fn structural_scalar_return_supports_repeated_carried_short_circuit_local_continuations() {
    let mut checked = hard_root_checked_fixture();
    install_structural_scalar_return_fixture(&mut checked);
    let plan = &mut checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .machines[0];
    plan.bindings = vec![
        psi_checked_trees::CheckedScalarBinding {
            statement_ordinal: 0,
            primitive_type: PrimitiveType::Bool,
            value: CheckedScalarBindingValue::Expression,
        },
        psi_checked_trees::CheckedScalarBinding {
            statement_ordinal: 1,
            primitive_type: PrimitiveType::Bool,
            value: CheckedScalarBindingValue::Expression,
        },
        psi_checked_trees::CheckedScalarBinding {
            statement_ordinal: 2,
            primitive_type: PrimitiveType::Bool,
            value: CheckedScalarBindingValue::Expression,
        },
        psi_checked_trees::CheckedScalarBinding {
            statement_ordinal: 3,
            primitive_type: PrimitiveType::Bool,
            value: CheckedScalarBindingValue::Expression,
        },
        psi_checked_trees::CheckedScalarBinding {
            statement_ordinal: 4,
            primitive_type: PrimitiveType::Bool,
            value: CheckedScalarBindingValue::Expression,
        },
        psi_checked_trees::CheckedScalarBinding {
            statement_ordinal: 5,
            primitive_type: PrimitiveType::Bool,
            value: CheckedScalarBindingValue::Expression,
        },
        psi_checked_trees::CheckedScalarBinding {
            statement_ordinal: 6,
            primitive_type: PrimitiveType::Bool,
            value: CheckedScalarBindingValue::Expression,
        },
    ];
    plan.result_type = PrimitiveType::Bool;
    plan.return_statement_ordinal = 7;
    checked.facts.values.scalar_expressions.expressions = vec![
        psi_checked_trees::CheckedLocatedScalarExpression {
            state: plan.state,
            statement_ordinal: 0,
            role: CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 0 },
            expression: CheckedScalarExpression::Boolean(Box::new(
                CheckedBooleanExpression::Constant(true),
            )),
        },
        psi_checked_trees::CheckedLocatedScalarExpression {
            state: plan.state,
            statement_ordinal: 1,
            role: CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 1 },
            expression: CheckedScalarExpression::Boolean(Box::new(CheckedBooleanExpression::And {
                left: Box::new(CheckedBooleanExpression::Local { position: 0 }),
                right: Box::new(CheckedBooleanExpression::Constant(false)),
            })),
        },
        psi_checked_trees::CheckedLocatedScalarExpression {
            state: plan.state,
            statement_ordinal: 2,
            role: CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 2 },
            expression: CheckedScalarExpression::Boolean(Box::new(CheckedBooleanExpression::Not(
                Box::new(CheckedBooleanExpression::Local { position: 1 }),
            ))),
        },
        psi_checked_trees::CheckedLocatedScalarExpression {
            state: plan.state,
            statement_ordinal: 3,
            role: CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 3 },
            expression: CheckedScalarExpression::Boolean(Box::new(CheckedBooleanExpression::Or {
                left: Box::new(CheckedBooleanExpression::Local { position: 2 }),
                right: Box::new(CheckedBooleanExpression::Constant(false)),
            })),
        },
        psi_checked_trees::CheckedLocatedScalarExpression {
            state: plan.state,
            statement_ordinal: 4,
            role: CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 4 },
            expression: CheckedScalarExpression::Boolean(Box::new(CheckedBooleanExpression::Not(
                Box::new(CheckedBooleanExpression::Local { position: 3 }),
            ))),
        },
        psi_checked_trees::CheckedLocatedScalarExpression {
            state: plan.state,
            statement_ordinal: 5,
            role: CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 5 },
            expression: CheckedScalarExpression::Boolean(Box::new(CheckedBooleanExpression::And {
                left: Box::new(CheckedBooleanExpression::Local { position: 4 }),
                right: Box::new(CheckedBooleanExpression::Constant(true)),
            })),
        },
        psi_checked_trees::CheckedLocatedScalarExpression {
            state: plan.state,
            statement_ordinal: 6,
            role: CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 6 },
            expression: CheckedScalarExpression::Boolean(Box::new(CheckedBooleanExpression::Not(
                Box::new(CheckedBooleanExpression::Local { position: 5 }),
            ))),
        },
        psi_checked_trees::CheckedLocatedScalarExpression {
            state: plan.state,
            statement_ordinal: 7,
            role: CheckedScalarExpressionRole::Return,
            expression: CheckedScalarExpression::Boolean(Box::new(
                CheckedBooleanExpression::Local { position: 6 },
            )),
        },
    ];

    let lowered = lower_machine(&checked, "example::Root::enter")
        .expect("repeated short-circuit locals should compose through carried continuations");
    let machine = &lowered.semantic_module.machines[0];
    assert_eq!(machine.blocks.len(), 16);
    let second_stage = machine
        .blocks
        .iter()
        .find(|block| block.id == block_id(6))
        .expect("the first short-circuit result enters the second decision stage");
    let third_stage = machine
        .blocks
        .iter()
        .find(|block| block.id == block_id(11))
        .expect("the second short-circuit result enters the third decision stage");
    let continuation = machine
        .blocks
        .iter()
        .find(|block| block.id == block_id(16))
        .expect("the final short-circuit result enters the return continuation");
    assert!(matches!(
        machine.blocks[0].operations.first(),
        Some(Operation {
            kind: OperationKind::BooleanConstant { value: true },
            ..
        })
    ));
    assert!(matches!(
        second_stage.parameters.as_slice(),
        [ValueDeclaration {
            scalar_type: ScalarType::Boolean,
            ..
        }]
    ));
    assert!(matches!(
        second_stage.operations.as_slice(),
        [Operation {
            kind: OperationKind::BooleanNot { .. },
            ..
        }]
    ));
    assert!(matches!(
        third_stage.parameters.as_slice(),
        [ValueDeclaration {
            scalar_type: ScalarType::Boolean,
            ..
        }]
    ));
    assert!(matches!(
        third_stage.operations.as_slice(),
        [Operation {
            kind: OperationKind::BooleanNot { .. },
            ..
        }]
    ));
    assert!(matches!(
        continuation.parameters.as_slice(),
        [ValueDeclaration {
            scalar_type: ScalarType::Boolean,
            ..
        }]
    ));
    assert!(matches!(
        continuation.operations.as_slice(),
        [Operation {
            kind: OperationKind::BooleanNot { .. },
            ..
        }]
    ));
    assert!(matches!(
        &continuation.terminator,
        Terminator::Return {
            cleanup_actions,
            ..
        } if cleanup_actions == &[
            TerminalAffineCleanupAction::DiscardRoot(place_id(2)),
            TerminalAffineCleanupAction::DiscardRoot(place_id(1)),
        ]
    ));
    assert!(
        machine.blocks[..15]
            .iter()
            .all(|block| match &block.terminator {
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } =>
                    when_true.trivial_affine_discards.is_empty()
                        && when_false.trivial_affine_discards.is_empty(),
                Terminator::Jump {
                    target,
                    trivial_affine_discards,
                    ..
                } => {
                    matches!(*target, target if target == block_id(6)
                            || target == block_id(11)
                            || target == block_id(16))
                        && trivial_affine_discards.is_empty()
                }
                _ => false,
            })
    );
    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &psi_proof_kernel::AdmissionProfile::default(),
    )
    .expect("short-circuit local convergence should preserve the structural frontier");
    let bytes = psi_terminal_codec::encode_module(&lowered.semantic_module)
        .expect("short-circuit local cleanup should encode canonically");
    assert_eq!(
        psi_terminal_codec::decode_module(&bytes)
            .expect("short-circuit local cleanup should decode canonically"),
        lowered.semantic_module
    );

    checked.facts.values.scalar_expressions.expressions[7].expression =
        CheckedScalarExpression::Boolean(Box::new(CheckedBooleanExpression::And {
            left: Box::new(CheckedBooleanExpression::Local { position: 6 }),
            right: Box::new(CheckedBooleanExpression::Constant(false)),
        }));
    let lowered = lower_machine(&checked, "example::Root::enter")
        .expect("repeated local decisions should feed a final short-circuit return");
    let machine = &lowered.semantic_module.machines[0];
    assert_eq!(machine.blocks.len(), 20);
    let final_decision = &machine.blocks[15..];
    assert!(matches!(
        final_decision[0].parameters.as_slice(),
        [ValueDeclaration {
            scalar_type: ScalarType::Boolean,
            ..
        }]
    ));
    assert!(matches!(
        final_decision[0].operations.as_slice(),
        [Operation {
            kind: OperationKind::BooleanNot { .. },
            ..
        }]
    ));
    assert!(final_decision.iter().all(|block| match &block.terminator {
        Terminator::Conditional {
            when_true,
            when_false,
            ..
        } => {
            when_true.trivial_affine_discards.is_empty()
                && when_false.trivial_affine_discards.is_empty()
        }
        Terminator::Return {
            cleanup_actions, ..
        } =>
            cleanup_actions
                == &[
                    TerminalAffineCleanupAction::DiscardRoot(place_id(2)),
                    TerminalAffineCleanupAction::DiscardRoot(place_id(1)),
                ],
        _ => false,
    }));
    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &psi_proof_kernel::AdmissionProfile::default(),
    )
    .expect("final short-circuit cleanup should verify after repeated local convergence");
    let bytes = psi_terminal_codec::encode_module(&lowered.semantic_module)
        .expect("composed final short-circuit cleanup should encode canonically");
    assert_eq!(
        psi_terminal_codec::decode_module(&bytes)
            .expect("composed final short-circuit cleanup should decode canonically"),
        lowered.semantic_module
    );

    checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .machines[0]
        .bindings[5]
        .primitive_type = PrimitiveType::I32;
    assert!(matches!(
        lower_machine(&checked, "example::Root::enter"),
        Err(LoweringError::Unsupported(
            "structural scalar short-circuit binding has a non-Boolean carrier"
        ))
    ));
}

#[test]
fn structural_scalar_return_maps_interleaved_scalar_parameters_before_cleanup() {
    let mut checked = hard_root_checked_fixture();
    install_structural_scalar_return_fixture(&mut checked);
    let plan = &mut checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .machines[0];
    plan.structural_parameters[1].position = 2;
    plan.scalar_parameters = vec![
        psi_checked_trees::CheckedStructuralScalarParameterPlan {
            source_position: 1,
            primitive_type: PrimitiveType::I32,
        },
        psi_checked_trees::CheckedStructuralScalarParameterPlan {
            source_position: 3,
            primitive_type: PrimitiveType::Bool,
        },
    ];
    plan.result_type = PrimitiveType::Bool;
    plan.cleanup_actions = vec![
        CheckedStructuralScalarReturnCleanupAction::DiscardRoot(2),
        CheckedStructuralScalarReturnCleanupAction::DiscardRoot(0),
    ];
    checked.facts.values.scalar_expressions.expressions[0].expression =
        CheckedScalarExpression::Boolean(Box::new(CheckedBooleanExpression::Not(Box::new(
            CheckedBooleanExpression::Parameter { position: 1 },
        ))));

    let lowered = lower_machine(&checked, "example::Root::enter")
        .expect("exact mixed parameter map should lower before affine cleanup");
    let machine = &lowered.semantic_module.machines[0];
    assert!(matches!(
        machine.parameters.as_slice(),
        [
            ValueDeclaration {
                id,
                scalar_type: ScalarType::Integer(_),
            },
            ValueDeclaration {
                id: bool_id,
                scalar_type: ScalarType::Boolean,
            }
        ] if *id == value_id(1) && *bool_id == value_id(2)
    ));
    assert_eq!(machine.structural_parameters.len(), 2);
    assert!(matches!(
        machine.blocks[0].operations.as_slice(),
        [Operation {
            result: psi_terminal::OperationResult::Scalar(ValueDeclaration {
                id,
                scalar_type: ScalarType::Boolean,
            }),
            kind: OperationKind::BooleanNot { operand },
            ..
        }] if *id == value_id(3) && *operand == value_id(2)
    ));
    assert!(matches!(
        &machine.blocks[0].terminator,
        Terminator::Return {
            value,
            cleanup_actions,
            ..
        } if *value == value_id(3)
            && cleanup_actions == &[
                TerminalAffineCleanupAction::DiscardRoot(place_id(2)),
                TerminalAffineCleanupAction::DiscardRoot(place_id(1)),
            ]
    ));
    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &psi_proof_kernel::AdmissionProfile::default(),
    )
    .expect("mixed scalar/structural parameter module should verify");

    checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .machines[0]
        .scalar_parameters[0]
        .source_position = 0;
    assert!(matches!(
        lower_machine(&checked, "example::Root::enter"),
        Err(LoweringError::Unsupported(
            "structural scalar return parameter maps overlap or repeat a source position"
        ))
    ));
}

#[test]
fn structural_scalar_return_emits_boolean_paths_before_cleanup() {
    let mut checked = hard_root_checked_fixture();
    install_structural_scalar_return_fixture(&mut checked);
    checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .machines[0]
        .result_type = PrimitiveType::Bool;
    checked.facts.values.scalar_expressions.expressions[0].expression =
        CheckedScalarExpression::Boolean(Box::new(CheckedBooleanExpression::Not(Box::new(
            CheckedBooleanExpression::Equal {
                left: Box::new(CheckedBooleanExpression::Constant(true)),
                right: Box::new(CheckedBooleanExpression::Constant(false)),
            },
        ))));

    let lowered = lower_machine(&checked, "example::Root::enter")
        .expect("closed branch-free Boolean should lower before structural cleanup");
    let machine = &lowered.semantic_module.machines[0];
    assert!(matches!(
        machine.result,
        TerminalMachineResult::Scalar(ValueDeclaration {
            scalar_type: ScalarType::Boolean,
            ..
        })
    ));
    assert!(matches!(
        machine.blocks[0].operations.as_slice(),
        [
            Operation {
                kind: OperationKind::BooleanConstant { value: true },
                ..
            },
            Operation {
                kind: OperationKind::BooleanConstant { value: false },
                ..
            },
            Operation {
                kind: OperationKind::BooleanEqual { .. },
                ..
            },
            Operation {
                kind: OperationKind::BooleanNot { .. },
                ..
            }
        ]
    ));
    assert!(matches!(
        &machine.blocks[0].terminator,
        Terminator::Return {
            cleanup_actions,
            ..
        } if cleanup_actions == &[
            TerminalAffineCleanupAction::DiscardRoot(place_id(2)),
            TerminalAffineCleanupAction::DiscardRoot(place_id(1)),
        ]
    ));
    assert!(lowered.proof_bundle.evidence.is_empty());
    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &psi_proof_kernel::AdmissionProfile::default(),
    )
    .expect("closed Boolean return and cleanup should verify");
    let bytes = psi_terminal_codec::encode_module(&lowered.semantic_module)
        .expect("closed Boolean cleanup module should encode canonically");
    assert_eq!(
        psi_terminal_codec::decode_module(&bytes)
            .expect("closed Boolean cleanup module should decode canonically"),
        lowered.semantic_module
    );

    let plan = &mut checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .machines[0];
    plan.bindings = vec![psi_checked_trees::CheckedScalarBinding {
        statement_ordinal: 0,
        primitive_type: PrimitiveType::Bool,
        value: CheckedScalarBindingValue::Expression,
    }];
    plan.return_statement_ordinal = 1;
    checked.facts.values.scalar_expressions.expressions = vec![
        psi_checked_trees::CheckedLocatedScalarExpression {
            state: plan.state,
            statement_ordinal: 0,
            role: CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 0 },
            expression: CheckedScalarExpression::Boolean(Box::new(
                CheckedBooleanExpression::Constant(true),
            )),
        },
        psi_checked_trees::CheckedLocatedScalarExpression {
            state: plan.state,
            statement_ordinal: 1,
            role: CheckedScalarExpressionRole::Return,
            expression: CheckedScalarExpression::Boolean(Box::new(CheckedBooleanExpression::And {
                left: Box::new(CheckedBooleanExpression::Local { position: 0 }),
                right: Box::new(CheckedBooleanExpression::Constant(false)),
            })),
        },
    ];
    let lowered = lower_machine(&checked, "example::Root::enter")
        .expect("short-circuit Boolean leaves should each perform exact affine cleanup");
    let blocks = &lowered.semantic_module.machines[0].blocks;
    assert_eq!(blocks.len(), 5);
    assert!(matches!(
        blocks[0].operations.first(),
        Some(Operation {
            kind: OperationKind::BooleanConstant { value: true },
            ..
        })
    ));
    let mut return_count = 0;
    for block in blocks {
        match &block.terminator {
            Terminator::Return {
                cleanup_actions, ..
            } => {
                return_count += 1;
                assert_eq!(
                    cleanup_actions,
                    &[
                        TerminalAffineCleanupAction::DiscardRoot(place_id(2)),
                        TerminalAffineCleanupAction::DiscardRoot(place_id(1)),
                    ]
                );
            }
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                assert!(when_true.trivial_affine_discards.is_empty());
                assert!(when_false.trivial_affine_discards.is_empty());
            }
            _ => panic!("short-circuit return emits only decisions and scalar leaves"),
        }
    }
    assert_eq!(return_count, 3);
    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &psi_proof_kernel::AdmissionProfile::default(),
    )
    .expect("short-circuit cleanup frontiers should verify on every path");
    let bytes = psi_terminal_codec::encode_module(&lowered.semantic_module)
        .expect("short-circuit structural cleanup should encode canonically");
    assert_eq!(
        psi_terminal_codec::decode_module(&bytes)
            .expect("short-circuit structural cleanup should decode canonically"),
        lowered.semantic_module
    );
}
