use register_model::{RegisterOperandAccess, RegisterUnitId, RegisterWriteSemantics};

use crate::PostAllocationMachinePlan;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PostAllocationMachineIdentity([u8; 32]);

impl PostAllocationMachineIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

pub fn post_allocation_machine_identity(
    plan: &PostAllocationMachinePlan,
) -> PostAllocationMachineIdentity {
    post_allocation_machine_identity_with_domain(
        plan,
        b"omega.terminal-postallocation-machine.v20\0",
    )
}

fn post_allocation_machine_identity_with_domain(
    plan: &PostAllocationMachinePlan,
    domain: &[u8],
) -> PostAllocationMachineIdentity {
    let mut digest = crate::digest_encoder::DomainDigestEncoder::new(domain);
    digest.raw(&encode_terminal_post_allocation_machine_content(plan));
    PostAllocationMachineIdentity::from_bytes(digest.finish().into())
}

pub(crate) fn encode_terminal_post_allocation_machine_content(
    plan: &PostAllocationMachinePlan,
) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&plan.selected.bytes());
    bytes.extend_from_slice(&plan.effects.bytes());
    bytes.extend_from_slice(&plan.ranges.bytes());
    bytes.extend_from_slice(&plan.legality.bytes());
    bytes.extend_from_slice(&plan.homes.bytes());
    bytes.extend_from_slice(&plan.post_allocation_manifest.bytes());
    bytes.extend_from_slice(&plan.target.identity_bytes());
    bytes.extend_from_slice(&plan.register_environment.bytes());
    bytes.extend_from_slice(&plan.physical_register_model.bytes());
    bytes.extend_from_slice(&plan.register_constraints.bytes());
    bytes.extend_from_slice(&plan.machine_effect_catalog.bytes());
    bytes.push(match plan.choice_rule {
        crate::MachineAlternativeChoiceRule::UniqueApplicableInCatalogOrderV1 => 0,
    });
    encode_len(&mut bytes, plan.functions.len());
    for function in &plan.functions {
        crate::codec_bytes::push_encoded(&mut bytes, function.machine.get().to_le_bytes());
        encode_len(&mut bytes, function.local_storage_slots.len());
        for slot in &function.local_storage_slots {
            slot.id.encode_identity(&mut bytes);
            crate::codec_bytes::push_encoded(&mut bytes, slot.byte_size.to_le_bytes());
            crate::codec_bytes::push_encoded(&mut bytes, slot.alignment.to_le_bytes());
        }
        encode_len(&mut bytes, function.outgoing_arguments.len());
        for slot in &function.outgoing_arguments {
            slot.id.encode_identity(&mut bytes);
            crate::codec_bytes::push_encoded(&mut bytes, slot.byte_size.to_le_bytes());
            crate::codec_bytes::push_encoded(&mut bytes, slot.alignment.to_le_bytes());
            crate::codec_bytes::push_encoded(&mut bytes, slot.abi_stack_byte_offset.to_le_bytes());
        }
        encode_len(&mut bytes, function.blocks.len());
        for block in &function.blocks {
            crate::codec_bytes::push_encoded(&mut bytes, block.block.0.to_le_bytes());
            encode_len(&mut bytes, block.instructions.len());
            for instruction in &block.instructions {
                encode_instruction(&mut bytes, instruction);
            }
        }
    }

    bytes
}

fn encode_instruction(bytes: &mut Vec<u8>, instruction: &crate::PostAllocationMachineInstruction) {
    crate::codec_bytes::push_encoded(bytes, instruction.instruction.0.to_le_bytes());
    selected_instructions::encode_machine_alternative_identity(bytes, &instruction.alternative);
    encode_len(bytes, instruction.operands.len());
    for operand in &instruction.operands {
        crate::codec_bytes::push_encoded(bytes, operand.operand.to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, operand.virtual_register.0.to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, operand.class.0.to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, operand.view.0.to_le_bytes());
        bytes.push(match operand.access {
            RegisterOperandAccess::Use => 0,
            RegisterOperandAccess::Def => 1,
            RegisterOperandAccess::UseDef => 2,
        });
        encode_units(bytes, &operand.storage_units);
        encode_units(bytes, &operand.read_units);
        encode_units(bytes, &operand.write_units);
        match operand.write_semantics {
            None => bytes.push(0),
            Some(semantics) => {
                bytes.push(1);
                bytes.push(match semantics {
                    RegisterWriteSemantics::ExactView => 0,
                    RegisterWriteSemantics::PreservesUnwritten => 1,
                    RegisterWriteSemantics::ZeroExtendsParent => 2,
                    RegisterWriteSemantics::ZeroExtendsWithinUnit => 3,
                    RegisterWriteSemantics::Discards => 4,
                    RegisterWriteSemantics::InstructionDefined => 5,
                });
            }
        }
    }
    match instruction.address {
        Some(crate::PhysicalAddressOperation::PlacedScalarAccess {
            base_operand,
            byte_offset,
            form,
            contract,
        }) => {
            bytes.push(16);
            bytes.extend_from_slice(&base_operand.to_le_bytes());
            bytes.extend_from_slice(&byte_offset.to_le_bytes());
            bytes.push(form.machine_payload());
            bytes.extend_from_slice(&contract);
        }
        Some(crate::PhysicalAddressOperation::SaveFloatingControl { slot }) => {
            bytes.push(14);
            slot.encode_identity(bytes);
        }
        Some(crate::PhysicalAddressOperation::RestoreFloatingControl { slot }) => {
            bytes.push(15);
            slot.encode_identity(bytes);
        }
        Some(crate::PhysicalAddressOperation::Store {
            base_operand,
            byte_offset,
            byte_size,
        }) => {
            bytes.push(6);
            crate::codec_bytes::push_encoded(bytes, base_operand.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, byte_offset.to_le_bytes());
            bytes.push(byte_size);
        }
        Some(crate::PhysicalAddressOperation::AddressOffset {
            base_operand,
            byte_offset,
        }) => {
            bytes.push(7);
            crate::codec_bytes::push_encoded(bytes, base_operand.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, byte_offset.to_le_bytes());
        }
        Some(crate::PhysicalAddressOperation::Load8Indexed {
            base_operand,
            index_operand,
        }) => {
            bytes.push(4);
            crate::codec_bytes::push_encoded(bytes, base_operand.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, index_operand.to_le_bytes());
        }
        Some(crate::PhysicalAddressOperation::HostedReadByte { slot }) => {
            bytes.push(9);
            slot.encode_identity(bytes);
        }
        Some(crate::PhysicalAddressOperation::HostedWriteByteI32 { slot }) => {
            bytes.push(5);
            slot.encode_identity(bytes);
        }
        None => bytes.push(0),
        Some(crate::PhysicalAddressOperation::LoadPacked {
            base_operand,
            byte_offset,
            width,
        }) => {
            bytes.push(12);
            crate::codec_bytes::push_encoded(bytes, base_operand.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, byte_offset.to_le_bytes());
            bytes.push(width.byte_size());
        }
        Some(crate::PhysicalAddressOperation::StorePacked {
            base_operand,
            byte_offset,
            width,
        }) => {
            bytes.push(13);
            crate::codec_bytes::push_encoded(bytes, base_operand.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, byte_offset.to_le_bytes());
            bytes.push(width.byte_size());
        }
        Some(crate::PhysicalAddressOperation::Load8 {
            base_operand,
            byte_offset,
        }) => {
            bytes.push(10);
            crate::codec_bytes::push_encoded(bytes, base_operand.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, byte_offset.to_le_bytes());
        }
        Some(crate::PhysicalAddressOperation::Load16 {
            base_operand,
            byte_offset,
        }) => {
            bytes.push(11);
            crate::codec_bytes::push_encoded(bytes, base_operand.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, byte_offset.to_le_bytes());
        }
        Some(crate::PhysicalAddressOperation::Load32 {
            base_operand,
            byte_offset,
        }) => {
            bytes.push(8);
            crate::codec_bytes::push_encoded(bytes, base_operand.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, byte_offset.to_le_bytes());
        }
        Some(crate::PhysicalAddressOperation::Load64 {
            base_operand,
            byte_offset,
        }) => {
            bytes.push(1);
            crate::codec_bytes::push_encoded(bytes, base_operand.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, byte_offset.to_le_bytes());
        }
        Some(crate::PhysicalAddressOperation::Store64 { slot, byte_offset })
        | Some(crate::PhysicalAddressOperation::FrameAddress { slot, byte_offset }) => {
            bytes.push(
                if matches!(
                    instruction.address,
                    Some(crate::PhysicalAddressOperation::Store64 { .. })
                ) {
                    2
                } else {
                    3
                },
            );
            match slot {
                selected_instructions::FrameStorageSlotId::Incoming {
                    parameter_index,
                    abi_stack_byte_offset,
                } => {
                    bytes.push(2);
                    crate::codec_bytes::push_encoded(bytes, parameter_index.to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, abi_stack_byte_offset.to_le_bytes());
                }
                selected_instructions::FrameStorageSlotId::Outgoing(slot) => {
                    bytes.push(match slot.role {
                        selected_instructions::OutgoingArgumentSlotRole::Argument => 0,
                        selected_instructions::OutgoingArgumentSlotRole::ValueCopy => 3,
                    });
                    slot.invocation_site.encode_identity(bytes);
                    crate::codec_bytes::push_encoded(bytes, slot.argument_index.to_le_bytes());
                }
                selected_instructions::FrameStorageSlotId::Local(slot) => {
                    bytes.push(1);
                    slot.encode_identity(bytes);
                }
            }
            crate::codec_bytes::push_encoded(bytes, byte_offset.to_le_bytes());
        }
    }
    encode_units(bytes, &instruction.implicit_unit_uses);
    encode_units(bytes, &instruction.implicit_unit_defs);
    encode_units(bytes, &instruction.implicit_unit_clobbers);
    encode_units(bytes, &instruction.unit_uses);
    encode_units(bytes, &instruction.unit_defs);
    encode_units(bytes, &instruction.unit_clobbers);
}

fn encode_units(bytes: &mut Vec<u8>, units: &[RegisterUnitId]) {
    encode_len(bytes, units.len());
    for unit in units {
        crate::codec_bytes::push_encoded(bytes, unit.0.to_le_bytes());
    }
}

fn encode_len(bytes: &mut Vec<u8>, length: usize) {
    bytes.extend_from_slice(
        &u64::try_from(length)
            .expect("in-memory artifact length fits u64")
            .to_le_bytes(),
    );
}
