//! Independent acceptance of pre-emission scalar argument transport.
//!
//! This checker consumes retained storage decisions. It does not select slots,
//! rebuild a producer plan, or repair a candidate during byte emission.

use assigned_target_operations::{
    AssignedCallDestination, AssignedScalarLocation, AssignedUnitScalarArgumentSource,
    AssignedUnitScalarCallArgument, UnitScalarTransportPlan,
};
use calling_conventions::{CallPlan, MachineRegister};
use target::Architecture;

use super::{
    EmissionError, align_u32, outgoing_placement_extent, outgoing_placement_extent_with_copy,
};

fn source_register(argument: &AssignedUnitScalarCallArgument) -> Option<MachineRegister> {
    match argument.source {
        AssignedUnitScalarArgumentSource::Parameter {
            location: AssignedScalarLocation::Register(register),
            ..
        } => Some(register),
        _ => None,
    }
}

pub(super) fn validate_scalar_transport(
    architecture: Architecture,
    call: &CallPlan,
    arguments: &[AssignedUnitScalarCallArgument],
    minimum_outgoing_bytes: u32,
    transport: &UnitScalarTransportPlan,
) -> Result<(), EmissionError> {
    let invalid = || EmissionError::UnitCallStackAreaNotEncodable;
    let mut outgoing_end = u32::from(call.shadow_bytes).max(minimum_outgoing_bytes);
    for placement in &call.parameters {
        let end = match architecture {
            Architecture::X86_64 => outgoing_placement_extent(placement)?,
            Architecture::Aarch64 => outgoing_placement_extent_with_copy(placement)?,
        };
        outgoing_end = outgoing_end.max(end);
    }

    // One non-identity write into any incoming register requires the complete
    // first-seen incoming register roster to be saved before argument writes.
    let snapshot_required = arguments.iter().any(|written| {
        let AssignedCallDestination::Register(destination) = written.destination else {
            return false;
        };
        source_register(written) != Some(destination)
            && arguments
                .iter()
                .any(|read| source_register(read) == Some(destination))
    });
    let mut occupied_end = outgoing_end;
    if snapshot_required {
        occupied_end = align_u32(outgoing_end, 8)?;
        let mut slot_index = 0;
        for (argument_index, argument) in arguments.iter().enumerate() {
            let Some(register) = source_register(argument) else {
                continue;
            };
            if arguments[..argument_index]
                .iter()
                .any(|earlier| source_register(earlier) == Some(register))
            {
                continue;
            }
            if transport.snapshot_slots.get(slot_index) != Some(&(register, occupied_end)) {
                return Err(invalid());
            }
            occupied_end = occupied_end.checked_add(8).ok_or_else(invalid)?;
            slot_index += 1;
        }
        if slot_index != transport.snapshot_slots.len() {
            return Err(invalid());
        }
    } else if !transport.snapshot_slots.is_empty() {
        return Err(invalid());
    }

    // A minimal aligned outbound extent is uniquely determined by its lower
    // bound and congruence. No second plan needs to be manufactured to check it.
    let expected_remainder = match architecture {
        Architecture::X86_64 => 8,
        Architecture::Aarch64 => 0,
    };
    let Some(padding) = transport.call_stack_bytes.checked_sub(occupied_end) else {
        return Err(invalid());
    };
    if padding >= 16 || transport.call_stack_bytes % 16 != expected_remainder {
        return Err(invalid());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use calling_conventions::{
        CallSignature, CallingPolicy, ValueLocation, ValueShape, evaluate_call_plan,
    };
    use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType, ValueId};
    use target::NativeTarget;

    #[test]
    fn exact_transport_checks_every_slot_and_extent_on_each_abi() {
        for target in [
            NativeTarget::windows_x64(),
            NativeTarget::linux_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
        ] {
            let call = evaluate_call_plan(
                CallingPolicy::native_for_target(target),
                &CallSignature {
                    parameters: vec![ValueShape::integer(8, 8); 2],
                    result: None,
                },
            )
            .unwrap();
            let registers = call
                .parameters
                .iter()
                .map(|parameter| match parameter.locations[0] {
                    ValueLocation::Register { register, .. } => register,
                    _ => panic!("two register parameters"),
                })
                .collect::<Vec<_>>();
            let arguments = (0..2)
                .map(|index| AssignedUnitScalarCallArgument {
                    parameter_index: index as u32,
                    source: AssignedUnitScalarArgumentSource::Parameter {
                        parameter_index: (1 - index) as u32,
                        source_value: ValueId::new(1 + (1 - index) as u64).unwrap(),
                        scalar_type: ScalarType::Integer(
                            IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                        ),
                        location: AssignedScalarLocation::Register(registers[1 - index]),
                    },
                    destination: AssignedCallDestination::Register(registers[index]),
                })
                .collect::<Vec<_>>();
            let start = u32::from(call.shadow_bytes);
            let plan = UnitScalarTransportPlan {
                call_stack_bytes: start
                    + if target.architecture == Architecture::X86_64 {
                        24
                    } else {
                        16
                    },
                snapshot_slots: vec![(registers[1], start), (registers[0], start + 8)],
            };
            validate_scalar_transport(target.architecture, &call, &arguments, 0, &plan).unwrap();
            let mut corruptions = Vec::new();
            let mut changed = plan.clone();
            changed.snapshot_slots.clear();
            corruptions.push(changed);
            let mut changed = plan.clone();
            changed.snapshot_slots.swap(0, 1);
            corruptions.push(changed);
            let mut changed = plan.clone();
            changed.snapshot_slots[1].1 = changed.snapshot_slots[0].1;
            corruptions.push(changed);
            let mut changed = plan.clone();
            changed.snapshot_slots.push(changed.snapshot_slots[0]);
            corruptions.push(changed);
            let mut changed = plan.clone();
            changed.call_stack_bytes += 16;
            corruptions.push(changed);
            let mut changed = plan.clone();
            changed.call_stack_bytes -= 8;
            corruptions.push(changed);
            for changed in corruptions {
                assert!(
                    validate_scalar_transport(target.architecture, &call, &arguments, 0, &changed)
                        .is_err()
                );
            }
            let mut identity_arguments = arguments.clone();
            for (index, argument) in identity_arguments.iter_mut().enumerate() {
                argument.destination = AssignedCallDestination::Register(registers[1 - index]);
            }
            assert!(
                validate_scalar_transport(
                    target.architecture,
                    &call,
                    &identity_arguments,
                    0,
                    &plan
                )
                .is_err(),
                "unnecessary retained snapshots are not canonical"
            );
        }
    }
}
