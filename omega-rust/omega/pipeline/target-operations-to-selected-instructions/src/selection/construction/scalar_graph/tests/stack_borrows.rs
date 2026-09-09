//! Incoming pointer-bit transport controls over raw selection and independent replay.
use super::*;
use calling_conventions::ValuePlacement;
use selected_instructions::FrameStorageSlotId;

mod narrow_unsigned;

#[test]
fn ieee_stack_fragments_replay_exact_width_source_and_outgoing_slot() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::macos_arm64(),
    ] {
        for format in [
            semantic_vocabulary::IeeeFloatFormat::Binary32,
            semantic_vocabulary::IeeeFloatFormat::Binary64,
        ] {
            let environment =
                register_environment::baseline_target_register_environment(target).unwrap();
            let scalar_type = ScalarType::IeeeFloat(format);
            let shape = crate::selection::scalar_call_abi::scalar_shape(scalar_type).unwrap();
            let mut source = projected_borrows::projected_call(target);
            let root_shape = source.call_plan.parameters[0].shape;
            source.call_plan = evaluate_call_plan(
                CallingPolicy::native_for_target(target),
                &CallSignature {
                    parameters: std::iter::repeat_n(shape, 9).chain([root_shape]).collect(),
                    result: None,
                },
            )
            .unwrap();
            source.parameters = source.call_plan.parameters[..9]
                .iter()
                .enumerate()
                .map(|(position, placement)| LegalizedScalarParameter {
                    value: ValueId::new(100 + position as u64).unwrap(),
                    scalar_type,
                    definition_site: ValueDefinitionSite::FunctionParameter(position as u32),
                    placement: placement.clone(),
                })
                .collect();
            let value = source.parameters[8].value;
            source.structural.as_mut().unwrap().parameters[0]
                .target
                .placement = source.call_plan.parameters[9].clone();
            let LegalizedScalarInstructionKind::Call(call) =
                &mut source.blocks[0].instructions[0].kind
            else {
                panic!("call")
            };
            let mut reference = call.arguments[0].clone();
            call.call_plan = evaluate_call_plan(
                CallingPolicy::native_for_target(target),
                &CallSignature {
                    parameters: std::iter::repeat_n(shape, 9)
                        .chain([reference.placement().shape])
                        .collect(),
                    result: None,
                },
            )
            .unwrap();
            let LegalizedScalarArgument::Structural {
                target: argument, ..
            } = &mut reference
            else {
                panic!("reference")
            };
            argument.source = source.call_plan.parameters[9].clone().into();
            argument.destination = call.call_plan.parameters[9].clone();
            call.arguments = call.call_plan.parameters[..9]
                .iter()
                .map(|placement| LegalizedScalarArgument::Scalar {
                    source: value,
                    placement: placement.clone(),
                })
                .chain([reference])
                .collect();
            let constraints = SelectedSelectionConstraints {
                keys: environment.selected_keys(),
                projected_structural_call: None,
                fixed_inputs: Vec::new(),
            };
            let selected = build(
                0,
                &source,
                target,
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
                    target,
                    &constraints,
                    environment.physical(),
                    environment.constraints(),
                )
            };
            validate(&selected).unwrap();
            let address_rows = selected.virtual_registers.iter().filter(|register|
                matches!(register.origin, VirtualRegisterOrigin::ScalarAbiAddress { source_value, .. } if source_value == value)).collect::<Vec<_>>();
            assert!(
                address_rows.len() >= 2,
                "incoming and outgoing scalar stack addresses"
            );
            for address in address_rows {
                assert_eq!(
                    address.scalar_type,
                    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap())
                );
                let mut wrong_type = selected.clone();
                wrong_type.virtual_registers[address.id.0 as usize].scalar_type = scalar_type;
                assert!(
                    validate(&wrong_type).is_err(),
                    "address cannot inherit IEEE payload type"
                );
                let mut wrong_origin = selected.clone();
                let VirtualRegisterOrigin::ScalarAbiAddress {
                    instruction,
                    source_value,
                } = address.origin
                else {
                    unreachable!()
                };
                wrong_origin.virtual_registers[address.id.0 as usize].origin =
                    VirtualRegisterOrigin::InstructionResult {
                        instruction,
                        source_value,
                    };
                assert!(
                    validate(&wrong_origin).is_err(),
                    "address cannot impersonate scalar payload"
                );
            }
            let load = selected.blocks[0]
                .instructions
                .iter()
                .position(|row| {
                    matches!(
                        row.kind,
                        SelectedInstructionKind::Load32 { .. }
                            | SelectedInstructionKind::Load64 { .. }
                    ) && row.provenance.values == vec![value]
                })
                .unwrap();
            assert_eq!(
                selected.blocks[0].instructions[load].kind,
                if shape.byte_size == 4 {
                    SelectedInstructionKind::Load32 { byte_offset: 0 }
                } else {
                    SelectedInstructionKind::Load64 { byte_offset: 0 }
                }
            );
            let mut wrong_width = selected.clone();
            wrong_width.blocks[0].instructions[load].kind = if shape.byte_size == 4 {
                SelectedInstructionKind::Load64 { byte_offset: 0 }
            } else {
                SelectedInstructionKind::Load32 { byte_offset: 0 }
            };
            assert!(validate(&wrong_width).is_err());
            let mut wrong_source = selected.clone();
            wrong_source.blocks[0].instructions[load].provenance.values[0] =
                source.parameters[0].value;
            assert!(validate(&wrong_source).is_err());
            let mut wrong_slot = selected.clone();
            wrong_slot
                .outgoing_arguments
                .iter_mut()
                .find(|slot| slot.id.argument_index == 8)
                .unwrap()
                .abi_stack_byte_offset += 4;
            assert!(validate(&wrong_slot).is_err());
        }
    }
}

fn incoming_stack_call(
    target: target::NativeTarget,
    scalar_count: usize,
) -> LegalizedScalarFunction {
    let mut source = projected_borrows::projected_call(target);
    let reference = source.call_plan.parameters[0].shape;
    source.call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: std::iter::repeat_n(ValueShape::integer(8, 8), scalar_count)
                .chain(std::iter::once(reference))
                .collect(),
            result: None,
        },
    )
    .unwrap();
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    source.parameters = source.call_plan.parameters[..scalar_count]
        .iter()
        .enumerate()
        .map(|(position, placement)| LegalizedScalarParameter {
            value: ValueId::new(100 + position as u64).unwrap(),
            scalar_type: ScalarType::Integer(integer),
            definition_site: ValueDefinitionSite::FunctionParameter(position as u32),
            placement: placement.clone(),
        })
        .collect();
    let placement = source.call_plan.parameters[scalar_count].clone();
    source.structural.as_mut().unwrap().parameters[0]
        .target
        .placement = placement.clone();
    let LegalizedScalarInstructionKind::Call(call) = &mut source.blocks[0].instructions[0].kind
    else {
        panic!("projected call fixture")
    };
    let LegalizedScalarArgument::Structural { target, .. } = &mut call.arguments[0] else {
        panic!("structural argument")
    };
    target.source = placement.into();
    source
}

fn stack_offset(placement: &ValuePlacement) -> u32 {
    match placement.locations.as_slice() {
        [
            ValueLocation::Stack {
                stack_byte_offset,
                value_byte_offset: 0,
                byte_size: 8,
                alignment: 8,
            },
        ] => *stack_byte_offset,
        [
            ValueLocation::Indirect {
                pointer:
                    IndirectPointerLocation::Stack {
                        stack_byte_offset,
                        alignment: 8,
                    },
                copy_stack_byte_offset: None,
                ..
            },
        ] => *stack_byte_offset,
        _ => panic!("fixture requires an incoming stack pointer"),
    }
}

#[test]
fn incoming_stack_borrow_replay_retains_native_ordinal_pointer_load_and_fuel() {
    for (target, capacity) in [
        (target::NativeTarget::linux_x64(), 6),
        (target::NativeTarget::linux_arm64(), 8),
        (target::NativeTarget::windows_x64(), 4),
        (target::NativeTarget::macos_arm64(), 8),
    ] {
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let constraints = SelectedSelectionConstraints {
            keys: environment.selected_keys(),
            projected_structural_call: None,
            fixed_inputs: Vec::new(),
        };
        for extra_stack_scalars in [0, 1, 3] {
            let native_ordinal = capacity + extra_stack_scalars;
            let source = incoming_stack_call(target, native_ordinal);
            let offset = stack_offset(&source.call_plan.parameters[native_ordinal]);
            if extra_stack_scalars != 0 {
                assert!(
                    offset > 0,
                    "non-first stack argument must retain a nonzero ABI offset"
                );
            }
            let construct = |input: &LegalizedScalarFunction| {
                build(
                    0,
                    input,
                    target,
                    &constraints,
                    environment.physical(),
                    environment.constraints(),
                )
            };
            let selected =
                construct(&source).expect("stack reference enters as original pointer bits");
            let validate = |input: &LegalizedScalarFunction, candidate: &SelectedFunction| {
                crate::selection::validation::scalar_graph::validate(
                    0,
                    input,
                    candidate,
                    target,
                    &constraints,
                    environment.physical(),
                    environment.constraints(),
                )
            };
            validate(&source, &selected).unwrap();
            let rows = &selected.blocks[0].instructions;
            let incoming = rows
                .iter()
                .position(|row| {
                    matches!(
                        row.kind,
                        SelectedInstructionKind::FrameAddress {
                            slot: FrameStorageSlotId::Incoming { .. },
                            ..
                        }
                    )
                })
                .expect("incoming ABI address");
            assert_eq!(
                rows[incoming].kind,
                SelectedInstructionKind::FrameAddress {
                    slot: FrameStorageSlotId::Incoming {
                        parameter_index: native_ordinal as u32,
                        abi_stack_byte_offset: offset,
                    },
                    byte_offset: 0,
                }
            );
            assert_eq!(
                rows[incoming + 1].kind,
                SelectedInstructionKind::Load64 { byte_offset: 0 }
            );
            assert_eq!(
                rows[incoming].operands[0].virtual_register,
                rows[incoming + 1].operands[0].virtual_register
            );
            let pointer = rows[incoming + 1].operands[1].virtual_register;
            let incoming_address = rows[incoming].operands[0].virtual_register;
            assert!(matches!(
                selected.virtual_registers[pointer.0 as usize].origin,
                VirtualRegisterOrigin::AbiTransport { byte_offset: 0, .. }
            ));
            let projected = rows
                .iter()
                .find(|row| {
                    matches!(
                        row.kind,
                        SelectedInstructionKind::AddressOffset { byte_offset: 2 }
                    )
                })
                .unwrap();
            assert_eq!(projected.operands[0].virtual_register, pointer);
            for row in [&rows[incoming], &rows[incoming + 1], projected] {
                assert_eq!(row.provenance, SelectedInstructionProvenance::default());
            }
            assert!(selected.outgoing_arguments.is_empty());
            assert!(
                selected.local_storage_slots.is_empty(),
                "no referent snapshot is manufactured"
            );
            let call = rows
                .iter()
                .find(|row| matches!(row.kind, SelectedInstructionKind::CallUnit { .. }))
                .unwrap();
            assert_eq!(call.provenance.fuel, source.blocks[0].instructions[0].fuel);

            for mutation in 0..11 {
                let mut changed = selected.clone();
                let rows = &mut changed.blocks[0].instructions;
                match mutation {
                    0 => {
                        rows[incoming].kind = SelectedInstructionKind::FrameAddress {
                            slot: FrameStorageSlotId::Incoming {
                                parameter_index: 0,
                                abi_stack_byte_offset: offset,
                            },
                            byte_offset: 0,
                        }
                    }
                    1 => {
                        rows[incoming].kind = SelectedInstructionKind::FrameAddress {
                            slot: FrameStorageSlotId::Incoming {
                                parameter_index: native_ordinal as u32,
                                abi_stack_byte_offset: offset + 8,
                            },
                            byte_offset: 0,
                        }
                    }
                    2 => {
                        rows[incoming].kind = SelectedInstructionKind::FrameAddress {
                            slot: FrameStorageSlotId::Incoming {
                                parameter_index: native_ordinal as u32,
                                abi_stack_byte_offset: offset,
                            },
                            byte_offset: 8,
                        }
                    }
                    3 => {
                        rows[incoming].kind = SelectedInstructionKind::FrameAddress {
                            slot: FrameStorageSlotId::Outgoing(
                                selected_instructions::OutgoingArgumentSlotId {
                                    operation: source.blocks[0].instructions[0].operation,
                                    argument_index: 0,
                                },
                            ),
                            byte_offset: 0,
                        }
                    }
                    4 => {
                        rows[incoming + 1].kind = SelectedInstructionKind::Load64 { byte_offset: 8 }
                    }
                    5 => rows[incoming + 1].kind = SelectedInstructionKind::CopyI64,
                    6 => rows[incoming + 1].operands.swap(0, 1),
                    7 => {
                        rows[incoming + 1].provenance.fuel =
                            source.blocks[0].instructions[0].fuel.clone()
                    }
                    8 => {
                        changed.virtual_registers[pointer.0 as usize].origin =
                            VirtualRegisterOrigin::StructuralParameter {
                                place: source.structural.as_ref().unwrap().parameters[0]
                                    .semantic
                                    .place,
                                parameter_index: 0,
                            }
                    }
                    9 => {
                        rows.iter_mut()
                            .find(|row| {
                                matches!(row.kind, SelectedInstructionKind::AddressOffset { .. })
                            })
                            .unwrap()
                            .operands[0]
                            .virtual_register = incoming_address
                    }
                    _ => {
                        changed.virtual_registers[pointer.0 as usize].definition_site =
                            Some(ValueDefinitionSite::FunctionParameter(0))
                    }
                }
                assert!(
                    validate(&source, &changed).is_err(),
                    "incoming mutation {mutation}, native ordinal {native_ordinal}"
                );
            }

            let mut changed = source.clone();
            let placement = &mut changed.structural.as_mut().unwrap().parameters[0]
                .target
                .placement;
            match &mut placement.locations[0] {
                ValueLocation::Stack {
                    stack_byte_offset, ..
                } => *stack_byte_offset += 8,
                ValueLocation::Indirect {
                    pointer:
                        IndirectPointerLocation::Stack {
                            stack_byte_offset, ..
                        },
                    ..
                } => *stack_byte_offset += 8,
                _ => panic!("stack pointer fixture"),
            }
            assert!(
                construct(&changed).is_err(),
                "source location must match the exact native signature"
            );
            assert!(validate(&changed, &selected).is_err());
        }
    }
}
