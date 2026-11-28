//! Lay out one caller-owned fixed stack. Geometry is not a platform switch permit.
use crate::callable_entry::PlannedCallableEntryTransport;
use calling_conventions::{CallingPolicy, EntryControl, MachineRegister, ValueShape};
use frame_planning::stack_demand::{
    derive_physical_stack_demand, plan_physical_stack_program_for_entry,
};
use target::NativeTarget;

/// A SysV caller can keep these registers live across a call. Each side of
/// the transfer needs its own saved values and floating-point controls.
pub const SAVED_REGISTERS: [MachineRegister; 7] = [
    MachineRegister::X86Rsp,
    MachineRegister::X86Rbx,
    MachineRegister::X86Rbp,
    MachineRegister::X86R12,
    MachineRegister::X86R13,
    MachineRegister::X86R14,
    MachineRegister::X86R15,
];
const WORD: u32 = 8;
pub const MXCSR: u32 = SAVED_REGISTERS.len() as u32 * WORD;
pub const X87_CONTROL: u32 = MXCSR + 4;
const CONTEXT_BYTES: u32 = MXCSR + WORD;

// All offsets are from the start of the exclusive retained byte loan.
pub const CALLER_CONTEXT: u32 = 0;
pub const WORKER_CONTEXT: u32 = CALLER_CONTEXT + CONTEXT_BYTES;
pub const BACKING_LENGTH: u32 = WORKER_CONTEXT + CONTEXT_BYTES;
pub const PENDING_PUBLICATION: u32 = BACKING_LENGTH + WORD;
pub const REPLY_DESTINATION: u32 = PENDING_PUBLICATION + WORD;
const CONTROL_BYTES: u32 = REPLY_DESTINATION + WORD;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedActivationLayout {
    argument_offset: u64,
    result_offset: u64,
    stack_bottom: u64,
    stack_top: u64,
    alignment: u64,
}

impl FixedActivationLayout {
    pub const fn argument_offset(&self) -> u64 {
        self.argument_offset
    }
    pub const fn result_offset(&self) -> u64 {
        self.result_offset
    }
    pub const fn stack_range(&self) -> std::ops::Range<u64> {
        self.stack_bottom..self.stack_top
    }
    pub const fn alignment(&self) -> u64 {
        self.alignment
    }

    /// Size the retained homes from the worker's actual entry and measured
    /// physical call graph. An unbounded hosted call cannot run on these bytes.
    pub fn for_worker(
        frame: &crate::PhysicalFrameProgram,
        entry: PlannedCallableEntryTransport,
    ) -> Result<Self, &'static str> {
        let entry = entry.into_parts();
        let call = &entry.context.inbound.call;
        if frame.instructions.psi != entry.context.psi
            || entry
                .cpu
                .require_body(
                    frame,
                    entry.context.entry,
                    entry.context.inbound.state.preemption,
                )
                .is_err()
        {
            return Err("fixed activation entry belongs to another physical worker");
        }
        if frame.instructions.target != NativeTarget::linux_x64()
            || call.policy != CallingPolicy::SystemVAMD64
            || call.entry_control != EntryControl::CallReturn
            || call.stack_alignment != 16
            || call.shadow_bytes != 0
        {
            return Err("fixed activation has no stack-transfer layout for this target entry");
        }
        let argument = match call.parameters.as_slice() {
            [] => None, // An erased job needs no payload home.
            [argument] => Some(argument),
            _ => return Err("fixed activation requires its single checked job argument"),
        };
        if argument.is_some_and(|a| !a.is_complete_scalar_result())
            || call
                .result
                .as_ref()
                .is_some_and(|r| !r.is_complete_scalar_result())
        {
            return Err("fixed activation entry requires unsupported stack argument placement");
        }
        let bridge_bytes = entry.bridge.as_ref().map_or(0, |bridge| {
            u64::from(bridge.frame_bytes()) + u64::from(bridge.call_stack_bias())
        });
        let program = plan_physical_stack_program_for_entry(frame, entry.context.entry)?;
        // A callback with independent stack demands needs its own measured
        // physical body. Until that body is supplied, composition refuses it.
        let body = derive_physical_stack_demand(&program, entry.context.entry, vec![])
            .map_err(|_| "fixed activation cannot close the physical worker call graph")?;
        let stack = body
            .ceiling_bytes()
            .ok_or("fixed activation needs a bounded worker call graph")?
            .checked_add(bridge_bytes)
            .and_then(|n| n.checked_add(u64::from(WORD)))
            .ok_or("fixed activation entry stack size overflows")?;
        Self::pack(
            argument.map(|a| a.shape),
            call.result.as_ref().map(|r| r.shape),
            stack,
            u64::from(body.stack_alignment()),
        )
    }

    fn pack(
        argument: Option<ValueShape>,
        result: Option<ValueShape>,
        stack: u64,
        stack_alignment: u64,
    ) -> Result<Self, &'static str> {
        let mut end = u64::from(CONTROL_BYTES);
        let mut alignment = u64::from(WORD);
        let mut reserve = |bytes: u64, align: u64| -> Result<u64, &'static str> {
            if !align.is_power_of_two() {
                return Err("fixed activation home has invalid alignment");
            }
            alignment = alignment.max(align);
            let offset = end
                .checked_next_multiple_of(align)
                .ok_or("fixed activation home alignment overflows")?;
            end = offset
                .checked_add(bytes)
                .ok_or("fixed activation home size overflows")?;
            Ok(offset)
        };
        let mut home = |value: Option<ValueShape>| match value {
            // A borrow retains its pointer, not a copy of the referent.
            Some(v)
                if matches!(
                    v.class,
                    calling_conventions::ValueClass::BorrowedReference
                        | calling_conventions::ValueClass::BorrowedScalarReference
                ) =>
            {
                reserve(u64::from(WORD), u64::from(WORD))
            }
            Some(v) => reserve(u64::from(v.byte_size), u64::from(v.alignment)),
            None => reserve(0, 1),
        };
        let argument_offset = home(argument)?;
        let result_offset = home(result)?;
        let stack_bottom = reserve(stack, stack_alignment)?;
        let stack_top = end
            .checked_next_multiple_of(stack_alignment)
            .ok_or("fixed activation stack top overflows")?;
        Ok(Self {
            argument_offset,
            result_offset,
            stack_bottom,
            stack_top,
            alignment,
        })
    }

    /// Check the full view before consuming the job or writing either context.
    /// The retained loan, not a successful numeric check, provides custody.
    pub fn fits(&self, base: u64, available: u64) -> bool {
        base != 0
            && base.is_multiple_of(self.alignment)
            && available >= self.stack_top
            && base.checked_add(self.stack_top).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retained_control_and_payloads_end_before_the_aligned_worker_stack() {
        let layout = FixedActivationLayout::pack(
            Some(ValueShape::integer_aggregate(24, 16)),
            Some(ValueShape::integer(8, 8)),
            113,
            16,
        )
        .unwrap();
        assert_eq!(
            (
                layout.argument_offset,
                layout.result_offset,
                layout.stack_bottom,
                layout.stack_top
            ),
            (160, 184, 192, 320)
        );
        assert!(layout.fits(0x1000, 320));
        assert!(!layout.fits(0x1000, 319));
        assert!(!layout.fits(0x1008, 320));
        assert!(!layout.fits(u64::MAX - 15, 320));
        assert!(
            FixedActivationLayout::pack(Some(ValueShape::integer(1, 1)), None, u64::MAX, 16)
                .is_err()
        );
    }
}
