/*
We encode and decode the version-27 post-allocation machine-plan wire artifact
(`OMGPMX\0\0`) and authenticate its content identity.

The wire format prepends the 8-byte magic marker, little-endian version `27`,
and the 32-byte `PostAllocationMachineIdentity` before the canonical content
stream emitted by `write_terminal_post_allocation_machine_content`. Decoding
rejects unknown versions, invalid closed-tag bytes, trailing bytes, or any
mismatch between the header identity and the recomputed content digest.
*/

#[cfg(test)]
mod tests;

use optimization_core::PostAllocationOptimizationManifestIdentity;
use register_homes::{AllocationLegalityIdentity, RegisterHomeIdentity};
use register_model::{
    PhysicalRegisterModelIdentity, RegisterClassId, RegisterConstraintCatalogIdentity,
    RegisterOperandAccess, RegisterViewId, RegisterWriteSemantics,
    TargetRegisterEnvironmentIdentity,
};
use selected_instructions::selected_instructions::effects::program::encoding as effect_codec;
use selected_instructions::{
    LiveRangeIdentity, MachineEffectCatalogIdentity, PreAllocationMachineEffectIdentity,
    SelectedBlockId, SelectedInstructionId, SelectedInstructionPlanIdentity, VirtualRegisterId,
};
use semantic_vocabulary::MachineId;

use crate::{
    MachineAlternativeChoiceRule, PhysicalOperandFootprint, PostAllocationMachineBlock,
    PostAllocationMachineFunction, PostAllocationMachineIdentity, PostAllocationMachineInstruction,
    PostAllocationMachinePlan, post_allocation_machine_identity,
};

const MAGIC: &[u8; 8] = b"OMGPMX\0\0";
const VERSION: u32 = 27;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostAllocationMachineDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    InvalidField,
    InvalidIdentity,
    TrailingBytes,
}

impl std::fmt::Display for PostAllocationMachineDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid post-allocation machine artifact: {self:?}"
        )
    }
}

impl std::error::Error for PostAllocationMachineDecodeError {}

pub(crate) fn encode_terminal_post_allocation_machine_plan(
    plan: &PostAllocationMachinePlan,
) -> Vec<u8> {
    let mut encoded = Vec::with_capacity(44);
    encoded.extend_from_slice(MAGIC);
    crate::codec_bytes::push_encoded(&mut encoded, VERSION.to_le_bytes());
    encoded.extend_from_slice(&plan.identity.bytes());
    super::identity::write_terminal_post_allocation_machine_content(plan, |chunk| {
        encoded.extend_from_slice(chunk);
    });
    encoded
}

pub(crate) fn decode_terminal_post_allocation_machine_plan(
    encoded: &[u8],
) -> Result<PostAllocationMachinePlan, PostAllocationMachineDecodeError> {
    let mut cursor = effect_codec::Cursor::new(encoded);
    if take(&mut cursor, MAGIC.len())? != MAGIC {
        return Err(PostAllocationMachineDecodeError::WrongMagic);
    }
    let version = u32_field(&mut cursor)?;
    if version != VERSION {
        return Err(PostAllocationMachineDecodeError::UnsupportedVersion(
            version,
        ));
    }
    let identity = PostAllocationMachineIdentity::from_bytes(array(&mut cursor)?);
    let plan = decode_content(&mut cursor, identity)?;
    if cursor.remaining() != 0 {
        return Err(PostAllocationMachineDecodeError::TrailingBytes);
    }
    if plan.identity != post_allocation_machine_identity(&plan) {
        return Err(PostAllocationMachineDecodeError::InvalidIdentity);
    }
    Ok(plan)
}

fn map_field_error(
    error: selected_instructions::PreAllocationMachineEffectDecodeError,
) -> PostAllocationMachineDecodeError {
    match error {
        selected_instructions::PreAllocationMachineEffectDecodeError::Truncated => {
            PostAllocationMachineDecodeError::Truncated
        }
        _ => PostAllocationMachineDecodeError::InvalidField,
    }
}

fn take<'a>(
    cursor: &mut effect_codec::Cursor<'a>,
    count: usize,
) -> Result<&'a [u8], PostAllocationMachineDecodeError> {
    cursor.take(count).map_err(map_field_error)
}

fn array<const N: usize>(
    cursor: &mut effect_codec::Cursor<'_>,
) -> Result<[u8; N], PostAllocationMachineDecodeError> {
    cursor.array().map_err(map_field_error)
}

fn byte(cursor: &mut effect_codec::Cursor<'_>) -> Result<u8, PostAllocationMachineDecodeError> {
    cursor.byte().map_err(map_field_error)
}

fn u16_field(
    cursor: &mut effect_codec::Cursor<'_>,
) -> Result<u16, PostAllocationMachineDecodeError> {
    cursor.u16().map_err(map_field_error)
}

fn u32_field(
    cursor: &mut effect_codec::Cursor<'_>,
) -> Result<u32, PostAllocationMachineDecodeError> {
    cursor.u32().map_err(map_field_error)
}

fn u64_field(
    cursor: &mut effect_codec::Cursor<'_>,
) -> Result<u64, PostAllocationMachineDecodeError> {
    cursor.u64().map_err(map_field_error)
}

fn length(
    cursor: &mut effect_codec::Cursor<'_>,
) -> Result<usize, PostAllocationMachineDecodeError> {
    cursor.length().map_err(map_field_error)
}

fn decode_units(
    cursor: &mut effect_codec::Cursor<'_>,
) -> Result<Vec<register_model::RegisterUnitId>, PostAllocationMachineDecodeError> {
    effect_codec::decode_units(cursor).map_err(map_field_error)
}

fn decode_content(
    cursor: &mut effect_codec::Cursor<'_>,
    identity: PostAllocationMachineIdentity,
) -> Result<PostAllocationMachinePlan, PostAllocationMachineDecodeError> {
    let selected = SelectedInstructionPlanIdentity::from_bytes(array(cursor)?);
    let effects = PreAllocationMachineEffectIdentity::from_bytes(array(cursor)?);
    let ranges = LiveRangeIdentity::from_bytes(array(cursor)?);
    let legality = AllocationLegalityIdentity::from_bytes(array(cursor)?);
    let homes = RegisterHomeIdentity::from_bytes(array(cursor)?);
    let post_allocation_manifest =
        PostAllocationOptimizationManifestIdentity::from_bytes(array(cursor)?);
    let target = effect_codec::decode_target(cursor).map_err(map_field_error)?;
    let register_environment = TargetRegisterEnvironmentIdentity::from_bytes(array(cursor)?);
    let physical_register_model = PhysicalRegisterModelIdentity::from_bytes(array(cursor)?);
    let register_constraints = RegisterConstraintCatalogIdentity::from_bytes(array(cursor)?);
    let machine_effect_catalog = MachineEffectCatalogIdentity::from_bytes(array(cursor)?);
    let choice_rule = match byte(cursor)? {
        0 => MachineAlternativeChoiceRule::UniqueApplicableInCatalogOrderV1,
        _ => return Err(PostAllocationMachineDecodeError::InvalidField),
    };
    let function_count = length(cursor)?;
    let mut functions = Vec::with_capacity(function_count.min(cursor.remaining()));
    for _ in 0..function_count {
        let machine = MachineId::new(u64_field(cursor)?)
            .ok_or(PostAllocationMachineDecodeError::InvalidField)?;
        let local_count = length(cursor)?;
        let mut local_storage_slots = Vec::with_capacity(local_count.min(cursor.remaining()));
        for _ in 0..local_count {
            local_storage_slots.push(selected_instructions::SelectedLocalStorageSlot {
                id: effect_codec::decode_local_storage_slot(cursor).map_err(map_field_error)?,
                byte_size: u32_field(cursor)?,
                alignment: u16_field(cursor)?,
            });
        }
        let slot_count = length(cursor)?;
        let mut outgoing_arguments = Vec::with_capacity(slot_count.min(cursor.remaining()));
        for _ in 0..slot_count {
            outgoing_arguments.push(selected_instructions::SelectedOutgoingArgumentSlot {
                id: selected_instructions::OutgoingArgumentSlotId {
                    invocation_site: semantic_vocabulary::InvocationSite::from_tagged_identity(
                        byte(cursor)?,
                        u64_field(cursor)?,
                    )
                    .ok_or(PostAllocationMachineDecodeError::InvalidField)?,
                    argument_index: u32_field(cursor)?,
                    role: match byte(cursor)? {
                        0 => selected_instructions::OutgoingArgumentSlotRole::Argument,
                        1 => selected_instructions::OutgoingArgumentSlotRole::ValueCopy,
                        _ => return Err(PostAllocationMachineDecodeError::InvalidField),
                    },
                },
                byte_size: u32_field(cursor)?,
                alignment: u16_field(cursor)?,
                abi_stack_byte_offset: u32_field(cursor)?,
            });
        }
        let block_count = length(cursor)?;
        let mut blocks = Vec::with_capacity(block_count.min(cursor.remaining()));
        for _ in 0..block_count {
            let block = SelectedBlockId(u32_field(cursor)?);
            let instruction_count = length(cursor)?;
            let mut instructions = Vec::with_capacity(instruction_count.min(cursor.remaining()));
            for _ in 0..instruction_count {
                instructions.push(decode_instruction(cursor)?);
            }
            blocks.push(PostAllocationMachineBlock {
                block,
                instructions,
            });
        }
        functions.push(PostAllocationMachineFunction {
            machine,
            outgoing_arguments,
            local_storage_slots,
            blocks,
        });
    }
    Ok(PostAllocationMachinePlan {
        identity,
        selected,
        effects,
        ranges,
        legality,
        homes,
        post_allocation_manifest,
        target,
        register_environment,
        physical_register_model,
        register_constraints,
        machine_effect_catalog,
        choice_rule,
        functions,
    })
}

fn decode_instruction(
    cursor: &mut effect_codec::Cursor<'_>,
) -> Result<PostAllocationMachineInstruction, PostAllocationMachineDecodeError> {
    let instruction = SelectedInstructionId(u32_field(cursor)?);
    let alternative = effect_codec::decode_alternative(cursor).map_err(map_field_error)?;
    let operand_count = length(cursor)?;
    let mut operands = Vec::with_capacity(operand_count.min(cursor.remaining()));
    for _ in 0..operand_count {
        operands.push(decode_operand(cursor)?);
    }
    let address = match byte(cursor)? {
        14 => Some(crate::PhysicalAddressOperation::SaveFloatingControl {
            slot: effect_codec::decode_local_storage_slot(cursor).map_err(map_field_error)?,
        }),
        15 => Some(crate::PhysicalAddressOperation::RestoreFloatingControl {
            slot: effect_codec::decode_local_storage_slot(cursor).map_err(map_field_error)?,
        }),
        9 => Some(crate::PhysicalAddressOperation::HostedReadByte {
            slot: effect_codec::decode_local_storage_slot(cursor).map_err(map_field_error)?,
        }),
        5 => Some(crate::PhysicalAddressOperation::HostedWriteByteI32 {
            slot: effect_codec::decode_local_storage_slot(cursor).map_err(map_field_error)?,
        }),
        6 => {
            let base_operand = u16_field(cursor)?;
            let byte_offset = u32_field(cursor)?;
            let byte_size = byte(cursor)?;
            if !matches!(byte_size, 1 | 2 | 4 | 8) {
                return Err(PostAllocationMachineDecodeError::InvalidField);
            }
            Some(crate::PhysicalAddressOperation::Store {
                base_operand,
                byte_offset,
                byte_size,
            })
        }
        7 => Some(crate::PhysicalAddressOperation::AddressOffset {
            base_operand: u16_field(cursor)?,
            byte_offset: u32_field(cursor)?,
        }),
        16 => Some(crate::PhysicalAddressOperation::AccessorScalarAccess {
            base_operand: u16_field(cursor)?,
            byte_offset: u32_field(cursor)?,
            form: selected_instructions::AccessorScalarForm::from_machine_payload(byte(cursor)?)
                .ok_or(PostAllocationMachineDecodeError::InvalidField)?,
            contract: array(cursor)?,
        }),
        0 => None,
        4 => Some(crate::PhysicalAddressOperation::Load8Indexed {
            base_operand: u16_field(cursor)?,
            index_operand: u16_field(cursor)?,
        }),
        tag @ (12 | 13) => {
            let base_operand = u16_field(cursor)?;
            let byte_offset = u32_field(cursor)?;
            let width = selected_instructions::PackedByteWidth::from_byte_size(byte(cursor)?)
                .ok_or(PostAllocationMachineDecodeError::InvalidField)?;
            Some(if tag == 12 {
                crate::PhysicalAddressOperation::LoadPacked {
                    base_operand,
                    byte_offset,
                    width,
                }
            } else {
                crate::PhysicalAddressOperation::StorePacked {
                    base_operand,
                    byte_offset,
                    width,
                }
            })
        }
        10 => Some(crate::PhysicalAddressOperation::Load8 {
            base_operand: u16_field(cursor)?,
            byte_offset: u32_field(cursor)?,
        }),
        11 => Some(crate::PhysicalAddressOperation::Load16 {
            base_operand: u16_field(cursor)?,
            byte_offset: u32_field(cursor)?,
        }),
        8 => Some(crate::PhysicalAddressOperation::Load32 {
            base_operand: u16_field(cursor)?,
            byte_offset: u32_field(cursor)?,
        }),
        1 => Some(crate::PhysicalAddressOperation::Load64 {
            base_operand: u16_field(cursor)?,
            byte_offset: u32_field(cursor)?,
        }),
        tag @ (2 | 3) => {
            let slot = match byte(cursor)? {
                4 => selected_instructions::FrameStorageSlotId::ActivationBase,
                2 => selected_instructions::FrameStorageSlotId::Incoming {
                    parameter_index: u32_field(cursor)?,
                    abi_stack_byte_offset: u32_field(cursor)?,
                },
                slot_tag @ (0 | 3) => selected_instructions::FrameStorageSlotId::Outgoing(
                    selected_instructions::OutgoingArgumentSlotId {
                        role: if slot_tag == 0 {
                            selected_instructions::OutgoingArgumentSlotRole::Argument
                        } else {
                            selected_instructions::OutgoingArgumentSlotRole::ValueCopy
                        },
                        invocation_site: semantic_vocabulary::InvocationSite::from_tagged_identity(
                            byte(cursor)?,
                            u64_field(cursor)?,
                        )
                        .ok_or(PostAllocationMachineDecodeError::InvalidField)?,
                        argument_index: u32_field(cursor)?,
                    },
                ),
                1 => selected_instructions::FrameStorageSlotId::Local(
                    effect_codec::decode_local_storage_slot(cursor).map_err(map_field_error)?,
                ),
                _ => return Err(PostAllocationMachineDecodeError::InvalidField),
            };
            let byte_offset = u32_field(cursor)?;
            if slot == selected_instructions::FrameStorageSlotId::ActivationBase
                && (tag != 3 || byte_offset != 0)
            {
                return Err(PostAllocationMachineDecodeError::InvalidField);
            }
            Some(if tag == 2 {
                crate::PhysicalAddressOperation::Store64 { slot, byte_offset }
            } else {
                crate::PhysicalAddressOperation::FrameAddress { slot, byte_offset }
            })
        }
        _ => return Err(PostAllocationMachineDecodeError::InvalidField),
    };
    Ok(PostAllocationMachineInstruction {
        instruction,
        alternative,
        operands,
        address,
        implicit_unit_uses: decode_units(cursor)?,
        implicit_unit_defs: decode_units(cursor)?,
        implicit_unit_clobbers: decode_units(cursor)?,
        unit_uses: decode_units(cursor)?,
        unit_defs: decode_units(cursor)?,
        unit_clobbers: decode_units(cursor)?,
    })
}

fn decode_operand(
    cursor: &mut effect_codec::Cursor<'_>,
) -> Result<PhysicalOperandFootprint, PostAllocationMachineDecodeError> {
    let operand = u16_field(cursor)?;
    let virtual_register = VirtualRegisterId(u32_field(cursor)?);
    let class = RegisterClassId(u16_field(cursor)?);
    let view = RegisterViewId(u16_field(cursor)?);
    let access = match byte(cursor)? {
        0 => RegisterOperandAccess::Use,
        1 => RegisterOperandAccess::Def,
        2 => RegisterOperandAccess::UseDef,
        _ => return Err(PostAllocationMachineDecodeError::InvalidField),
    };
    let storage_units = decode_units(cursor)?;
    let read_units = decode_units(cursor)?;
    let write_units = decode_units(cursor)?;
    let write_semantics = match byte(cursor)? {
        0 => None,
        1 => Some(match byte(cursor)? {
            0 => RegisterWriteSemantics::ExactView,
            1 => RegisterWriteSemantics::PreservesUnwritten,
            2 => RegisterWriteSemantics::ZeroExtendsParent,
            3 => RegisterWriteSemantics::ZeroExtendsWithinUnit,
            4 => RegisterWriteSemantics::Discards,
            5 => RegisterWriteSemantics::InstructionDefined,
            _ => return Err(PostAllocationMachineDecodeError::InvalidField),
        }),
        _ => return Err(PostAllocationMachineDecodeError::InvalidField),
    };
    Ok(PhysicalOperandFootprint {
        operand,
        virtual_register,
        class,
        view,
        access,
        storage_units,
        read_units,
        write_units,
        write_semantics,
    })
}
