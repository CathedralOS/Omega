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

/// The entry takes one record address in the ordinary ABI. Its caller must
/// establish that the record fits these homes and remains exclusive during
/// the call. This adapter does not allocate backing or check its length.
pub fn plan_returning_job_bridge(
    request: &NativeForeignBridgeRequest,
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
