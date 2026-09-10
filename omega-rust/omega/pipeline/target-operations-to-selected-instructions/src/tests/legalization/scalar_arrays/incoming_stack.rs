//! Inline incoming bytes retain ABI coordinates, exact widths and place identity.
use super::*;
use selected_instructions::{
    FrameStorageSlotId, SelectedInstructionKind as Instruction, VirtualRegisterOrigin,
};

#[test]
fn incoming_stack_array_replay_rejects_transport_and_charge_substitution() {
    for native in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
    ] {
        for array_length in [3, 11] {
            let (mut source, _, _) = fixture(0);
            let StructuralTypeShape::FixedArray { length, .. } =
                &mut source.structural_types[0].shape
            else {
                panic!("array");
            };
            *length = array_length;
            source.structural_types[1].shape =
                StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
                    semantic_vocabulary::IntegerType::new(
                        semantic_vocabulary::IntegerSign::Unsigned,
                        8,
                    )
                    .unwrap(),
                ));
            let integer = semantic_vocabulary::IntegerType::new(
                semantic_vocabulary::IntegerSign::Unsigned,
                64,
            )
            .unwrap();
            let function = &mut source.functions[0];
            function.operations.remove(0);
            function.parameters = (0..8)
                .map(|ordinal| abstract_operations::AbstractParameter {
                    value: ValueId::new(10 + ordinal).unwrap(),
                    scalar_type: ScalarType::Integer(integer),
                })
                .collect();
            function
                .structural_parameters
                .push(terminal_psi::StructuralParameterDeclaration {
                    place: PlaceId::new(1).unwrap(),
                    position: 0,
                    is_self: false,
                    structural_type: StructuralTypeId::new(1).unwrap(),
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    access: terminal_psi::StructuralAccess::Owned,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                });
            let target = abstract_operations_to_target_operations::lower_to_target_operations(
                &source, native,
            )
            .unwrap();
            let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
                &source,
                FuelScheduleIdentity::new(1).unwrap(),
            )
            .unwrap();
            optimization_unit_semantics::validate_psi_optimization_unit(&unit).unwrap();
            let legalized = legalize_target_operations(&target, &source, &unit).unwrap();
            let environment =
                register_environment::baseline_target_register_environment(native).unwrap();
            let constraints = crate::selection_constraints(&legalized, &environment);
            let selected = crate::select_instructions(
                &legalized,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
            .unwrap();
            let function = &selected.plan().functions[0];
            assert!(function.local_storage_slots.is_empty());
            assert!(function.outgoing_arguments.is_empty());
            let instructions = &function.blocks[0].instructions;
            let address_index = instructions
                .iter()
                .position(|row| {
                    matches!(
                        row.kind,
                        Instruction::FrameAddress {
                            slot: FrameStorageSlotId::Incoming {
                                parameter_index: 8,
                                ..
                            },
                            ..
                        }
                    )
                })
                .unwrap();
            let packed_index = instructions
                .iter()
                .position(|row| matches!(row.kind, Instruction::LoadPacked { .. }))
                .unwrap();
            let payload_register = instructions[packed_index].operands[1].virtual_register;
            assert!(
                instructions
                    .iter()
                    .all(|row| row.provenance.fuel.is_empty())
            );
            assert_eq!(
                function
                    .memory_accesses
                    .iter()
                    .map(|row| u64::from(row.byte_count))
                    .sum::<u64>(),
                array_length
            );
            for mutation in 0..7 {
                let mut changed = selected.plan().clone();
                let function = &mut changed.functions[0];
                match mutation {
                    0 | 1 => {
                        let Instruction::FrameAddress {
                            slot:
                                FrameStorageSlotId::Incoming {
                                    parameter_index,
                                    abi_stack_byte_offset,
                                },
                            ..
                        } = &mut function.blocks[0].instructions[address_index].kind
                        else {
                            panic!("incoming");
                        };
                        if mutation == 0 {
                            *parameter_index = 7;
                        } else {
                            *abi_stack_byte_offset += 8;
                        }
                    }
                    2 => {
                        let Instruction::LoadPacked { width, .. } =
                            &mut function.blocks[0].instructions[packed_index].kind
                        else {
                            panic!("packed");
                        };
                        *width = selected_instructions::PackedByteWidth::Five;
                    }
                    3 | 4 => {
                        let VirtualRegisterOrigin::AbiTransport {
                            place, byte_offset, ..
                        } = &mut function.virtual_registers[payload_register.0 as usize].origin
                        else {
                            panic!("transport");
                        };
                        if mutation == 3 {
                            *place = PlaceId::new(99).unwrap();
                        } else {
                            *byte_offset += 1;
                        }
                    }
                    5 => function.blocks[0].instructions[packed_index]
                        .provenance
                        .fuel
                        .push(optimization_unit::FuelSettlement {
                            site: optimization_unit::PsiProvenance::Operation(
                                OperationId::new(99).unwrap(),
                            ),
                            units: 1,
                        }),
                    _ => function.memory_accesses[0].byte_count += 1,
                }
                assert!(
                    crate::validate_selected_instructions(
                        &legalized,
                        &constraints,
                        environment.physical(),
                        environment.constraints(),
                        changed
                    )
                    .is_err(),
                    "{native:?} length {array_length} mutation {mutation}"
                );
            }
        }
    }
}
