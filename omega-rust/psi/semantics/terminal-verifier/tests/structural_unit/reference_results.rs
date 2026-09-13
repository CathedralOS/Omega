//! Returned permission must preserve its parent through arbitrary ordinary work.

use super::*;
use terminal_psi::{StructuralOperationResult, StructuralReferenceResultSource};

fn argument(place: u64, through_reference: bool) -> StructuralArgument {
    StructuralArgument {
        place: place_id(place),
        path: if through_reference {
            vec![StructuralPathSegment::Referent]
        } else {
            Vec::new()
        },
        access: StructuralAccess::MutableBorrow,
    }
}

fn reference_result(place: u64) -> OperationResult {
    OperationResult::Structural(StructuralOperationResult {
        place: place_id(place),
        structural_type: structural_type_id(2),
        multiplicity: StructuralMultiplicity::Affine,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        claims: Vec::new(),
    })
}

fn reference_place(place: u64, producer: u64) -> StructuralPlaceDeclaration {
    StructuralPlaceDeclaration {
        id: place_id(place),
        kind: StructuralPlaceKind::OperationResult {
            producer: operation_id(producer),
            structural_type: structural_type_id(2),
        },
    }
}

fn release(operation: u64, place: u64) -> Operation {
    Operation {
        static_reach_binding: None,
        id: operation_id(operation),
        result: OperationResult::Unit,
        kind: OperationKind::ReleaseReference {
            source: place_id(place),
        },
    }
}

fn establish(operation: u64, place: u64, source: StructuralArgument) -> Operation {
    Operation {
        static_reach_binding: None,
        id: operation_id(operation),
        result: reference_result(place),
        kind: OperationKind::EstablishReference { source },
    }
}

fn relay_module(mark_before_return: bool) -> TerminalModule {
    let mut module = write_only_primitive_store_module();
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type_id(2),
        identity: "MutableI8Reference".into(),
        shape: StructuralTypeShape::Reference {
            referent: structural_type_id(1),
            access: StructuralAccess::MutableBorrow,
        },
    });
    let mut relay = module.machines[0].clone();
    relay.id = machine_id(2);
    relay.entry = block_id(2);
    relay.parameters.clear();
    relay.contract = empty_contract(contract_id(2));
    relay.structural_parameters[0].place = place_id(10);
    relay.structural_parameters[0].access = StructuralAccess::MutableBorrow;
    relay.structural_places = vec![
        StructuralPlaceDeclaration {
            id: place_id(10),
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        },
        StructuralPlaceDeclaration {
            id: place_id(11),
            kind: StructuralPlaceKind::Result,
        },
        reference_place(12, 20),
    ];
    relay.result = TerminalMachineResult::Structural(StructuralResultDeclaration {
        place: place_id(11),
        structural_type: structural_type_id(2),
        multiplicity: StructuralMultiplicity::Affine,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        reference_sources: vec![StructuralReferenceResultSource {
            path: Vec::new(),
            source: argument(10, false),
        }],
    });
    relay.blocks[0].id = block_id(2);
    relay.blocks[0].operations = vec![establish(20, 12, argument(10, false))];
    if mark_before_return {
        relay.blocks[0].operations.splice(
            0..0,
            [
                Operation {
                    static_reach_binding: None,
                    id: operation_id(18),
                    result: OperationResult::Scalar(ValueDeclaration {
                        id: value_id(10),
                        scalar_type: signed_i8(),
                        qualifications: Default::default(),
                    }),
                    kind: OperationKind::IntegerConstant {
                        value: semantic_vocabulary::IntegerValue::Signed(7),
                    },
                },
                Operation {
                    static_reach_binding: None,
                    id: operation_id(19),
                    result: OperationResult::Unit,
                    kind: OperationKind::WriteOnlyPrimitiveStore {
                        destination: place_id(10),
                        value: value_id(10),
                    },
                },
            ],
        );
    }
    relay.blocks[0].terminator = Terminator::ReturnStructural {
        edge: edge_id(2),
        source: place_id(12),
        returned_claims: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };

    let mut writer = module.machines[0].clone();
    writer.id = machine_id(3);
    writer.entry = block_id(3);
    writer.contract = empty_contract(contract_id(3));
    writer.parameters[0].id = value_id(30);
    writer.structural_parameters[0].place = place_id(30);
    writer.structural_parameters[0].access = StructuralAccess::MutableBorrow;
    writer.structural_places[0].id = place_id(30);
    writer.blocks[0].id = block_id(3);
    writer.blocks[0].operations = vec![Operation {
        static_reach_binding: None,
        id: operation_id(30),
        result: OperationResult::Unit,
        kind: OperationKind::WriteOnlyPrimitiveStore {
            destination: place_id(30),
            value: value_id(30),
        },
    }];
    writer.blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(3),
        trivial_affine_discards: Vec::new(),
    };

    let caller = &mut module.machines[0];
    caller.structural_parameters[0].access = StructuralAccess::MutableBorrow;
    caller.structural_places.push(reference_place(2, 1));
    caller.result = TerminalMachineResult::Scalar(ValueDeclaration {
        id: value_id(99),
        scalar_type: signed_i8(),
        qualifications: Default::default(),
    });
    caller.blocks[0].operations = vec![
        Operation {
            static_reach_binding: None,
            id: operation_id(1),
            result: reference_result(2),
            kind: OperationKind::CallStructural {
                callee: machine_id(2),
                structural_arguments: vec![argument(1, false)],
                claim_transfers: Vec::new(),
                returned_claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
                selected_evidence: Vec::new(),
            },
        },
        Operation {
            static_reach_binding: None,
            id: operation_id(2),
            result: OperationResult::Unit,
            kind: OperationKind::CallUnit {
                callee: machine_id(3),
                arguments: vec![value_id(1)],
                structural_arguments: vec![argument(2, true)],
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        },
        release(3, 2),
        Operation {
            static_reach_binding: None,
            id: operation_id(4),
            result: OperationResult::Scalar(ValueDeclaration {
                id: value_id(2),
                scalar_type: signed_i8(),
                qualifications: Default::default(),
            }),
            kind: OperationKind::PrimitiveScalarRead {
                source: place_id(1),
            },
        },
    ];
    caller.blocks[0].terminator = Terminator::Return {
        edge: edge_id(1),
        value: value_id(2),
        cleanup_actions: Vec::new(),
    };
    module.machines.extend([relay, writer]);
    module
}

fn rejects_custody(module: &TerminalModule) {
    assert!(
        matches!(
            validate_module(module),
            Err(ModuleError::InvalidReferenceCustody { .. })
        ),
        "{:?}",
        validate_module(module)
    );
}

#[test]
fn reference_result_replays_ordinary_relay_and_mark_before_return() {
    for mark in [false, true] {
        let module = relay_module(mark);
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        )
        .expect("reference custody is reconstructed without a supplied loan manifest");
    }
}

#[test]
fn reference_result_rejects_parent_read_write_and_call_before_release() {
    let module = relay_module(false);
    let mut read = module.clone();
    read.machines[0].blocks[0].operations.swap(2, 3);
    rejects_custody(&read);
    let mut write = module.clone();
    write.machines[0].blocks[0].operations.insert(
        2,
        Operation {
            static_reach_binding: None,
            id: operation_id(5),
            result: OperationResult::Unit,
            kind: OperationKind::WriteOnlyPrimitiveStore {
                destination: place_id(1),
                value: value_id(1),
            },
        },
    );
    rejects_custody(&write);
    let mut call = module;
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut call.machines[0].blocks[0].operations[1].kind
    else {
        panic!("writer");
    };
    structural_arguments[0] = argument(1, false);
    rejects_custody(&call);
}

#[test]
fn reference_result_rejects_duplicate_release_and_stale_carrier_use() {
    let mut duplicate = relay_module(false);
    duplicate.machines[0].blocks[0]
        .operations
        .insert(3, release(5, 2));
    rejects_custody(&duplicate);
    let mut stale = relay_module(false);
    stale.machines[0].blocks[0].operations.swap(1, 2);
    rejects_custody(&stale);
}

#[test]
fn reference_result_rejects_wrong_formal_and_local_escape() {
    let mut wrong = relay_module(false);
    let relay = &mut wrong.machines[1];
    let mut parameter = relay.structural_parameters[0].clone();
    parameter.place = place_id(13);
    parameter.position = 1;
    relay.structural_parameters.push(parameter);
    relay.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(13),
        kind: StructuralPlaceKind::Parameter {
            position: 1,
            is_self: false,
        },
    });
    let TerminalMachineResult::Structural(result) = &mut relay.result else {
        panic!("reference result");
    };
    result.reference_sources[0].source = argument(13, false);
    // Validate the relay directly so unrelated caller arity is not the control.
    wrong.entry = machine_id(2);
    wrong.machines.remove(0);
    rejects_custody(&wrong);

    let mut escaped = relay_module(true);
    let relay = &mut escaped.machines[1];
    relay.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(13),
        kind: StructuralPlaceKind::OperationResult {
            producer: operation_id(19),
            structural_type: structural_type_id(1),
        },
    });
    relay.blocks[0].operations[1] = Operation {
        static_reach_binding: None,
        id: operation_id(19),
        result: OperationResult::Structural(StructuralOperationResult {
            place: place_id(13),
            structural_type: structural_type_id(1),
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        }),
        kind: OperationKind::EstablishPrimitiveLocal {
            value: value_id(10),
        },
    };
    relay.blocks[0].operations[2].kind = OperationKind::EstablishReference {
        source: argument(13, false),
    };
    rejects_custody(&escaped);
}

#[test]
fn reference_result_reborrow_suspends_immediate_parent_and_restores_deepest_first() {
    let mut module = relay_module(false);
    let caller = &mut module.machines[0];
    caller.structural_places.push(reference_place(3, 5));
    caller.blocks[0]
        .operations
        .splice(1..1, [establish(5, 3, argument(2, true)), release(6, 3)]);
    validate_module(&module).expect("ordinary nested reborrow can end before its parent is reused");
    let mut early_parent = module.clone();
    early_parent.machines[0].blocks[0].operations[2] = release(6, 2);
    rejects_custody(&early_parent);
    let mut parent_use = module;
    parent_use.machines[0].blocks[0].operations.swap(2, 3);
    rejects_custody(&parent_use);
}

#[test]
fn reference_result_second_carrier_cannot_duplicate_root_authority() {
    let mut module = relay_module(false);
    let caller = &mut module.machines[0];
    caller.structural_places.push(reference_place(3, 5));
    caller.blocks[0]
        .operations
        .insert(1, establish(5, 3, argument(1, false)));
    rejects_custody(&module);
}

#[test]
fn reference_result_interface_and_access_counterfeits_reject() {
    for change in 0..5 {
        let mut module = relay_module(false);
        let TerminalMachineResult::Structural(result) = &mut module.machines[1].result else {
            panic!("reference result");
        };
        match change {
            0 => result.reference_sources.clear(),
            1 => result
                .reference_sources
                .push(result.reference_sources[0].clone()),
            2 => result.reference_sources[0].source.access = StructuralAccess::Owned,
            3 => result.reference_sources[0]
                .path
                .push(StructuralPathSegment::Referent),
            4 => result.multiplicity = StructuralMultiplicity::Unrestricted,
            _ => unreachable!(),
        }
        assert!(validate_module(&module).is_err());
    }
}

fn jump(edge: u64, target: u64, discards: Vec<PlaceId>) -> Terminator {
    Terminator::Jump {
        edge: edge_id(edge),
        target: block_id(target),
        arguments: Vec::new(),
        structural_arguments: Vec::new(),
        trivial_affine_discards: discards,
        residual_affine_discards: Vec::new(),
    }
}

#[test]
fn reference_result_edge_discard_releases_permission_before_continuing_read() {
    let mut module = relay_module(false);
    let caller = &mut module.machines[0];
    let read = caller.blocks[0].operations.pop().expect("final read");
    caller.blocks[0].operations.pop().expect("explicit release");
    let completion = caller.blocks[0].terminator.clone();
    caller.blocks[0].terminator = jump(4, 4, vec![place_id(2)]);
    caller.blocks.push(Block {
        id: block_id(4),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        operations: vec![read],
        terminator: completion,
    });
    validate_module(&module).expect("edge no-code disposal releases the loan, never the referent");
    let Terminator::Jump {
        trivial_affine_discards,
        ..
    } = &mut module.machines[0].blocks[0].terminator
    else {
        panic!("jump");
    };
    trivial_affine_discards.clear();
    rejects_custody(&module);
}

#[test]
fn reference_result_join_requires_equal_reference_custody_on_every_arrival() {
    let mut module = relay_module(false);
    let caller = &mut module.machines[0];
    let read = caller.blocks[0].operations.pop().expect("final read");
    let release = caller.blocks[0].operations.pop().expect("release");
    let completion = caller.blocks[0].terminator.clone();
    caller.blocks[0].operations.push(Operation {
        id: operation_id(5),
        static_reach_binding: None,
        result: OperationResult::Scalar(ValueDeclaration {
            id: value_id(5),
            scalar_type: ScalarType::Boolean,
            qualifications: Default::default(),
        }),
        kind: OperationKind::BooleanConstant { value: true },
    });
    let successor = |edge, target| SuccessorEdge {
        edge: edge_id(edge),
        target: block_id(target),
        arguments: Vec::new(),
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    caller.blocks[0].terminator = Terminator::Conditional {
        condition: value_id(5),
        when_true: successor(4, 4),
        when_false: successor(5, 5),
    };
    let mut other_release = release.clone();
    other_release.id = operation_id(6);
    caller.blocks.extend([
        Block {
            id: block_id(4),
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            operations: vec![release],
            terminator: jump(6, 6, Vec::new()),
        },
        Block {
            id: block_id(5),
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            operations: vec![other_release],
            terminator: jump(7, 6, Vec::new()),
        },
        Block {
            id: block_id(6),
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            operations: vec![read],
            terminator: completion,
        },
    ]);
    validate_module(&module).expect("both branch-local releases restore the same original root");
    module.machines[0].blocks[2].operations.clear();
    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::OwnedStructuralFrontierJoinMismatch(block_id(6))
    );
}

#[test]
fn reference_result_alias_writes_invalidate_reaching_store_but_permission_events_do_not() {
    let mut module = relay_module(false);
    let caller = &mut module.machines[0];
    // Isolate the reference lifetime from the otherwise conservative effects
    // of a result-producing call; the later writer still uses the reference.
    caller.blocks[0].operations[0] = establish(1, 2, argument(1, false));
    caller.blocks[0].operations.insert(
        0,
        Operation {
            static_reach_binding: None,
            id: operation_id(7),
            result: OperationResult::Unit,
            kind: OperationKind::WriteOnlyPrimitiveStore {
                destination: place_id(1),
                value: value_id(1),
            },
        },
    );
    let term = |value| ScalarTerm::value(value_id(value), signed_i8());
    caller.contract.ensures.push(ContractClause {
        obligation: obligation_id(1),
        proposition: Proposition::Equal(term(99), term(1)),
    });
    let snapshot = Proposition::Equal(term(2), term(1));
    let has_snapshot = |module: &TerminalModule| {
        terminal_verifier::reconstruct_terminal_obligations(module)
            .expect("valid reference module with a return obligation")
            .obligations()
            .iter()
            .any(|obligation| obligation.semantic_axioms.contains(&snapshot))
    };
    assert!(
        !has_snapshot(&module),
        "a distinct carrier ID must not hide the writer's alias of the stored root"
    );
    module.machines[0].blocks[0].operations.remove(2);
    assert!(
        has_snapshot(&module),
        "forming and releasing permission does not modify the referent"
    );
}

#[test]
fn reference_result_mutable_carrier_lends_only_the_callees_declared_access() {
    for access in [
        StructuralAccess::SharedBorrow,
        StructuralAccess::WriteOnlyBorrow,
    ] {
        let mut module = relay_module(false);
        let OperationKind::CallUnit {
            structural_arguments,
            ..
        } = &mut module.machines[0].blocks[0].operations[1].kind
        else {
            panic!("ordinary primitive call");
        };
        structural_arguments[0].access = access;
        let callee = &mut module.machines[2];
        callee.structural_parameters[0].access = access;
        if access == StructuralAccess::SharedBorrow {
            callee.blocks[0].operations[0] = Operation {
                static_reach_binding: None,
                id: operation_id(30),
                result: OperationResult::Scalar(ValueDeclaration {
                    id: value_id(31),
                    scalar_type: signed_i8(),
                    qualifications: Default::default(),
                }),
                kind: OperationKind::PrimitiveScalarRead {
                    source: place_id(30),
                },
            };
        }
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        )
        .expect("call-local attenuation preserves mutable carrier custody and original backing");
        let OperationKind::CallUnit {
            structural_arguments,
            ..
        } = &mut module.machines[0].blocks[0].operations[1].kind
        else {
            panic!("ordinary primitive call");
        };
        structural_arguments[0].access = StructuralAccess::Owned;
        assert!(
            validate_module(&module).is_err(),
            "dereference cannot confer owned access"
        );
    }
}

#[test]
fn reference_result_host_projection_rejects_without_widening_boundary_support() {
    let mut module = relay_module(false);
    let mut parameter = module.machines[2].structural_parameters[0].clone();
    parameter.place = place_id(40);
    module.boundary_machines.push(BoundaryMachineDeclaration {
        id: boundary_id(1),
        identity: "primitive_boundary".into(),
        attachment: None,
        scalar_parameters: vec![signed_i8()],
        structural_parameters: vec![parameter],
        result: terminal_psi::BoundaryMachineResult::Unit,
        crash_routes: Vec::new(),
        requires: Vec::new(),
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        fixed_service_reach: Vec::new(),
        published_service_ceiling: Vec::new(),
    });
    let operations = &mut module.machines[0].blocks[0].operations;
    operations[1].kind = OperationKind::BoundaryCall {
        boundary: boundary_id(1),
        arguments: vec![value_id(1)],
        structural_arguments: vec![argument(1, false)],
        completion_receipts: Vec::new(),
    };
    operations.swap(1, 2);
    validate_module(&module)
        .expect("ordinary primitive boundary argument after restored use remains supported");
    let operations = &mut module.machines[0].blocks[0].operations;
    let OperationKind::BoundaryCall {
        structural_arguments,
        ..
    } = &mut operations[2].kind
    else {
        panic!("boundary call");
    };
    structural_arguments[0] = argument(2, true);
    operations.swap(1, 2);
    rejects_custody(&module);
}
