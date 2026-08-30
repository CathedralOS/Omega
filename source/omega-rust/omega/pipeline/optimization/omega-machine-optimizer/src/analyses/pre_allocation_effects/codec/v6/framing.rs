use super::*;

pub(crate) fn encode_terminal_pre_allocation_machine_effect_plan(
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

pub(crate) fn decode_terminal_pre_allocation_machine_effect_plan(
    encoded: &[u8],
) -> Result<PreAllocationMachineEffectPlan, PreAllocationMachineEffectDecodeError> {
    let mut cursor = Cursor::new(encoded);
    if cursor.take(8)? != MAGIC {
        return Err(PreAllocationMachineEffectDecodeError::WrongMagic);
    }
    let version = cursor.u32()?;
    if version != VERSION {
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
                instructions.push(decode_instruction(&mut cursor)?);
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
        structural_unit_functions.push(decode_structural_function(&mut cursor)?);
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
    if plan.identity != pre_allocation_machine_effect_identity(&plan) {
        return Err(PreAllocationMachineEffectDecodeError::InvalidIdentity);
    }
    Ok(plan)
}
