//! Ordered V3 content decoding after frame admission.

use optimization_core::PostAllocationOptimizationManifestIdentity;
use register_homes::{AllocationLegalityIdentity, LiveRangeIdentity, RegisterHomeIdentity};
use register_model::{
    PhysicalRegisterModelIdentity, RegisterConstraintCatalogIdentity,
    TargetRegisterEnvironmentIdentity,
};
use selected_instructions::PreAllocationMachineEffectIdentity;
use selected_instructions::{
    MachineEffectCatalogIdentity, SelectedBlockId, SelectedInstructionPlanIdentity,
};
use semantic_vocabulary::MachineId;

use crate::{
    MachineAlternativeChoiceRule, PostAllocationMachineBlock, PostAllocationMachineFunction,
    PostAllocationMachineIdentity, PostAllocationMachinePlan, PostAllocationStructuralUnitFunction,
};
use selected_instructions::selected_instructions::effects::program::encoding as effect_codec;

use super::instruction::decode_instruction;

use super::super::{
    PostAllocationMachineDecodeError,
    cursor::{array, byte, length, map_field_error, u32_field, u64_field},
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
        functions.push(PostAllocationMachineFunction { machine, blocks });
    }
    let structural_count = length(cursor)?;
    let mut structural_unit_functions =
        Vec::with_capacity(structural_count.min(cursor.remaining()));
    for _ in 0..structural_count {
        let machine = MachineId::new(u64_field(cursor)?)
            .ok_or(PostAllocationMachineDecodeError::InvalidField)?;
        let block = SelectedBlockId(u32_field(cursor)?);
        let call = match byte(cursor)? {
            0 => None,
            1 => Some(effect_codec::decode_structural_call(cursor).map_err(map_field_error)?),
            _ => return Err(PostAllocationMachineDecodeError::InvalidField),
        };
        let return_instruction = decode_instruction(cursor, allow_i64_less_than, false, false)?;
        let return_provenance = effect_codec::decode_provenance(cursor).map_err(map_field_error)?;
        let return_effect = effect_codec::decode_effect_link(cursor).map_err(map_field_error)?;
        let return_ownership = effect_codec::decode_ownership(cursor).map_err(map_field_error)?;
        structural_unit_functions.push(PostAllocationStructuralUnitFunction {
            machine,
            block,
            call,
            return_instruction,
            return_provenance,
            return_effect,
            return_ownership,
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
        structural_unit_functions,
    };
    Ok(plan)
}
