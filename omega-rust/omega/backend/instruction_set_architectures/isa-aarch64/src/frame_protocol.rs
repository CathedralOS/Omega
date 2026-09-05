//! Canonical AAPCS64 fixed-frame save/restore encoding.

use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Aarch64FrameSlot {
    pub view: RegisterViewId,
    pub offset_bytes: u64,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Aarch64FrameProtocolError {
    PhysicalRegisterModelMismatch,
    InvalidFrameSize,
    NonCanonicalSlots,
    UnknownOrUnsupportedView(RegisterViewId),
    OffsetOutOfRange,
}

impl std::fmt::Display for Aarch64FrameProtocolError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "AArch64 frame protocol encoding failed: {self:?}"
        )
    }
}

impl std::error::Error for Aarch64FrameProtocolError {}

/// Encode one canonical fixed AAPCS64 frame. The returned epilogue excludes
/// `ret`; the selected return instruction retains that semantic operation.
pub fn encode_aapcs64_frame_protocol(
    model: &ValidatedPhysicalRegisterModel,
    frame_size_bytes: u64,
    slots: &[Aarch64FrameSlot],
) -> Result<(Vec<u8>, Vec<u8>), Aarch64FrameProtocolError> {
    if model.model() != &crate::aarch64_physical_register_model() {
        return Err(Aarch64FrameProtocolError::PhysicalRegisterModelMismatch);
    }
    if !frame_size_bytes.is_multiple_of(16) || frame_size_bytes > 4095 {
        return Err(Aarch64FrameProtocolError::InvalidFrameSize);
    }
    validate_slots(frame_size_bytes, slots)?;
    let mut prologue = Vec::new();
    let mut epilogue = Vec::new();
    if frame_size_bytes != 0 {
        append_word(
            &mut prologue,
            0xd100_03ff | ((frame_size_bytes as u32) << 10),
        );
    }
    for slot in slots {
        append_memory(
            &mut prologue,
            register(model, slot.view)?,
            slot.offset_bytes,
            false,
        )?;
    }
    for slot in slots.iter().rev() {
        append_memory(
            &mut epilogue,
            register(model, slot.view)?,
            slot.offset_bytes,
            true,
        )?;
    }
    if frame_size_bytes != 0 {
        append_word(
            &mut epilogue,
            0x9100_03ff | ((frame_size_bytes as u32) << 10),
        );
    }
    Ok((prologue, epilogue))
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
            || slot.offset_bytes / 8 > 4095
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
enum Register {
    X(u8),
    D(u8),
}

fn register(
    model: &ValidatedPhysicalRegisterModel,
    view: RegisterViewId,
) -> Result<Register, Aarch64FrameProtocolError> {
    let name = model
        .model()
        .views
        .iter()
        .find(|row| row.id == view)
        .map(|row| row.name.as_str())
        .ok_or(Aarch64FrameProtocolError::UnknownOrUnsupportedView(view))?;
    let (prefix, number) = name.split_at(1);
    let number = number
        .parse::<u8>()
        .map_err(|_| Aarch64FrameProtocolError::UnknownOrUnsupportedView(view))?;
    match (prefix, number) {
        ("x", 0..=30) => Ok(Register::X(number)),
        ("d", 0..=31) => Ok(Register::D(number)),
        _ => Err(Aarch64FrameProtocolError::UnknownOrUnsupportedView(view)),
    }
}

fn append_memory(
    bytes: &mut Vec<u8>,
    register: Register,
    offset_bytes: u64,
    load: bool,
) -> Result<(), Aarch64FrameProtocolError> {
    if !offset_bytes.is_multiple_of(8) || offset_bytes / 8 > 4095 {
        return Err(Aarch64FrameProtocolError::OffsetOutOfRange);
    }
    let scaled = (offset_bytes / 8) as u32;
    let (base, number) = match (register, load) {
        (Register::X(number), false) => (0xf900_03e0, number),
        (Register::X(number), true) => (0xf940_03e0, number),
        (Register::D(number), false) => (0xfd00_03e0, number),
        (Register::D(number), true) => (0xfd40_03e0, number),
    };
    append_word(bytes, base | (scaled << 10) | u32::from(number));
    Ok(())
}

fn append_word(bytes: &mut Vec<u8>, word: u32) {
    bytes.extend_from_slice(&word.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use register_model::validate_physical_register_model;

    #[test]
    fn canonical_frame_encodes_sp_adjustment_and_link_custody() {
        let model =
            validate_physical_register_model(crate::aarch64_physical_register_model()).unwrap();
        let x19 = model.model().view_named("x19").unwrap().id;
        let x30 = model.model().view_named("x30").unwrap().id;
        let (prologue, epilogue) = encode_aapcs64_frame_protocol(
            &model,
            16,
            &[
                Aarch64FrameSlot {
                    view: x19,
                    offset_bytes: 0,
                    size_bytes: 8,
                },
                Aarch64FrameSlot {
                    view: x30,
                    offset_bytes: 8,
                    size_bytes: 8,
                },
            ],
        )
        .unwrap();
        assert_eq!(&prologue[..4], &0xd100_43ff_u32.to_le_bytes());
        assert_eq!(
            &epilogue[epilogue.len() - 4..],
            &0x9100_43ff_u32.to_le_bytes()
        );
    }

    #[test]
    fn unaligned_frame_rejects() {
        let model =
            validate_physical_register_model(crate::aarch64_physical_register_model()).unwrap();
        let x19 = model.model().view_named("x19").unwrap().id;
        assert_eq!(
            encode_aapcs64_frame_protocol(
                &model,
                8,
                &[Aarch64FrameSlot {
                    view: x19,
                    offset_bytes: 0,
                    size_bytes: 8,
                }],
            ),
            Err(Aarch64FrameProtocolError::InvalidFrameSize)
        );
    }
}
