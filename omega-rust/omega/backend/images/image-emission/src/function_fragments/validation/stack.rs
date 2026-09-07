//! Replay numeric image-publisher facts against admitted frame and call inputs.

use super::{Error, host};
use crate::{ObjectScalarCallStack, ObjectScalarStack, ObjectUnitCallStack, ObjectUnitStack};
use machine_code::PlacedInternalMachineCallResolution;
use target::Architecture;

pub(super) fn validate(
    unit: bool,
    geometry: (u64, u16, bool),
    architecture: Architecture,
    calls: &[&PlacedInternalMachineCallResolution],
    unit_stack: Option<ObjectUnitStack>,
    scalar_stack: Option<ObjectScalarStack>,
    unit_call_stacks: &[ObjectUnitCallStack],
    scalar_call_stacks: &[ObjectScalarCallStack],
) -> Result<(), Error> {
    let (frame_bytes, alignment, contains_call) = geometry;
    if (unit && (!scalar_call_stacks.is_empty() || unit_call_stacks.len() != calls.len()))
        || (!unit && (!unit_call_stacks.is_empty() || scalar_call_stacks.len() != calls.len()))
        || alignment == 0
    {
        return Err(Error::Mismatch("shared call stack roster changed"));
    }
    let return_bytes = match architecture {
        Architecture::X86_64 => 8u64,
        Architecture::Aarch64 => 0,
    };
    let mut peak = frame_bytes;
    let edges = unit_call_stacks
        .iter()
        .map(|row| {
            (
                row.owner,
                row.target,
                row.text_offset,
                row.caller_live_bytes,
            )
        })
        .chain(scalar_call_stacks.iter().map(|row| {
            (
                row.owner,
                row.target,
                row.text_offset,
                row.caller_live_bytes,
            )
        }));
    for ((owner, target, text_offset, caller_live_bytes), call) in edges.zip(calls) {
        if !contains_call
            || owner != target_operations::CallSiteOwner::Operation(call.operation)
            || target != call.callee
            || text_offset != host(call.field_section_offset)?
            || u64::from(caller_live_bytes)
                != frame_bytes
                    .checked_add(return_bytes)
                    .ok_or(Error::Overflow)?
            || !caller_live_bytes.is_multiple_of(u32::from(alignment))
        {
            return Err(Error::Mismatch(
                "shared call prefix differs from its validated frame and call",
            ));
        }
        peak = peak.max(u64::from(caller_live_bytes));
    }
    if unit_call_stacks.iter().any(|row| {
        u64::from(row.active_frame_bytes) != frame_bytes
            || u64::from(row.transient_bytes) != return_bytes
    }) {
        return Err(Error::Mismatch("shared call frame decomposition changed"));
    }
    match (unit, unit_stack, scalar_stack) {
        (true, Some(stack), None)
            if u64::from(stack.frame_bytes) == frame_bytes
                && u64::from(stack.local_peak_bytes) == peak
                && stack.stack_alignment == u32::from(alignment) =>
        {
            Ok(())
        }
        (false, None, Some(stack))
            if u64::from(stack.local_peak_bytes) == peak
                && stack.stack_alignment == u32::from(alignment) =>
        {
            Ok(())
        }
        _ => Err(Error::Mismatch("shared function stack envelope changed")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use machine_code::{InternalMachineCallResolutionKind, InternalMachineCallResolutionState};
    use selected_instructions::{SelectedBlockId, SelectedInstructionId};
    use semantic_vocabulary::{MachineId, OperationId};

    fn call(architecture: Architecture) -> PlacedInternalMachineCallResolution {
        let x86 = architecture == Architecture::X86_64;
        let opcode = 12;
        let field = opcode + u64::from(x86);
        let next = opcode + if x86 { 5 } else { 4 };
        PlacedInternalMachineCallResolution {
            kind: if x86 {
                InternalMachineCallResolutionKind::X86Relative32FromNextInstructionToInternalMachineV1
            } else {
                InternalMachineCallResolutionKind::Aarch64BranchLinkImmediate26FromInstructionToInternalMachineV1
            },
            state: InternalMachineCallResolutionState::ResolvedInSectionV1,
            caller: MachineId::new(1).unwrap(),
            callee: MachineId::new(2).unwrap(),
            block: SelectedBlockId(0),
            instruction: SelectedInstructionId(3),
            operation: OperationId::new(3).unwrap(),
            call_function_offset: opcode,
            call_section_offset: opcode,
            call_byte_count: next - opcode,
            opcode_function_offset: opcode,
            opcode_section_offset: opcode,
            field_function_offset: field,
            field_section_offset: field,
            next_instruction_function_offset: next,
            next_instruction_section_offset: next,
            callee_section_offset: 64,
            field_byte_width: 4,
            addend: 0,
            displacement: if x86 { 47 } else { 13 },
        }
    }

    #[test]
    fn scalar_call_edges_retain_exact_frame_prefix_owner_target_and_role() {
        for architecture in [Architecture::X86_64, Architecture::Aarch64] {
            let call = call(architecture);
            let (frame_bytes, peak) = match architecture {
                Architecture::X86_64 => (24, 32),
                Architecture::Aarch64 => (16, 16),
            };
            let stack = ObjectScalarStack {
                local_peak_bytes: peak,
                stack_alignment: 16,
            };
            let row = ObjectScalarCallStack {
                owner: target_operations::CallSiteOwner::Operation(call.operation),
                target: call.callee,
                text_offset: call.field_section_offset as usize,
                caller_live_bytes: peak,
            };
            let check = |stack, rows: &[ObjectScalarCallStack]| {
                validate(
                    false,
                    (frame_bytes, 16, true),
                    architecture,
                    &[&call],
                    None,
                    Some(stack),
                    &[],
                    rows,
                )
            };
            check(stack, &[row]).unwrap();
            let mut changed = row;
            changed.caller_live_bytes += 16;
            assert!(check(stack, &[changed]).is_err());
            changed = row;
            changed.target = call.caller;
            assert!(check(stack, &[changed]).is_err());
            changed = row;
            changed.owner =
                target_operations::CallSiteOwner::Operation(OperationId::new(9).unwrap());
            assert!(check(stack, &[changed]).is_err());
            changed = row;
            changed.text_offset += 1;
            assert!(check(stack, &[changed]).is_err());
            let mut changed_stack = stack;
            changed_stack.local_peak_bytes += 16;
            assert!(check(changed_stack, &[row]).is_err());
            assert!(check(stack, &[]).is_err());
            assert!(check(stack, &[row, row]).is_err());
            assert!(
                validate(
                    true,
                    (frame_bytes, 16, true),
                    architecture,
                    &[&call],
                    Some(ObjectUnitStack {
                        frame_bytes: frame_bytes as u32,
                        local_peak_bytes: peak,
                        stack_alignment: 16
                    }),
                    None,
                    &[],
                    &[row]
                )
                .is_err()
            );
        }
    }

    #[test]
    fn retained_call_prefix_and_local_peak_cannot_be_changed_or_removed() {
        for architecture in [Architecture::X86_64, Architecture::Aarch64] {
            let call = call(architecture);
            let (frame_bytes, transient_bytes, peak) = match architecture {
                Architecture::X86_64 => (24, 8, 32),
                Architecture::Aarch64 => (16, 0, 16),
            };
            let stack = ObjectUnitStack {
                frame_bytes,
                local_peak_bytes: peak,
                stack_alignment: 16,
            };
            let row = ObjectUnitCallStack {
                owner: target_operations::CallSiteOwner::Operation(call.operation),
                target: call.callee,
                text_offset: call.field_section_offset as usize,
                active_frame_bytes: frame_bytes,
                transient_bytes,
                caller_live_bytes: peak,
            };
            let check = |stack, rows: &[ObjectUnitCallStack]| {
                validate(
                    true,
                    (u64::from(frame_bytes), 16, true),
                    architecture,
                    &[&call],
                    Some(stack),
                    None,
                    rows,
                    &[],
                )
            };
            check(stack, &[row]).unwrap();
            let mut prefix = row;
            prefix.active_frame_bytes += 16;
            prefix.caller_live_bytes += 16;
            assert!(check(stack, &[prefix]).is_err());
            let mut transient = row;
            transient.transient_bytes += 16;
            transient.caller_live_bytes += 16;
            assert!(check(stack, &[transient]).is_err());
            let mut peak = stack;
            peak.local_peak_bytes += 16;
            assert!(check(peak, &[row]).is_err());
            assert!(check(stack, &[]).is_err());
        }
    }
}
