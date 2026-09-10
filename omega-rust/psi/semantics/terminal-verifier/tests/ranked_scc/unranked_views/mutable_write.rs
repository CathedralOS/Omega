use super::*;

fn fixture() -> TerminalModule {
    let mut module = view_cycle();
    let machine = &mut module.machines[0];
    let terminator = machine.blocks[3].terminator.clone();
    machine.blocks.truncate(2);
    machine.structural_places.truncate(2);
    machine.structural_parameters[0].access = StructuralAccess::MutableBorrow;
    machine.blocks[1].structural_parameters[0].access = StructuralAccess::MutableBorrow;
    machine.parameters.push(ValueDeclaration {
        qualifications: Default::default(),
        id: id(30, ValueId::new),
        scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap()),
    });
    machine.blocks[0]
        .operations
        .push(length_operation(20, 21, 1));
    let Terminator::Jump {
        structural_arguments,
        ..
    } = &mut machine.blocks[0].terminator
    else {
        unreachable!()
    };
    structural_arguments[0].access = StructuralAccess::MutableBorrow;
    machine.blocks[1].operations.push(Operation {
        id: id(30, OperationId::new),
        result: OperationResult::Unit,
        kind: OperationKind::ByteSequenceWrite {
            destination: id(2, PlaceId::new),
            index: id(1, ValueId::new),
            value: id(30, ValueId::new),
            length: id(10, ValueId::new),
            obligation: id(30, ObligationId::new),
        },
    });
    machine.blocks[1].terminator = terminator;
    module
}

#[test]
fn mutable_write_retains_exact_edge_observation_and_requires_bounds_evidence() {
    let module = fixture();
    validate_module(&module).unwrap();
    let obligations = reconstruct_interpretable_operation_obligations(
        validate_module_for_interpretation(&module).unwrap(),
    )
    .unwrap();
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    assert!(obligations[0].semantic_axioms.contains(&Proposition::Equal(
        ScalarTerm::value(id(10, ValueId::new), scalar_type),
        ScalarTerm::value(id(21, ValueId::new), scalar_type),
    )));
    assert!(
        verify_module_for_interpretation(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default()
        )
        .is_err(),
        "a length equation is not an index bounds proof"
    );
}

#[test]
fn mutable_write_rejects_shared_destination_wrong_witness_and_wrong_byte_type() {
    for mutation in 0..3 {
        let mut module = fixture();
        let machine = &mut module.machines[0];
        match mutation {
            0 => machine.blocks[1].structural_parameters[0].access = StructuralAccess::SharedBorrow,
            1 => {
                let OperationKind::ByteSequenceWrite { length, .. } =
                    &mut machine.blocks[1].operations[1].kind
                else {
                    unreachable!()
                };
                *length = id(21, ValueId::new);
            }
            _ => machine.parameters.last_mut().unwrap().scalar_type = ScalarType::Boolean,
        }
        assert!(validate_module(&module).is_err(), "mutation {mutation}");
    }
}

#[test]
fn mutable_write_retains_equation_for_same_origin_incoming_edges() {
    let mut module = fixture();
    let machine = &mut module.machines[0];
    let Terminator::Jump {
        target,
        structural_arguments,
        ..
    } = &machine.blocks[0].terminator
    else {
        unreachable!()
    };
    let edge = SuccessorEdge {
        edge: id(40, EdgeId::new),
        target: *target,
        arguments: Vec::new(),
        structural_arguments: structural_arguments.clone(),
        trivial_affine_discards: Vec::new(),
    };
    machine.blocks[0].terminator = Terminator::Conditional {
        condition: id(20, ValueId::new),
        when_true: edge.clone(),
        when_false: SuccessorEdge {
            edge: id(41, EdgeId::new),
            ..edge
        },
    };
    validate_module(&module).unwrap();
    let obligations = reconstruct_interpretable_operation_obligations(
        validate_module_for_interpretation(&module).unwrap(),
    )
    .unwrap();
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    assert!(obligations[0].semantic_axioms.contains(&Proposition::Equal(
        ScalarTerm::value(id(10, ValueId::new), scalar_type),
        ScalarTerm::value(id(21, ValueId::new), scalar_type),
    )));
    assert!(
        verify_module_for_interpretation(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default()
        )
        .is_err(),
        "same-origin arrivals establish length equality, not index bounds"
    );
}

fn mutable_argument(place: u64) -> StructuralArgument {
    StructuralArgument {
        access: StructuralAccess::MutableBorrow,
        ..view_argument(place)
    }
}

fn append_target(machine: &mut TerminalMachine, block: u64, place: u64) {
    let mut target = machine.blocks[1].clone();
    target.id = id(block, BlockId::new);
    target.operations.clear();
    target.structural_parameters[0].place = id(place, PlaceId::new);
    target.terminator = Terminator::ReturnUnit {
        edge: id(block + 100, EdgeId::new),
        trivial_affine_discards: Vec::new(),
    };
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: id(place, PlaceId::new),
        kind: StructuralPlaceKind::BlockParameter {
            block: target.id,
            position: 0,
        },
    });
    machine.blocks.push(target);
}

fn jump(edge: u64, target: u64, arguments: Vec<StructuralArgument>) -> Terminator {
    Terminator::Jump {
        edge: id(edge, EdgeId::new),
        target: id(target, BlockId::new),
        arguments: Vec::new(),
        structural_arguments: arguments,
        residual_affine_discards: Vec::new(),
        trivial_affine_discards: Vec::new(),
    }
}

#[test]
fn transferred_machine_view_cannot_be_observed_or_written_again() {
    for write in [false, true] {
        let mut module = fixture();
        validate_module(&module).unwrap();
        let operation = if write {
            Operation {
                id: id(50, OperationId::new),
                result: OperationResult::Unit,
                kind: OperationKind::ByteSequenceWrite {
                    destination: id(1, PlaceId::new),
                    index: id(1, ValueId::new),
                    value: id(30, ValueId::new),
                    length: id(21, ValueId::new),
                    obligation: id(50, ObligationId::new),
                },
            }
        } else {
            length_operation(50, 50, 1)
        };
        module.machines[0].blocks[1].operations.push(operation);
        assert_eq!(
            validate_module(&module).map(|_| ()),
            Err(ModuleError::ByteSequenceViewNotEstablished {
                operation: id(50, OperationId::new),
                place: id(1, PlaceId::new),
            })
        );
    }
}

#[test]
fn transferred_machine_view_cannot_be_forwarded_again() {
    let mut module = fixture();
    let machine = &mut module.machines[0];
    append_target(machine, 3, 3);
    machine.blocks[1].terminator = jump(50, 3, vec![mutable_argument(2)]);
    validate_module(&module).unwrap();
    module.machines[0].blocks[1].terminator = jump(50, 3, vec![mutable_argument(1)]);
    assert_eq!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::InvalidStructuralSuccessorArgument {
            edge: id(50, EdgeId::new),
            place: id(1, PlaceId::new),
        })
    );
}

#[test]
fn duplicate_exclusive_edge_arguments_are_not_two_loans() {
    let mut module = fixture();
    let machine = &mut module.machines[0];
    let mut parameter = machine.blocks[1].structural_parameters[0].clone();
    parameter.place = id(3, PlaceId::new);
    parameter.position = 1;
    machine.blocks[1].structural_parameters.push(parameter);
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: id(3, PlaceId::new),
        kind: StructuralPlaceKind::BlockParameter {
            block: id(2, BlockId::new),
            position: 1,
        },
    });
    machine.blocks[0].terminator = jump(1, 2, vec![mutable_argument(1), mutable_argument(1)]);
    assert_eq!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::InvalidStructuralSuccessorArgument {
            edge: id(1, EdgeId::new),
            place: id(1, PlaceId::new),
        })
    );
}

#[test]
fn reconverging_branch_does_not_resurrect_a_moved_machine_view() {
    let mut module = fixture();
    let machine = &mut module.machines[0];
    let terminator = machine.blocks[1].terminator.clone();
    machine.blocks[1].operations.clear();
    machine.blocks[1].terminator = jump(51, 4, Vec::new());
    machine.blocks.push(Block {
        id: id(3, BlockId::new),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        operations: Vec::new(),
        terminator: jump(52, 4, Vec::new()),
    });
    machine.blocks.push(Block {
        id: id(4, BlockId::new),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        operations: Vec::new(),
        terminator,
    });
    let edge = |identity, target, structural_arguments| SuccessorEdge {
        edge: id(identity, EdgeId::new),
        target: id(target, BlockId::new),
        arguments: Vec::new(),
        structural_arguments,
        trivial_affine_discards: Vec::new(),
    };
    machine.blocks[0].terminator = Terminator::Conditional {
        condition: id(20, ValueId::new),
        when_true: edge(53, 2, vec![mutable_argument(1)]),
        when_false: edge(54, 3, Vec::new()),
    };
    validate_module(&module).unwrap();
    module.machines[0].blocks[3]
        .operations
        .push(length_operation(50, 50, 1));
    assert_eq!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::ByteSequenceViewNotEstablished {
            operation: id(50, OperationId::new),
            place: id(1, PlaceId::new),
        })
    );
}

#[test]
fn mutable_forwarding_preserves_distinct_arguments_in_reversed_target_order() {
    let mut module = fixture();
    let machine = &mut module.machines[0];
    let mut second = machine.structural_parameters[0].clone();
    second.place = id(3, PlaceId::new);
    second.position = 1;
    machine.structural_parameters.push(second.clone());
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: second.place,
        kind: StructuralPlaceKind::Parameter {
            position: 1,
            is_self: false,
        },
    });
    second.place = id(4, PlaceId::new);
    machine.blocks[1].structural_parameters.push(second);
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: id(4, PlaceId::new),
        kind: StructuralPlaceKind::BlockParameter {
            block: id(2, BlockId::new),
            position: 1,
        },
    });
    machine.blocks[0].terminator = jump(1, 2, vec![mutable_argument(3), mutable_argument(1)]);
    machine.blocks[1]
        .operations
        .push(length_operation(50, 50, 4));
    validate_module(&module).unwrap();
    module.machines[0].blocks.reverse();
    validate_module(&module).unwrap();
}

#[test]
fn sequential_reborrows_work_but_transferred_machine_alias_cannot_call() {
    let mut module = fixture();
    let mut helper = module.machines[0].clone();
    helper.id = id(2, MachineId::new);
    helper.entry = id(100, BlockId::new);
    helper.parameters.clear();
    helper.contract = MachineContract {
        id: id(2, ContractId::new),
        requires: Vec::new(),
        ensures: Vec::new(),
        crash_routes: Vec::new(),
        outcome_specific_ensures: Vec::new(),
    };
    helper.structural_parameters[0].place = id(100, PlaceId::new);
    helper.structural_places = vec![StructuralPlaceDeclaration {
        id: id(100, PlaceId::new),
        kind: StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        },
    }];
    helper.blocks = vec![Block {
        id: id(100, BlockId::new),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        operations: Vec::new(),
        terminator: Terminator::ReturnUnit {
            edge: id(100, EdgeId::new),
            trivial_affine_discards: Vec::new(),
        },
    }];
    module.machines.push(helper);
    let call = |identity, place| Operation {
        id: id(identity, OperationId::new),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            callee: id(2, MachineId::new),
            arguments: Vec::new(),
            structural_arguments: vec![mutable_argument(place)],
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    };
    module.machines[0].blocks[0]
        .operations
        .insert(0, call(48, 1));
    module.machines[0].blocks[0]
        .operations
        .insert(1, call(49, 1));
    module.machines[0].blocks[1]
        .operations
        .extend([call(50, 2), call(51, 2)]);
    validate_module(&module).unwrap();
    module.machines[0].blocks[1].operations[3] = call(51, 1);
    assert_eq!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::ByteSequenceViewNotEstablished {
            operation: id(51, OperationId::new),
            place: id(1, PlaceId::new),
        })
    );
}
