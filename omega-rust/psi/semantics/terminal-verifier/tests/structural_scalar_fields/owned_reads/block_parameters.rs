use super::*;

fn block_reader(scalar_type: ScalarType, multiplicity: StructuralMultiplicity) -> TerminalModule {
    let mut module = owned_reader(multiplicity, scalar_type);
    let machine = &mut module.machines[0];
    let mut body = machine.blocks[0].clone();
    body.id = id::<BlockId>(3);
    let mut parameter = machine.structural_parameters[0].clone();
    parameter.place = id::<PlaceId>(3);
    body.structural_parameters.push(parameter.clone());
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: parameter.place,
        kind: StructuralPlaceKind::BlockParameter {
            block: body.id,
            position: 0,
        },
    });
    match &mut body.operations[0].kind {
        OperationKind::IntegerStructuralField { source, .. }
        | OperationKind::BooleanStructuralField { source, .. } => *source = parameter.place,
        _ => unreachable!(),
    }
    if multiplicity == StructuralMultiplicity::Affine {
        let Terminator::Return {
            cleanup_actions, ..
        } = &mut body.terminator
        else {
            unreachable!();
        };
        *cleanup_actions = vec![TerminalAffineCleanupAction::DiscardRoot(parameter.place)];
    }
    machine.blocks[0].operations.clear();
    machine.blocks[0].terminator = Terminator::Jump {
        edge: id::<EdgeId>(3),
        target: body.id,
        arguments: Vec::new(),
        structural_arguments: vec![StructuralArgument {
            place: machine.structural_parameters[0].place,
            path: Vec::new(),
            access: StructuralAccess::Owned,
        }],
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    };
    machine.blocks.push(body);
    module
}

#[test]
fn integer_and_boolean_reads_accept_exact_owned_block_parameters() {
    for scalar_type in [integer_type(), ScalarType::Boolean] {
        for multiplicity in [
            StructuralMultiplicity::Affine,
            StructuralMultiplicity::Unrestricted,
        ] {
            validate_module(&block_reader(scalar_type, multiplicity))
                .expect("read from the exact transferred block descriptor");
        }
    }
}

#[test]
fn owned_successors_reject_same_arity_aliases_and_transfer_after_disposal() {
    let mut original = block_reader(integer_type(), StructuralMultiplicity::Affine);
    let machine = &mut original.machines[0];
    let mut input = machine.structural_parameters[0].clone();
    input.position = 1;
    input.place = id::<PlaceId>(4);
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: input.place,
        kind: StructuralPlaceKind::Parameter {
            position: 1,
            is_self: false,
        },
    });
    machine.structural_parameters.push(input.clone());
    let mut target = machine.blocks[1].structural_parameters[0].clone();
    target.position = 1;
    target.place = id::<PlaceId>(5);
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: target.place,
        kind: StructuralPlaceKind::BlockParameter {
            block: machine.blocks[1].id,
            position: 1,
        },
    });
    machine.blocks[1].structural_parameters.push(target.clone());
    let Terminator::Jump {
        structural_arguments,
        ..
    } = &mut machine.blocks[0].terminator
    else {
        unreachable!()
    };
    structural_arguments.push(StructuralArgument {
        place: input.place,
        path: Vec::new(),
        access: StructuralAccess::Owned,
    });
    let Terminator::Return {
        cleanup_actions, ..
    } = &mut machine.blocks[1].terminator
    else {
        unreachable!()
    };
    cleanup_actions.insert(0, TerminalAffineCleanupAction::DiscardRoot(target.place));
    validate_module(&original).expect("two independently owned descriptors transfer once each");
    for duplicate in [false, true] {
        let mut module = original.clone();
        let Terminator::Jump {
            edge,
            structural_arguments,
            trivial_affine_discards,
            ..
        } = &mut module.machines[0].blocks[0].terminator
        else {
            unreachable!()
        };
        let source = structural_arguments[0].place;
        if duplicate {
            structural_arguments[1].place = source;
        } else {
            trivial_affine_discards.push(source);
        }
        let expected = ModuleError::InvalidStructuralSuccessorArgument {
            edge: *edge,
            place: source,
        };
        assert_eq!(validate_module(&module).map(|_| ()), Err(expected));
    }
}

#[test]
fn owned_backedge_rejoins_the_rebound_frontier_and_cannot_reuse_invocation_custody() {
    let mut module = block_reader(integer_type(), StructuralMultiplicity::Affine);
    let machine = &mut module.machines[0];
    let header = machine.blocks[1].id;
    let owned = machine.blocks[1].structural_parameters[0].place;
    let exit = Block {
        id: id::<BlockId>(4),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        operations: Vec::new(),
        terminator: machine.blocks[1].terminator.clone(),
    };
    machine.blocks[1].operations.push(Operation {
        id: id::<OperationId>(5),
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: id::<ValueId>(5),
            scalar_type: ScalarType::Boolean,
        }),
        kind: OperationKind::BooleanConstant { value: false },
    });
    machine.blocks[1].terminator = Terminator::Conditional {
        condition: id::<ValueId>(5),
        when_true: terminal_psi::SuccessorEdge {
            edge: id::<EdgeId>(5),
            target: header,
            arguments: Vec::new(),
            structural_arguments: vec![StructuralArgument {
                place: owned,
                path: Vec::new(),
                access: StructuralAccess::Owned,
            }],
            trivial_affine_discards: Vec::new(),
        },
        when_false: terminal_psi::SuccessorEdge {
            edge: id::<EdgeId>(6),
            target: exit.id,
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
    };
    machine.blocks.push(exit);
    validate_module(&module).expect("all arrivals carry the same rebound owned frontier");
    let invocation = module.machines[0].structural_parameters[0].place;
    let Terminator::Conditional { when_true, .. } = &mut module.machines[0].blocks[1].terminator
    else {
        unreachable!()
    };
    when_true.structural_arguments[0].place = invocation;
    assert_eq!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::InvalidStructuralSuccessorArgument {
            edge: id::<EdgeId>(5),
            place: invocation
        })
    );
}

#[test]
fn ordinary_calls_consume_the_block_owner_without_invalidating_prior_scalar_snapshots() {
    let mut module = block_reader(integer_type(), StructuralMultiplicity::Affine);
    let mut consumer = owned_reader(StructuralMultiplicity::Affine, integer_type())
        .machines
        .remove(0);
    consumer.id = id::<MachineId>(3);
    consumer.contract = contract(3);
    consumer.entry = id::<BlockId>(99);
    consumer.result = TerminalMachineResult::Unit;
    consumer.structural_parameters[0].place = id::<PlaceId>(99);
    consumer.structural_places[0].id = id::<PlaceId>(99);
    consumer.blocks[0].id = consumer.entry;
    consumer.blocks[0].operations.clear();
    consumer.blocks[0].terminator = Terminator::ReturnUnit {
        edge: id::<EdgeId>(99),
        trivial_affine_discards: vec![id::<PlaceId>(99)],
    };
    let body = &mut module.machines[0].blocks[1];
    let place = body.structural_parameters[0].place;
    body.operations.push(Operation {
        id: id::<OperationId>(5),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            callee: consumer.id,
            arguments: Vec::new(),
            structural_arguments: vec![StructuralArgument {
                place,
                path: Vec::new(),
                access: StructuralAccess::Owned,
            }],
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    });
    let Terminator::Return {
        cleanup_actions, ..
    } = &mut body.terminator
    else {
        unreachable!()
    };
    cleanup_actions.clear();
    module.machines.push(consumer);
    validate_module(&module).expect("a read snapshot survives the later owned transfer");
    module.machines[0].blocks[1].operations.swap(0, 1);
    assert_eq!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::OwnedStructuralPlaceNotLiveAtOperation {
            operation: id::<OperationId>(4),
            place
        })
    );
}

#[test]
fn block_field_reads_reject_declaration_access_type_and_dominance_drift() {
    for scalar_type in [integer_type(), ScalarType::Boolean] {
        for mutation in [
            "block",
            "position",
            "place",
            "type",
            "access",
            "field",
            "claim",
            "dominance",
        ] {
            let mut module = block_reader(scalar_type, StructuralMultiplicity::Affine);
            validate_module(&module).unwrap();
            let machine = &mut module.machines[0];
            match mutation {
                "block" => {
                    machine.structural_places[1].kind = StructuralPlaceKind::BlockParameter {
                        block: machine.entry,
                        position: 0,
                    }
                }
                "position" => machine.blocks[1].structural_parameters[0].position = 1,
                "place" => machine.blocks[1].structural_parameters[0].place = id::<PlaceId>(4),
                "type" => {
                    machine.blocks[1].structural_parameters[0].structural_type =
                        id::<StructuralTypeId>(1)
                }
                "access" => {
                    machine.blocks[1].structural_parameters[0].access =
                        StructuralAccess::WriteOnlyBorrow
                }
                "claim" => machine.entry_claims.push(terminal_psi::EntryClaim {
                    claim: id::<semantic_vocabulary::ClaimId>(1),
                    input: machine.blocks[1].structural_parameters[0].place,
                    path: Vec::new(),
                }),
                "field" => match &mut machine.blocks[1].operations[0].kind {
                    OperationKind::IntegerStructuralField { field, .. }
                    | OperationKind::BooleanStructuralField { field, .. } => {
                        *field = id::<StructuralFieldId>(99)
                    }
                    _ => unreachable!(),
                },
                "dominance" => {
                    let read = machine.blocks[1].operations.remove(0);
                    machine.blocks[0].operations.push(read);
                }
                _ => unreachable!(),
            }
            assert!(
                validate_module(&module).is_err(),
                "accepted {scalar_type:?} {mutation}"
            );
        }
    }
}

#[test]
fn owned_block_field_reads_reject_after_a_selected_discard() {
    for scalar_type in [integer_type(), ScalarType::Boolean] {
        let mut module = block_reader(scalar_type, StructuralMultiplicity::Affine);
        validate_module(&module).unwrap();
        let machine = &mut module.machines[0];
        let place = machine.blocks[1].structural_parameters[0].place;
        let mut continuation = machine.blocks[1].clone();
        continuation.id = id::<BlockId>(4);
        continuation.structural_parameters.clear();
        let Terminator::Return {
            cleanup_actions, ..
        } = &mut continuation.terminator
        else {
            unreachable!();
        };
        cleanup_actions.clear();
        machine.blocks[1].operations.clear();
        machine.blocks[1].terminator = Terminator::Jump {
            edge: id::<EdgeId>(4),
            target: continuation.id,
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            trivial_affine_discards: vec![place],
            residual_affine_discards: Vec::new(),
        };
        machine.blocks.push(continuation);
        assert_eq!(
            validate_module(&module).map(|_| ()),
            Err(ModuleError::OwnedStructuralPlaceNotLiveAtOperation {
                operation: id::<OperationId>(4),
                place
            })
        );
    }
}

#[test]
fn owned_block_field_reads_reject_the_old_descriptor_after_successor_transfer() {
    for scalar_type in [integer_type(), ScalarType::Boolean] {
        let mut module = block_reader(scalar_type, StructuralMultiplicity::Affine);
        let machine = &mut module.machines[0];
        let previous_place = machine.blocks[1].structural_parameters[0].place;
        let mut continuation = machine.blocks[1].clone();
        continuation.id = id::<BlockId>(4);
        let next_place = id::<PlaceId>(4);
        continuation.structural_parameters[0].place = next_place;
        machine.structural_places.push(StructuralPlaceDeclaration {
            id: next_place,
            kind: StructuralPlaceKind::BlockParameter {
                block: continuation.id,
                position: 0,
            },
        });
        match &mut continuation.operations[0].kind {
            OperationKind::IntegerStructuralField { source, .. }
            | OperationKind::BooleanStructuralField { source, .. } => *source = next_place,
            _ => unreachable!(),
        }
        let Terminator::Return {
            cleanup_actions, ..
        } = &mut continuation.terminator
        else {
            unreachable!();
        };
        *cleanup_actions = vec![TerminalAffineCleanupAction::DiscardRoot(next_place)];
        machine.blocks[1].operations.clear();
        machine.blocks[1].terminator = Terminator::Jump {
            edge: id::<EdgeId>(4),
            target: continuation.id,
            arguments: Vec::new(),
            structural_arguments: vec![StructuralArgument {
                place: previous_place,
                path: Vec::new(),
                access: StructuralAccess::Owned,
            }],
            trivial_affine_discards: Vec::new(),
            residual_affine_discards: Vec::new(),
        };
        machine.blocks.push(continuation);
        validate_module(&module).expect("the new descriptor is readable after exact transfer");
        match &mut module.machines[0].blocks[2].operations[0].kind {
            OperationKind::IntegerStructuralField { source, .. }
            | OperationKind::BooleanStructuralField { source, .. } => *source = previous_place,
            _ => unreachable!(),
        }
        assert_eq!(
            validate_module(&module).map(|_| ()),
            Err(ModuleError::OwnedStructuralPlaceNotLiveAtOperation {
                operation: id::<OperationId>(4),
                place: previous_place
            })
        );
    }
}
