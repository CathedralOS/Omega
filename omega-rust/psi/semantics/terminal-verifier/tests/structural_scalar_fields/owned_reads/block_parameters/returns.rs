use super::*;

fn returning() -> TerminalModule {
    let mut module = block_reader(integer_type(), StructuralMultiplicity::Affine);
    let machine = &mut module.machines[0];
    let parameter = &machine.blocks[1].structural_parameters[0];
    let returned = parameter.place;
    machine.result = TerminalMachineResult::Structural(terminal_psi::StructuralResultDeclaration {
        place: id::<PlaceId>(90),
        structural_type: parameter.structural_type,
        multiplicity: parameter.multiplicity,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    });
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: id::<PlaceId>(90),
        kind: StructuralPlaceKind::Result,
    });
    machine.blocks[1].terminator = Terminator::ReturnStructural {
        edge: id::<EdgeId>(90),
        source: returned,
        returned_claims: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    module
}

#[test]
fn owned_block_return_consumes_the_rebound_frontier() {
    validate_module(&returning()).expect("owned block result preserves transferred custody");
}

#[test]
fn owned_block_return_rejects_missing_and_discarded_incoming_values() {
    for missing in [false, true] {
        let mut module = returning();
        let Terminator::Jump {
            structural_arguments,
            trivial_affine_discards,
            ..
        } = &mut module.machines[0].blocks[0].terminator
        else {
            panic!("incoming transfer");
        };
        if missing {
            structural_arguments.clear();
        } else {
            trivial_affine_discards.push(structural_arguments[0].place);
        }
        assert!(validate_module(&module).is_err());
    }
}

#[test]
fn owned_block_return_rejects_a_nondominating_binding() {
    let mut module = returning();
    let machine = &mut module.machines[0];
    let mut bypass = machine.blocks[1].clone();
    bypass.id = id::<BlockId>(91);
    bypass.structural_parameters.clear();
    bypass.operations.clear();
    let Terminator::ReturnStructural { edge, source, .. } = &mut bypass.terminator else {
        panic!("return");
    };
    *edge = id::<EdgeId>(91);
    let returned = *source;
    let Terminator::Jump {
        edge,
        target,
        arguments,
        structural_arguments,
        trivial_affine_discards,
        ..
    } = machine.blocks[0].terminator.clone()
    else {
        panic!("incoming transfer");
    };
    machine.blocks[0].operations.push(Operation {
        id: id::<OperationId>(91),
        result: OperationResult::Scalar(ValueDeclaration {
            id: id::<ValueId>(91),
            scalar_type: ScalarType::Boolean,
            qualifications: Default::default(),
        }),
        kind: OperationKind::BooleanConstant { value: true },
    });
    machine.blocks[0].terminator = Terminator::Conditional {
        condition: id::<ValueId>(91),
        when_true: terminal_psi::SuccessorEdge {
            edge,
            target,
            arguments,
            structural_arguments,
            trivial_affine_discards,
        },
        when_false: terminal_psi::SuccessorEdge {
            edge: id::<EdgeId>(92),
            target: bypass.id,
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
    };
    let expected = ModuleError::StructuralReturnSourceNotLive {
        machine: machine.id,
        block: bypass.id,
        place: returned,
    };
    machine.blocks.push(bypass);
    assert_eq!(validate_module(&module).map(|_| ()), Err(expected));
}

#[test]
fn owned_block_return_cannot_exchange_borrowed_access() {
    let mut module = returning();
    module.machines[0].blocks[1].structural_parameters[0].access = StructuralAccess::SharedBorrow;
    assert!(validate_module(&module).is_err());
}
