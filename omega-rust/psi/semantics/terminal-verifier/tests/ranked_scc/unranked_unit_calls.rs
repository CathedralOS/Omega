use super::*;
use terminal_psi::{ByteSequenceCarrier, StructuralArgument};

fn shared_parameter(place: u64) -> StructuralParameterDeclaration {
    StructuralParameterDeclaration {
        place: id(place, PlaceId::new),
        position: 0,
        is_self: false,
        structural_type: id(1, StructuralTypeId::new),
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::SharedBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }
}

fn helper_call(module: &mut TerminalModule, source: u64) -> Operation {
    let mut helper = module.machines[0].clone();
    helper.id = id(2, MachineId::new);
    helper.entry = id(100, BlockId::new);
    helper.parameters.clear();
    helper.ranked_scc = None;
    helper.contract = MachineContract {
        id: id(2, ContractId::new),
        requires: Vec::new(),
        ensures: Vec::new(),
        crash_routes: Vec::new(),
        outcome_specific_ensures: Vec::new(),
    };
    helper.structural_parameters = vec![shared_parameter(100)];
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
        operations: vec![Operation {
            id: id(100, OperationId::new),
            result: OperationResult::Scalar(ValueDeclaration {
                id: id(100, ValueId::new),
                scalar_type: ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                ),
            }),
            kind: OperationKind::ByteSequenceLength {
                source: id(100, PlaceId::new),
            },
        }],
        terminator: Terminator::ReturnUnit {
            edge: id(100, EdgeId::new),
            trivial_affine_discards: Vec::new(),
        },
    }];
    module.machines.push(helper);
    Operation {
        id: id(40, OperationId::new),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            callee: id(2, MachineId::new),
            arguments: Vec::new(),
            structural_arguments: vec![StructuralArgument {
                place: id(source, PlaceId::new),
                path: Vec::new(),
                access: StructuralAccess::SharedBorrow,
            }],
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    }
}

fn literal_cycle() -> TerminalModule {
    let mut module = ranked_countdown_with_width(64);
    module.structural_types.push(StructuralTypeDeclaration {
        id: id(1, StructuralTypeId::new),
        identity: "test::ImmutableBytes".into(),
        shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView),
    });
    let machine = &mut module.machines[0];
    machine.ranked_scc = None;
    machine.parameters = vec![ValueDeclaration {
        id: id(20, ValueId::new),
        scalar_type: ScalarType::Boolean,
    }];
    machine.structural_places = vec![StructuralPlaceDeclaration {
        id: id(1, PlaceId::new),
        kind: StructuralPlaceKind::ByteSequenceLiteral {
            declaration_ordinal: 0,
            structural_type: id(1, StructuralTypeId::new),
        },
    }];
    for block in &mut machine.blocks {
        block.parameters.clear();
        block.operations.clear();
        match &mut block.terminator {
            Terminator::Jump { arguments, .. } => arguments.clear(),
            Terminator::Conditional { condition, .. } => *condition = id(20, ValueId::new),
            _ => {}
        }
    }
    machine.blocks[2].operations.push(Operation {
        id: id(30, OperationId::new),
        result: OperationResult::Unit,
        kind: OperationKind::EstablishByteSequenceLiteral {
            destination: id(1, PlaceId::new),
            bytes: vec![0, 255, 10],
        },
    });
    let call = helper_call(&mut module, 1);
    module.machines[0].blocks[2].operations.push(call);
    module
}

#[test]
fn cyclic_unit_call_borrows_exact_reestablished_literal() {
    let mut module = literal_cycle();
    verify_module_for_interpretation(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .unwrap();
    module.machines[0].blocks.reverse();
    verify_module_for_interpretation(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .unwrap();
}

#[test]
fn cyclic_unit_call_accepts_machine_block_and_subslice_views() {
    for source in [1, 2, 3] {
        let mut module = super::unranked_views::view_cycle();
        let call = helper_call(&mut module, source);
        module.machines[0].blocks[2].operations.push(call);
        validate_module(&module).unwrap();
    }
}

#[test]
fn cyclic_literal_unit_call_rejects_access_widening_and_projected_arguments() {
    for mutation in 0..4 {
        let mut module = literal_cycle();
        let OperationKind::CallUnit {
            structural_arguments,
            ..
        } = &mut module.machines[0].blocks[2].operations[1].kind
        else {
            unreachable!()
        };
        match mutation {
            0 => structural_arguments[0].access = StructuralAccess::MutableBorrow,
            1 => structural_arguments[0].access = StructuralAccess::WriteOnlyBorrow,
            2 => structural_arguments[0].access = StructuralAccess::Owned,
            3 => structural_arguments[0].path.push("invented".into()),
            _ => unreachable!(),
        }
        assert!(validate_module(&module).is_err());
    }
}

#[test]
fn cyclic_literal_unit_call_keeps_current_iteration_and_sibling_dominance() {
    for block_position in [0, 1, 2, 3] {
        let mut module = literal_cycle();
        let call = module.machines[0].blocks[2].operations.pop().unwrap();
        module.machines[0].blocks[block_position]
            .operations
            .insert(0, call);
        assert_eq!(
            validate_module(&module).map(|_| ()),
            Err(ModuleError::ByteSequenceViewNotEstablished {
                operation: id(40, OperationId::new),
                place: id(1, PlaceId::new),
            })
        );
    }
}

#[test]
fn cyclic_unit_view_call_rejects_forged_descriptor_producer_and_type() {
    for mutation in 0..2 {
        let mut module = literal_cycle();
        if mutation == 0 {
            module.machines[0].structural_places[0].kind = StructuralPlaceKind::OperationResult {
                producer: id(30, OperationId::new),
                structural_type: id(1, StructuralTypeId::new),
            };
        } else {
            let mut other = module.structural_types[0].clone();
            other.id = id(2, StructuralTypeId::new);
            other.identity = "test::DifferentBytes".into();
            module.structural_types.push(other);
            module.machines[1].structural_parameters[0].structural_type =
                id(2, StructuralTypeId::new);
        }
        assert!(validate_module(&module).is_err());
    }
}

#[test]
fn immutable_byte_call_extension_does_not_admit_shared_record_calls() {
    let mut module = literal_cycle();
    module.structural_types[0].shape = StructuralTypeShape::Record { fields: Vec::new() };
    let caller = &mut module.machines[0];
    caller.attachment = Some(id(1, StructuralTypeId::new));
    caller.structural_parameters = vec![StructuralParameterDeclaration {
        is_self: true,
        access: StructuralAccess::MutableBorrow,
        ..shared_parameter(1)
    }];
    caller.structural_places[0].kind = StructuralPlaceKind::Parameter {
        position: 0,
        is_self: true,
    };
    caller.blocks[2].operations.remove(0);
    module.machines[1].blocks[0].operations.clear();
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::ControlCycle(_))
    ));
}
