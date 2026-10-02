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

fn dispose_after_last_loan() -> TerminalModule {
    let mut module = module();
    let machine = &mut module.machines[0];
    machine.blocks[2].terminator = Terminator::Jump {
        edge: id(14, EdgeId::new),
        target: id(6, BlockId::new),
        arguments: Vec::new(),
        structural_arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        residual_affine_discards: Vec::new(),
        trivial_affine_discards: vec![id(5, PlaceId::new)],
    };
    machine.blocks.push(Block {
        id: id(6, BlockId::new),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
        operations: Vec::new(),
        terminator: Terminator::ReturnUnit {
            edge: id(15, EdgeId::new),
            trivial_affine_discards: Vec::new(),
        },
    });
    module
}

#[test]
fn affine_home_can_be_disposed_when_leaving_its_last_loan() {
    let module = dispose_after_last_loan();
    validate_module(&module).expect("the successor retains no loan of the disposed home");
    verify_module_for_optimization(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("ending the loan and disposing its home leaves no outstanding custody");
}

#[test]
fn disposed_home_cannot_be_read_through_a_dominating_name() {
    for place in [5, 7] {
        let mut module = dispose_after_last_loan();
        module.machines[0].blocks[3]
            .operations
            .push(read(24, place, 23));
        assert!(
            validate_module(&module).is_err(),
            "disposed storage cannot be observed through place {place}"
        );
    }
}

#[test]
fn disposed_home_cannot_be_reborrowed_through_a_dominating_name() {
    let mut module = dispose_after_last_loan();
    module.machines[0].blocks[3].terminator = Terminator::Jump {
        edge: id(15, EdgeId::new),
        target: id(1, BlockId::new),
        arguments: Vec::new(),
        structural_arguments: vec![argument(7, StructuralAccess::MutableBorrow)],
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        residual_affine_discards: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    assert!(
        validate_module(&module).is_err(),
        "an old loan cannot resurrect disposed backing on a later edge"
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

/// Repeat the record beneath a fixed-array root without changing its loan
/// lifecycle. Zero initialization establishes the affine backing once; every
/// state observes and mutates the selected element through its current name.
fn array_module() -> TerminalModule {
    use semantic_vocabulary::CanonicalStructuralPathSegment;
    use terminal_psi::{StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape};

    let mut module = module();
    let array = id(2, StructuralTypeId::new);
    module.structural_types.push(StructuralTypeDeclaration {
        id: array,
        identity: "test::Pairs".into(),
        shape: StructuralTypeShape::FixedArray {
            element: id(1, StructuralTypeId::new),
            length: 3,
        },
    });
    let machine = &mut module.machines[0];
    let StructuralPlaceKind::OperationResult {
        structural_type, ..
    } = &mut machine.structural_places[0].kind
    else {
        panic!("backing home");
    };
    *structural_type = array;
    let constructor = &mut machine.blocks[0].operations[0];
    constructor.kind = OperationKind::EstablishZeroedStructural {};
    let OperationResult::Structural(result) = &mut constructor.result else {
        panic!("owned array result");
    };
    result.structural_type = array;
    for block in &mut machine.blocks[1..] {
        block.structural_parameters[0].structural_type = array;
        for operation in &mut block.operations {
            operation.kind = match &operation.kind {
                OperationKind::IntegerStructuralField { source, field, .. } => {
                    OperationKind::IntegerStructuralField {
                        source: *source,
                        path: vec![CanonicalStructuralPathSegment::FixedIndex(2)],
                        field: *field,
                    }
                }
                OperationKind::StructuralScalarFieldStore {
                    destination,
                    field,
                    value,
                    ..
                } => OperationKind::StructuralScalarFieldStore {
                    destination: *destination,
                    path: vec![StructuralPathSegment::FixedIndex(2)],
                    field: *field,
                    value: *value,
                    range_obligation: None,
                },
                _ => continue,
            };
        }
    }
    module
}

#[test]
fn array_home_survives_mutable_loop_with_exact_element_access() {
    verify_module_for_optimization(
        &array_module(),
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("array backing follows the same independently checked loan lifecycle as a record");
}

#[test]
fn initialized_scalar_array_uses_the_same_state_loan_lifecycle() {
    use terminal_psi::{StructuralPathSegment, StructuralTypeShape};

    let mut module = array_module();
    module.structural_types[0].shape = StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
        IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
    ));
    let machine = &mut module.machines[0];
    let constructor = &mut machine.blocks[0].operations[0];
    constructor.kind = OperationKind::EstablishScalarArray {
        elements: vec![id(20, ValueId::new); 3],
    };
    let OperationResult::Structural(result) = &mut constructor.result else {
        panic!("array constructor");
    };
    result.multiplicity = StructuralMultiplicity::Unrestricted;
    for block in &mut machine.blocks[1..] {
        for operation in &mut block.operations {
            operation.kind = match &operation.kind {
                OperationKind::IntegerStructuralField { source, .. } => {
                    OperationKind::PrimitiveScalarRead {
                        source: *source,
                        path: vec![StructuralPathSegment::FixedIndex(2)],
                    }
                }
                OperationKind::StructuralScalarFieldStore {
                    destination, value, ..
                } => OperationKind::WriteOnlyPrimitiveStore {
                    destination: *destination,
                    path: vec![StructuralPathSegment::FixedIndex(2)],
                    value: *value,
                },
                _ => continue,
            };
        }
    }
    let Terminator::ReturnUnit {
        trivial_affine_discards,
        ..
    } = &mut machine.blocks[2].terminator
    else {
        panic!("normal return");
    };
    trivial_affine_discards.clear();
    verify_module_for_optimization(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("an initialized array is the same owned backing, not a different loan kind");
}

#[test]
fn array_home_is_unavailable_through_old_owner_while_state_loan_is_live() {
    for write in [false, true] {
        let mut module = array_module();
        let operation = &mut module.machines[0].blocks[1].operations[usize::from(write)];
        match &mut operation.kind {
            OperationKind::IntegerStructuralField { source, .. } => *source = id(5, PlaceId::new),
            OperationKind::StructuralScalarFieldStore { destination, .. } => {
                *destination = id(5, PlaceId::new);
            }
            _ => panic!("array element access"),
        }
        let expected = operation.id;
        assert!(matches!(
            validate_module(&module),
            Err(super::ModuleError::OwnedStructuralPlaceNotLiveAtOperation { operation, place })
                if operation == expected && place == id(5, PlaceId::new)
        ));
    }
}

#[test]
fn array_loan_rejects_same_layout_type_substitution_and_early_disposal() {
    let mut module = array_module();
    let mut duplicate = module.structural_types[1].clone();
    duplicate.id = id(3, StructuralTypeId::new);
    duplicate.identity = "test::OtherPairs".into();
    module.structural_types.push(duplicate);
    module.machines[0].blocks[1].structural_parameters[0].structural_type =
        id(3, StructuralTypeId::new);
    assert!(matches!(
        validate_module(&module),
        Err(super::ModuleError::InvalidStructuralSuccessorArgument { .. })
    ));

    let mut module = array_module();
    let Terminator::Jump {
        trivial_affine_discards,
        ..
    } = &mut module.machines[0].blocks[0].terminator
    else {
        panic!("loan onset");
    };
    trivial_affine_discards.push(id(5, PlaceId::new));
    let error = validate_module(&module).expect_err("backing cannot die on its loan edge");
    assert!(
        matches!(error,
            super::ModuleError::OwnedStructuralPlaceNotLiveAtOperation { operation, place }
                if operation == id(21, OperationId::new) && place == id(6, PlaceId::new)
        ),
        "{error:?}"
    );
}

#[test]
fn array_loan_cannot_duplicate_exclusive_arrival() {
    for second_access in [
        StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow,
    ] {
        let mut module = array_module();
        let machine = &mut module.machines[0];
        let mut extra = state_parameter(8, id(2, StructuralTypeId::new));
        extra.position = 1;
        extra.access = second_access;
        machine.structural_places.push(StructuralPlaceDeclaration {
            id: extra.place,
            kind: StructuralPlaceKind::BlockParameter {
                block: id(1, BlockId::new),
                position: 1,
            },
        });
        machine.blocks[1].structural_parameters.push(extra);
        let Terminator::Jump {
            structural_arguments,
            ..
        } = &mut machine.blocks[0].terminator
        else {
            panic!("loan onset");
        };
        structural_arguments.push(argument(5, second_access));
        let Terminator::Conditional { when_true, .. } = &mut machine.blocks[1].terminator else {
            panic!("loop backedge");
        };
        when_true
            .structural_arguments
            .push(argument(6, second_access));
        assert!(matches!(
            validate_module(&module),
            Err(super::ModuleError::InvalidStructuralSuccessorArgument { .. })
        ));
    }
}

#[test]
fn array_state_loan_rejects_permission_carriers_and_shared_to_mutable_transfer() {
    // Unlike arrays of Reference, a record with a borrowed view passes type
    // formation. The state-loan classifier itself must exclude that carrier.
    let mut module = array_module();
    let terminal_psi::StructuralTypeShape::Record { fields } =
        &mut module.structural_types[0].shape
    else {
        panic!("array element record");
    };
    fields[1].field_type = terminal_psi::StructuralFieldType::ByteSequence(
        terminal_psi::ByteSequenceCarrier::BorrowedView {
            access: Some(StructuralAccess::SharedBorrow),
        },
    );
    let error = validate_module(&module).expect_err("array loans require plain element storage");
    assert!(
        matches!(
            error,
            super::ModuleError::InvalidBlockStructuralParameter { .. }
        ),
        "{error:?}"
    );

    let mut module = array_module();
    module.structural_types[1].shape = terminal_psi::StructuralTypeShape::FixedArray {
        element: id(3, StructuralTypeId::new),
        length: 3,
    };
    module
        .structural_types
        .push(terminal_psi::StructuralTypeDeclaration {
            id: id(3, StructuralTypeId::new),
            identity: "test::BorrowedPair".into(),
            shape: terminal_psi::StructuralTypeShape::Reference {
                referent: id(1, StructuralTypeId::new),
                access: StructuralAccess::SharedBorrow,
            },
        });
    let error = validate_module(&module).expect_err("a plain array cannot hide a permission");
    assert!(
        matches!(
            error,
            super::ModuleError::InvalidStructuralTypeIdentity(structural_type)
                if structural_type == id(2, StructuralTypeId::new)
        ),
        "{error:?}"
    );

    let mut module = array_module();
    let Terminator::Jump {
        structural_arguments,
        ..
    } = &mut module.machines[0].blocks[0].terminator
    else {
        panic!("loan onset");
    };
    structural_arguments[0].access = StructuralAccess::SharedBorrow;
    let error = validate_module(&module).expect_err("shared access cannot grant a mutable name");
    assert!(
        matches!(
            error,
            super::ModuleError::InvalidStructuralSuccessorArgument { .. }
        ),
        "{error:?}"
    );
}

/// Whole-name attenuation ends the old exclusive spelling while retaining
/// the backing home's independent disposal and the shared arrival's reads.
fn attenuated_array_module() -> TerminalModule {
    let mut module = array_module();
    module.machines[0].blocks[2].structural_parameters[0].access = StructuralAccess::SharedBorrow;
    let Terminator::Conditional { when_false, .. } = &mut module.machines[0].blocks[1].terminator
    else {
        panic!("loop exit");
    };
    when_false.structural_arguments[0].access = StructuralAccess::SharedBorrow;
    module
}

#[test]
fn exclusive_state_loan_can_attenuate_into_a_shared_arrival() {
    verify_module_for_optimization(
        &attenuated_array_module(),
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("the shared arrival observes the same live backing after retiring the exclusive name");
}

#[test]
fn attenuated_state_loan_cannot_reuse_the_old_exclusive_name() {
    for write in [false, true] {
        let mut module = attenuated_array_module();
        let operation = &mut module.machines[0].blocks[2].operations[0];
        if write {
            operation.result = OperationResult::Unit;
            operation.kind = OperationKind::StructuralScalarFieldStore {
                destination: id(6, PlaceId::new),
                path: vec![terminal_psi::StructuralPathSegment::FixedIndex(2)],
                field: id(1, StructuralFieldId::new),
                value: id(20, ValueId::new),
                range_obligation: None,
            };
        } else {
            let OperationKind::IntegerStructuralField { source, .. } = &mut operation.kind else {
                panic!("array observation");
            };
            *source = id(6, PlaceId::new);
        }
        assert!(
            matches!(validate_module(&module), Err(super::ModuleError::OwnedStructuralPlaceNotLiveAtOperation { place, .. }) if place == id(6, PlaceId::new))
        );
    }
}

#[test]
fn shared_state_arrival_cannot_recover_exclusive_write_access() {
    let mut module = attenuated_array_module();
    module.machines[0].blocks[2].operations[0].result = OperationResult::Unit;
    module.machines[0].blocks[2].operations[0].kind = OperationKind::StructuralScalarFieldStore {
        destination: id(7, PlaceId::new),
        path: vec![terminal_psi::StructuralPathSegment::FixedIndex(2)],
        field: id(1, StructuralFieldId::new),
        value: id(20, ValueId::new),
        range_obligation: None,
    };
    assert!(
        validate_module(&module).is_err(),
        "shared arrival cannot grant a write"
    );
}
