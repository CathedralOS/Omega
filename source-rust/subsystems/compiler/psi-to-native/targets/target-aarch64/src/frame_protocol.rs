//! Validate the fixed frame and choose the registers used to address it.
use ::register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Aarch64FrameSlot {
    pub view: RegisterViewId,
    pub offset_bytes: u64,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Aarch64StackProbe {
    pub interval_bytes: u64,
    pub touches: u32,
}

pub const MAX_STACK_PROBE_INTERVAL_BYTES: u64 = 65_536;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Aarch64FrameProtocolError {
    PhysicalRegisterModelMismatch,
    InvalidFrameSize,
    NonCanonicalSlots,
    NonCanonicalProbe,
    UnknownOrUnsupportedView(RegisterViewId),
    OffsetOutOfRange,
}

diagnostics::failed_display!(
    Aarch64FrameProtocolError,
    "AArch64 frame protocol encoding failed"
);

impl std::error::Error for Aarch64FrameProtocolError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aarch64FrameProtocolPlan {
    model: register_model::PhysicalRegisterModelIdentity,
    frame_size_bytes: u64,
    probe: Aarch64StackProbe,
    slots: Vec<Aarch64FrameSlot>,
    scratch: Option<u8>,
}

impl Aarch64FrameProtocolPlan {
    pub fn address_scratch(&self) -> Option<u8> {
        self.scratch
    }
}

pub fn plan_aapcs64_frame_protocol(
    model: &ValidatedPhysicalRegisterModel,
    frame_size_bytes: u64,
    probe: Aarch64StackProbe,
    slots: &[Aarch64FrameSlot],
) -> Result<Aarch64FrameProtocolPlan, Aarch64FrameProtocolError> {
    require_canonical_model(model)?;
    if !frame_size_bytes.is_multiple_of(16) || frame_size_bytes > u64::from(u32::MAX) {
        return Err(Aarch64FrameProtocolError::InvalidFrameSize);
    }
    validate_probe(frame_size_bytes, probe)?;
    validate_slots(frame_size_bytes, slots)?;
    let scratch = frame_scratch(model, slots)?;
    Ok(Aarch64FrameProtocolPlan {
        model: model.identity(),
        frame_size_bytes,
        probe,
        slots: slots.to_vec(),
        scratch,
    })
}

fn validate_probe(
    frame_size_bytes: u64,
    probe: Aarch64StackProbe,
) -> Result<(), Aarch64FrameProtocolError> {
    if !probe.interval_bytes.is_power_of_two()
        || probe.interval_bytes > MAX_STACK_PROBE_INTERVAL_BYTES
    {
        return Err(Aarch64FrameProtocolError::NonCanonicalProbe);
    }
    let expected = if frame_size_bytes > probe.interval_bytes {
        frame_size_bytes.div_ceil(probe.interval_bytes)
    } else {
        0
    };
    if u64::from(probe.touches) != expected {
        return Err(Aarch64FrameProtocolError::NonCanonicalProbe);
    }
    Ok(())
}

fn validate_slots(
    frame_size_bytes: u64,
    slots: &[Aarch64FrameSlot],
) -> Result<(), Aarch64FrameProtocolError> {
    let mut previous_end = 0_u64;
    for (index, slot) in slots.iter().enumerate() {
        let end = slot
            .offset_bytes
            .checked_add(slot.size_bytes)
            .ok_or(Aarch64FrameProtocolError::OffsetOutOfRange)?;
        if slot.size_bytes != 8
            || !slot.offset_bytes.is_multiple_of(8)
            || end > frame_size_bytes
            || (index != 0 && slot.offset_bytes < previous_end)
        {
            return Err(Aarch64FrameProtocolError::NonCanonicalSlots);
        }
        previous_end = end;
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
pub enum Register {
    X(u8),
    D(u8),
    Q(u8),
}

pub fn require_canonical_model(
    model: &ValidatedPhysicalRegisterModel,
) -> Result<(), Aarch64FrameProtocolError> {
    if model.identity() != crate::canonical_aarch64_physical_register_model_identity() {
        return Err(Aarch64FrameProtocolError::PhysicalRegisterModelMismatch);
    }
    Ok(())
}

pub fn register(
    model: &ValidatedPhysicalRegisterModel,
    view: RegisterViewId,
) -> Result<Register, Aarch64FrameProtocolError> {
    let name = model
        .model()
        .view(view)
        .map(|row| row.name.as_str())
        .ok_or(Aarch64FrameProtocolError::UnknownOrUnsupportedView(view))?;
    let (prefix, number) = name.split_at(1);
    let number = number
        .parse::<u8>()
        .map_err(|_| Aarch64FrameProtocolError::UnknownOrUnsupportedView(view))?;
    match (prefix, number) {
        ("x", 0..=30) => Ok(Register::X(number)),
        ("d", 0..=31) => Ok(Register::D(number)),
        ("q", 0..=31) => Ok(Register::Q(number)),
        _ => Err(Aarch64FrameProtocolError::UnknownOrUnsupportedView(view)),
    }
}

fn frame_scratch(
    model: &ValidatedPhysicalRegisterModel,
    slots: &[Aarch64FrameSlot],
) -> Result<Option<u8>, Aarch64FrameProtocolError> {
    if slots.iter().all(|slot| slot.offset_bytes / 8 <= 4095) {
        return Ok(None);
    }
    let mut available = [true, true];
    for slot in slots {
        match register(model, slot.view)? {
            Register::X(16) => available[0] = false,
            Register::X(17) => available[1] = false,
            _ => {}
        }
    }
    if available[0] {
        Ok(Some(16))
    } else if available[1] {
        Ok(Some(17))
    } else {
        Err(Aarch64FrameProtocolError::NonCanonicalSlots)
    }
}
impl Aarch64FrameProtocolPlan {
    pub fn model(&self) -> ::register_model::PhysicalRegisterModelIdentity {
        self.model
    }
    pub fn frame_size_bytes(&self) -> u64 {
        self.frame_size_bytes
    }
    pub fn probe(&self) -> Aarch64StackProbe {
        self.probe
    }
    pub fn slots(&self) -> &[Aarch64FrameSlot] {
        &self.slots
    }
}
