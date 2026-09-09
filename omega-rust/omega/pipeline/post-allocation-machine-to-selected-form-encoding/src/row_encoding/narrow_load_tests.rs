//! Current target catalogs, physical addresses and independent byte replay agree on narrow loads.
use super::*;
use physical_instructions::{
    PhysicalAddressOperation, PhysicalOperandFootprint, PostAllocationMachineFunction,
};
use register_model::{RegisterOperandAccess, validate_physical_register_model};
use selected_instructions::{
    MachineSemanticKind, SelectedInstructionProvenance, SelectedOperand, VirtualRegisterId,
};
use semantic_vocabulary::MachineId;

#[test]
fn narrow_loads_join_catalog_address_encoder_and_receiving_replay_on_four_targets() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let x86 = target.architecture == Architecture::X86_64;
        let physical = validate_physical_register_model(if x86 {
            isa_x86_64::x86_64_physical_register_model()
        } else {
            isa_aarch64::aarch64_physical_register_model()
        })
        .unwrap();
        let constraints = if x86 {
            isa_x86_64::validate_x86_64_register_constraint_catalog(
                isa_x86_64::x86_64_register_constraint_catalog(&physical),
                &physical,
            )
            .unwrap()
        } else {
            isa_aarch64::validate_aarch64_register_constraint_catalog(
                isa_aarch64::aarch64_register_constraint_catalog(&physical),
                &physical,
            )
            .unwrap()
        };
        let catalog = if x86 {
            isa_x86_64::x86_64_machine_effect_catalog(target, &constraints).unwrap()
        } else {
            isa_aarch64::aarch64_machine_effect_catalog(target, &constraints).unwrap()
        };
        let catalog = if x86 {
            isa_x86_64::validate_x86_64_machine_effect_catalog(target, &constraints, catalog)
                .unwrap()
        } else {
            isa_aarch64::validate_aarch64_machine_effect_catalog(target, &constraints, catalog)
                .unwrap()
        };
        for (semantic, kind, address, width) in [
            (
                MachineSemanticKind::Load8,
                SelectedInstructionKind::Load8 { byte_offset: 2 },
                PhysicalAddressOperation::Load8 {
                    base_operand: 0,
                    byte_offset: 2,
                },
                1,
            ),
            (
                MachineSemanticKind::Load16,
                SelectedInstructionKind::Load16 { byte_offset: 2 },
                PhysicalAddressOperation::Load16 {
                    base_operand: 0,
                    byte_offset: 2,
                },
                2,
            ),
        ] {
            let declaration = catalog
                .catalog()
                .declarations
                .iter()
                .find(|row| row.semantic == semantic)
                .unwrap();
            let mut selected = SelectedInstruction {
                id: SelectedInstructionId(1),
                kind,
                constraint: declaration.constraint,
                operands: Vec::new(),
                implicit_uses: Vec::new(),
                implicit_defs: Vec::new(),
                clobbers: Vec::new(),
                provenance: SelectedInstructionProvenance::default(),
            };
            let mut machine = PostAllocationMachineInstruction {
                instruction: selected.id,
                alternative: declaration.alternatives[0].clone(),
                operands: Vec::new(),
                address: Some(address),
                implicit_unit_uses: Vec::new(),
                implicit_unit_defs: Vec::new(),
                implicit_unit_clobbers: Vec::new(),
                unit_uses: Vec::new(),
                unit_defs: Vec::new(),
                unit_clobbers: Vec::new(),
            };
            for (operand, name, access) in [
                (
                    0,
                    if x86 { "r12" } else { "x1" },
                    RegisterOperandAccess::Use,
                ),
                (
                    1,
                    if x86 { "r11" } else { "x2" },
                    RegisterOperandAccess::Def,
                ),
            ] {
                let view = physical.model().view_named(name).unwrap();
                let virtual_register = VirtualRegisterId(u32::from(operand));
                let reads = access == RegisterOperandAccess::Use;
                selected.operands.push(SelectedOperand {
                    operand,
                    virtual_register,
                    access,
                    class: view.class,
                    fixed_view: None,
                    tied_to: None,
                    early_clobber: false,
                });
                machine.operands.push(PhysicalOperandFootprint {
                    operand,
                    virtual_register,
                    class: view.class,
                    view: view.id,
                    access,
                    storage_units: view.units.clone(),
                    read_units: if reads {
                        view.units.clone()
                    } else {
                        Vec::new()
                    },
                    write_units: if reads {
                        Vec::new()
                    } else {
                        view.write_units.clone()
                    },
                    write_semantics: (!reads).then_some(view.write_semantics),
                });
                if reads {
                    machine.unit_uses.extend_from_slice(&view.units);
                } else {
                    machine.unit_defs.extend_from_slice(&view.write_units);
                }
            }
            let function = PostAllocationMachineFunction {
                machine: MachineId::new(1).unwrap(),
                outgoing_arguments: Vec::new(),
                local_storage_slots: Vec::new(),
                blocks: Vec::new(),
            };
            let resolved = crate::frame_address::resolve(&function, None, &machine).unwrap();
            crate::frame_address::validate_address(&function, None, &machine, resolved).unwrap();
            let row = encode_row(target, &selected, &machine, &physical, resolved).unwrap();
            crate::validation::row::validate(target, &selected, &machine, &physical, &row).unwrap();
            assert_eq!(
                machine.alternative.encoded.memory,
                selected_instructions::MachineEncodedMemoryEffect::ReadPointerV1 {
                    pointer_operand: 0,
                    byte_count: width,
                }
            );
            for mutation in 0..5 {
                let mut changed_machine = machine.clone();
                let mut changed_row = row.clone();
                match mutation {
                    0 => {
                        changed_machine.alternative.encoded.memory =
                            selected_instructions::MachineEncodedMemoryEffect::ReadPointerV1 {
                                pointer_operand: 0,
                                byte_count: 8,
                            }
                    }
                    1 => changed_row.address.as_mut().unwrap().displacement = 4,
                    2 => changed_machine.operands[0].view = machine.operands[1].view,
                    3 => {
                        let SelectedFormEncodingState::Encoded { bytes, .. } =
                            &mut changed_row.state
                        else {
                            panic!("load bytes");
                        };
                        bytes[0] ^= 1;
                    }
                    _ => {
                        changed_machine.alternative.key.family =
                            selected_instructions::MachineAlternativeFamily::Load64
                    }
                }
                assert!(
                    crate::validation::row::validate(
                        target,
                        &selected,
                        &changed_machine,
                        &physical,
                        &changed_row
                    )
                    .is_err(),
                    "{target:?} {kind:?} mutation {mutation}"
                );
            }
            let mut changed = resolved.unwrap();
            changed.symbolic = PhysicalAddressOperation::Load64 {
                base_operand: 0,
                byte_offset: 2,
            };
            assert!(
                crate::frame_address::validate_address(&function, None, &machine, Some(changed))
                    .is_err()
            );
        }
    }
}
