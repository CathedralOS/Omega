use super::*;

fn block_view(access: StructuralAccess, multiplicity: StructuralMultiplicity) -> TerminalModule {
    let mut module = view_cycle();
    let machine = &mut module.machines[0];
    machine.blocks.truncate(2);
    machine.structural_places.truncate(2);
    machine.structural_parameters[0].access = access;
    machine.structural_parameters[0].multiplicity = multiplicity;
    machine.blocks[1].structural_parameters[0].access = access;
    machine.blocks[1].structural_parameters[0].multiplicity = multiplicity;
    let Terminator::Jump {
        structural_arguments,
        ..
    } = &mut machine.blocks[0].terminator
    else {
        panic!("invocation binds the block view")
    };
    structural_arguments[0].access = access;
    machine.blocks[1].operations.clear();
    machine.blocks[1].terminator = Terminator::ReturnUnit {
        edge: id(5, EdgeId::new),
        trivial_affine_discards: if access == StructuralAccess::Owned
            && multiplicity == StructuralMultiplicity::Affine
        {
            vec![id(2, PlaceId::new)]
        } else {
            Vec::new()
        },
    };
    module
}

#[test]
fn block_byte_observation_requires_shared_unrestricted_access() {
    for (access, multiplicity) in [
        (
            StructuralAccess::SharedBorrow,
            StructuralMultiplicity::Unrestricted,
        ),
        (
            StructuralAccess::Owned,
            StructuralMultiplicity::Unrestricted,
        ),
        (StructuralAccess::Owned, StructuralMultiplicity::Affine),
    ] {
        let mut module = block_view(access, multiplicity);
        validate_module(&module).expect("exact transfer and cleanup without observation");
        module.machines[0].blocks[1]
            .operations
            .push(length_operation(10, 10, 2));
        let expected = if access == StructuralAccess::SharedBorrow {
            Ok(())
        } else {
            Err(ModuleError::InvalidByteSequenceLengthSource {
                operation: id(10, OperationId::new),
                source: id(2, PlaceId::new),
            })
        };
        assert_eq!(validate_module(&module).map(|_| ()), expected);
    }
}

#[test]
fn discarded_owned_block_byte_view_cannot_be_observed_in_a_successor() {
    let mut module = block_view(StructuralAccess::Owned, StructuralMultiplicity::Affine);
    let machine = &mut module.machines[0];
    let continuation = Block {
        id: id(3, BlockId::new),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        operations: Vec::new(),
        terminator: Terminator::ReturnUnit {
            edge: id(5, EdgeId::new),
            trivial_affine_discards: Vec::new(),
        },
    };
    machine.blocks[1].terminator = Terminator::Jump {
        edge: id(2, EdgeId::new),
        target: continuation.id,
        arguments: Vec::new(),
        structural_arguments: Vec::new(),
        trivial_affine_discards: vec![id(2, PlaceId::new)],
        residual_affine_discards: Vec::new(),
    };
    machine.blocks.push(continuation);
    validate_module(&module).expect("selected edge disposes of the transferred owner exactly once");
    module.machines[0].blocks[2]
        .operations
        .push(length_operation(10, 10, 2));
    assert_eq!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::InvalidByteSequenceLengthSource {
            operation: id(10, OperationId::new),
            source: id(2, PlaceId::new),
        })
    );
}
