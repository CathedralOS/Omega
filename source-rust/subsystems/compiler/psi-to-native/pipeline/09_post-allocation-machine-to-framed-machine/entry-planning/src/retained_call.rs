//! An ordinary call can consume an inline job and write back its result.
//! Its native call frames still require provisioned execution storage: this
//! transport alone is not a Fixed activation or permission to use a host stack.
use crate::native_foreign_bridge::{NativeForeignBridgeError as Error, NativeForeignBridgeRequest};
use calling_conventions::{ValueClass, ValueShape};
use post_allocation_machine::adapters::AdapterInstructions;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReturningJobHomes {
    argument: u32,
    result: u32,
    bytes: u32,
    alignment: u16,
}

impl ReturningJobHomes {
    /// The returned loan has the caller's full original length, not just
    /// the prefix used by the worker. The first word retains that length.
    pub const BACKING_LENGTH: u32 = 0;

    pub const fn argument_offset(&self) -> u32 {
        self.argument
    }
    pub const fn result_offset(&self) -> u32 {
        self.result
    }
    pub const fn byte_size(&self) -> u32 {
        self.bytes
    }
    pub const fn alignment(&self) -> u16 {
        self.alignment
    }

    pub fn fits(&self, base: u64, length: u64) -> bool {
        base != 0
            && base.is_multiple_of(u64::from(self.alignment))
            && u64::from(self.bytes) <= length
            && base.checked_add(length).is_some()
    }

    fn pack(argument: Option<ValueShape>, result: Option<ValueShape>) -> Result<Self, Error> {
        let mut end = 8u32;
        let mut alignment = 8u16;
        let mut add = |shape: Option<ValueShape>| {
            let Some(shape) = shape else { return Ok(end) };
            if !matches!(shape.class, ValueClass::Integer | ValueClass::Float)
                || !shape.alignment.is_power_of_two()
                || shape.byte_size == 0
            {
                return Err(Error::UnsupportedValueTransport);
            }
            alignment = alignment.max(shape.alignment);
            let offset = end
                .checked_next_multiple_of(u32::from(shape.alignment))
                .ok_or(Error::FrameOverflow)?;
            end = offset
                .checked_add(shape.byte_size)
                .ok_or(Error::FrameOverflow)?;
            Ok(offset)
        };
        let argument = add(argument)?;
        let result = add(result)?;
        Ok(Self {
            argument,
            result,
            bytes: end,
            alignment,
        })
    }
}

/// The direct call instruction has one relocation. The image producer must
/// link it to the prepared worker's checked entry. No address is read from
/// the job and this recipe alone grants no right to step an owner.
#[derive(Debug, Clone)]
pub struct ReturningJobBridge {
    homes: ReturningJobHomes,
    instructions: AdapterInstructions,
    frame_bytes: u32,
}

impl ReturningJobBridge {
    pub const fn homes(&self) -> ReturningJobHomes {
        self.homes
    }
    pub fn instructions(&self) -> &AdapterInstructions {
        &self.instructions
    }
    pub const fn frame_bytes(&self) -> u32 {
        self.frame_bytes
    }
}

pub fn plan_returning_job_bridge(
    request: NativeForeignBridgeRequest,
) -> Result<ReturningJobBridge, Error> {
    use target::NativeTarget;
    if ![NativeTarget::linux_x64(), NativeTarget::windows_x64()].contains(&request.target) {
        return Err(Error::IncompatiblePlan);
    }
    let call = &request.foreign.plan().call;
    let argument = match (
        call.parameters.as_slice(),
        request.scalar_parameters.as_slice(),
    ) {
        ([], []) => None,
        ([job], [Some(_)]) => Some(job.shape),
        _ => return Err(Error::UnsupportedValueTransport),
    };
    let homes = ReturningJobHomes::pack(argument, call.result.as_ref().map(|r| r.shape))?;
    let code = crate::native_foreign_bridge::plan_call(request, Some(homes))?;
    Ok(ReturningJobBridge {
        homes,
        instructions: code.instructions,
        frame_bytes: code.frame_bytes,
    })
}

/// Keep the direct target, its published calling convention and its entry
/// adapter together. Ordinary call frames are charged to the stepping caller;
/// they never borrow the retained slice as a hardware stack.
#[derive(Debug)]
pub struct ReturningWorker {
    pub psi: terminal_psi::TerminalPsiIdentity,
    pub worker: semantic_vocabulary::MachineId,
    bridge: ReturningJobBridge,
    entry: crate::callable_entry::PlannedCallableEntryTransport,
    stack_bytes: u64,
}

impl ReturningWorker {
    pub fn for_body(
        frame: &crate::PhysicalFrameProgram,
        entry: crate::callable_entry::PlannedCallableEntryTransport,
    ) -> Result<Self, &'static str> {
        use calling_conventions::{
            CallSignature, CallingPolicy, evaluate_ordinary_boundary_entry_plan,
        };
        use frame_planning::stack_demand::{
            derive_physical_stack_demand, plan_physical_stack_program_for_entry,
        };
        let physical = &frame.instructions;
        let callable = entry.clone().into_parts();
        let context = &callable.context;
        let worker = context.entry;
        if context.psi != physical.psi
            || callable
                .cpu
                .require_body(frame, worker, context.inbound.state.preemption)
                .is_err()
        {
            return Err("returning worker entry belongs to another physical program");
        }
        let envelope = physical
            .contracts
            .publication()
            .code_catalogs()
            .callables()
            .machine_envelope(worker)
            .ok_or("returning worker needs its published suspension contract")?;
        if envelope.suspends {
            return Err("a worker that may park needs a resumable entry");
        }
        let abi = physical
            .contracts
            .function_abi(worker)
            .ok_or("returning worker lacks its checked calling convention")?;
        let scalar = abi
            .scalar
            .as_ref()
            .ok_or("returning worker requires a closed scalar calling convention")?;
        let target = physical.target;
        let signature = CallSignature {
            parameters: scalar
                .parameters
                .iter()
                .map(|arg| arg.placement.shape)
                .collect(),
            result: scalar.result.as_ref().map(|arg| arg.placement.shape),
        };
        let foreign = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::native_for_target(target),
            &signature,
        )
        .map_err(|_| "returning worker has no ordinary target call")?;
        if foreign.plan() != &context.inbound {
            return Err("ordinary call differs from the selected worker entry");
        }
        let bridge = plan_returning_job_bridge(NativeForeignBridgeRequest {
            target,
            foreign,
            scalar_parameters: scalar
                .parameters
                .iter()
                .map(|a| Some(a.scalar_type))
                .collect(),
        })
        .map_err(|_| "returning worker cannot transport its retained arguments or result")?;
        let program = plan_physical_stack_program_for_entry(frame, worker)?;
        let body = derive_physical_stack_demand(&program, worker, vec![])
            .map_err(|_| "returning worker has unresolved nested call frames")?;
        let link = u64::from(physical_recipes::native_entry_bridge::call_stack_bias(
            target.architecture,
        ));
        let entry_bytes = callable
            .bridge
            .as_ref()
            .map_or(0, |b| u64::from(b.frame_bytes()) + link);
        // The outer step calls the adapter, which calls the published entry.
        // A non-identity entry then makes one more call into its worker body.
        let stack_bytes = body
            .ceiling_bytes()
            .and_then(|bytes| bytes.checked_add(entry_bytes))
            .and_then(|bytes| bytes.checked_add(u64::from(bridge.frame_bytes()) + 2 * link))
            .ok_or("returning worker has no bounded ordinarily-called frame")?;
        Ok(Self {
            psi: physical.psi,
            worker,
            bridge,
            entry,
            stack_bytes,
        })
    }

    pub const fn bridge(&self) -> &ReturningJobBridge {
        &self.bridge
    }
    pub const fn entry(&self) -> &crate::callable_entry::PlannedCallableEntryTransport {
        &self.entry
    }
    pub const fn stack_bytes(&self) -> u64 {
        self.stack_bytes
    }
}
