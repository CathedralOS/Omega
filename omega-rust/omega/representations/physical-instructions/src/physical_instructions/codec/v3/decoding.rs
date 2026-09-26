//! Ordered V3 content decoding after frame admission.

use optimization_core::PostAllocationOptimizationManifestIdentity;
use register_homes::{AllocationLegalityIdentity, RegisterHomeIdentity};
use register_model::{
    PhysicalRegisterModelIdentity, RegisterConstraintCatalogIdentity,
    TargetRegisterEnvironmentIdentity,
};
use selected_instructions::LiveRangeIdentity;
use selected_instructions::PreAllocationMachineEffectIdentity;
use selected_instructions::{
    MachineEffectCatalogIdentity, SelectedBlockId, SelectedInstructionPlanIdentity,
};
use semantic_vocabulary::MachineId;

use crate::{
    MachineAlternativeChoiceRule, PostAllocationMachineBlock, PostAllocationMachineFunction,
    PostAllocationMachineIdentity, PostAllocationMachinePlan,
};
use selected_instructions::selected_instructions::effects::program::encoding as effect_codec;

use super::instruction::decode_instruction;

use super::super::{
    PostAllocationMachineDecodeError,
    cursor::{array, byte, length, map_field_error, u16_field, u32_field, u64_field},
};

pub(in crate::physical_instructions::codec) fn decode_content(
    cursor: &mut effect_codec::Cursor<'_>,
    identity: PostAllocationMachineIdentity,
    allow_i64_less_than: bool,
    allow_scalar_call: bool,
    allow_jump: bool,
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
                    operation: semantic_vocabulary::OperationId::new(u64_field(cursor)?)
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
                instructions.push(decode_instruction(
                    cursor,
                    allow_i64_less_than,
                    allow_scalar_call,
                    allow_jump,
                )?);
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
    let plan = PostAllocationMachinePlan {
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
    };
    Ok(plan)
}
