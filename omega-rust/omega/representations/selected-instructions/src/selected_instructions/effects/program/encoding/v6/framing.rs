use super::*;

pub fn encode_terminal_pre_allocation_machine_effect_plan(
    plan: &PreAllocationMachineEffectPlan,
) -> Vec<u8> {
    let content = identity::encode_terminal_pre_allocation_machine_effect_content(plan);
    let mut encoded = Vec::with_capacity(44 + content.len());
    encoded.extend_from_slice(MAGIC);
    encoded.extend_from_slice(&VERSION.to_le_bytes());
    encoded.extend_from_slice(&plan.identity.bytes());
    encoded.extend_from_slice(&content);
    encoded
}

pub fn decode_terminal_pre_allocation_machine_effect_plan(
    encoded: &[u8],
) -> Result<PreAllocationMachineEffectPlan, PreAllocationMachineEffectDecodeError> {
    let mut cursor = Cursor::new(encoded);
    if cursor.take(8)? != MAGIC {
        return Err(PreAllocationMachineEffectDecodeError::WrongMagic);
    }
    let version = cursor.u32()?;
    if !matches!(
        version,
        LEGACY_V6_VERSION | LEGACY_V7_VERSION | LEGACY_V8_VERSION | VERSION
    ) {
        return Err(PreAllocationMachineEffectDecodeError::UnsupportedVersion(
            version,
        ));
    }
    let identity = PreAllocationMachineEffectIdentity::from_bytes(cursor.array()?);
    let selected = SelectedInstructionPlanIdentity::from_bytes(cursor.array()?);
    let optimization_unit = OptimizationUnitIdentity::from_bytes(cursor.array()?);
    let fuel_schedule = FuelScheduleIdentity::new(cursor.u32()?)
        .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?;
    let target = decode_target(&mut cursor)?;
    let register_environment = TargetRegisterEnvironmentIdentity::from_bytes(cursor.array()?);
    let register_constraints = RegisterConstraintCatalogIdentity::from_bytes(cursor.array()?);
    let machine_effect_catalog = MachineEffectCatalogIdentity::from_bytes(cursor.array()?);
    let function_count = cursor.length()?;
    let mut functions = Vec::with_capacity(function_count.min(cursor.remaining()));
    for _ in 0..function_count {
        let machine = MachineId::new(cursor.u64()?)
            .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?;
        let block_count = cursor.length()?;
        let mut blocks = Vec::with_capacity(block_count.min(cursor.remaining()));
        for _ in 0..block_count {
            let block = SelectedBlockId(cursor.u32()?);
            let instruction_count = cursor.length()?;
            let mut instructions = Vec::with_capacity(instruction_count.min(cursor.remaining()));
            for _ in 0..instruction_count {
                instructions.push(decode_instruction(
                    &mut cursor,
                    matches!(version, LEGACY_V8_VERSION | VERSION),
                    version == VERSION,
                )?);
            }
            blocks.push(BlockMachineEffects {
                block,
                instructions,
            });
        }
        functions.push(FunctionMachineEffects { machine, blocks });
    }
    let structural_count = cursor.length()?;
    let mut structural_unit_functions =
        Vec::with_capacity(structural_count.min(cursor.remaining()));
    for _ in 0..structural_count {
        structural_unit_functions.push(decode_structural_function(
            &mut cursor,
            matches!(version, LEGACY_V8_VERSION | VERSION),
        )?);
    }
    if cursor.remaining() != 0 {
        return Err(PreAllocationMachineEffectDecodeError::TrailingBytes);
    }
    let plan = PreAllocationMachineEffectPlan {
        identity,
        selected,
        optimization_unit,
        fuel_schedule,
        target,
        register_environment,
        register_constraints,
        machine_effect_catalog,
        functions,
        structural_unit_functions,
    };
    let expected_identity = match version {
        LEGACY_V6_VERSION => crate::selected_instructions::effects::program::identity::pre_allocation_machine_effect_identity_v5_legacy(&plan),
        LEGACY_V7_VERSION => crate::selected_instructions::effects::program::identity::pre_allocation_machine_effect_identity_v6_legacy(&plan),
        LEGACY_V8_VERSION => crate::selected_instructions::effects::program::identity::pre_allocation_machine_effect_identity_v7_legacy(&plan),
        VERSION => pre_allocation_machine_effect_identity(&plan),
        _ => unreachable!("wire version admitted above"),
    };
    if plan.identity != expected_identity {
        return Err(PreAllocationMachineEffectDecodeError::InvalidIdentity);
    }
    Ok(plan)
}
