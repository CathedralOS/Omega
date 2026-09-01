use omega_optimization_core::{
    OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage,
};
use omega_register_model::{RegisterClassId, RegisterViewId, TargetRegisterEnvironmentIdentity};
use omega_selected_instructions::{
    SelectedBlockId, SelectedInstructionId, SelectedInstructionPlanIdentity, VirtualRegisterId,
};
use psi_core::{FuelScheduleIdentity, MachineId};

use crate::{
    AllocationLegalityIdentity, AllocatorAvailabilityIdentity, LiveRangeIdentity, LiveRangePoint,
    SpillChoiceIdentity,
};

use super::super::*;
use super::cursor::Cursor;
use super::values::{decode_definition_site, decode_origin, decode_scalar_type};
use super::{MAGIC, VERSION};

pub(super) fn decode(
    encoded: &[u8],
) -> Result<LogicalSpillOperationPlan, LogicalSpillOperationDecodeError> {
    let mut cursor = Cursor::new(encoded);
    if cursor.take(MAGIC.len())? != MAGIC {
        return Err(LogicalSpillOperationDecodeError::WrongMagic);
    }
    let version = u32::from_le_bytes(cursor.array()?);
    if version != VERSION {
        return Err(LogicalSpillOperationDecodeError::UnsupportedVersion(
            version,
        ));
    }
    let claimed = LogicalSpillOperationIdentity::from_bytes(cursor.array()?);
    let selected = SelectedInstructionPlanIdentity::from_bytes(cursor.array()?);
    let ranges = LiveRangeIdentity::from_bytes(cursor.array()?);
    let legality = AllocationLegalityIdentity::from_bytes(cursor.array()?);
    let spill_choices = SpillChoiceIdentity::from_bytes(cursor.array()?);
    let register_environment = TargetRegisterEnvironmentIdentity::from_bytes(cursor.array()?);
    let allocator_availability = AllocatorAvailabilityIdentity::from_bytes(cursor.array()?);
    let optimization_unit = OptimizationUnitIdentity::from_bytes(cursor.array()?);
    let marker = u32::from_le_bytes(cursor.array()?);
    let fuel_schedule = FuelScheduleIdentity::new(marker).ok_or(
        LogicalSpillOperationDecodeError::InvalidFuelSchedule(marker),
    )?;
    let policy = match cursor.byte()? {
        0 => LogicalSpillOperationPolicy::SelectedActiveResidentInstructionResultU64StoreBeforePressureReloadBeforeFirstFutureFlexibleUseV1,
        tag => return Err(LogicalSpillOperationDecodeError::UnknownPolicy(tag)),
    };
    let budget = OptimizationWorkBudget::decode(cursor.take(40)?)
        .map_err(|_| LogicalSpillOperationDecodeError::InvalidBudget)?;
    let usage = OptimizationWorkUsage::decode(cursor.take(40)?)
        .map_err(|_| LogicalSpillOperationDecodeError::InvalidUsage)?;
    let function_count = cursor.length()?;
    let mut functions = Vec::with_capacity(function_count.min(cursor.remaining()));
    for _ in 0..function_count {
        let raw_machine = u64::from_le_bytes(cursor.array()?);
        let machine = MachineId::new(raw_machine).ok_or(
            LogicalSpillOperationDecodeError::InvalidMachineId(raw_machine),
        )?;
        let action = match cursor.byte()? {
            0 => None,
            1 => Some(decode_action(&mut cursor)?),
            tag => return Err(LogicalSpillOperationDecodeError::UnknownOption(tag)),
        };
        functions.push(FunctionLogicalSpillOperations { machine, action });
    }
    if cursor.remaining() != 0 {
        return Err(LogicalSpillOperationDecodeError::TrailingBytes);
    }
    let plan = LogicalSpillOperationPlan {
        selected,
        ranges,
        legality,
        spill_choices,
        register_environment,
        allocator_availability,
        optimization_unit,
        fuel_schedule,
        policy,
        budget,
        usage,
        functions,
    };
    if super::super::logical_spill_operation_identity(&plan) != claimed {
        return Err(LogicalSpillOperationDecodeError::IdentityMismatch);
    }
    Ok(plan)
}

fn decode_action(
    cursor: &mut Cursor<'_>,
) -> Result<LogicalSpillAction, LogicalSpillOperationDecodeError> {
    let block = SelectedBlockId(u32::from_le_bytes(cursor.array()?));
    let pressure_point = LiveRangePoint(u32::from_le_bytes(cursor.array()?));
    let incoming = VirtualRegisterId(u32::from_le_bytes(cursor.array()?));
    let incoming_class = RegisterClassId(u16::from_le_bytes(cursor.array()?));
    let victim = VirtualRegisterId(u32::from_le_bytes(cursor.array()?));
    let victim_class = RegisterClassId(u16::from_le_bytes(cursor.array()?));
    let victim_scalar_type = decode_scalar_type(cursor)?;
    let victim_origin = decode_origin(cursor)?;
    let victim_definition_site = decode_definition_site(cursor)?;
    let current_view = RegisterViewId(u16::from_le_bytes(cursor.array()?));
    let reclaimed_view = RegisterViewId(u16::from_le_bytes(cursor.array()?));
    let storage = LogicalSpillStorage {
        id: LogicalSpillStorageId(u32::from_le_bytes(cursor.array()?)),
        class: match cursor.byte()? {
            0 => LogicalSpillStorageClass::NonAddressUnsignedU64V1,
            tag => return Err(LogicalSpillOperationDecodeError::UnknownStorageClass(tag)),
        },
    };
    let store = LogicalSpillStore {
        before_instruction: SelectedInstructionId(u32::from_le_bytes(cursor.array()?)),
        source: VirtualRegisterId(u32::from_le_bytes(cursor.array()?)),
        storage: LogicalSpillStorageId(u32::from_le_bytes(cursor.array()?)),
    };
    let reload = LogicalSpillReload {
        before_instruction: SelectedInstructionId(u32::from_le_bytes(cursor.array()?)),
        storage: LogicalSpillStorageId(u32::from_le_bytes(cursor.array()?)),
        result: LogicalReloadValueId(u32::from_le_bytes(cursor.array()?)),
    };
    let rewrite_count = cursor.length()?;
    let mut rewrites = Vec::with_capacity(rewrite_count.min(cursor.remaining()));
    for _ in 0..rewrite_count {
        rewrites.push(LogicalSpillUseRewrite {
            block: SelectedBlockId(u32::from_le_bytes(cursor.array()?)),
            point: LiveRangePoint(u32::from_le_bytes(cursor.array()?)),
            instruction: SelectedInstructionId(u32::from_le_bytes(cursor.array()?)),
            operand: u16::from_le_bytes(cursor.array()?),
            result: LogicalReloadValueId(u32::from_le_bytes(cursor.array()?)),
        });
    }
    Ok(LogicalSpillAction {
        block,
        pressure_point,
        incoming,
        incoming_class,
        victim,
        victim_class,
        victim_scalar_type,
        victim_origin,
        victim_definition_site,
        current_view,
        reclaimed_view,
        storage,
        store,
        reload,
        rewrites,
    })
}
