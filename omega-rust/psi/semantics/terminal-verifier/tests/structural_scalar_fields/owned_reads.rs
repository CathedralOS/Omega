use super::*;
use terminal_psi::TerminalAffineCleanupAction;

#[path = "owned_reads/block_parameters.rs"]
mod block_parameters;

fn owned_reader(multiplicity: StructuralMultiplicity, scalar_type: ScalarType) -> TerminalModule {
    let mut module = structural_scalar_field_module();
    module.machines.remove(0);
    let machine = &mut module.machines[0];
    module.entry = machine.id;
    machine.attachment = None;
    machine.parameters.clear();
    let parameter = &mut machine.structural_parameters[0];
    parameter.is_self = false;
    parameter.access = StructuralAccess::Owned;
    parameter.multiplicity = multiplicity;
    let declaration = module
        .structural_types
        .iter_mut()
        .find(|declaration| declaration.id == parameter.structural_type)
        .unwrap();
    let StructuralTypeShape::Record { fields } = &mut declaration.shape else {
        unreachable!();
    };
    fields[0].field_type = StructuralFieldType::Scalar(scalar_type);
    let TerminalMachineResult::Scalar(result) = &mut machine.result else {
        unreachable!();
    };
    result.scalar_type = scalar_type;
    let operation = &mut machine.blocks[0].operations[0];
    operation.result.scalar_mut().unwrap().scalar_type = scalar_type;
    operation.kind = if scalar_type == ScalarType::Boolean {
        OperationKind::BooleanStructuralField {
            path: Vec::new(),
            source: parameter.place,
            field: fields[0].id,
        }
    } else {
        OperationKind::IntegerStructuralField {
            path: Vec::new(),
            source: parameter.place,
            field: fields[0].id,
        }
    };
    machine.structural_places[0].kind = StructuralPlaceKind::Parameter {
        position: 0,
        is_self: false,
    };
    if multiplicity == StructuralMultiplicity::Affine {
        let Terminator::Return {
            cleanup_actions, ..
        } = &mut machine.blocks[0].terminator
        else {
            unreachable!();
        };
        cleanup_actions.push(TerminalAffineCleanupAction::DiscardRoot(parameter.place));
    }
    module
}

#[test]
fn claim_free_owned_scalar_reads_accept_affine_and_unrestricted_parameters() {
    for scalar_type in [integer_type(), ScalarType::Boolean] {
        for multiplicity in [
            StructuralMultiplicity::Affine,
            StructuralMultiplicity::Unrestricted,
        ] {
            validate_module(&owned_reader(multiplicity, scalar_type))
                .expect("owned scalar observation validates");
        }
    }
}

#[test]
fn scalar_only_call_cannot_omit_an_owned_record_input() {
    for multiplicity in [
        StructuralMultiplicity::Affine,
        StructuralMultiplicity::Unrestricted,
    ] {
        let mut module = owned_reader(multiplicity, integer_type());
        validate_module(&module).unwrap();
        let mut caller = structural_scalar_field_module().machines.remove(0);
        caller.attachment = None;
        caller.structural_parameters.clear();
        caller.structural_places.clear();
        let mut call = caller.blocks[0].operations.pop().unwrap();
        call.kind = OperationKind::Call {
            callee: module.entry,
            arguments: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        };
        caller.blocks[0].operations = vec![call];
        module.entry = caller.id;
        module.machines.insert(0, caller);
        assert!(
            matches!(
                validate_module(&module),
                Err(ModuleError::CallTargetHasStructuralContract { .. })
            ),
            "scalar-only call cannot acquire an implicit {multiplicity:?} record"
        );
    }
}

#[test]
fn scalar_field_read_after_discard_on_predecessor_edge_rejects() {
    for scalar_type in [integer_type(), ScalarType::Boolean] {
        let mut module = owned_reader(StructuralMultiplicity::Affine, scalar_type);
        validate_module(&module).unwrap();
        let machine = &mut module.machines[0];
        let place = machine.structural_parameters[0].place;
        let mut continuation = machine.blocks[0].clone();
        continuation.id = id::<BlockId>(3);
        let Terminator::Return {
            cleanup_actions, ..
        } = &mut continuation.terminator
        else {
            unreachable!();
        };
        cleanup_actions.clear();
        let entry = &mut machine.blocks[0];
        entry.operations.clear();
        entry.terminator = Terminator::Jump {
            edge: id::<EdgeId>(3),
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
fn scalar_field_read_after_owned_call_transfer_rejects_but_before_transfer_is_valid() {
    for scalar_type in [integer_type(), ScalarType::Boolean] {
        let mut module = owned_reader(StructuralMultiplicity::Affine, scalar_type);
        let mut consumer = module.machines[0].clone();
        consumer.id = id::<MachineId>(3);
        consumer.contract = contract(3);
        consumer.entry = id::<BlockId>(3);
        consumer.result = TerminalMachineResult::Unit;
        consumer.structural_parameters[0].place = id::<PlaceId>(3);
        consumer.structural_places[0].id = id::<PlaceId>(3);
        consumer.blocks[0].id = consumer.entry;
        consumer.blocks[0].operations.clear();
        consumer.blocks[0].terminator = Terminator::ReturnUnit {
            edge: id::<EdgeId>(3),
            trivial_affine_discards: vec![id::<PlaceId>(3)],
        };
        let caller = &mut module.machines[0];
        let place = caller.structural_parameters[0].place;
        caller.blocks[0].operations.push(Operation {
            static_reach_binding: None,
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
        } = &mut caller.blocks[0].terminator
        else {
            unreachable!();
        };
        cleanup_actions.clear();
        module.machines.push(consumer);
        validate_module(&module).expect("snapshot before the owned transfer remains valid");
        module.machines[0].blocks[0].operations.swap(0, 1);
        assert_eq!(
            validate_module(&module).map(|_| ()),
            Err(ModuleError::OwnedStructuralPlaceNotLiveAtOperation {
                operation: id::<OperationId>(4),
                place
            })
        );
    }
}
