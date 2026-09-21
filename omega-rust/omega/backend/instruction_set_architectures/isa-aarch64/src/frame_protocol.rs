//! Canonical AAPCS64 fixed-frame save/restore encoding.

use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Aarch64FrameSlot {
    pub view: RegisterViewId,
    pub offset_bytes: u64,
    pub size_bytes: u64,
}

/// Stack-commit probe roster for one frame, in the order the prologue
/// performs it. The committed-stack granule is target-owned — Darwin AArch64
/// commits 16 KiB pages where Linux AArch64 commits 4 KiB pages — so the
/// interval arrives from the validated layout instead of a codec constant.
/// A frame larger than one granule is committed one granule at a time,
/// reading each newly entered page so lazily backed stacks grow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Aarch64StackProbe {
    /// Stack-commit granule the roster probes, in bytes.
    pub interval_bytes: u64,
    /// Ordered touches, one per committed chunk, each at the new stack
    /// pointer position after that chunk is subtracted.
    pub touches: u32,
}

/// One `add`/`sub` immediate pair covers any adjustment through 16 MiB - 1:
/// `op sp, sp, #(amount >> 12), lsl #12` then `op sp, sp, #(amount & 0xfff)`.
/// Probed frames subtract one granule chunk per instruction pair, so the
/// bound also caps the largest commit granule the codec can emit. The same
/// pair bounds `FrameAddress` displacements: an address one shifted plus one
/// unshifted `add` away from `sp` is exactly as far as the protocol can move
/// `sp`.
pub(crate) const MAX_STACK_POINTER_ADJUST_BYTES: u64 = (4095 << 12) | 4095;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Aarch64FrameProtocolError {
    PhysicalRegisterModelMismatch,
    InvalidFrameSize,
    NonCanonicalSlots,
    NonCanonicalProbe,
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
/// `probe` is the validated layout's committed-stack roster: it must name the
/// exact granule and touch count this frame size commits under, because the
/// encoder emits a probe touch after every chunk it subtracts.
pub fn encode_aapcs64_frame_protocol(
    model: &ValidatedPhysicalRegisterModel,
    frame_size_bytes: u64,
    probe: Aarch64StackProbe,
    slots: &[Aarch64FrameSlot],
) -> Result<(Vec<u8>, Vec<u8>), Aarch64FrameProtocolError> {
    if model.identity() != crate::canonical_aarch64_physical_register_model_identity() {
        return Err(Aarch64FrameProtocolError::PhysicalRegisterModelMismatch);
    }
    if !frame_size_bytes.is_multiple_of(16) || frame_size_bytes > u64::from(u32::MAX) {
        return Err(Aarch64FrameProtocolError::InvalidFrameSize);
    }
    validate_probe(frame_size_bytes, probe)?;
    validate_slots(frame_size_bytes, slots)?;
    let mut prologue = Vec::new();
    let mut epilogue = Vec::new();
    if probe.touches == 0 {
        append_stack_adjust(&mut prologue, frame_size_bytes, true)?;
    } else {
        let mut remaining = frame_size_bytes;
        loop {
            let chunk = remaining.min(probe.interval_bytes);
            append_stack_adjust(&mut prologue, chunk, true)?;
            append_probe_touch(&mut prologue);
            remaining -= chunk;
            if remaining == 0 {
                break;
            }
        }
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
    append_stack_adjust(&mut epilogue, frame_size_bytes, false)?;
    Ok((prologue, epilogue))
}

/// The roster must be the exact commit schedule for this frame size: no
/// touches inside one granule, and exactly one touch per granule chunk past
/// it so the emitted prologue cannot skip or pad the commit sequence. The
/// granule itself must be a page-shaped interval a single subtract pair can
/// carry.
fn validate_probe(
    frame_size_bytes: u64,
    probe: Aarch64StackProbe,
) -> Result<(), Aarch64FrameProtocolError> {
    if !probe.interval_bytes.is_power_of_two()
        || probe.interval_bytes > MAX_STACK_POINTER_ADJUST_BYTES
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

/// `ldr xzr, [sp]` reads the lowest newly committed byte: a read is enough to
/// fault on the guard region, and the zero register discards the loaded value.
fn append_probe_touch(bytes: &mut Vec<u8>) {
    append_word(bytes, 0xf940_03ff);
}

/// Emit `sub`/`add sp, sp, #amount` across the shifted then unshifted
/// immediate forms, keeping the single-instruction encoding byte-identical
/// for every amount the existing protocol already admitted.
fn append_stack_adjust(
    bytes: &mut Vec<u8>,
    amount: u64,
    subtract: bool,
) -> Result<(), Aarch64FrameProtocolError> {
    if amount == 0 {
        return Ok(());
    }
    if amount > MAX_STACK_POINTER_ADJUST_BYTES {
        return Err(Aarch64FrameProtocolError::InvalidFrameSize);
    }
    let base = if subtract { 0xd100_03ff } else { 0x9100_03ff };
    let high = (amount >> 12) as u32;
    if high != 0 {
        append_word(bytes, base | (1 << 22) | (high << 10));
    }
    let low = (amount & 4095) as u32;
    if low != 0 {
        append_word(bytes, base | (low << 10));
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
    use super::{
        Aarch64FrameProtocolError, Aarch64FrameSlot, Aarch64StackProbe,
        MAX_STACK_POINTER_ADJUST_BYTES, encode_aapcs64_frame_protocol,
    };
    use register_model::validate_physical_register_model;

    const NO_PROBE: Aarch64StackProbe = Aarch64StackProbe {
        interval_bytes: 4_096,
        touches: 0,
    };
    const DARWIN_PROBE: Aarch64StackProbe = Aarch64StackProbe {
        interval_bytes: 16_384,
        touches: 0,
    };

    #[test]
    fn canonical_frame_encodes_sp_adjustment_and_link_custody() {
        let model =
            validate_physical_register_model(crate::aarch64_physical_register_model()).unwrap();
        let x19 = model.model().view_named("x19").unwrap().id;
        let x30 = model.model().view_named("x30").unwrap().id;
        let (prologue, epilogue) = encode_aapcs64_frame_protocol(
            &model,
            16,
            NO_PROBE,
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
                NO_PROBE,
                &[Aarch64FrameSlot {
                    view: x19,
                    offset_bytes: 0,
                    size_bytes: 8,
                }],
            ),
            Err(Aarch64FrameProtocolError::InvalidFrameSize)
        );
    }

    #[test]
    fn frame_past_one_instruction_uses_shifted_then_unshifted_adjust() {
        let model =
            validate_physical_register_model(crate::aarch64_physical_register_model()).unwrap();
        // 4208 = 4096 + 112: `sub sp, sp, #1, lsl #12` then `sub sp, sp, #112`.
        let (prologue, epilogue) =
            encode_aapcs64_frame_protocol(&model, 4_208, DARWIN_PROBE, &[]).unwrap();
        assert_eq!(
            &prologue,
            &[0xd140_07ff_u32.to_le_bytes(), 0xd101_c3ff_u32.to_le_bytes()].concat()
        );
        assert_eq!(
            &epilogue,
            &[0x9140_07ff_u32.to_le_bytes(), 0x9101_c3ff_u32.to_le_bytes()].concat()
        );
    }

    #[test]
    fn probed_frame_commits_one_granule_per_touch_then_saves() {
        let model =
            validate_physical_register_model(crate::aarch64_physical_register_model()).unwrap();
        let x19 = model.model().view_named("x19").unwrap().id;
        let probe = Aarch64StackProbe {
            interval_bytes: 4_096,
            touches: 2,
        };
        let (prologue, epilogue) = encode_aapcs64_frame_protocol(
            &model,
            8_192,
            probe,
            &[Aarch64FrameSlot {
                view: x19,
                offset_bytes: 0,
                size_bytes: 8,
            }],
        )
        .unwrap();
        // sub sp,sp,#1,lsl#12 ; ldr xzr,[sp] ; sub sp,sp,#1,lsl#12 ; ldr xzr,[sp] ; save
        let chunk = [0xd140_07ff_u32.to_le_bytes(), 0xf940_03ff_u32.to_le_bytes()].concat();
        assert_eq!(&prologue[..8], &chunk);
        assert_eq!(&prologue[8..16], &chunk);
        assert_eq!(&prologue[16..20], &0xf900_03f3_u32.to_le_bytes());
        assert_eq!(prologue.len(), 20);
        // The epilogue restores the whole frame in one adjustment: only the
        // growing direction needs the commit touches.
        assert_eq!(
            &epilogue[epilogue.len() - 4..],
            &0x9140_0bff_u32.to_le_bytes()
        );
    }

    #[test]
    fn probed_frame_touches_the_partial_tail_chunk() {
        let model =
            validate_physical_register_model(crate::aarch64_physical_register_model()).unwrap();
        let probe = Aarch64StackProbe {
            interval_bytes: 4_096,
            touches: 2,
        };
        let (prologue, _) = encode_aapcs64_frame_protocol(&model, 4_208, probe, &[]).unwrap();
        // sub sp,sp,#1,lsl#12 ; touch ; sub sp,sp,#112 ; touch
        assert_eq!(
            &prologue,
            &[
                0xd140_07ff_u32.to_le_bytes(),
                0xf940_03ff_u32.to_le_bytes(),
                0xd101_c3ff_u32.to_le_bytes(),
                0xf940_03ff_u32.to_le_bytes(),
            ]
            .concat()
        );
    }

    #[test]
    fn probe_roster_must_match_the_committed_frame() {
        let model =
            validate_physical_register_model(crate::aarch64_physical_register_model()).unwrap();
        for (frame_size, touches) in [(8_192_u64, 0), (8_192, 1), (8_192, 3), (4_096, 1), (16, 1)] {
            let probe = Aarch64StackProbe {
                interval_bytes: 4_096,
                touches,
            };
            assert_eq!(
                encode_aapcs64_frame_protocol(&model, frame_size, probe, &[]),
                Err(Aarch64FrameProtocolError::NonCanonicalProbe),
                "size {frame_size} touches {touches}"
            );
        }
        for interval in [0_u64, 1, 4_000, MAX_STACK_POINTER_ADJUST_BYTES + 1] {
            let probe = Aarch64StackProbe {
                interval_bytes: interval,
                touches: 1,
            };
            assert_eq!(
                encode_aapcs64_frame_protocol(&model, 8_192, probe, &[]),
                Err(Aarch64FrameProtocolError::NonCanonicalProbe),
                "interval {interval}"
            );
        }
    }
}
