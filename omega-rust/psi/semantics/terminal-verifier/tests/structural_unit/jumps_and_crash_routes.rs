use super::{content_predicate, hard_root_module, structural_parameter, unit_call_mut};
use crate::structural_unit::{
    block_id, boundary_id, claim_id, edge_id, machine_id, operation_id, place_id, service_id,
    value_id,
};
use semantic_vocabulary::{Proposition, ScalarTerm, ScalarType, StructuralPlaceKind};
use terminal_psi::{
    Block, ClaimTransfer, CrashCause, CrashPredicateTerm, CrashRouteBucket, CrashRouteGuard,
    EntryClaim, Operation, OperationKind, OperationResult, ServiceDeclaration, StructuralAccess,
    StructuralArgument, StructuralMultiplicity, StructuralPlaceDeclaration, SuccessorEdge,
    TerminalAffineCleanupAction, TerminalMachineResult, Terminator, ValueDeclaration,
};
use terminal_verifier::{ModuleError, validate_module};

#[test]
fn jump_applies_a_canonical_subset_of_affine_discards() {
    let mut module = hard_root_module();
    let mut machine = module.machines.pop().expect("callee machine");
    machine.blocks[0].operations.clear();
    machine.structural_parameters[0].multiplicity = StructuralMultiplicity::Affine;
    machine.entry_claims.clear();
    let mut second_parameter = structural_parameter(place_id(4));
    second_parameter.position = 1;
    second_parameter.multiplicity = StructuralMultiplicity::Affine;
    machine.structural_parameters.push(second_parameter);
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(4),
        kind: StructuralPlaceKind::Parameter {
            position: 1,
            is_self: false,
        },
    });
    machine.parameters = vec![ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(10),
        scalar_type: ScalarType::Boolean,
    }];
    machine.result = TerminalMachineResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(11),
        scalar_type: ScalarType::Boolean,
    });
    machine.blocks = vec![
        Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: block_id(2),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::Jump {
                structural_arguments: Vec::new(),
                edge: edge_id(2),
                target: block_id(3),
                arguments: vec![value_id(10)],
                erased_arguments: Vec::new(),
                residual_affine_discards: Vec::new(),
                trivial_affine_discards: vec![place_id(4)],
            },
        },
        Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: block_id(3),
            parameters: vec![ValueDeclaration {
                qualifications: Default::default(),
                id: value_id(12),
                scalar_type: ScalarType::Boolean,
            }],
            operations: Vec::new(),
            terminator: Terminator::Return {
                edge: edge_id(3),
                value: value_id(12),
                cleanup_actions: vec![TerminalAffineCleanupAction::DiscardRoot(place_id(2))],
            },
        },
    ];
    module.entry = machine.id;
    module.machines = vec![machine];
    module.root_service_reach.concrete.clear();
    validate_module(&module).expect("jump may discard a canonical eligible subset");

    let mut reordered = module.clone();
    let Terminator::Jump {
        trivial_affine_discards,
        ..
    } = &mut reordered.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    *trivial_affine_discards = vec![place_id(2), place_id(4)];
    assert_eq!(
        validate_module(&reordered).unwrap_err(),
        ModuleError::EdgeAffineDiscardsInvalid { edge: edge_id(2) }
    );

    let mut claim_bearing = module;
    claim_bearing.machines[0].entry_claims.push(EntryClaim {
        claim: claim_id(1),
        input: place_id(4),
        path: Vec::new(),
    });
    assert_eq!(
        validate_module(&claim_bearing).unwrap_err(),
        ModuleError::EdgeAffineDiscardsInvalid { edge: edge_id(2) }
    );
}

#[test]
fn conditional_applies_affine_discards_only_to_each_selected_successor() {
    let mut module = hard_root_module();
    let mut machine = module.machines.pop().expect("callee machine");
    machine.structural_parameters[0].multiplicity = StructuralMultiplicity::Affine;
    machine.entry_claims.clear();
    let mut second_parameter = structural_parameter(place_id(4));
    second_parameter.position = 1;
    second_parameter.multiplicity = StructuralMultiplicity::Affine;
    machine.structural_parameters.push(second_parameter);
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(4),
        kind: StructuralPlaceKind::Parameter {
            position: 1,
            is_self: false,
        },
    });
    machine.parameters = vec![ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(10),
        scalar_type: ScalarType::Boolean,
    }];
    machine.result = TerminalMachineResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(11),
        scalar_type: ScalarType::Boolean,
    });
    machine.blocks = vec![
        Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: block_id(2),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::Conditional {
                condition: value_id(10),
                when_true: SuccessorEdge {
                    structural_arguments: Vec::new(),
                    edge: edge_id(2),
                    target: block_id(3),
                    arguments: vec![value_id(10)],
                    erased_arguments: Vec::new(),
                    trivial_affine_discards: vec![place_id(4)],
                },
                when_false: SuccessorEdge {
                    structural_arguments: Vec::new(),
                    edge: edge_id(3),
                    target: block_id(4),
                    arguments: vec![value_id(10)],
                    erased_arguments: Vec::new(),
                    trivial_affine_discards: vec![place_id(2)],
                },
            },
        },
        Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: block_id(3),
            parameters: vec![ValueDeclaration {
                qualifications: Default::default(),
                id: value_id(12),
                scalar_type: ScalarType::Boolean,
            }],
            operations: Vec::new(),
            terminator: Terminator::Return {
                edge: edge_id(4),
                value: value_id(12),
                cleanup_actions: vec![TerminalAffineCleanupAction::DiscardRoot(place_id(2))],
            },
        },
        Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: block_id(4),
            parameters: vec![ValueDeclaration {
                qualifications: Default::default(),
                id: value_id(13),
                scalar_type: ScalarType::Boolean,
            }],
            operations: Vec::new(),
            terminator: Terminator::Return {
                edge: edge_id(5),
                value: value_id(13),
                cleanup_actions: vec![TerminalAffineCleanupAction::DiscardRoot(place_id(4))],
            },
        },
    ];
    module.entry = machine.id;
    module.machines = vec![machine];
    module.root_service_reach.concrete.clear();
    validate_module(&module).expect("each conditional successor owns its cleanup subset");

    let mut reordered = module;
    let Terminator::Conditional { when_true, .. } = &mut reordered.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    when_true.trivial_affine_discards = vec![place_id(2), place_id(4)];
    assert_eq!(
        validate_module(&reordered).unwrap_err(),
        ModuleError::EdgeAffineDiscardsInvalid { edge: edge_id(2) }
    );
}

#[test]
fn affine_structural_arguments_transfer_at_most_once() {
    let mut repeated = hard_root_module();
    for machine in &mut repeated.machines {
        machine.structural_parameters[0].multiplicity = StructuralMultiplicity::Affine;
        machine.entry_claims.clear();
    }
    unit_call_mut(&mut repeated).clear();
    repeated.machines[1].blocks[0].operations.truncate(1);
    let mut second_call = repeated.machines[0].blocks[0].operations[0].clone();
    second_call.id = operation_id(4);
    repeated.machines[0].blocks[0].operations.push(second_call);
    assert_eq!(
        validate_module(&repeated).unwrap_err(),
        ModuleError::OwnedStructuralPlaceNotLiveAtOperation {
            operation: operation_id(4),
            place: place_id(1),
        }
    );

    for machine in &mut repeated.machines {
        machine.structural_parameters[0].multiplicity = StructuralMultiplicity::Unrestricted;
    }
    validate_module(&repeated).expect("unrestricted structural arguments remain reusable");

    let mut repeated_boundary = hard_root_module();
    repeated_boundary.machines[0].structural_parameters[0].multiplicity =
        StructuralMultiplicity::Affine;
    repeated_boundary.machines[0].entry_claims.clear();
    repeated_boundary.boundary_machines[0].structural_parameters[0].multiplicity =
        StructuralMultiplicity::Affine;
    let boundary_call = Operation {
        static_reach_binding: None,
        id: operation_id(1),
        result: OperationResult::Unit,
        kind: OperationKind::BoundaryCall {
            boundary: boundary_id(1),
            arguments: Vec::new(),
            structural_arguments: vec![StructuralArgument {
                place: place_id(1),
                access: StructuralAccess::Owned,
                path: Vec::new(),
            }],
            completion_receipts: Vec::new(),
        },
    };
    let mut second_boundary_call = boundary_call.clone();
    second_boundary_call.id = operation_id(4);
    repeated_boundary.machines[0].blocks[0].operations = vec![boundary_call, second_boundary_call];
    assert_eq!(
        validate_module(&repeated_boundary).unwrap_err(),
        ModuleError::OwnedStructuralPlaceNotLiveAtOperation {
            operation: operation_id(4),
            place: place_id(1),
        }
    );
}

#[test]
fn port_write_requires_a_declared_reachable_service_and_preserves_claims() {
    let mut outside_ceiling = hard_root_module();
    outside_ceiling.services.push(ServiceDeclaration {
        id: service_id(2),
        identity: "DebugIo".into(),
        parents: Vec::new(),
    });
    let OperationKind::PortWrite { service, .. } =
        &mut outside_ceiling.machines[1].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    *service = service_id(2);
    assert_eq!(
        validate_module(&outside_ceiling).unwrap_err(),
        ModuleError::OperationServiceOutsidePublishedCeiling {
            operation: operation_id(2),
            service: service_id(2),
        }
    );
}

#[test]
fn crash_frontier_is_the_exact_live_frontier_after_transfer() {
    let mut module = hard_root_module();
    module.machines[0].contract.crash_routes = vec![CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![CrashRouteGuard::Truth],
    }];
    module.machines[0].blocks[0].terminator = Terminator::Crash {
        edge: edge_id(1),
        cause: CrashCause::Trap,
        site_guard: Vec::new(),
        frontier_lower_bound: Vec::new(),
    };
    validate_module(&module).expect("the transferred claim is absent from the crash frontier");

    let Terminator::Crash {
        frontier_lower_bound,
        ..
    } = &mut module.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    frontier_lower_bound.push(claim_id(1));
    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::CrashFrontierMismatch { block: block_id(1) }
    );
}

#[test]
fn unit_calls_preserve_exact_crash_routes_and_remain_acyclic() {
    let mut crash_erasing = hard_root_module();
    crash_erasing.machines[1].contract.crash_routes = vec![CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![CrashRouteGuard::Truth],
    }];
    assert_eq!(
        validate_module(&crash_erasing).unwrap_err(),
        ModuleError::CallCrashContinuationsMismatch {
            operation: operation_id(1),
            callee: machine_id(2),
        }
    );

    let mut recursive = hard_root_module();
    recursive.machines[1].blocks[0].operations = vec![Operation {
        static_reach_binding: None,
        id: operation_id(2),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            erased_arguments: Vec::new(),
            arguments: Vec::new(),
            callee: machine_id(2),
            structural_arguments: vec![StructuralArgument {
                place: place_id(2),
                access: StructuralAccess::Owned,
                path: Vec::new(),
            }],
            claim_transfers: vec![ClaimTransfer {
                claim: claim_id(1),
                argument_index: 0,
            }],
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    }];
    assert_eq!(
        validate_module(&recursive).unwrap_err(),
        ModuleError::RecursiveCallSliceNotYetSupported(machine_id(2))
    );
}

#[test]
fn unit_call_crash_routes_substitute_structural_parameters() {
    let mut module = hard_root_module();
    let callee_route = CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![CrashRouteGuard::Predicate(CrashPredicateTerm::new(
            content_predicate(place_id(2)),
        ))],
    };
    let caller_route = CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![CrashRouteGuard::Predicate(CrashPredicateTerm::new(
            content_predicate(place_id(1)),
        ))],
    };
    module.machines[0].contract.crash_routes = vec![caller_route.clone()];
    module.machines[1].contract.crash_routes = vec![callee_route.clone()];
    let OperationKind::CallUnit {
        crash_continuations,
        ..
    } = &mut module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    *crash_continuations = vec![caller_route];
    validate_module(&module).expect("callee structural crash places substitute to caller places");

    let OperationKind::CallUnit {
        crash_continuations,
        ..
    } = &mut module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    *crash_continuations = vec![callee_route];
    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::CallCrashContinuationsMismatch {
            operation: operation_id(1),
            callee: machine_id(2),
        }
    );
}

#[test]
fn unit_crash_ceiling_follows_only_unanimous_cfg_formal_copies() {
    let mut module = hard_root_module();
    let declaration = |id| ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(id),
        scalar_type: ScalarType::Boolean,
    };
    let route = |id| {
        let mut terms = [
            ScalarTerm::boolean(true),
            ScalarTerm::value(value_id(id), ScalarType::Boolean),
        ];
        terms.sort();
        CrashRouteBucket {
            cause: CrashCause::Abort,
            alternatives: vec![CrashRouteGuard::Predicate(CrashPredicateTerm::new(
                Proposition::Equal(terms[0].clone(), terms[1].clone()),
            ))],
        }
    };
    module.machines[0].parameters = vec![declaration(10), declaration(11)];
    module.machines[1].parameters = vec![declaration(20)];
    module.machines[0].contract.crash_routes = vec![route(11)];
    module.machines[1].contract.crash_routes = vec![route(20)];
    let mut completion = module.machines[0].blocks[0].clone();
    completion.id = block_id(103);
    completion.parameters = vec![declaration(30)];
    let OperationKind::CallUnit {
        arguments,
        crash_continuations,
        ..
    } = &mut completion.operations[0].kind
    else {
        unreachable!()
    };
    *arguments = vec![value_id(30)];
    *crash_continuations = vec![route(30)];
    let bridge = |block, parameter, edge| Block {
        erased_scalar_formals: Vec::new(),
        structural_parameters: Vec::new(),
        id: block_id(block),
        parameters: vec![declaration(parameter)],
        operations: Vec::new(),
        terminator: Terminator::Jump {
            structural_arguments: Vec::new(),
            edge: edge_id(edge),
            target: completion.id,
            arguments: vec![value_id(parameter)],
            erased_arguments: Vec::new(),
            residual_affine_discards: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
    };
    module.machines[0].blocks = vec![
        Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: block_id(1),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::Conditional {
                condition: value_id(10),
                when_true: SuccessorEdge {
                    structural_arguments: Vec::new(),
                    edge: edge_id(100),
                    target: block_id(101),
                    arguments: vec![value_id(11)],
                    erased_arguments: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                },
                when_false: SuccessorEdge {
                    structural_arguments: Vec::new(),
                    edge: edge_id(101),
                    target: block_id(102),
                    arguments: vec![value_id(11)],
                    erased_arguments: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                },
            },
        },
        bridge(101, 40, 102),
        bridge(102, 50, 103),
        completion,
    ];
    validate_module(&module).expect("both incoming paths retain the same machine formal");

    let mut conflicting = module.clone();
    let Terminator::Conditional { when_false, .. } =
        &mut conflicting.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    when_false.arguments[0] = value_id(10);
    assert_eq!(
        validate_module(&conflicting).unwrap_err(),
        ModuleError::CallCrashContinuationUncovered {
            operation: operation_id(1),
            cause: CrashCause::Abort
        }
    );

    let mut cyclic = module.clone();
    cyclic.machines[0].blocks[2].terminator = Terminator::Conditional {
        condition: value_id(10),
        when_true: SuccessorEdge {
            structural_arguments: Vec::new(),
            edge: edge_id(103),
            target: block_id(102),
            arguments: vec![value_id(50)],
            erased_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
        when_false: SuccessorEdge {
            structural_arguments: Vec::new(),
            edge: edge_id(104),
            target: block_id(103),
            arguments: vec![value_id(50)],
            erased_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
    };
    // Even a backedge carrying the same slot cannot manufacture a known
    // formal when that slot's own origin has not been established.
    assert_eq!(
        validate_module(&cyclic).unwrap_err(),
        ModuleError::CallCrashContinuationUncovered {
            operation: operation_id(1),
            cause: CrashCause::Abort
        }
    );

    let mut foreign = module.clone();
    let OperationKind::CallUnit {
        crash_continuations,
        ..
    } = &mut foreign.machines[0].blocks[3].operations[0].kind
    else {
        unreachable!()
    };
    *crash_continuations = vec![route(20)];
    assert_eq!(
        validate_module(&foreign).unwrap_err(),
        ModuleError::CallCrashContinuationsMismatch {
            operation: operation_id(1),
            callee: machine_id(2)
        }
    );

    let mut computed = module;
    computed.machines[0].blocks[2].operations.push(Operation {
        static_reach_binding: None,
        id: operation_id(100),
        result: OperationResult::Scalar(declaration(60)),
        kind: OperationKind::BooleanConstant { value: true },
    });
    let Terminator::Jump { arguments, .. } = &mut computed.machines[0].blocks[2].terminator else {
        unreachable!()
    };
    arguments[0] = value_id(60);
    assert_eq!(
        validate_module(&computed).unwrap_err(),
        ModuleError::CallCrashContinuationUncovered {
            operation: operation_id(1),
            cause: CrashCause::Abort
        }
    );
}
