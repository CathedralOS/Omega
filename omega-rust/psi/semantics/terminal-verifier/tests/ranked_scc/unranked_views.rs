use super::*;
use semantic_vocabulary::BoundaryMachineId;
use terminal_psi::{
    BoundaryMachineDeclaration, BoundaryMachineResult, ByteSequenceCarrier, StructuralArgument,
    StructuralOperationResult,
};

fn view_argument(place: u64) -> StructuralArgument {
    StructuralArgument {
        place: id(place, PlaceId::new),
        path: Vec::new(),
        access: StructuralAccess::SharedBorrow,
    }
}

fn length_operation(operation: u64, value: u64, source: u64) -> Operation {
    Operation {
        id: id(operation, OperationId::new),
        result: OperationResult::Scalar(ValueDeclaration {
            id: id(value, ValueId::new),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
        }),
        kind: OperationKind::ByteSequenceLength {
            source: id(source, PlaceId::new),
        },
    }
}

fn view_cycle() -> TerminalModule {
    let mut module = ranked_countdown_with_width(64);
    let structural_type = id(1, StructuralTypeId::new);
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type,
        identity: "test::CyclicBytes".into(),
        shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView),
    });
    let machine = &mut module.machines[0];
    machine.ranked_scc = None;
    machine.parameters.push(ValueDeclaration {
        id: id(20, ValueId::new),
        scalar_type: ScalarType::Boolean,
    });
    let parameter = StructuralParameterDeclaration {
        place: id(1, PlaceId::new),
        position: 0,
        is_self: false,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::SharedBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    machine.structural_parameters.push(parameter.clone());
    machine.blocks[1]
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: id(2, PlaceId::new),
            ..parameter
        });
    machine.structural_places = vec![
        StructuralPlaceDeclaration {
            id: id(1, PlaceId::new),
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        },
        StructuralPlaceDeclaration {
            id: id(2, PlaceId::new),
            kind: StructuralPlaceKind::BlockParameter {
                block: id(2, BlockId::new),
                position: 0,
            },
        },
        StructuralPlaceDeclaration {
            id: id(3, PlaceId::new),
            kind: StructuralPlaceKind::OperationResult {
                producer: id(12, OperationId::new),
                structural_type,
            },
        },
    ];
    for block in &mut machine.blocks {
        block.parameters.clear();
        block.operations.clear();
        if let Terminator::Jump {
            arguments,
            structural_arguments,
            ..
        } = &mut block.terminator
        {
            arguments.clear();
            structural_arguments.push(view_argument(if block.id == machine.entry { 1 } else { 3 }));
        }
    }
    machine.blocks[1]
        .operations
        .push(length_operation(10, 10, 2));
    let Terminator::Conditional { condition, .. } = &mut machine.blocks[1].terminator else {
        panic!("header guard")
    };
    *condition = id(20, ValueId::new);
    machine.blocks[2].operations = vec![
        Operation {
            id: id(11, OperationId::new),
            result: OperationResult::Scalar(ValueDeclaration {
                id: id(11, ValueId::new),
                scalar_type: ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
                ),
            }),
            kind: OperationKind::ByteSequenceRead {
                source: id(2, PlaceId::new),
                index: id(1, ValueId::new),
                length: id(10, ValueId::new),
                obligation: id(10, ObligationId::new),
            },
        },
        Operation {
            id: id(12, OperationId::new),
            result: OperationResult::Structural(StructuralOperationResult {
                place: id(3, PlaceId::new),
                structural_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            }),
            kind: OperationKind::ByteSequenceSubslice {
                source: id(2, PlaceId::new),
                start: id(1, ValueId::new),
                end: id(10, ValueId::new),
                length: id(10, ValueId::new),
                obligation: id(11, ObligationId::new),
            },
        },
        length_operation(13, 13, 3),
    ];
    module
}

#[test]
fn cyclic_views_allow_dominating_observations_and_exact_tail_transfer() {
    let mut module = view_cycle();
    validate_module(&module)
        .map(|_| ())
        .expect("immutable views and scalar observations have exact CFG scope");
    module.machines[0].blocks.reverse();
    validate_module(&module)
        .map(|_| ())
        .expect("view scope does not depend on roster order");
    assert!(matches!(
        verify_module_for_interpretation(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        ),
        Err(VerificationError::MissingEvidence(obligation)) if obligation == id(10, ObligationId::new)
    ));
}

#[test]
fn cyclic_view_first_arrival_cannot_observe_latch_result() {
    let mut module = view_cycle();
    module.machines[0].blocks[1]
        .operations
        .push(length_operation(30, 30, 3));
    assert_eq!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::ByteSequenceViewNotEstablished {
            operation: id(30, OperationId::new),
            place: id(3, PlaceId::new),
        })
    );
}

#[test]
fn cyclic_view_sibling_path_cannot_observe_latch_result() {
    let mut module = view_cycle();
    module.machines[0].blocks[3]
        .operations
        .push(length_operation(30, 30, 3));
    assert_eq!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::ByteSequenceViewNotEstablished {
            operation: id(30, OperationId::new),
            place: id(3, PlaceId::new),
        })
    );
}

#[test]
fn cyclic_view_definition_must_precede_same_block_observation() {
    let mut module = view_cycle();
    module.machines[0].blocks[2].operations.swap(1, 2);
    assert_eq!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::ByteSequenceViewNotEstablished {
            operation: id(13, OperationId::new),
            place: id(3, PlaceId::new),
        })
    );
}

#[test]
fn cyclic_view_parameter_is_unavailable_before_first_binding() {
    let mut module = view_cycle();
    module.machines[0].blocks[0]
        .operations
        .push(length_operation(30, 30, 2));
    assert_eq!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::ByteSequenceViewNotEstablished {
            operation: id(30, OperationId::new),
            place: id(2, PlaceId::new),
        })
    );
}

#[test]
fn multiple_entry_cycle_cannot_bypass_view_parameter_binding() {
    let mut module = view_cycle();
    module.machines[0].blocks[0].terminator = Terminator::Conditional {
        condition: id(20, ValueId::new),
        when_true: SuccessorEdge {
            edge: id(1, EdgeId::new),
            target: id(2, BlockId::new),
            arguments: Vec::new(),
            structural_arguments: vec![view_argument(1)],
            trivial_affine_discards: Vec::new(),
        },
        when_false: SuccessorEdge {
            edge: id(6, EdgeId::new),
            target: id(3, BlockId::new),
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
    };
    assert_eq!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::ByteSequenceViewNotEstablished {
            operation: id(11, OperationId::new),
            place: id(2, PlaceId::new),
        })
    );
}

#[test]
fn every_cyclic_view_jump_checks_exact_arity() {
    for block_position in [0, 2] {
        let mut module = view_cycle();
        let Terminator::Jump {
            structural_arguments,
            edge,
            ..
        } = &mut module.machines[0].blocks[block_position].terminator
        else {
            panic!("preheader or backedge")
        };
        structural_arguments.clear();
        let edge = *edge;
        assert_eq!(
            validate_module(&module).map(|_| ()),
            Err(ModuleError::StructuralJumpArityMismatch {
                edge,
                expected: 1,
                actual: 0,
            })
        );
    }
}

#[test]
fn cyclic_view_jump_cannot_use_a_future_descriptor() {
    let mut module = view_cycle();
    let Terminator::Jump {
        structural_arguments,
        ..
    } = &mut module.machines[0].blocks[0].terminator
    else {
        panic!("preheader")
    };
    structural_arguments[0] = view_argument(3);
    assert_eq!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::InvalidStructuralSuccessorArgument {
            edge: id(1, EdgeId::new),
            place: id(3, PlaceId::new),
        })
    );
}

#[test]
fn cyclic_view_jump_rejects_authority_widening() {
    let mut module = view_cycle();
    let Terminator::Jump {
        structural_arguments,
        ..
    } = &mut module.machines[0].blocks[2].terminator
    else {
        panic!("backedge")
    };
    structural_arguments[0].access = StructuralAccess::MutableBorrow;
    assert_eq!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::InvalidStructuralSuccessorArgument {
            edge: id(4, EdgeId::new),
            place: id(3, PlaceId::new),
        })
    );
}

#[test]
fn cyclic_view_jump_rejects_same_shape_different_type() {
    let mut module = view_cycle();
    let mut second_type = module.structural_types[0].clone();
    second_type.id = id(2, StructuralTypeId::new);
    second_type.identity = "test::OtherCyclicBytes".into();
    module.structural_types.push(second_type);
    let machine = &mut module.machines[0];
    let mut parameter = machine.structural_parameters[0].clone();
    parameter.place = id(4, PlaceId::new);
    parameter.position = 1;
    parameter.structural_type = id(2, StructuralTypeId::new);
    machine.structural_parameters.push(parameter);
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: id(4, PlaceId::new),
        kind: StructuralPlaceKind::Parameter {
            position: 1,
            is_self: false,
        },
    });
    let Terminator::Jump {
        structural_arguments,
        ..
    } = &mut machine.blocks[0].terminator
    else {
        panic!("preheader")
    };
    structural_arguments[0] = view_argument(4);
    assert_eq!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::InvalidStructuralSuccessorArgument {
            edge: id(1, EdgeId::new),
            place: id(4, PlaceId::new),
        })
    );
}

fn boundary_view_call(module: &mut TerminalModule, source: u64) -> Operation {
    let mut parameter = module.machines[0].structural_parameters[0].clone();
    parameter.place = id(100, PlaceId::new);
    let boundary = id(1, BoundaryMachineId::new);
    module.boundary_machines.push(BoundaryMachineDeclaration {
        id: boundary,
        identity: "test::Output::write".into(),
        attachment: None,
        scalar_parameters: vec![ScalarType::Boolean],
        structural_parameters: vec![parameter],
        result: BoundaryMachineResult::Unit,
        requires: Vec::new(),
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        published_service_ceiling: Vec::new(),
    });
    Operation {
        id: id(40, OperationId::new),
        result: OperationResult::Unit,
        kind: OperationKind::BoundaryCall {
            boundary,
            arguments: vec![id(20, ValueId::new)],
            structural_arguments: vec![view_argument(source)],
            completion_receipts: Vec::new(),
        },
    }
}

#[test]
fn cyclic_boundary_call_accepts_each_established_immutable_view_source() {
    for source in [1, 2, 3] {
        let mut module = view_cycle();
        let call = boundary_view_call(&mut module, source);
        module.machines[0].blocks[2].operations.push(call);
        validate_module(&module).expect("whole immutable byte view and marker reach boundary");
    }
}

#[test]
fn cyclic_boundary_view_arguments_retain_first_arrival_sibling_and_local_order_checks() {
    for (block_position, source) in [(0, 2), (1, 3), (2, 3), (3, 3)] {
        let mut module = view_cycle();
        let call = boundary_view_call(&mut module, source);
        module.machines[0].blocks[block_position]
            .operations
            .insert(0, call);
        assert_eq!(
            validate_module(&module).map(|_| ()),
            Err(ModuleError::ByteSequenceViewNotEstablished {
                operation: id(40, OperationId::new),
                place: id(source, PlaceId::new),
            })
        );
    }
}

#[test]
fn cyclic_boundary_view_argument_keeps_exact_type_check() {
    let mut module = view_cycle();
    let call = boundary_view_call(&mut module, 3);
    module.machines[0].blocks[2].operations.push(call);
    let mut second_type = module.structural_types[0].clone();
    second_type.id = id(2, StructuralTypeId::new);
    second_type.identity = "test::OtherBoundaryBytes".into();
    module.structural_types.push(second_type);
    module.boundary_machines[0].structural_parameters[0].structural_type =
        id(2, StructuralTypeId::new);
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::StructuralArgumentTypeMismatch { .. })
    ));
}

#[test]
fn cyclic_boundary_view_argument_cannot_widen_borrow_access() {
    for access in [
        StructuralAccess::Owned,
        StructuralAccess::MutableBorrow,
        StructuralAccess::WriteOnlyBorrow,
    ] {
        let mut module = view_cycle();
        let mut call = boundary_view_call(&mut module, 1);
        let OperationKind::BoundaryCall {
            structural_arguments,
            ..
        } = &mut call.kind
        else {
            panic!("boundary call")
        };
        structural_arguments[0].access = access;
        module.machines[0].blocks[2].operations.push(call);
        assert!(matches!(
            validate_module(&module),
            Err(ModuleError::StructuralArgumentAccessMismatch { .. })
        ));
    }
}
