//! Owned indirection retains separate payload and pointer identities through replay.
use super::*;
use calling_conventions::IndirectPointerLocation;
use selected_instructions::{
    FrameStorageSlotId, OutgoingArgumentSlotRole, SelectedMemoryAccessRole,
};
use semantic_vocabulary::{PlaceId, StructuralPlaceKind};
use terminal_psi::{StructuralAccess, StructuralMultiplicity};

#[derive(Debug, Clone, Copy)]
enum InputStorage {
    Parameter,
    Constructed,
}

/// Reuse the scalar-array fixture, adding one ordinary aggregate-result call.
/// Repeated byte SSA arguments exhaust the ABI registers without extra producers.
fn owned_call(
    native: target::NativeTarget,
    storage: InputStorage,
    scalar_count: usize,
    length: u16,
) -> LegalizedScalarFunction {
    let mut source = super::scalar_arrays::array_fixture(native, length);
    let mut row = source.blocks[0].instructions[2].clone();
    let LegalizedScalarInstructionKind::EstablishScalarArray {
        result: input,
        shape,
        ..
    } = row.kind.clone()
    else {
        panic!("array fixture");
    };
    let producer = row.operation;
    let input_source = match storage {
        InputStorage::Constructed => {
            target_operations::TargetStructuralArgumentSource::StructuralHome {
                psi_operation: producer,
            }
        }
        InputStorage::Parameter => {
            source.blocks[0].instructions.pop();
            source.call_plan = evaluate_call_plan(
                source.call_plan.policy,
                &CallSignature {
                    parameters: vec![shape],
                    result: Some(shape),
                },
            )
            .unwrap();
            let placement = source.call_plan.parameters[0].clone();
            let signature = source.structural.as_mut().unwrap();
            signature.structural_places[0].kind = StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            };
            signature
                .parameters
                .push(legalized_operations::LegalizedCallUnitParameter {
                    semantic: terminal_psi::StructuralParameterDeclaration {
                        place: input.place,
                        position: 0,
                        is_self: false,
                        structural_type: input.structural_type,
                        multiplicity: StructuralMultiplicity::Unrestricted,
                        access: StructuralAccess::Owned,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                    },
                    target: target_operations::TargetStructuralParameter {
                        place: input.place,
                        structural_type: input.structural_type,
                        multiplicity: StructuralMultiplicity::Unrestricted,
                        access: StructuralAccess::Owned,
                        projected_qualifications: Vec::new(),
                        shape,
                        placement: placement.clone(),
                    },
                });
            placement.into()
        }
    };
    row.operation = OperationId::new(4).unwrap();
    row.fuel = vec![FuelSettlement {
        site: PsiProvenance::Operation(row.operation),
        units: 1,
    }];
    row.ownership.clear();
    let mut output = input.clone();
    output.place = PlaceId::new(3).unwrap();
    let mut parameters = vec![ValueShape::integer(1, 1); scalar_count];
    parameters.push(shape);
    let call_plan = evaluate_call_plan(
        source.call_plan.policy,
        &CallSignature {
            parameters,
            result: Some(shape),
        },
    )
    .unwrap();
    let mut arguments = call_plan.parameters[..scalar_count]
        .iter()
        .map(|placement| LegalizedScalarArgument::Scalar {
            source: ValueId::new(1).unwrap(),
            placement: placement.clone(),
        })
        .collect::<Vec<_>>();
    arguments.push(LegalizedScalarArgument::Structural {
        semantic: terminal_psi::StructuralArgument {
            place: input.place,
            access: StructuralAccess::Owned,
            path: Vec::new(),
        },
        target: target_operations::TargetStructuralArgument {
            place: input.place,
            access: StructuralAccess::Owned,
            path: Vec::new(),
            root_structural_type: input.structural_type,
            structural_type: input.structural_type,
            shape,
            source_byte_offset: 0,
            fixed_array_length: None,
            element_stride: None,
            source: input_source,
            destination: call_plan.parameters[scalar_count].clone(),
        },
    });
    row.kind = LegalizedScalarInstructionKind::Call(LegalizedScalarCall {
        source: LegalizedCallUnitSource::AuthoredCallUnit,
        callee: MachineId::new(2).unwrap(),
        arguments,
        structural_result: Some(output.clone()),
        result_placement: call_plan.result.clone(),
        call_plan,
        claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    });
    source.structural.as_mut().unwrap().structural_places.push(
        terminal_psi::StructuralPlaceDeclaration {
            id: output.place,
            kind: StructuralPlaceKind::OperationResult {
                producer: row.operation,
                structural_type: output.structural_type,
            },
        },
    );
    returned(&mut source.blocks[0]).value = LegalizedScalarReturnValue::Structural {
        source: legalized_operations::LegalizedStructuralCaseSource::OperationResult {
            operation: row.operation,
            result: output,
        },
    };
    source.blocks[0].instructions.push(row);
    source.provenance.operations = source.blocks[0]
        .instructions
        .iter()
        .map(|row| row.operation)
        .collect();
    source
}

fn call_mut(source: &mut LegalizedScalarFunction) -> &mut LegalizedScalarCall {
    let LegalizedScalarInstructionKind::Call(call) =
        &mut source.blocks[0].instructions.last_mut().unwrap().kind
    else {
        panic!("owned call fixture");
    };
    call
}

#[test]
fn indirect_owned_output_rejects_slot_copy_and_pointer_substitution() {
    for native in [
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
        target::NativeTarget::windows_x64(),
    ] {
        let environment =
            register_environment::baseline_target_register_environment(native).unwrap();
        let constraints = SelectedSelectionConstraints {
            keys: environment.selected_keys(),
            fixed_inputs: Vec::new(),
        };
        for (storage, scalar_count, length) in [
            (InputStorage::Parameter, 0, 24),
            (InputStorage::Parameter, 8, 24),
            (InputStorage::Constructed, 8, 23),
        ] {
            let mut source = owned_call(native, storage, scalar_count, length);
            let call = call_mut(&mut source);
            let [
                ValueLocation::Indirect {
                    pointer,
                    copy_stack_byte_offset: Some(copy_offset),
                    ..
                },
            ] = call
                .arguments
                .last()
                .unwrap()
                .placement()
                .locations
                .as_slice()
            else {
                panic!("fixture must exercise indirect owned placement");
            };
            let (pointer, copy_offset) = (*pointer, *copy_offset);
            let selected = build(
                0,
                &source,
                native,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
            .unwrap();
            let validate = |candidate: &SelectedFunction| {
                crate::selection::validation::scalar_graph::validate(
                    0,
                    &source,
                    candidate,
                    native,
                    &constraints,
                    environment.physical(),
                    environment.constraints(),
                )
            };
            validate(&selected).unwrap();
            let copy_position = selected
                .outgoing_arguments
                .iter()
                .position(|slot| {
                    slot.id.operation == OperationId::new(4).unwrap()
                        && slot.id.argument_index == scalar_count as u32
                        && slot.id.role == OutgoingArgumentSlotRole::ValueCopy
                })
                .unwrap();
            let copy = &selected.outgoing_arguments[copy_position];
            assert_eq!(copy.abi_stack_byte_offset, copy_offset);
            assert_eq!(copy.byte_size, u32::from(length));
            if native == target::NativeTarget::windows_x64() {
                assert_eq!(copy.alignment, 16);
                assert_eq!(copy_offset % 16, 0);
            }
            let instructions = &selected.blocks[0].instructions;
            let address_position = instructions
                .iter()
                .position(|row| {
                    row.kind
                        == SelectedInstructionKind::FrameAddress {
                            slot: FrameStorageSlotId::Outgoing(copy.id),
                            byte_offset: 0,
                        }
                })
                .unwrap();
            let copy_pointer = instructions[address_position].operands[0].virtual_register;
            let reads = selected
                .memory_accesses
                .iter()
                .find(|access| {
                    access.place == PlaceId::new(1).unwrap()
                        && access.role == SelectedMemoryAccessRole::ReadPlace
                })
                .unwrap();
            let read_position = instructions
                .iter()
                .position(|row| row.id == reads.instruction)
                .unwrap();
            let source_pointer = instructions[read_position].operands[0].virtual_register;
            assert_ne!(copy_pointer, source_pointer);
            let writes = selected
                .memory_accesses
                .iter()
                .filter(|access| {
                    access.role == SelectedMemoryAccessRole::WriteOutgoing { slot: copy.id }
                })
                .collect::<Vec<_>>();
            assert_eq!(
                writes
                    .iter()
                    .map(|row| (row.byte_offset, row.byte_count))
                    .collect::<Vec<_>>(),
                vec![(0, 8), (8, 8), (16, u32::from(length) - 16)]
            );
            let write_position = instructions
                .iter()
                .position(|row| row.id == writes[0].instruction)
                .unwrap();
            let call_position = instructions
                .iter()
                .position(|row| matches!(row.kind, SelectedInstructionKind::CallAggregate { .. }))
                .unwrap();
            let pointer_position = match pointer {
                IndirectPointerLocation::Register(_) => {
                    assert_eq!(scalar_count, 0);
                    assert_eq!(
                        instructions[call_position].operands[0].virtual_register,
                        copy_pointer
                    );
                    None
                }
                IndirectPointerLocation::Stack {
                    stack_byte_offset,
                    alignment,
                } => {
                    assert_eq!(scalar_count, 8);
                    let slot = selected
                        .outgoing_arguments
                        .iter()
                        .find(|slot| {
                            slot.id.operation == copy.id.operation
                                && slot.id.argument_index == copy.id.argument_index
                                && slot.id.role == OutgoingArgumentSlotRole::Argument
                        })
                        .unwrap();
                    assert_eq!(
                        (slot.byte_size, slot.alignment, slot.abi_stack_byte_offset),
                        (8, alignment, stack_byte_offset)
                    );
                    assert!(stack_byte_offset + 8 <= copy_offset);
                    let position = instructions
                        .iter()
                        .position(|row| {
                            row.kind
                                == SelectedInstructionKind::Store64 {
                                    slot: FrameStorageSlotId::Outgoing(slot.id),
                                    byte_offset: 0,
                                }
                        })
                        .unwrap();
                    assert_eq!(
                        instructions[position].operands[0].virtual_register,
                        copy_pointer
                    );
                    assert!(write_position < position && position < call_position);
                    Some(position)
                }
            };
            for mutation in [
                "copy role",
                "copy offset",
                "copy alignment",
                "copy extent",
                "missing write",
                "wrong source pointer",
                "write into source",
                "forward source pointer",
                "memory copy role",
            ] {
                let mut changed = selected.clone();
                match mutation {
                    "copy role" => {
                        changed.outgoing_arguments[copy_position].id.role =
                            OutgoingArgumentSlotRole::Argument
                    }
                    "copy offset" => {
                        changed.outgoing_arguments[copy_position].abi_stack_byte_offset += 8
                    }
                    "copy alignment" => {
                        changed.outgoing_arguments[copy_position].alignment =
                            if copy.alignment == 16 { 8 } else { 2 }
                    }
                    "copy extent" => changed.outgoing_arguments[copy_position].byte_size -= 1,
                    "missing write" => {
                        changed.blocks[0].instructions.remove(write_position);
                    }
                    "wrong source pointer" => {
                        changed.blocks[0].instructions[read_position].operands[0].virtual_register =
                            copy_pointer
                    }
                    "write into source" => {
                        changed.blocks[0].instructions[write_position].operands[0]
                            .virtual_register = source_pointer
                    }
                    "forward source pointer" => {
                        changed.blocks[0].instructions[pointer_position.unwrap_or(call_position)]
                            .operands[0]
                            .virtual_register = source_pointer
                    }
                    "memory copy role" => {
                        let access = changed
                            .memory_accesses
                            .iter_mut()
                            .find(|access| {
                                access.role
                                    == SelectedMemoryAccessRole::WriteOutgoing { slot: copy.id }
                            })
                            .unwrap();
                        access.role = SelectedMemoryAccessRole::WriteOutgoing {
                            slot: selected_instructions::OutgoingArgumentSlotId {
                                role: OutgoingArgumentSlotRole::Argument,
                                ..copy.id
                            },
                        };
                    }
                    _ => unreachable!(),
                }
                assert!(
                    validate(&changed).is_err(),
                    "{native:?}/{storage:?}/{scalar_count}: {mutation}"
                );
            }
        }
    }
}

#[test]
fn indirect_owned_input_rejects_changed_access_type_producer_and_plan() {
    for native in [
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
        target::NativeTarget::windows_x64(),
    ] {
        let environment =
            register_environment::baseline_target_register_environment(native).unwrap();
        let constraints = SelectedSelectionConstraints {
            keys: environment.selected_keys(),
            fixed_inputs: Vec::new(),
        };
        for storage in [InputStorage::Parameter, InputStorage::Constructed] {
            let source = owned_call(native, storage, 8, 24);
            let selected = build(
                0,
                &source,
                native,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
            .unwrap();
            let validate = |input: &LegalizedScalarFunction, output: &SelectedFunction| {
                crate::selection::validation::scalar_graph::validate(
                    0,
                    input,
                    output,
                    native,
                    &constraints,
                    environment.physical(),
                    environment.constraints(),
                )
            };
            validate(&source, &selected).unwrap();
            for mutation in [
                "actual access",
                "actual type",
                "producer",
                "parameter access",
                "parameter type",
                "copy offset",
                "copy alignment",
                "pointer offset",
            ] {
                if matches!(storage, InputStorage::Constructed)
                    && matches!(mutation, "parameter access" | "parameter type")
                {
                    continue;
                }
                let mut changed = source.clone();
                let call = call_mut(&mut changed);
                let LegalizedScalarArgument::Structural { semantic, target } =
                    call.arguments.last_mut().unwrap()
                else {
                    panic!("owned argument");
                };
                match mutation {
                    "actual access" => {
                        semantic.access = StructuralAccess::SharedBorrow;
                        target.access = StructuralAccess::SharedBorrow;
                    }
                    "actual type" => {
                        target.root_structural_type = StructuralTypeId::new(2).unwrap();
                        target.structural_type = StructuralTypeId::new(2).unwrap();
                    }
                    "producer" => {
                        target.source =
                            target_operations::TargetStructuralArgumentSource::StructuralHome {
                                psi_operation: OperationId::new(99).unwrap(),
                            }
                    }
                    "parameter access" | "parameter type" => {}
                    _ => {
                        let [
                            ValueLocation::Indirect {
                                pointer,
                                copy_stack_byte_offset,
                                alignment,
                                ..
                            },
                        ] = target.destination.locations.as_mut_slice()
                        else {
                            panic!("indirect");
                        };
                        match mutation {
                            "copy offset" => {
                                *copy_stack_byte_offset =
                                    copy_stack_byte_offset.map(|offset| offset + 8)
                            }
                            "copy alignment" => *alignment = 2,
                            "pointer offset" => {
                                let IndirectPointerLocation::Stack {
                                    stack_byte_offset, ..
                                } = pointer
                                else {
                                    panic!("stack pointer");
                                };
                                *stack_byte_offset += 8;
                            }
                            _ => unreachable!(),
                        }
                        // Keep the call's submitted placement and actual aligned: canonical
                        // reconstruction must reject the substitution, not just their inequality.
                        *call.call_plan.parameters.last_mut().unwrap() = target.destination.clone();
                    }
                }
                if matches!(mutation, "parameter access" | "parameter type") {
                    let parameter = &mut changed.structural.as_mut().unwrap().parameters[0];
                    if mutation == "parameter access" {
                        parameter.semantic.access = StructuralAccess::SharedBorrow;
                        parameter.target.access = StructuralAccess::SharedBorrow;
                    } else {
                        parameter.semantic.structural_type = StructuralTypeId::new(2).unwrap();
                        parameter.target.structural_type = StructuralTypeId::new(2).unwrap();
                    }
                }
                assert!(
                    build(
                        0,
                        &changed,
                        native,
                        &constraints,
                        environment.physical(),
                        environment.constraints()
                    )
                    .is_err(),
                    "{native:?}/{storage:?}: producer accepted {mutation}"
                );
                let mut matching_contracts = selected.clone();
                matching_contracts.structural = changed.structural.clone();
                matching_contracts.calls[0].call = call_mut(&mut changed).clone();
                assert!(
                    validate(&changed, &matching_contracts).is_err(),
                    "{native:?}/{storage:?}: replay accepted {mutation}"
                );
            }
        }
    }
}
