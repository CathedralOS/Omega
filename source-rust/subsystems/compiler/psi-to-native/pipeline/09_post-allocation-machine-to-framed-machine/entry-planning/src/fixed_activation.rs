//! Lay out one caller-owned fixed stack. Geometry is not a platform switch permit.
mod wrapper;
use crate::{
    callable_entry::PlannedCallableEntryTransport,
    native_foreign_bridge::{self, InlineJob, NativeForeignBridgeRequest},
};
use calling_conventions::{
    CallSignature, CallingPolicy, EntryControl, ValueShape, evaluate_ordinary_boundary_entry_plan,
};
use frame_planning::stack_demand::{
    derive_physical_stack_demand, plan_physical_stack_program_for_entry,
};
use target::NativeTarget;
pub use wrapper::plan_call;

pub use physical_recipes::fixed_activation::{
    BACKING_LENGTH, CALLER_CONTEXT, MXCSR, PENDING_PUBLICATION, REPLY_DESTINATION, SAVED_REGISTERS,
    WORKER_CONTEXT, X87_CONTROL,
};
use physical_recipes::fixed_activation::{CONTROL_BYTES, FixedTransfer, FixedTransferPlan};
const WORD: u32 = 8;

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
        plan_transfer(frame, entry).map(|(layout, _)| layout)
    }

    fn measure(
        frame: &crate::PhysicalFrameProgram,
        entry: PlannedCallableEntryTransport,
    ) -> Result<(Self, FixedTransfer), &'static str> {
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
        let argument = argument.map(|value| value.shape);
        let result = call.result.as_ref().map(|value| value.shape);
        let signature = CallSignature {
            parameters: argument.into_iter().collect(),
            result,
        };
        let foreign = evaluate_ordinary_boundary_entry_plan(call.policy, &signature)
            .map_err(|_| "fixed worker has no closed native calling plan")?;
        if foreign.plan() != &entry.context.inbound {
            return Err("the fixed worker has a different published calling convention");
        }
        let abi = frame
            .instructions
            .contracts
            .function_abi(entry.context.entry)
            .ok_or("fixed worker lacks its published argument types")?;
        if abi.scalar_parameters.len() > 1 {
            return Err("fixed worker requires one job");
        }
        let scalar_parameters = argument
            .into_iter()
            .map(|_| abi.scalar_parameters.first().map(|value| value.scalar_type))
            .collect();
        let homes = Self::pack(argument, result, 0, u64::from(call.stack_alignment))?;
        let argument_offset = u32::try_from(homes.argument_offset)
            .map_err(|_| "fixed argument address exceeds target displacement")?;
        let result_offset = u32::try_from(homes.result_offset)
            .map_err(|_| "fixed result address exceeds target displacement")?;
        let marshal = native_foreign_bridge::plan_call(
            &NativeForeignBridgeRequest {
                target: frame.instructions.target,
                foreign,
                scalar_parameters,
            },
            Some(InlineJob {
                argument: argument_offset,
                result: result_offset,
            }),
        )
        .map_err(|_| "fixed worker cannot transport its job or result")?;
        let bridge_bytes = entry.bridge.as_ref().map_or(0, |bridge| {
            u64::from(bridge.frame_bytes()) + u64::from(bridge.call_stack_bias())
        });
        let program = plan_physical_stack_program_for_entry(frame, entry.context.entry)?;
        // A callback with independent stack demands needs its own measured
        // physical body. Until that body is supplied, composition refuses it.
        let body = derive_physical_stack_demand(&program, entry.context.entry, vec![])
            .map_err(|_| "fixed activation cannot close the physical worker call graph")?;
        // A callee-saved register can be reused inside an ordinary machine.
        // Park reads this private owner at the call, so verify the actual worker
        // code after register allocation, including every callable descendant.
        let owner_register = target_x86_64::validated_x86_64_physical_register_model()
            .model()
            .view_named("r12")
            .ok_or("fixed activation has no execution owner register")?;
        for machine in frame
            .instructions
            .machine
            .functions
            .iter()
            .filter(|m| body.contributing_machines().contains(&m.machine))
        {
            if machine.instructions().any(|row| {
                row.unit_defs
                    .iter()
                    .chain(&row.unit_clobbers)
                    .any(|unit| owner_register.units.contains(unit))
            }) {
                return Err("fixed worker modifies its live execution owner register");
            }
        }
        for machine in frame
            .instructions
            .functions
            .iter()
            .filter(|m| body.contributing_machines().contains(&m.machine))
        {
            if machine
                .normalized_foreign_calls
                .iter()
                .any(|row| !row.call.callbacks.is_empty())
            {
                return Err("a fixed worker cannot reenter its owner through a foreign callback");
            }
        }
        let stack = body
            .ceiling_bytes()
            .ok_or("fixed activation needs a bounded worker call graph")?
            .checked_add(bridge_bytes)
            // The step calls the job adapter, which calls the selected entry.
            .and_then(|n| n.checked_add(u64::from(marshal.frame_bytes) + 2 * u64::from(WORD)))
            .ok_or("fixed activation entry stack size overflows")?;
        let layout = Self::pack(argument, result, stack, u64::from(body.stack_alignment()))?;
        let displacement = i32::try_from(layout.stack_top)
            .map_err(|_| "fixed worker stack exceeds the target displacement range")?;
        let transfer = FixedTransfer {
            psi: frame.instructions.psi,
            worker: entry.context.entry,
            stack_top: displacement as u32,
            alignment: u32::try_from(layout.alignment).map_err(|_| "invalid fixed alignment")?,
            argument_offset,
            argument_bytes: argument.map_or(0, native_foreign_bridge::extent),
            result_offset,
            job: marshal.instructions,
        };
        Ok((layout, transfer))
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
            && base
                .checked_next_multiple_of(self.alignment)
                .and_then(|aligned| aligned.checked_add(self.stack_top))
                .zip(base.checked_add(available))
                .is_some_and(|(required, end)| required <= end)
    }
}

/// Bind the stack switch to the same machine whose frames and ABI were measured.
pub fn plan_transfer(
    frame: &crate::PhysicalFrameProgram,
    entry: PlannedCallableEntryTransport,
) -> Result<
    (
        FixedActivationLayout,
        FixedTransferPlan<crate::PhysicalRecipeOrigin>,
    ),
    &'static str,
> {
    let (layout, transfer) = FixedActivationLayout::measure(frame, entry)?;
    let plan = FixedTransferPlan::from_planner(crate::PhysicalRecipeOrigin(()), transfer);
    Ok((layout, plan))
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
            (192, 216, 224, 352)
        );
        assert!(layout.fits(0x1000, 352));
        assert!(!layout.fits(0x1000, 351));
        assert!(!layout.fits(0x1008, 352));
        assert!(!layout.fits(u64::MAX - 15, 352));
        assert!(layout.fits(0x1008, 360));
        assert!(!layout.fits(0x1000, u64::MAX));
        assert!(
            FixedActivationLayout::pack(Some(ValueShape::integer(1, 1)), None, u64::MAX, 16)
                .is_err()
        );
    }
}
