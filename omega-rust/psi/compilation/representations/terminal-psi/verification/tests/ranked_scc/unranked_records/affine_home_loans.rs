//! An affine backing home survives a cyclic exclusive loan until final disposal.
use super::{
    AdmissionProfile, BlockId, IntegerSign, IntegerType, Operation, OperationId, OperationKind,
    OperationResult, PlaceId, ProofBundle, ScalarType, StructuralAccess, StructuralArgument,
    StructuralFieldId, StructuralMultiplicity, StructuralPlaceDeclaration, StructuralPlaceKind,
    StructuralTypeId, TerminalModule, ValueDeclaration, ValueId, declare_pair, id,
    record_establishment, record_place, scalar_field_initializer, unranked_scalar_cycle,
    validate_module, verify_module_for_optimization,
};
use semantic_vocabulary::EdgeId;
use terminal_psi::{Block, StructuralParameterDeclaration, Terminator};

fn state_parameter(
    place: u64,
    structural_type: StructuralTypeId,
) -> StructuralParameterDeclaration {
    StructuralParameterDeclaration {
        place: id(place, PlaceId::new),
        position: 0,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::MutableBorrow,
        is_self: false,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }
}

fn argument(place: u64, access: StructuralAccess) -> StructuralArgument {
    StructuralArgument {
        place: id(place, PlaceId::new),
        access,
        path: Vec::new(),
    }
}

fn read(operation: u64, place: u64, value: u64) -> Operation {
    Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: id(operation, OperationId::new),
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: id(value, ValueId::new),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
        }),
        kind: OperationKind::IntegerStructuralField {
            source: id(place, PlaceId::new),
            path: Vec::new(),
            field: id(1, StructuralFieldId::new),
        },
    }
}

/// The constructor runs once, outside the SCC. The loop can read and write only
/// its current mutable name; the original affine owner remains owed at return.
fn module() -> TerminalModule {
    let mut module = unranked_scalar_cycle();
    let record = declare_pair(&mut module);
    let machine = &mut module.machines[0];
    machine.parameters.push(ValueDeclaration {
        qualifications: Default::default(),
        id: id(20, ValueId::new),
        scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
    });
    machine.structural_places.push(record_place(5, 20, record));
    for (place, block) in [(6, 1), (7, 4)] {
        machine.structural_places.push(StructuralPlaceDeclaration {
            id: id(place, PlaceId::new),
            kind: StructuralPlaceKind::BlockParameter {
                block: id(block, BlockId::new),
                position: 0,
            },
        });
    }
    machine.blocks[0]
        .structural_parameters
        .push(state_parameter(6, record));
    machine.blocks[1]
        .structural_parameters
        .push(state_parameter(7, record));
    machine.blocks[0].operations.push(read(21, 6, 21));
    machine.blocks[0].operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: id(22, OperationId::new),
        result: OperationResult::Unit,
        kind: OperationKind::StructuralScalarFieldStore {
            destination: id(6, PlaceId::new),
            path: Vec::new(),
            field: id(1, StructuralFieldId::new),
            value: id(21, ValueId::new),
            range_obligation: None,
        },
    });
    let Terminator::Conditional {
        when_true,
        when_false,
        ..
    } = &mut machine.blocks[0].terminator
    else {
        panic!("cyclic fixture has a conditional");
    };
    when_true
        .structural_arguments
        .push(argument(6, StructuralAccess::MutableBorrow));
    when_false
        .structural_arguments
        .push(argument(6, StructuralAccess::MutableBorrow));
    machine.blocks[1].operations.push(read(23, 7, 22));
    let Terminator::ReturnUnit {
        trivial_affine_discards,
        ..
    } = &mut machine.blocks[1].terminator
    else {
        panic!("fixture has a normal return");
    };
    trivial_affine_discards.push(id(5, PlaceId::new));

    let mut constructor = record_establishment(
        20,
        5,
        record,
        vec![
            scalar_field_initializer(1, 20),
            scalar_field_initializer(2, 10),
        ],
    );
    let OperationResult::Structural(result) = &mut constructor.result else {
        panic!("record constructor");
    };
    result.multiplicity = StructuralMultiplicity::Affine;
    machine.entry = id(5, BlockId::new);
    machine.blocks.insert(
        0,
        Block {
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: machine.entry,
            parameters: Vec::new(),
            operations: vec![constructor],
            terminator: Terminator::Jump {
                edge: id(13, EdgeId::new),
                target: id(1, BlockId::new),
                arguments: Vec::new(),
                structural_arguments: vec![argument(5, StructuralAccess::MutableBorrow)],
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                residual_affine_discards: Vec::new(),
                trivial_affine_discards: Vec::new(),
            },
        },
    );
    module
}

#[test]
fn affine_home_survives_mutable_loop_until_normal_return_disposal() {
    let module = module();
    validate_module(&module).expect("one live backing owner crosses the whole loan cycle");
    verify_module_for_optimization(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("custody is independently reconstructed without producer evidence");
}

#[test]
fn affine_home_cannot_escape_final_disposal() {
    let mut module = module();
    let Terminator::ReturnUnit {
        trivial_affine_discards,
        ..
    } = &mut module.machines[0].blocks[2].terminator
    else {
        panic!("normal return");
    };
    trivial_affine_discards.clear();
    assert!(
        validate_module(&module).is_err(),
        "the loan does not consume its affine backing owner"
    );
}

#[test]
fn affine_home_cannot_be_disposed_on_its_loan_edge() {
    let mut module = module();
    let Terminator::Jump {
        trivial_affine_discards,
        ..
    } = &mut module.machines[0].blocks[0].terminator
    else {
        panic!("loan onset");
    };
    trivial_affine_discards.push(id(5, PlaceId::new));
    assert!(
        validate_module(&module).is_err(),
        "a state cannot arrive with a pointer to disposed backing"
    );
}

#[test]
fn affine_home_cannot_be_read_while_its_mutable_state_name_is_live() {
    let mut module = module();
    let OperationKind::IntegerStructuralField { source, .. } =
        &mut module.machines[0].blocks[1].operations[0].kind
    else {
        panic!("loop read");
    };
    *source = id(5, PlaceId::new);
    assert!(
        validate_module(&module).is_err(),
        "only the current exclusive name may read the backing"
    );
}

#[test]
fn affine_home_cannot_lend_shared_and_mutable_names_on_the_same_edge() {
    let mut module = module();
    let record = module.structural_types[0].id;
    let machine = &mut module.machines[0];
    let mut shared = state_parameter(8, record);
    shared.position = 1;
    shared.access = StructuralAccess::SharedBorrow;
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: shared.place,
        kind: StructuralPlaceKind::BlockParameter {
            block: id(1, BlockId::new),
            position: 1,
        },
    });
    machine.blocks[1].structural_parameters.push(shared);
    let Terminator::Jump {
        structural_arguments,
        ..
    } = &mut machine.blocks[0].terminator
    else {
        panic!("loan onset");
    };
    structural_arguments.push(argument(5, StructuralAccess::SharedBorrow));
    let Terminator::Conditional { when_true, .. } = &mut machine.blocks[1].terminator else {
        panic!("loop backedge");
    };
    when_true
        .structural_arguments
        .push(argument(6, StructuralAccess::SharedBorrow));
    assert!(
        validate_module(&module).is_err(),
        "two differently typed names cannot duplicate exclusive custody"
    );
}
