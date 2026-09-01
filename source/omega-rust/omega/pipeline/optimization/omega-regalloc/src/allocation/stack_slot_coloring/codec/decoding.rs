use omega_optimization_core::{
    OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage,
};
use omega_register_model::TargetRegisterEnvironmentIdentity;
use omega_selected_instructions::SelectedBlockId;
use psi_core::{FuelScheduleIdentity, MachineId};

use crate::{
    AllocatorAvailabilityIdentity, LiveRangePoint, LogicalSpillOperationIdentity,
    LogicalSpillStorageClass, LogicalSpillStorageId,
};

use super::super::*;
use super::cursor::Cursor;
use super::{MAGIC, VERSION};

pub(super) fn decode(
    encoded: &[u8],
) -> Result<StackSlotColoringPlan, StackSlotColoringDecodeError> {
    let mut cursor = Cursor::new(encoded);
    if cursor.take(MAGIC.len())? != MAGIC {
        return Err(StackSlotColoringDecodeError::WrongMagic);
    }
    let version = u32::from_le_bytes(cursor.array()?);
    if version != VERSION {
        return Err(StackSlotColoringDecodeError::UnsupportedVersion(version));
    }
    let claimed = StackSlotColoringIdentity::from_bytes(cursor.array()?);
    let logical_spill_operations = LogicalSpillOperationIdentity::from_bytes(cursor.array()?);
    let register_environment = TargetRegisterEnvironmentIdentity::from_bytes(cursor.array()?);
    let allocator_availability = AllocatorAvailabilityIdentity::from_bytes(cursor.array()?);
    let optimization_unit = OptimizationUnitIdentity::from_bytes(cursor.array()?);
    let marker = u32::from_le_bytes(cursor.array()?);
    let fuel_schedule = FuelScheduleIdentity::new(marker)
        .ok_or(StackSlotColoringDecodeError::InvalidFuelSchedule(marker))?;
    let policy = match cursor.byte()? {
        0 => StackSlotColoringPolicy::BlockLocalNonAddressUnsignedU64ClosedIntervalFirstFitV1,
        tag => return Err(StackSlotColoringDecodeError::UnknownPolicy(tag)),
    };
    let budget = OptimizationWorkBudget::decode(cursor.take(40)?)
        .map_err(|_| StackSlotColoringDecodeError::InvalidBudget)?;
    let usage = OptimizationWorkUsage::decode(cursor.take(40)?)
        .map_err(|_| StackSlotColoringDecodeError::InvalidUsage)?;
    let function_count = cursor.length()?;
    let mut functions = Vec::with_capacity(function_count.min(cursor.remaining()));
    for _ in 0..function_count {
        let raw_machine = u64::from_le_bytes(cursor.array()?);
        let machine = MachineId::new(raw_machine)
            .ok_or(StackSlotColoringDecodeError::InvalidMachineId(raw_machine))?;
        let spill_area_bytes = u64::from_le_bytes(cursor.array()?);
        let assignment_count = cursor.length()?;
        let mut assignments = Vec::with_capacity(assignment_count.min(cursor.remaining()));
        for _ in 0..assignment_count {
            let storage = LogicalSpillStorageId(u32::from_le_bytes(cursor.array()?));
            let class = match cursor.byte()? {
                0 => LogicalSpillStorageClass::NonAddressUnsignedU64V1,
                tag => return Err(StackSlotColoringDecodeError::UnknownStorageClass(tag)),
            };
            assignments.push(StackSlotAssignment {
                storage,
                class,
                block: SelectedBlockId(u32::from_le_bytes(cursor.array()?)),
                live_from: LiveRangePoint(u32::from_le_bytes(cursor.array()?)),
                live_through: LiveRangePoint(u32::from_le_bytes(cursor.array()?)),
                size_bytes: u64::from_le_bytes(cursor.array()?),
                alignment_bytes: u64::from_le_bytes(cursor.array()?),
                spill_area_offset: u64::from_le_bytes(cursor.array()?),
            });
        }
        functions.push(FunctionStackSlotColoring {
            machine,
            assignments,
            spill_area_bytes,
        });
    }
    if cursor.remaining() != 0 {
        return Err(StackSlotColoringDecodeError::TrailingBytes);
    }
    let plan = StackSlotColoringPlan {
        logical_spill_operations,
        register_environment,
        allocator_availability,
        optimization_unit,
        fuel_schedule,
        policy,
        budget,
        usage,
        functions,
    };
    if super::super::stack_slot_coloring_identity(&plan) != claimed {
        return Err(StackSlotColoringDecodeError::IdentityMismatch);
    }
    Ok(plan)
}
