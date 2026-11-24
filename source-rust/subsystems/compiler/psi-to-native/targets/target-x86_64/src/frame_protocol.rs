//! Validate the fixed frame and choose the registers used to address it.
use ::register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X86_64FrameSlot {
    pub view: RegisterViewId,
    pub offset_bytes: u64,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X86_64StackProbe {
    pub interval_bytes: u64,
    pub touches: u32,
}

pub const X86_64_STACK_PROBE_INTERVAL_BYTES: u64 = 4_096;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum X86_64FrameProtocolError {
    PhysicalRegisterModelMismatch,
    InvalidFrameSize,
    NonCanonicalSlots,
    NonCanonicalProbe,
    UnknownOrUnsupportedView(RegisterViewId),
    OffsetOutOfRange,
    UnsupportedTransferWidth(u8),
}

diagnostics::failed_display!(
    X86_64FrameProtocolError,
    "x86-64 frame protocol encoding failed"
);

impl std::error::Error for X86_64FrameProtocolError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X86_64FrameProtocolPlan {
    model: register_model::PhysicalRegisterModelIdentity,
    frame_size_bytes: u64,
    probe: X86_64StackProbe,
    slots: Vec<X86_64FrameSlot>,
}

pub fn plan_system_v_amd64_frame_protocol(
    model: &ValidatedPhysicalRegisterModel,
    frame_size_bytes: u64,
    probe: X86_64StackProbe,
    slots: &[X86_64FrameSlot],
) -> Result<X86_64FrameProtocolPlan, X86_64FrameProtocolError> {
    if model.identity() != crate::canonical_x86_64_physical_register_model_identity() {
        return Err(X86_64FrameProtocolError::PhysicalRegisterModelMismatch);
    }
    if frame_size_bytes > i32::MAX as u64 {
        return Err(X86_64FrameProtocolError::InvalidFrameSize);
    }
    validate_probe(frame_size_bytes, probe)?;
    validate_slots(frame_size_bytes, slots)?;
    Ok(X86_64FrameProtocolPlan {
        model: model.identity(),
        frame_size_bytes,
        probe,
        slots: slots.to_vec(),
    })
}

fn validate_probe(
    frame_size_bytes: u64,
    probe: X86_64StackProbe,
) -> Result<(), X86_64FrameProtocolError> {
    let expected = if frame_size_bytes > X86_64_STACK_PROBE_INTERVAL_BYTES {
        frame_size_bytes.div_ceil(X86_64_STACK_PROBE_INTERVAL_BYTES)
    } else {
        0
    };
    if probe.interval_bytes != X86_64_STACK_PROBE_INTERVAL_BYTES
        || u64::from(probe.touches) != expected
    {
        return Err(X86_64FrameProtocolError::NonCanonicalProbe);
    }
    Ok(())
}

fn validate_slots(
    frame_size_bytes: u64,
    slots: &[X86_64FrameSlot],
) -> Result<(), X86_64FrameProtocolError> {
    let mut previous_end = 0_u64;
    for (index, slot) in slots.iter().enumerate() {
        let end = slot
            .offset_bytes
            .checked_add(slot.size_bytes)
            .ok_or(X86_64FrameProtocolError::OffsetOutOfRange)?;
        if slot.size_bytes != 8
            || !slot.offset_bytes.is_multiple_of(8)
            || end > frame_size_bytes
            || (index != 0 && slot.offset_bytes < previous_end)
        {
            return Err(X86_64FrameProtocolError::NonCanonicalSlots);
        }
        previous_end = end;
    }
    Ok(())
}
impl X86_64FrameProtocolPlan {
    pub fn model(&self) -> ::register_model::PhysicalRegisterModelIdentity {
        self.model
    }
    pub fn frame_size_bytes(&self) -> u64 {
        self.frame_size_bytes
    }
    pub fn probe(&self) -> X86_64StackProbe {
        self.probe
    }
    pub fn slots(&self) -> &[X86_64FrameSlot] {
        &self.slots
    }
}
