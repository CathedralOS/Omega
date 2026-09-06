//! Pre-encoding call transport scheduling. This producer chooses outgoing
//! storage and register snapshots; emission only validates and executes them.

use assigned_target_operations::{
    AssignedCallDestination, AssignedScalarLocation, AssignedUnitScalarArgumentSource,
    AssignedUnitScalarCallArgument, UnitScalarTransportPlan,
};
use calling_conventions::{CallPlan, IndirectPointerLocation, ValueLocation};
use target::{Architecture, NativeTarget, ObjectFormat};

use crate::AssignmentError;

#[derive(Clone, Copy)]
pub(super) enum CallTransportKind {
    ScalarResult,
    Mixed,
}

pub(super) fn assign(
    call_plan: &CallPlan,
    arguments: &[AssignedUnitScalarCallArgument],
    target: NativeTarget,
    kind: CallTransportKind,
) -> Result<UnitScalarTransportPlan, AssignmentError> {
    let minimum = if target.architecture == Architecture::X86_64
        && target.object_format == ObjectFormat::Coff
        && matches!(kind, CallTransportKind::ScalarResult)
    {
        32
    } else {
        0
    };
    let mut extent = u32::from(call_plan.shadow_bytes).max(minimum);
    for placement in &call_plan.parameters {
        for location in &placement.locations {
            let end = match *location {
                ValueLocation::Register { .. } => 0,
                ValueLocation::Stack {
                    stack_byte_offset,
                    byte_size,
                    ..
                } => add(stack_byte_offset, u32::from(byte_size.max(8)))?,
                ValueLocation::Indirect {
                    pointer,
                    copy_stack_byte_offset,
                    byte_size,
                    ..
                } => {
                    let pointer_end = match pointer {
                        IndirectPointerLocation::Register(_) => 0,
                        IndirectPointerLocation::Stack {
                            stack_byte_offset, ..
                        } => add(stack_byte_offset, 8)?,
                    };
                    if target.architecture == Architecture::Aarch64 {
                        if let Some(offset) = copy_stack_byte_offset {
                            pointer_end.max(add(offset, align(u32::from(byte_size), 8)?)?)
                        } else {
                            pointer_end
                        }
                    } else {
                        pointer_end
                    }
                }
            };
            extent = extent.max(end);
        }
    }

    let source_registers = arguments
        .iter()
        .filter_map(|argument| match argument.source {
            AssignedUnitScalarArgumentSource::Parameter {
                location: AssignedScalarLocation::Register(register),
                ..
            } => Some(register),
            _ => None,
        })
        .collect::<Vec<_>>();
    let needs_snapshot = arguments.iter().any(|argument| {
        let AssignedCallDestination::Register(destination) = argument.destination else {
            return false;
        };
        source_registers.contains(&destination)
            && !matches!(argument.source,
                AssignedUnitScalarArgumentSource::Parameter {
                    location: AssignedScalarLocation::Register(source), ..
                } if source == destination)
    });
    let mut snapshot_slots = Vec::new();
    if needs_snapshot {
        extent = align(extent, 8)?;
        for register in source_registers {
            if snapshot_slots
                .iter()
                .any(|(existing, _)| *existing == register)
            {
                continue;
            }
            snapshot_slots.push((register, extent));
            extent = add(extent, 8)?;
        }
    }
    let call_stack_bytes = if target.architecture == Architecture::X86_64 {
        add(extent, (8 + 16 - (extent % 16)) % 16)?
    } else {
        align(extent, 16)?
    };
    Ok(UnitScalarTransportPlan {
        call_stack_bytes,
        snapshot_slots,
    })
}

fn add(left: u32, right: u32) -> Result<u32, AssignmentError> {
    left.checked_add(right)
        .ok_or(AssignmentError::UnitScalarFrameNotEncodable)
}

fn align(value: u32, alignment: u32) -> Result<u32, AssignmentError> {
    add(value, (alignment - value % alignment) % alignment)
}

#[cfg(test)]
mod tests;
