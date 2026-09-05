//! Canonical System V AMD64 fixed-frame save/restore encoding.

use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X86_64FrameSlot {
    pub view: RegisterViewId,
    pub offset_bytes: u64,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum X86_64FrameProtocolError {
    PhysicalRegisterModelMismatch,
    InvalidFrameSize,
    NonCanonicalSlots,
    UnknownOrUnsupportedView(RegisterViewId),
    OffsetOutOfRange,
}

impl std::fmt::Display for X86_64FrameProtocolError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "x86-64 frame protocol encoding failed: {self:?}")
    }
}

impl std::error::Error for X86_64FrameProtocolError {}

/// Encode one canonical fixed System V frame. The returned epilogue excludes
/// `ret`; the selected return instruction retains that semantic operation.
pub fn encode_system_v_amd64_frame_protocol(
    model: &ValidatedPhysicalRegisterModel,
    frame_size_bytes: u64,
    slots: &[X86_64FrameSlot],
) -> Result<(Vec<u8>, Vec<u8>), X86_64FrameProtocolError> {
    if model.model() != &crate::x86_64_physical_register_model() {
        return Err(X86_64FrameProtocolError::PhysicalRegisterModelMismatch);
    }
    if frame_size_bytes > u64::from(u32::MAX) {
        return Err(X86_64FrameProtocolError::InvalidFrameSize);
    }
    validate_slots(frame_size_bytes, slots)?;
    let mut prologue = Vec::new();
    let mut epilogue = Vec::new();
    append_stack_adjust(&mut prologue, frame_size_bytes, true)?;
    for slot in slots {
        append_register_memory(
            &mut prologue,
            register_code(model, slot.view)?,
            slot.offset_bytes,
            false,
        )?;
    }
    for slot in slots.iter().rev() {
        append_register_memory(
            &mut epilogue,
            register_code(model, slot.view)?,
            slot.offset_bytes,
            true,
        )?;
    }
    append_stack_adjust(&mut epilogue, frame_size_bytes, false)?;
    Ok((prologue, epilogue))
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

fn register_code(
    model: &ValidatedPhysicalRegisterModel,
    view: RegisterViewId,
) -> Result<u8, X86_64FrameProtocolError> {
    let name = model
        .model()
        .views
        .iter()
        .find(|row| row.id == view)
        .map(|row| row.name.as_str())
        .ok_or(X86_64FrameProtocolError::UnknownOrUnsupportedView(view))?;
    let code = match name {
        "rax" => 0,
        "rcx" => 1,
        "rdx" => 2,
        "rbx" => 3,
        "rbp" => 5,
        "rsi" => 6,
        "rdi" => 7,
        "r8" => 8,
        "r9" => 9,
        "r10" => 10,
        "r11" => 11,
        "r12" => 12,
        "r13" => 13,
        "r14" => 14,
        "r15" => 15,
        _ => return Err(X86_64FrameProtocolError::UnknownOrUnsupportedView(view)),
    };
    Ok(code)
}

fn append_stack_adjust(
    bytes: &mut Vec<u8>,
    amount: u64,
    subtract: bool,
) -> Result<(), X86_64FrameProtocolError> {
    if amount == 0 {
        return Ok(());
    }
    let operation = if subtract { 0xec } else { 0xc4 };
    if amount <= 127 {
        bytes.extend_from_slice(&[0x48, 0x83, operation, amount as u8]);
    } else {
        let amount =
            u32::try_from(amount).map_err(|_| X86_64FrameProtocolError::InvalidFrameSize)?;
        bytes.extend_from_slice(&[0x48, 0x81, operation]);
        bytes.extend_from_slice(&amount.to_le_bytes());
    }
    Ok(())
}

fn append_register_memory(
    bytes: &mut Vec<u8>,
    register: u8,
    offset: u64,
    load: bool,
) -> Result<(), X86_64FrameProtocolError> {
    let rex = 0x48 | ((register >> 3) << 2);
    bytes.push(rex);
    bytes.push(if load { 0x8b } else { 0x89 });
    let low = register & 7;
    if offset == 0 {
        bytes.push((low << 3) | 0x04);
        bytes.push(0x24);
    } else if offset <= 127 {
        bytes.push(0x40 | (low << 3) | 0x04);
        bytes.push(0x24);
        bytes.push(offset as u8);
    } else {
        let offset =
            u32::try_from(offset).map_err(|_| X86_64FrameProtocolError::OffsetOutOfRange)?;
        if offset > i32::MAX as u32 {
            return Err(X86_64FrameProtocolError::OffsetOutOfRange);
        }
        bytes.push(0x80 | (low << 3) | 0x04);
        bytes.push(0x24);
        bytes.extend_from_slice(&offset.to_le_bytes());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use register_model::validate_physical_register_model;

    #[test]
    fn canonical_frame_encodes_adjust_save_restore_and_inverse_adjust() {
        let model =
            validate_physical_register_model(crate::x86_64_physical_register_model()).unwrap();
        let rbx = model.model().view_named("rbx").unwrap().id;
        let r12 = model.model().view_named("r12").unwrap().id;
        let (prologue, epilogue) = encode_system_v_amd64_frame_protocol(
            &model,
            24,
            &[
                X86_64FrameSlot {
                    view: rbx,
                    offset_bytes: 0,
                    size_bytes: 8,
                },
                X86_64FrameSlot {
                    view: r12,
                    offset_bytes: 8,
                    size_bytes: 8,
                },
            ],
        )
        .unwrap();
        assert_eq!(&prologue[..4], &[0x48, 0x83, 0xec, 24]);
        assert_eq!(&epilogue[epilogue.len() - 4..], &[0x48, 0x83, 0xc4, 24]);
        assert_ne!(prologue, epilogue);
    }

    #[test]
    fn out_of_frame_slot_rejects() {
        let model =
            validate_physical_register_model(crate::x86_64_physical_register_model()).unwrap();
        let rbx = model.model().view_named("rbx").unwrap().id;
        assert_eq!(
            encode_system_v_amd64_frame_protocol(
                &model,
                8,
                &[X86_64FrameSlot {
                    view: rbx,
                    offset_bytes: 8,
                    size_bytes: 8,
                }],
            ),
            Err(X86_64FrameProtocolError::NonCanonicalSlots)
        );
    }
}
