//! Ordered V3 content decoding after frame admission.

use omega_optimization_core::PostAllocationOptimizationManifestIdentity;
use omega_regalloc::{AllocationLegalityIdentity, LiveRangeIdentity, RegisterHomeIdentity};
use omega_register_model::{
    PhysicalRegisterModelIdentity, RegisterConstraintCatalogIdentity,
    TargetRegisterEnvironmentIdentity,
};
use omega_selected_instructions::{
    MachineEffectCatalogIdentity, SelectedBlockId, SelectedInstructionPlanIdentity,
};
use psi_core::MachineId;

use crate::analyses::pre_allocation_effects::codec as effect_codec;
use crate::{
    MachineAlternativeChoiceRule, PostAllocationMachineBlock, PostAllocationMachineFunction,
    PostAllocationMachineIdentity, PostAllocationMachinePlan, PostAllocationStructuralUnitFunction,
    PreAllocationMachineEffectIdentity,
};

use super::instruction::decode_instruction;

use super::super::{
    PostAllocationMachineDecodeError,
    cursor::{array, byte, length, map_field_error, u32_field, u64_field},
};

pub(in crate::planning::post_allocation::codec) fn decode_content(
    mut cursor: &mut effect_codec::Cursor<'_>,
    identity: PostAllocationMachineIdentity,
    allow_i64_less_than: bool,
) -> Result<PostAllocationMachinePlan, PostAllocationMachineDecodeError> {
    let selected = SelectedInstructionPlanIdentity::from_bytes(array(&mut cursor)?);
    let effects = PreAllocationMachineEffectIdentity::from_bytes(array(&mut cursor)?);
    let ranges = LiveRangeIdentity::from_bytes(array(&mut cursor)?);
    let legality = AllocationLegalityIdentity::from_bytes(array(&mut cursor)?);
    let homes = RegisterHomeIdentity::from_bytes(array(&mut cursor)?);
    let post_allocation_manifest =
        PostAllocationOptimizationManifestIdentity::from_bytes(array(&mut cursor)?);
    let target = effect_codec::decode_target(&mut cursor).map_err(map_field_error)?;
    let register_environment = TargetRegisterEnvironmentIdentity::from_bytes(array(&mut cursor)?);
    let physical_register_model = PhysicalRegisterModelIdentity::from_bytes(array(&mut cursor)?);
    let register_constraints = RegisterConstraintCatalogIdentity::from_bytes(array(&mut cursor)?);
    let machine_effect_catalog = MachineEffectCatalogIdentity::from_bytes(array(&mut cursor)?);
    let choice_rule = match byte(&mut cursor)? {
        0 => MachineAlternativeChoiceRule::UniqueApplicableInCatalogOrderV1,
        _ => return Err(PostAllocationMachineDecodeError::InvalidField),
    };
    let function_count = length(&mut cursor)?;
    let mut functions = Vec::with_capacity(function_count.min(cursor.remaining()));
    for _ in 0..function_count {
        let machine = MachineId::new(u64_field(&mut cursor)?)
            .ok_or(PostAllocationMachineDecodeError::InvalidField)?;
        let block_count = length(&mut cursor)?;
        let mut blocks = Vec::with_capacity(block_count.min(cursor.remaining()));
        for _ in 0..block_count {
            let block = SelectedBlockId(u32_field(&mut cursor)?);
            let instruction_count = length(&mut cursor)?;
            let mut instructions = Vec::with_capacity(instruction_count.min(cursor.remaining()));
            for _ in 0..instruction_count {
                instructions.push(decode_instruction(&mut cursor, allow_i64_less_than)?);
            }
            blocks.push(PostAllocationMachineBlock {
                block,
                instructions,
            });
        }
        functions.push(PostAllocationMachineFunction { machine, blocks });
    }
    let structural_count = length(&mut cursor)?;
    let mut structural_unit_functions =
        Vec::with_capacity(structural_count.min(cursor.remaining()));
    for _ in 0..structural_count {
        let machine = MachineId::new(u64_field(&mut cursor)?)
            .ok_or(PostAllocationMachineDecodeError::InvalidField)?;
        let block = SelectedBlockId(u32_field(&mut cursor)?);
        let call = match byte(&mut cursor)? {
            0 => None,
            1 => Some(effect_codec::decode_structural_call(&mut cursor).map_err(map_field_error)?),
            _ => return Err(PostAllocationMachineDecodeError::InvalidField),
        };
        let return_instruction = decode_instruction(&mut cursor, allow_i64_less_than)?;
        let return_provenance =
            effect_codec::decode_provenance(&mut cursor).map_err(map_field_error)?;
        let return_effect =
            effect_codec::decode_effect_link(&mut cursor).map_err(map_field_error)?;
        let return_ownership =
            effect_codec::decode_ownership(&mut cursor).map_err(map_field_error)?;
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
