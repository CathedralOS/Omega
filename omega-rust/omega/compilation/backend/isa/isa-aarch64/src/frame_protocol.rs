/*
We encode and validate the AAPCS64 fixed-frame prologue/epilogue protocol,
page-granule stack-commit probes (`Aarch64StackProbe`), and frame/pointer memory
accesses. Because Linux AArch64 (4 KiB) and Darwin AArch64 (16 KiB) commit stack
pages at different granules, we emit ordered probe loads across newly entered
pages before establishing callee-saved register slots (`Aarch64FrameSlot`) and
frame-relative addresses.
*/

use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};

use crate::immediate_offset::{
    ADD_IMMEDIATE, SUBTRACT_IMMEDIATE, append_offset_steps, append_page_steps,
};

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

pub(crate) const MAX_STACK_PROBE_INTERVAL_BYTES: u64 = 65_536;

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

pub fn encode_aapcs64_frame_protocol(
    model: &ValidatedPhysicalRegisterModel,
    frame_size_bytes: u64,
    probe: Aarch64StackProbe,
    slots: &[Aarch64FrameSlot],
) -> Result<(Vec<u8>, Vec<u8>), Aarch64FrameProtocolError> {
    require_canonical_model(model)?;
    if !frame_size_bytes.is_multiple_of(16) || frame_size_bytes > u64::from(u32::MAX) {
        return Err(Aarch64FrameProtocolError::InvalidFrameSize);
    }
    validate_probe(frame_size_bytes, probe)?;
    validate_slots(frame_size_bytes, slots)?;
    let scratch = frame_scratch(model, slots)?;
    let mut prologue = Vec::new();
    let mut epilogue = Vec::new();
    if probe.touches == 0 {
        append_stack_adjust(&mut prologue, frame_size_bytes, true);
    } else {
        let mut remaining = frame_size_bytes;
        loop {
            let chunk = remaining.min(probe.interval_bytes);
            append_stack_adjust(&mut prologue, chunk, true);
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
            scratch,
            false,
        )?;
    }
    for slot in slots.iter().rev() {
        append_memory(
            &mut epilogue,
            register(model, slot.view)?,
            slot.offset_bytes,
            scratch,
            true,
        )?;
    }
    append_stack_adjust(&mut epilogue, frame_size_bytes, false);
    Ok((prologue, epilogue))
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

fn append_probe_touch(bytes: &mut Vec<u8>) {
    append_word(bytes, 0xf940_03ff);
}

fn append_stack_adjust(bytes: &mut Vec<u8>, amount: u64, subtract: bool) {
    let mut words = Vec::new();
    let opcode = if subtract {
        SUBTRACT_IMMEDIATE
    } else {
        ADD_IMMEDIATE
    };
    append_offset_steps(&mut words, opcode, 31, 31, amount);
    for word in words {
        append_word(bytes, word);
    }
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
enum Register {
    X(u8),
    D(u8),
    Q(u8),
}

fn require_canonical_model(
    model: &ValidatedPhysicalRegisterModel,
) -> Result<(), Aarch64FrameProtocolError> {
    if model.identity() != crate::canonical_aarch64_physical_register_model_identity() {
        return Err(Aarch64FrameProtocolError::PhysicalRegisterModelMismatch);
    }
    Ok(())
}

fn register(
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

fn append_memory(
    bytes: &mut Vec<u8>,
    register: Register,
    offset_bytes: u64,
    scratch: Option<u8>,
    load: bool,
) -> Result<(), Aarch64FrameProtocolError> {
    append_sized_memory(bytes, register, 31, offset_bytes, scratch, 8, load)
}

pub fn encode_aarch64_frame_memory(
    model: &ValidatedPhysicalRegisterModel,
    view: RegisterViewId,
    offset_bytes: u32,
    byte_size: u8,
    load: bool,
    address_scratch: RegisterViewId,
) -> Result<Vec<u8>, Aarch64FrameProtocolError> {
    require_canonical_model(model)?;
    let value_register = register(model, view)?;
    let Register::X(scratch) = register(model, address_scratch)? else {
        return Err(Aarch64FrameProtocolError::UnknownOrUnsupportedView(
            address_scratch,
        ));
    };
    if !load && matches!(value_register, Register::X(number) if number == scratch) {
        return Err(Aarch64FrameProtocolError::NonCanonicalSlots);
    }
    let mut bytes = Vec::new();
    append_sized_memory(
        &mut bytes,
        value_register,
        31,
        u64::from(offset_bytes),
        Some(scratch),
        byte_size,
        load,
    )?;
    Ok(bytes)
}

pub fn encode_aarch64_pointer_memory(
    model: &ValidatedPhysicalRegisterModel,
    view: RegisterViewId,
    base: RegisterViewId,
    offset_bytes: u32,
    byte_size: u8,
    load: bool,
) -> Result<Vec<u8>, Aarch64FrameProtocolError> {
    require_canonical_model(model)?;
    let value_register = register(model, view)?;
    let Register::X(base_register) = register(model, base)? else {
        return Err(Aarch64FrameProtocolError::UnknownOrUnsupportedView(base));
    };
    if matches!(value_register, Register::X(number) if number == base_register) {
        return Err(Aarch64FrameProtocolError::NonCanonicalSlots);
    }
    let mut bytes = Vec::new();
    append_sized_memory(
        &mut bytes,
        value_register,
        base_register,
        u64::from(offset_bytes),
        Some(base_register),
        byte_size,
        load,
    )?;
    Ok(bytes)
}

pub fn encode_aarch64_frame_address(
    model: &ValidatedPhysicalRegisterModel,
    view: RegisterViewId,
    offset_bytes: u32,
) -> Result<Vec<u8>, Aarch64FrameProtocolError> {
    require_canonical_model(model)?;
    let Register::X(destination) = register(model, view)? else {
        return Err(Aarch64FrameProtocolError::UnknownOrUnsupportedView(view));
    };
    Ok(
        crate::immediate_offset::address_words(destination, 31, u64::from(offset_bytes))
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect(),
    )
}

pub fn encode_aarch64_aligned_frame_address(
    model: &ValidatedPhysicalRegisterModel,
    view: RegisterViewId,
    offset_bytes: u32,
    alignment_bytes: u16,
    member_offset_bytes: u32,
) -> Result<Vec<u8>, Aarch64FrameProtocolError> {
    require_canonical_model(model)?;
    let Register::X(destination) = register(model, view)? else {
        return Err(Aarch64FrameProtocolError::UnknownOrUnsupportedView(view));
    };
    let words = crate::immediate_offset::aligned_frame_address_words(
        destination,
        offset_bytes,
        alignment_bytes,
        member_offset_bytes,
    )
    .ok_or(Aarch64FrameProtocolError::UnknownOrUnsupportedView(view))?;
    Ok(words.into_iter().flat_map(u32::to_le_bytes).collect())
}

pub fn encode_aarch64_integer_constant(
    model: &ValidatedPhysicalRegisterModel,
    view: RegisterViewId,
    value: u64,
) -> Result<Vec<u8>, Aarch64FrameProtocolError> {
    require_canonical_model(model)?;
    let Register::X(destination) = register(model, view)? else {
        return Err(Aarch64FrameProtocolError::UnknownOrUnsupportedView(view));
    };
    let mut words = Vec::new();
    crate::selected_form_encoding::movn_materialization::append_canonical_materialization(
        &mut words,
        destination,
        value,
    );
    Ok(words.into_iter().flat_map(u32::to_le_bytes).collect())
}

fn append_sized_memory(
    bytes: &mut Vec<u8>,
    register: Register,
    base_register: u8,
    offset_bytes: u64,
    scratch: Option<u8>,
    byte_size: u8,
    load: bool,
) -> Result<(), Aarch64FrameProtocolError> {
    if !matches!(byte_size, 1 | 2 | 4 | 8 | 16)
        || !offset_bytes.is_multiple_of(u64::from(byte_size))
    {
        return Err(Aarch64FrameProtocolError::OffsetOutOfRange);
    }
    let (base_register, displacement) = if offset_bytes / u64::from(byte_size) <= 4095 {
        (base_register, offset_bytes)
    } else {
        let scratch = scratch.ok_or(Aarch64FrameProtocolError::NonCanonicalSlots)?;
        let mut words = Vec::new();
        append_page_steps(
            &mut words,
            ADD_IMMEDIATE,
            scratch,
            base_register,
            offset_bytes >> 12,
        );
        for word in words {
            append_word(bytes, word);
        }
        (scratch, offset_bytes & 4095)
    };
    let scaled = (displacement / u64::from(byte_size)) as u32;
    let (base, number) = match (register, byte_size) {
        (Register::X(number), 1) => (0x3900_0000, number),
        (Register::X(number), 2) => (0x7900_0000, number),
        (Register::X(number), 4) => (0xb900_0000, number),
        (Register::X(number), 8) => (0xf900_0000, number),
        (Register::D(number), 4) => (0xbd00_0000, number),
        (Register::D(number), 8) => (0xfd00_0000, number),
        (Register::Q(number), 16) => (0x3d80_0000, number),
        _ => return Err(Aarch64FrameProtocolError::OffsetOutOfRange),
    };
    let base = base | if load { 1 << 22 } else { 0 };
    append_word(
        bytes,
        base | (scaled << 10) | (u32::from(base_register) << 5) | u32::from(number),
    );
    Ok(())
}

fn append_word(bytes: &mut Vec<u8>, word: u32) {
    crate::codec_bytes::push_encoded(bytes, word.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::{
        Aarch64FrameProtocolError, Aarch64FrameSlot, Aarch64StackProbe,
        MAX_STACK_PROBE_INTERVAL_BYTES, encode_aapcs64_frame_protocol,
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

    fn frame_slot(
        view: super::RegisterViewId,
        offset_bytes: u64,
        size_bytes: u64,
    ) -> Aarch64FrameSlot {
        Aarch64FrameSlot {
            view,
            offset_bytes,
            size_bytes,
        }
    }

    #[test]
    fn physical_frame_transfers_encode_exact_scalar_widths() {
        let model =
            validate_physical_register_model(crate::aarch64_physical_register_model()).unwrap();
        let scratch = model.model().view_named("x17").unwrap().id;
        for (name, offset, width, load, expected) in [
            ("x0", 3, 1, false, 0x3900_0fe0_u32),
            ("x1", 6, 2, true, 0x7940_0fe1),
            ("x0", 16, 8, false, 0xf900_0be0),
            ("d0", 4, 4, false, 0xbd00_07e0),
            ("d0", 4, 4, true, 0xbd40_07e0),
        ] {
            let view = model.model().view_named(name).unwrap().id;
            assert_eq!(
                super::encode_aarch64_frame_memory(&model, view, offset, width, load, scratch)
                    .unwrap(),
                expected.to_le_bytes()
            );
        }
    }

    #[test]
    fn pointer_transfers_keep_base_and_large_offset_steps_explicit() {
        let model =
            validate_physical_register_model(crate::aarch64_physical_register_model()).unwrap();
        let value = model.model().view_named("x16").unwrap().id;
        let base = model.model().view_named("x17").unwrap().id;
        assert_eq!(
            super::encode_aarch64_pointer_memory(&model, value, base, 24, 8, false).unwrap(),
            0xf900_0e30_u32.to_le_bytes()
        );
        assert_eq!(
            super::encode_aarch64_pointer_memory(&model, value, base, 65_536, 8, true).unwrap(),
            [0x9140_4231_u32, 0xf940_0230]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>()
        );
        assert!(super::encode_aarch64_pointer_memory(&model, base, base, 0, 8, false).is_err());
    }

    #[test]
    fn full_vector_frame_transfer_retains_both_lanes() {
        let model =
            validate_physical_register_model(crate::aarch64_physical_register_model()).unwrap();
        let vector = model.model().view_named("q16").unwrap().id;
        let scratch = model.model().view_named("x17").unwrap().id;
        for (load, expected) in [(false, 0x3d80_07f0_u32), (true, 0x3dc0_07f0)] {
            assert_eq!(
                super::encode_aarch64_frame_memory(&model, vector, 16, 16, load, scratch).unwrap(),
                expected.to_le_bytes()
            );
        }
        assert!(super::encode_aarch64_frame_memory(&model, vector, 8, 16, false, scratch).is_err());
        assert!(super::encode_aarch64_frame_memory(&model, vector, 16, 8, false, scratch).is_err());
    }

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
            &[frame_slot(x19, 0, 8), frame_slot(x30, 8, 8)],
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
            encode_aapcs64_frame_protocol(&model, 8, NO_PROBE, &[frame_slot(x19, 0, 8)],),
            Err(Aarch64FrameProtocolError::InvalidFrameSize)
        );
    }

    #[test]
    fn frame_past_one_instruction_uses_shifted_then_unshifted_adjust() {
        let model =
            validate_physical_register_model(crate::aarch64_physical_register_model()).unwrap();
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
        let (prologue, epilogue) =
            encode_aapcs64_frame_protocol(&model, 8_192, probe, &[frame_slot(x19, 0, 8)]).unwrap();
        let chunk = [0xd140_07ff_u32.to_le_bytes(), 0xf940_03ff_u32.to_le_bytes()].concat();
        assert_eq!(&prologue[..8], &chunk);
        assert_eq!(&prologue[8..16], &chunk);
        assert_eq!(&prologue[16..20], &0xf900_03f3_u32.to_le_bytes());
        assert_eq!(prologue.len(), 20);
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
    fn large_frame_preserves_integer_float_and_link_register_slots() {
        let model =
            validate_physical_register_model(crate::aarch64_physical_register_model()).unwrap();
        let slots =
            [("x19", 65_536), ("d8", 65_544), ("x30", 65_552)].map(|(name, offset_bytes)| {
                frame_slot(model.model().view_named(name).unwrap().id, offset_bytes, 8)
            });
        let (prologue, epilogue) = encode_aapcs64_frame_protocol(
            &model,
            65_568,
            Aarch64StackProbe {
                touches: 5,
                ..DARWIN_PROBE
            },
            &slots,
        )
        .unwrap();
        let saves = [
            0x9140_43f0_u32,
            0xf900_0213,
            0x9140_43f0,
            0xfd00_0608,
            0x9140_43f0,
            0xf900_0a1e,
        ]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect::<Vec<_>>();
        let restores = [
            0x9140_43f0_u32,
            0xf940_0a1e,
            0x9140_43f0,
            0xfd40_0608,
            0x9140_43f0,
            0xf940_0213,
            0x9140_43ff,
            0x9100_83ff,
        ]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect::<Vec<_>>();
        assert!(prologue.ends_with(&saves));
        assert_eq!(epilogue, restores);
    }

    #[test]
    fn frame_past_sixteen_mebibytes_steps_its_release_and_slot_addresses() {
        let model =
            validate_physical_register_model(crate::aarch64_physical_register_model()).unwrap();
        let slots = [("x19", 33_554_448), ("x30", 33_554_456)].map(|(name, offset_bytes)| {
            frame_slot(model.model().view_named(name).unwrap().id, offset_bytes, 8)
        });
        let (prologue, epilogue) = encode_aapcs64_frame_protocol(
            &model,
            33_554_464,
            Aarch64StackProbe {
                touches: 2_049,
                ..DARWIN_PROBE
            },
            &slots,
        )
        .unwrap();
        let words = |bytes: &[u8]| {
            bytes
                .as_chunks::<4>()
                .0
                .iter()
                .map(|word| u32::from_le_bytes(*word))
                .collect::<Vec<_>>()
        };
        let prologue = words(&prologue);
        assert!(
            prologue[..4_096]
                .chunks(2)
                .all(|pair| pair == [0xd140_13ff, 0xf940_03ff])
        );
        assert_eq!(&prologue[4_096..4_098], &[0xd100_83ff, 0xf940_03ff]);
        let address = [0x917f_fff0, 0x917f_fe10, 0x9140_0a10];
        assert_eq!(&prologue[4_098..4_101], &address);
        assert_eq!(prologue[4_101], 0xf900_0a13);
        assert_eq!(&prologue[4_102..4_105], &address);
        assert_eq!(prologue[4_105], 0xf900_0e1e);
        assert_eq!(prologue.len(), 4_106);
        assert_eq!(
            words(&epilogue),
            [
                address.as_slice(),
                &[0xf940_0e1e],
                &address,
                &[0xf940_0a13],
                &[0x917f_ffff, 0x917f_ffff, 0x9140_0bff, 0x9100_83ff],
            ]
            .concat()
        );
    }

    #[test]
    fn large_slot_address_never_clobbers_a_saved_scratch() {
        let model =
            validate_physical_register_model(crate::aarch64_physical_register_model()).unwrap();
        let x16 = model.model().view_named("x16").unwrap().id;
        let x17 = model.model().view_named("x17").unwrap().id;
        let x19 = model.model().view_named("x19").unwrap().id;
        let probe = Aarch64StackProbe {
            touches: 5,
            ..DARWIN_PROBE
        };
        let slots = [frame_slot(x16, 0, 8), frame_slot(x19, 65_536, 8)];
        let (prologue, epilogue) =
            encode_aapcs64_frame_protocol(&model, 65_552, probe, &slots).unwrap();
        let save = [0x9140_43f1_u32.to_le_bytes(), 0xf900_0233_u32.to_le_bytes()].concat();
        assert!(prologue.ends_with(&save));
        assert_eq!(
            &epilogue[..8],
            &[0x9140_43f1_u32.to_le_bytes(), 0xf940_0233_u32.to_le_bytes()].concat()
        );
        let occupied = [slots[0], frame_slot(x17, 8, 8), slots[1]];
        assert_eq!(
            encode_aapcs64_frame_protocol(&model, 65_552, probe, &occupied),
            Err(Aarch64FrameProtocolError::NonCanonicalSlots),
        );
    }

    #[test]
    fn large_frames_still_reject_invalid_slot_geometry() {
        let model =
            validate_physical_register_model(crate::aarch64_physical_register_model()).unwrap();
        let x19 = model.model().view_named("x19").unwrap().id;
        let x30 = model.model().view_named("x30").unwrap().id;
        let first = frame_slot(x19, 65_536, 8);
        for (offset_bytes, size_bytes) in [(65_536, 8), (65_540, 8), (65_552, 8), (65_544, 4)] {
            assert_eq!(
                encode_aapcs64_frame_protocol(
                    &model,
                    65_552,
                    Aarch64StackProbe {
                        touches: 5,
                        ..DARWIN_PROBE
                    },
                    &[first, frame_slot(x30, offset_bytes, size_bytes)],
                ),
                Err(Aarch64FrameProtocolError::NonCanonicalSlots),
                "offset {offset_bytes}, size {size_bytes}",
            );
        }
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
        for interval in [0_u64, 1, 4_000, MAX_STACK_PROBE_INTERVAL_BYTES * 2] {
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
