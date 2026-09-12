use super::*;

fn forwarded(join: bool) -> TerminalModule {
    let mut module = fixture();
    let machine = &mut module.machines[0];
    machine.parameters.push(ValueDeclaration {
        qualifications: Default::default(),
        id: id(40, ValueId::new),
        scalar_type: ScalarType::Boolean,
    });
    let Terminator::Conditional { when_true, .. } = &mut machine.blocks[1].terminator else {
        panic!("guard");
    };
    when_true.target = id(5, BlockId::new);
    when_true.arguments.clear();
    when_true.structural_arguments.clear();
    let edge = |raw, target| SuccessorEdge {
        edge: id(raw, EdgeId::new),
        target: id(target, BlockId::new),
        arguments: vec![id(2, ValueId::new)],
        structural_arguments: vec![argument(2)],
        trivial_affine_discards: Vec::new(),
    };
    machine.blocks.push(Block {
        id: id(5, BlockId::new),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        operations: Vec::new(),
        terminator: if join {
            Terminator::Conditional {
                condition: id(40, ValueId::new),
                when_true: edge(50, 6),
                when_false: edge(51, 7),
            }
        } else {
            let edge = edge(50, 6);
            Terminator::Jump {
                edge: edge.edge,
                target: edge.target,
                arguments: edge.arguments,
                structural_arguments: edge.structural_arguments,
                trivial_affine_discards: Vec::new(),
                residual_affine_discards: Vec::new(),
            }
        },
    });
    for (block, place, scalar, edge) in
        [(6, 4, 41, 52), (7, 5, 42, 53)]
            .into_iter()
            .take(if join { 2 } else { 1 })
    {
        let mut parameter = machine.blocks[2].structural_parameters[0].clone();
        parameter.place = id(place, PlaceId::new);
        machine.structural_places.push(StructuralPlaceDeclaration {
            id: parameter.place,
            kind: StructuralPlaceKind::BlockParameter {
                block: id(block, BlockId::new),
                position: 0,
            },
        });
        machine.blocks.push(Block {
            id: id(block, BlockId::new),
            parameters: vec![ValueDeclaration {
                qualifications: Default::default(),
                id: id(scalar, ValueId::new),
                scalar_type: ScalarType::Integer(integer()),
            }],
            structural_parameters: vec![parameter],
            operations: Vec::new(),
            terminator: Terminator::Jump {
                edge: id(edge, EdgeId::new),
                target: id(3, BlockId::new),
                arguments: vec![id(scalar, ValueId::new)],
                structural_arguments: vec![argument(place)],
                trivial_affine_discards: Vec::new(),
                residual_affine_discards: Vec::new(),
            },
        });
    }
    module
}

fn has_forwarded_extent(module: &TerminalModule) -> bool {
    reconstruct_interpretable_operation_obligations(
        validate_module_for_interpretation(module).expect("valid descriptor transfers"),
    )
    .unwrap()
    .iter()
    .find(|obligation| obligation.obligation.id == id(1, ObligationId::new))
    .unwrap()
    .semantic_axioms
    .contains(&Proposition::Equal(value(12), value(10)))
}

#[test]
fn forwarded_extent_crosses_staging_and_same_origin_arrivals() {
    for join in [false, true] {
        let module = forwarded(join);
        assert!(has_forwarded_extent(&module));
        let bundle = proof(&module);
        verify_module_for_interpretation(&module, &bundle, &AdmissionProfile::default()).unwrap();
        let mut reordered = module.clone();
        reordered.machines[0].blocks.reverse();
        assert!(has_forwarded_extent(&reordered));
    }
}

#[test]
fn forwarded_extent_selects_the_nearest_latest_observation_independent_of_roster() {
    let mut module = forwarded(false);
    module.machines[0].blocks[1]
        .operations
        .insert(1, length_operation(16, 16, 2));
    module.machines[0].blocks[5]
        .operations
        .extend([length_operation(17, 17, 4), length_operation(18, 18, 4)]);
    for reverse in [false, true] {
        if reverse {
            module.machines[0].blocks.reverse();
        }
        let obligations = reconstruct_interpretable_operation_obligations(
            validate_module_for_interpretation(&module).unwrap(),
        )
        .unwrap();
        let bounds = obligations
            .iter()
            .find(|obligation| obligation.obligation.id == id(1, ObligationId::new))
            .unwrap();
        assert!(
            bounds
                .semantic_axioms
                .contains(&Proposition::Equal(value(12), value(18)))
        );
        assert!(
            !bounds
                .semantic_axioms
                .contains(&Proposition::Equal(value(12), value(10)))
        );
        assert!(
            !bounds
                .semantic_axioms
                .contains(&Proposition::Equal(value(12), value(17)))
        );
    }
}

#[test]
fn forwarded_extent_rejects_a_different_origin_on_one_arrival() {
    let mut module = forwarded(true);
    let machine = &mut module.machines[0];
    machine.blocks[2].terminator = Terminator::ReturnUnit {
        edge: id(60, EdgeId::new),
        trivial_affine_discards: Vec::new(),
    };
    let mut second = machine.structural_parameters[0].clone();
    second.place = id(9, PlaceId::new);
    second.position = 1;
    machine.structural_parameters.push(second.clone());
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: second.place,
        kind: StructuralPlaceKind::Parameter {
            position: 1,
            is_self: false,
        },
    });
    let Terminator::Conditional { when_false, .. } = &mut machine.blocks[4].terminator else {
        panic!("join selection");
    };
    when_false.structural_arguments[0] = argument(9);
    assert!(!has_forwarded_extent(&module));
}

#[test]
fn forwarded_extent_rejects_mutation_through_an_intermediate_binding() {
    let mut module = forwarded(false);
    // Keep this fixture acyclic: general mutable-view helper calls are outside
    // the separate unranked operation fence, not outside extent invalidation.
    module.machines[0].blocks[2].terminator = Terminator::ReturnUnit {
        edge: id(60, EdgeId::new),
        trivial_affine_discards: Vec::new(),
    };
    let mut helper = module.machines[0].clone();
    helper.id = id(2, MachineId::new);
    helper.parameters.clear();
    helper.contract.id = id(2, ContractId::new);
    helper.structural_parameters.truncate(1);
    helper.structural_parameters[0].place = id(20, PlaceId::new);
    helper.structural_places = vec![StructuralPlaceDeclaration {
        id: id(20, PlaceId::new),
        kind: StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        },
    }];
    helper.entry = id(20, BlockId::new);
    helper.blocks = vec![Block {
        id: helper.entry,
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        operations: Vec::new(),
        terminator: Terminator::ReturnUnit {
            edge: id(70, EdgeId::new),
            trivial_affine_discards: Vec::new(),
        },
    }];
    module.machines.push(helper);
    module.machines[0].blocks[5].operations.push(Operation {
        static_reach_binding: None,
        id: id(60, OperationId::new),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            callee: id(2, MachineId::new),
            arguments: Vec::new(),
            structural_arguments: vec![argument(4)],
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    });
    assert!(!has_forwarded_extent(&module));
}
