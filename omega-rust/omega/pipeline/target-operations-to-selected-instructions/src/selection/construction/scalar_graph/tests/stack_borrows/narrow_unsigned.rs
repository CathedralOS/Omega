//! Narrow payload widths remain separate from target-selected stack slot spacing.
use super::*;

fn narrow_stack_call(
    target: target::NativeTarget,
    sign: IntegerSign,
    bits: u16,
) -> LegalizedScalarFunction {
    let scalar_type = ScalarType::Integer(IntegerType::new(sign, bits).unwrap());
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
    let LegalizedScalarInstructionKind::Call(call) = &mut source.blocks[0].instructions[0].kind
    else {
        panic!("projected call");
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
        panic!("structural argument");
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
    source
}

#[test]
fn narrow_unsigned_stack_fragments_preserve_payload_and_reject_substitutions() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
        target::NativeTarget::windows_x64(),
    ] {
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let constraints = SelectedSelectionConstraints {
            keys: environment.selected_keys(),
            projected_structural_call: None,
            fixed_inputs: Vec::new(),
        };
        for bits in [8, 16] {
            let source = narrow_stack_call(target, IntegerSign::Unsigned, bits);
            let value = source.parameters[8].value;
            let shape = source.parameters[8].placement.shape;
            let ValueLocation::Stack {
                stack_byte_offset: incoming_offset,
                ..
            } = source.parameters[8].placement.locations[0]
            else {
                panic!("ninth scalar must enter on the stack");
            };
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
            let selected = construct(&source).expect("narrow stack argument and entry");
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
                            slot: FrameStorageSlotId::Incoming {
                                parameter_index: 8,
                                ..
                            },
                            ..
                        }
                    )
                })
                .expect("exact incoming scalar home");
            assert_eq!(
                rows[incoming].kind,
                SelectedInstructionKind::FrameAddress {
                    slot: FrameStorageSlotId::Incoming {
                        parameter_index: 8,
                        abi_stack_byte_offset: incoming_offset,
                    },
                    byte_offset: 0,
                }
            );
            assert_eq!(
                rows[incoming + 1].kind,
                if bits == 8 {
                    SelectedInstructionKind::Load8 { byte_offset: 0 }
                } else {
                    SelectedInstructionKind::Load16 { byte_offset: 0 }
                }
            );
            assert_eq!(rows[incoming].provenance.values, [value]);
            assert_eq!(rows[incoming + 1].provenance.values, [value]);
            assert!(rows[incoming].provenance.fuel.is_empty());
            assert!(rows[incoming + 1].provenance.fuel.is_empty());
            let loaded = rows[incoming + 1].operands[1].virtual_register;
            assert_eq!(
                selected.virtual_registers[loaded.0 as usize].scalar_type,
                source.parameters[8].scalar_type
            );

            let outgoing = selected
                .outgoing_arguments
                .iter()
                .position(|slot| slot.id.argument_index == 8)
                .expect("ninth outgoing scalar");
            let slot = &selected.outgoing_arguments[outgoing];
            assert_eq!(slot.byte_size, u32::from(shape.byte_size));
            let ValueLocation::Stack {
                stack_byte_offset: outgoing_offset,
                alignment,
                ..
            } = selected.calls[0].call.arguments[8].placement().locations[0]
            else {
                panic!("ninth outgoing scalar must use stack");
            };
            assert_eq!(slot.abi_stack_byte_offset, outgoing_offset);
            assert_eq!(slot.alignment, alignment);
            let outgoing_address = rows
                .iter()
                .position(|row| {
                    row.kind
                        == SelectedInstructionKind::FrameAddress {
                            slot: FrameStorageSlotId::Outgoing(slot.id),
                            byte_offset: 0,
                        }
                })
                .expect("exact outgoing scalar address");
            assert_eq!(
                rows[outgoing_address + 1].kind,
                SelectedInstructionKind::Store {
                    byte_offset: 0,
                    byte_size: shape.byte_size as u8,
                }
            );
            assert_eq!(rows[outgoing_address + 1].provenance.values, [value]);
            assert_eq!(
                rows[outgoing_address + 1].provenance.operations,
                [source.blocks[0].instructions[0].operation]
            );
            assert!(rows[outgoing_address + 1].provenance.fuel.is_empty());

            for mutation in 0..10 {
                let mut changed = selected.clone();
                let rows = &mut changed.blocks[0].instructions;
                match mutation {
                    0 => {
                        rows[incoming].kind = SelectedInstructionKind::FrameAddress {
                            slot: FrameStorageSlotId::Incoming {
                                parameter_index: 0,
                                abi_stack_byte_offset: incoming_offset,
                            },
                            byte_offset: 0,
                        }
                    }
                    1 => {
                        rows[incoming].kind = SelectedInstructionKind::FrameAddress {
                            slot: FrameStorageSlotId::Incoming {
                                parameter_index: 8,
                                abi_stack_byte_offset: incoming_offset + 8,
                            },
                            byte_offset: 0,
                        }
                    }
                    2 => {
                        rows[incoming + 1].kind = SelectedInstructionKind::Load32 { byte_offset: 0 }
                    }
                    3 => rows[incoming + 1].provenance.values[0] = source.parameters[0].value,
                    4 => changed.outgoing_arguments[outgoing].abi_stack_byte_offset += 8,
                    5 => changed.outgoing_arguments[outgoing].byte_size = 4,
                    6 => {
                        rows[outgoing_address + 1].kind = SelectedInstructionKind::Store {
                            byte_offset: 0,
                            byte_size: 4,
                        }
                    }
                    7 => {
                        rows[outgoing_address].kind = SelectedInstructionKind::FrameAddress {
                            slot: FrameStorageSlotId::Incoming {
                                parameter_index: 8,
                                abi_stack_byte_offset: incoming_offset,
                            },
                            byte_offset: 0,
                        }
                    }
                    8 => {
                        rows[outgoing_address + 1].provenance.values[0] = source.parameters[0].value
                    }
                    _ => {
                        rows[outgoing_address + 1].provenance.fuel =
                            source.blocks[0].instructions[0].fuel.clone()
                    }
                }
                assert!(
                    validate(&source, &changed).is_err(),
                    "{target:?}, u{bits}, mutation {mutation}"
                );
            }
            let mut changed_source = source.clone();
            let ValueLocation::Stack {
                stack_byte_offset, ..
            } = &mut changed_source.parameters[8].placement.locations[0]
            else {
                unreachable!();
            };
            *stack_byte_offset += 8;
            assert!(
                construct(&changed_source).is_err(),
                "parameter home must match caller ABI"
            );
            assert!(validate(&changed_source, &selected).is_err());
        }
    }
}

#[test]
fn signed_stack_entry_normalizes_loaded_payload_before_outgoing_calls() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let constraints = SelectedSelectionConstraints {
            keys: environment.selected_keys(),
            projected_structural_call: None,
            fixed_inputs: Vec::new(),
        };
        for (bits, expected, wrong_sign) in [
            (
                8,
                SelectedInstructionKind::SignExtendI8,
                SelectedInstructionKind::ZeroExtendU8,
            ),
            (
                16,
                SelectedInstructionKind::SignExtendI16,
                SelectedInstructionKind::ZeroExtendU16,
            ),
            (
                32,
                SelectedInstructionKind::SignExtendI32,
                SelectedInstructionKind::ZeroExtendU32,
            ),
        ] {
            let source = narrow_stack_call(target, IntegerSign::Signed, bits);
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
            let rows = &selected.blocks[0].instructions;
            let normalization = rows.iter().position(|row| row.kind == expected).unwrap();
            assert_eq!(
                rows[normalization].operands[0].virtual_register,
                rows[normalization - 1].operands[1].virtual_register
            );
            assert_eq!(
                rows[normalization].provenance.values,
                [source.parameters[8].value]
            );
            assert!(rows[normalization].provenance.fuel.is_empty());
            for replacement in [
                SelectedInstructionKind::CopyI64,
                wrong_sign,
                if bits == 8 {
                    SelectedInstructionKind::SignExtendI32
                } else {
                    SelectedInstructionKind::SignExtendI8
                },
            ] {
                let mut changed = selected.clone();
                changed.blocks[0].instructions[normalization].kind = replacement;
                assert!(validate(&changed).is_err());
            }
        }
    }
}
