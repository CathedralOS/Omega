//! Borrowed arguments do not transfer owned custody, regardless of multiplicity.

use super::*;

#[test]
fn affine_borrows_remain_reusable_across_internal_and_boundary_calls() {
    for access in [
        StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow,
        StructuralAccess::WriteOnlyBorrow,
    ] {
        let mut module = hard_root_module();
        for machine in &mut module.machines {
            machine.entry_claims.clear();
            let parameter = &mut machine.structural_parameters[0];
            parameter.multiplicity = StructuralMultiplicity::Affine;
            parameter.access = access;
            parameter.qualifications.clear();
            for operation in &mut machine.blocks[0].operations {
                match &mut operation.kind {
                    OperationKind::CallUnit {
                        structural_arguments,
                        claim_transfers,
                        ..
                    } => {
                        structural_arguments[0].access = access;
                        claim_transfers.clear();
                    }
                    OperationKind::BoundaryCall {
                        structural_arguments,
                        completion_receipts,
                        ..
                    } => {
                        structural_arguments[0].access = access;
                        completion_receipts.clear();
                    }
                    _ => {}
                }
            }
        }
        let boundary = &mut module.boundary_machines[0];
        boundary.requires.clear();
        boundary.structural_parameters[0].multiplicity = StructuralMultiplicity::Affine;
        boundary.structural_parameters[0].access = access;

        let mut second_call = module.machines[0].blocks[0].operations[0].clone();
        second_call.id = operation_id(4);
        module.machines[0].blocks[0].operations.push(second_call);
        let mut second_boundary = module.machines[1].blocks[0].operations[1].clone();
        second_boundary.id = operation_id(5);
        module.machines[1].blocks[0]
            .operations
            .push(second_boundary);
        validate_module(&module)
            .expect("a loan neither consumes nor demands owned frontier custody");
    }
}

fn shared_result_module(boundary: bool, scalar: bool, duplicate: bool) -> TerminalModule {
    let mut module = hard_root_module();
    let mut owned_parameter = structural_parameter(place_id(9));
    owned_parameter.multiplicity = StructuralMultiplicity::Affine;
    owned_parameter.qualifications.clear();
    let mut borrowed_parameter = owned_parameter.clone();
    borrowed_parameter.access = StructuralAccess::SharedBorrow;
    borrowed_parameter.multiplicity = StructuralMultiplicity::Unrestricted;
    let mut borrowed_parameters = vec![borrowed_parameter.clone()];
    if duplicate {
        borrowed_parameter.place = place_id(10);
        borrowed_parameter.position = 1;
        borrowed_parameters.push(borrowed_parameter);
    }
    let mut producer = module.boundary_machines[0].clone();
    producer.identity = "create_affine".into();
    producer.structural_parameters.clear();
    producer.requires.clear();
    producer.result = terminal_psi::BoundaryMachineResult::Structural(
        terminal_psi::BoundaryStructuralResultDeclaration {
            structural_type: structural_type_id(1),
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
        },
    );
    let mut reader = producer.clone();
    reader.id = boundary_id(2);
    reader.identity = "read_affine".into();
    reader.structural_parameters = borrowed_parameters.clone();
    reader.result = if scalar {
        terminal_psi::BoundaryMachineResult::Scalar(ScalarType::Boolean)
    } else {
        terminal_psi::BoundaryMachineResult::Unit
    };
    let mut consumer = producer.clone();
    consumer.id = boundary_id(3);
    consumer.identity = "consume_affine".into();
    consumer.structural_parameters = vec![owned_parameter];
    consumer.result = terminal_psi::BoundaryMachineResult::Unit;
    module.boundary_machines = vec![producer, reader, consumer];

    let callee = &mut module.machines[1];
    callee.entry_claims.clear();
    callee.published_service_ceiling.clear();
    callee.structural_parameters = borrowed_parameters;
    callee.structural_places = callee
        .structural_parameters
        .iter()
        .map(|parameter| StructuralPlaceDeclaration {
            id: parameter.place,
            kind: StructuralPlaceKind::Parameter {
                position: parameter.position,
                is_self: false,
            },
        })
        .collect();
    callee.blocks[0].operations.clear();
    if scalar {
        callee.result = TerminalMachineResult::Scalar(ValueDeclaration {
            id: ValueId::new(21).unwrap(),
            scalar_type: ScalarType::Boolean,
        });
        callee.blocks[0].operations.push(Operation {
            id: operation_id(10),
            result: OperationResult::Scalar(ValueDeclaration {
                id: ValueId::new(20).unwrap(),
                scalar_type: ScalarType::Boolean,
            }),
            kind: OperationKind::BooleanConstant { value: true },
        });
        callee.blocks[0].terminator = Terminator::Return {
            edge: edge_id(2),
            value: ValueId::new(20).unwrap(),
            cleanup_actions: Vec::new(),
        };
    }
    let caller = &mut module.machines[0];
    caller.structural_parameters.clear();
    caller.entry_claims.clear();
    caller.structural_places = vec![StructuralPlaceDeclaration {
        id: place_id(1),
        kind: StructuralPlaceKind::OperationResult {
            producer: operation_id(1),
            structural_type: structural_type_id(1),
        },
    }];
    caller.blocks[0].operations = vec![Operation {
        id: operation_id(1),
        result: OperationResult::Structural(terminal_psi::StructuralOperationResult {
            place: place_id(1),
            structural_type: structural_type_id(1),
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        }),
        kind: OperationKind::BoundaryCall {
            boundary: boundary_id(1),
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            completion_receipts: Vec::new(),
        },
    }];
    for ordinal in 2..=3 {
        let structural_arguments = vec![
            StructuralArgument {
                place: place_id(1),
                path: Vec::new(),
                access: StructuralAccess::SharedBorrow
            };
            if duplicate { 2 } else { 1 }
        ];
        let kind = if boundary {
            OperationKind::BoundaryCall {
                boundary: boundary_id(2),
                arguments: Vec::new(),
                structural_arguments,
                completion_receipts: Vec::new(),
            }
        } else if scalar {
            OperationKind::CallStructuralScalar {
                callee: machine_id(2),
                arguments: Vec::new(),
                structural_arguments,
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            }
        } else {
            OperationKind::CallUnit {
                callee: machine_id(2),
                arguments: Vec::new(),
                structural_arguments,
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            }
        };
        caller.blocks[0].operations.push(Operation {
            id: operation_id(ordinal),
            result: if scalar {
                OperationResult::Scalar(ValueDeclaration {
                    id: ValueId::new(20 + ordinal).unwrap(),
                    scalar_type: ScalarType::Boolean,
                })
            } else {
                OperationResult::Unit
            },
            kind,
        });
    }
    caller.blocks[0].operations.push(Operation {
        id: operation_id(4),
        result: OperationResult::Unit,
        kind: OperationKind::BoundaryCall {
            boundary: boundary_id(3),
            arguments: Vec::new(),
            structural_arguments: vec![StructuralArgument {
                place: place_id(1),
                path: Vec::new(),
                access: StructuralAccess::Owned,
            }],
            completion_receipts: Vec::new(),
        },
    });
    if boundary {
        module.machines.pop();
    }
    module
}

#[test]
fn shared_result_reads_require_live_custody_before_and_after_every_call_carrier() {
    for boundary in [false, true] {
        for scalar in [false, true] {
            for duplicate in [false, true] {
                let module = shared_result_module(boundary, scalar, duplicate);
                let frontiers =
                    reconstruct_structural_ownership_frontiers(&module).unwrap_or_else(|error| {
                        panic!(
                            "boundary={boundary} scalar={scalar} duplicate={duplicate}: {error:?}"
                        )
                    });
                let caller = frontiers.machine(machine_id(1)).unwrap();
                for operation in [operation_id(2), operation_id(3)] {
                    assert_eq!(
                        caller.operation_entry(operation).unwrap().owned_places(),
                        caller.operation_exit(operation).unwrap().owned_places()
                    );
                    assert_eq!(
                        caller
                            .operation_exit(operation)
                            .unwrap()
                            .owned_places()
                            .len(),
                        1
                    );
                }
                assert!(
                    caller
                        .operation_exit(operation_id(4))
                        .unwrap()
                        .owned_places()
                        .is_empty()
                );
                for before_production in [false, true] {
                    let mut changed = module.clone();
                    let operations = &mut changed.machines[0].blocks[0].operations;
                    let operation = if before_production {
                        operations.swap(0, 1);
                        operation_id(2)
                    } else {
                        operations.swap(2, 3);
                        operation_id(3)
                    };
                    assert_eq!(
                        validate_module(&changed).map(|_| ()),
                        Err(ModuleError::OwnedStructuralPlaceNotLiveAtOperation {
                            operation,
                            place: place_id(1)
                        }),
                        "boundary={boundary} scalar={scalar} duplicate={duplicate} before={before_production}"
                    );
                }
                let mut cleanup = module;
                cleanup.machines[0].blocks[0].operations.pop();
                let Terminator::ReturnUnit {
                    trivial_affine_discards,
                    ..
                } = &mut cleanup.machines[0].blocks[0].terminator
                else {
                    unreachable!()
                };
                trivial_affine_discards.push(place_id(1));
                validate_module(&cleanup)
                    .expect("repeated shared reads leave caller cleanup intact");
            }
        }
    }
}

#[test]
fn a_result_cannot_be_shared_and_moved_into_the_same_call() {
    let original = shared_result_module(true, false, true);
    validate_module(&original).expect("two shared reads are compatible");
    for owned_index in 0..2 {
        let mut changed = original.clone();
        let parameter = &mut changed.boundary_machines[1].structural_parameters[owned_index];
        parameter.access = StructuralAccess::Owned;
        parameter.multiplicity = StructuralMultiplicity::Affine;
        for operation in &mut changed.machines[0].blocks[0].operations[1..3] {
            let OperationKind::BoundaryCall {
                structural_arguments,
                ..
            } = &mut operation.kind
            else {
                unreachable!()
            };
            structural_arguments[owned_index].access = StructuralAccess::Owned;
        }
        assert_eq!(
            validate_module(&changed).map(|_| ()),
            Err(ModuleError::OverlappingExclusiveStructuralArguments {
                operation: operation_id(2),
                first_argument: 0,
                second_argument: 1,
            })
        );
    }
}

#[test]
fn shared_views_of_owned_parameters_require_the_same_live_custody() {
    for boundary in [false, true] {
        for scalar in [false, true] {
            let mut module = shared_result_module(boundary, scalar, false);
            let mut parameter = module.boundary_machines[2].structural_parameters[0].clone();
            parameter.place = place_id(1);
            module.machines[0].structural_parameters = vec![parameter];
            module.machines[0].structural_places[0].kind = StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            };
            module.machines[0].blocks[0].operations.remove(0);
            validate_module(&module).expect("owned parameter survives repeated shared reads");
            module.machines[0].blocks[0].operations.swap(1, 2);
            assert_eq!(
                validate_module(&module).map(|_| ()),
                Err(ModuleError::OwnedStructuralPlaceNotLiveAtOperation {
                    operation: operation_id(3),
                    place: place_id(1),
                })
            );
        }
    }
}
