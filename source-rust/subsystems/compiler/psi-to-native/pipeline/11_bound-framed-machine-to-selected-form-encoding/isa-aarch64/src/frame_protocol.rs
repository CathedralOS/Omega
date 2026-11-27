/*
We encode and validate the AAPCS64 fixed-frame prologue/epilogue protocol,
page-granule stack-commit probes (`Aarch64StackProbe`), and frame/pointer memory
accesses. Because Linux AArch64 (4 KiB) and Darwin AArch64 (16 KiB) commit stack
pages at different granules, we emit ordered probe loads across newly entered
pages before establishing callee-saved register slots (`Aarch64FrameSlot`) and
frame-relative addresses. Each prologue and epilogue instruction also records
its unwind effect (`FrameProtocolStep`), so image writers can describe the
frame without decoding these bytes.
*/

use calling_conventions::MachineRegister;
use calling_conventions::frame_unwind::{
    EncodedFrameProtocol, FrameProtocolEffect, FrameProtocolSteps,
};
use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
pub use target_aarch64::frame_protocol::*;

use crate::immediate_offset::{
    ADD_IMMEDIATE, SUBTRACT_IMMEDIATE, append_offset_steps, append_page_steps,
};

pub fn encode_planned_aapcs64_frame_protocol(
    model: &ValidatedPhysicalRegisterModel,
    plan: &Aarch64FrameProtocolPlan,
) -> Result<EncodedFrameProtocol, Aarch64FrameProtocolError> {
    if model.identity() != plan.model() {
        return Err(Aarch64FrameProtocolError::PhysicalRegisterModelMismatch);
    }
    let frame_size_bytes = plan.frame_size_bytes();
    let probe = plan.probe();
    let slots = plan.slots();
    let scratch = plan.address_scratch();
    let mut prologue = Vec::new();
    let mut epilogue = Vec::new();
    let mut prologue_steps = FrameProtocolSteps::default();
    let mut epilogue_steps = FrameProtocolSteps::default();
    if probe.touches == 0 {
        append_stack_adjust(&mut prologue, &mut prologue_steps, frame_size_bytes, true)?;
    } else {
        let mut remaining = frame_size_bytes;
        loop {
            let chunk = remaining.min(probe.interval_bytes);
            append_stack_adjust(&mut prologue, &mut prologue_steps, chunk, true)?;
            append_probe_touch(&mut prologue);
            record(
                &mut prologue_steps,
                &prologue,
                FrameProtocolEffect::Unchanged,
            )?;
            remaining -= chunk;
            if remaining == 0 {
                break;
            }
        }
    }
    for slot in slots {
        append_slot(
            &mut prologue,
            &mut prologue_steps,
            model,
            slot,
            scratch,
            false,
        )?;
    }
    for slot in slots.iter().rev() {
        append_slot(
            &mut epilogue,
            &mut epilogue_steps,
            model,
            slot,
            scratch,
            true,
        )?;
    }
    append_stack_adjust(&mut epilogue, &mut epilogue_steps, frame_size_bytes, false)?;
    Ok(EncodedFrameProtocol {
        prologue,
        epilogue,
        prologue_steps: prologue_steps.into_steps(),
        epilogue_steps: epilogue_steps.into_steps(),
    })
}

pub fn encode_aapcs64_frame_protocol(
    model: &ValidatedPhysicalRegisterModel,
    frame_size_bytes: u64,
    probe: Aarch64StackProbe,
    slots: &[Aarch64FrameSlot],
) -> Result<EncodedFrameProtocol, Aarch64FrameProtocolError> {
    encode_planned_aapcs64_frame_protocol(
        model,
        &plan_aapcs64_frame_protocol(model, frame_size_bytes, probe, slots)?,
    )
}

fn record(
    steps: &mut FrameProtocolSteps,
    bytes: &[u8],
    effect: FrameProtocolEffect,
) -> Result<(), Aarch64FrameProtocolError> {
    steps
        .push(bytes, effect)
        .ok_or(Aarch64FrameProtocolError::InvalidFrameSize)
}

/// Save or reload one slot. A slot beyond the scaled 12-bit displacement first
/// forms its address in the scratch register; those words change nothing an
/// unwinder restores, and the final store or load moves the register.
fn append_slot(
    bytes: &mut Vec<u8>,
    steps: &mut FrameProtocolSteps,
    model: &ValidatedPhysicalRegisterModel,
    slot: &Aarch64FrameSlot,
    scratch: Option<u8>,
    load: bool,
) -> Result<(), Aarch64FrameProtocolError> {
    let start = bytes.len();
    let saved = register(model, slot.view)?;
    append_memory(bytes, saved, slot.offset_bytes, scratch, load)?;
    let words = (bytes.len() - start) / 4;
    for word in 1..words {
        record(
            steps,
            &bytes[..start + 4 * word],
            FrameProtocolEffect::Unchanged,
        )?;
    }
    let register = match saved {
        Register::X(number) => MachineRegister::Aarch64X(number),
        Register::D(number) | Register::Q(number) => MachineRegister::Aarch64V(number),
    };
    record(
        steps,
        bytes,
        FrameProtocolEffect::Register {
            register,
            frame_offset_bytes: u32::try_from(slot.offset_bytes)
                .map_err(|_| Aarch64FrameProtocolError::OffsetOutOfRange)?,
        },
    )
}

fn append_probe_touch(bytes: &mut Vec<u8>) {
    append_word(bytes, 0xf940_03ff);
}

fn append_stack_adjust(
    bytes: &mut Vec<u8>,
    steps: &mut FrameProtocolSteps,
    amount: u64,
    subtract: bool,
) -> Result<(), Aarch64FrameProtocolError> {
    let mut words = Vec::new();
    let opcode = if subtract {
        SUBTRACT_IMMEDIATE
    } else {
        ADD_IMMEDIATE
    };
    append_offset_steps(&mut words, opcode, 31, 31, amount);
    for word in words {
        append_word(bytes, word);
        // Each `add`/`sub` immediate moves sp by imm12, shifted left 12 when bit 22 is set.
        let immediate = (word >> 10) & 0xfff;
        let moved = if word & (1 << 22) != 0 {
            immediate << 12
        } else {
            immediate
        };
        record(
            steps,
            bytes,
            FrameProtocolEffect::AdjustStack { bytes: moved },
        )?;
    }
    Ok(())
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
    let naturally_aligned = offset_bytes.is_multiple_of(u64::from(byte_size.max(1)));
    if !matches!(byte_size, 1 | 2 | 4 | 8 | 16)
        || (matches!(register, Register::Q(_)) && !naturally_aligned)
    {
        return Err(Aarch64FrameProtocolError::OffsetOutOfRange);
    }
    let (base_register, displacement) = if !naturally_aligned {
        // A composite's internal carrier can have weaker alignment than a
        // transfer word. Preserve its exact byte address instead of truncating
        // the scaled displacement or refusing an otherwise valid copy. The
        // caller reserves this scratch and rejects stores that would clobber
        // their own source while forming the address.
        let scratch = scratch.ok_or(Aarch64FrameProtocolError::NonCanonicalSlots)?;
        for word in crate::immediate_offset::address_words(scratch, base_register, offset_bytes) {
            append_word(bytes, word);
        }
        (scratch, 0)
    } else if offset_bytes / u64::from(byte_size) <= 4095 {
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
        encode_planned_aapcs64_frame_protocol, plan_aapcs64_frame_protocol,
    };
    use calling_conventions::MachineRegister;
    use calling_conventions::frame_unwind::{
        EncodedFrameProtocol, FrameProtocolEffect, FrameProtocolStep,
    };

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
        let model = crate::validated_aarch64_physical_register_model();
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
                super::encode_aarch64_frame_memory(model, view, offset, width, load, scratch)
                    .unwrap(),
                expected.to_le_bytes()
            );
        }
    }

    #[test]
    fn unaligned_scalar_frame_and_pointer_transfers_preserve_exact_byte_addresses() {
        let model = crate::validated_aarch64_physical_register_model();
        let value = model.model().view_named("x0").unwrap().id;
        let scratch = model.model().view_named("x17").unwrap().id;
        for (load, opcode) in [(false, 0xb900_0220_u32), (true, 0xb940_0220)] {
            assert_eq!(
                super::encode_aarch64_frame_memory(model, value, 2, 4, load, scratch).unwrap(),
                [0x9100_0bf1_u32, opcode]
                    .into_iter()
                    .flat_map(u32::to_le_bytes)
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                super::encode_aarch64_pointer_memory(model, value, scratch, 2, 4, load).unwrap(),
                [0x9100_0a31_u32, opcode]
                    .into_iter()
                    .flat_map(u32::to_le_bytes)
                    .collect::<Vec<_>>()
            );
        }
        assert!(super::encode_aarch64_frame_memory(model, scratch, 2, 4, false, scratch).is_err());
        assert!(super::encode_aarch64_pointer_memory(model, scratch, scratch, 2, 4, true).is_err());
        // The reserved address scratch may also receive a frame load after
        // its address has been consumed; a store must preserve its input.
        assert!(super::encode_aarch64_frame_memory(model, scratch, 2, 4, true, scratch).is_ok());
    }

    #[test]
    fn pointer_transfers_keep_base_and_large_offset_steps_explicit() {
        let model = crate::validated_aarch64_physical_register_model();
        let value = model.model().view_named("x16").unwrap().id;
        let base = model.model().view_named("x17").unwrap().id;
        assert_eq!(
            super::encode_aarch64_pointer_memory(model, value, base, 24, 8, false).unwrap(),
            0xf900_0e30_u32.to_le_bytes()
        );
        assert_eq!(
            super::encode_aarch64_pointer_memory(model, value, base, 65_536, 8, true).unwrap(),
            [0x9140_4231_u32, 0xf940_0230]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>()
        );
        assert!(super::encode_aarch64_pointer_memory(model, base, base, 0, 8, false).is_err());
    }

    #[test]
    fn full_vector_frame_transfer_retains_both_lanes() {
        let model = crate::validated_aarch64_physical_register_model();
        let vector = model.model().view_named("q16").unwrap().id;
        let scratch = model.model().view_named("x17").unwrap().id;
        for (load, expected) in [(false, 0x3d80_07f0_u32), (true, 0x3dc0_07f0)] {
            assert_eq!(
                super::encode_aarch64_frame_memory(model, vector, 16, 16, load, scratch).unwrap(),
                expected.to_le_bytes()
            );
        }
        assert!(super::encode_aarch64_frame_memory(model, vector, 8, 16, false, scratch).is_err());
        assert!(super::encode_aarch64_frame_memory(model, vector, 16, 8, false, scratch).is_err());
    }

    #[test]
    fn canonical_frame_encodes_sp_adjustment_and_link_custody() {
        let model = crate::validated_aarch64_physical_register_model();
        let x19 = model.model().view_named("x19").unwrap().id;
        let x30 = model.model().view_named("x30").unwrap().id;
        let EncodedFrameProtocol {
            prologue, epilogue, ..
        } = encode_aapcs64_frame_protocol(
            model,
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
        let model = crate::validated_aarch64_physical_register_model();
        let x19 = model.model().view_named("x19").unwrap().id;
        assert_eq!(
            encode_aapcs64_frame_protocol(model, 8, NO_PROBE, &[frame_slot(x19, 0, 8)],),
            Err(Aarch64FrameProtocolError::InvalidFrameSize)
        );
    }

    #[test]
    fn frame_past_one_instruction_uses_shifted_then_unshifted_adjust() {
        let model = crate::validated_aarch64_physical_register_model();
        let EncodedFrameProtocol {
            prologue, epilogue, ..
        } = encode_aapcs64_frame_protocol(model, 4_208, DARWIN_PROBE, &[]).unwrap();
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
        let model = crate::validated_aarch64_physical_register_model();
        let x19 = model.model().view_named("x19").unwrap().id;
        let probe = Aarch64StackProbe {
            interval_bytes: 4_096,
            touches: 2,
        };
        let EncodedFrameProtocol {
            prologue, epilogue, ..
        } = encode_aapcs64_frame_protocol(model, 8_192, probe, &[frame_slot(x19, 0, 8)]).unwrap();
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
        let model = crate::validated_aarch64_physical_register_model();
        let probe = Aarch64StackProbe {
            interval_bytes: 4_096,
            touches: 2,
        };
        let EncodedFrameProtocol { prologue, .. } =
            encode_aapcs64_frame_protocol(model, 4_208, probe, &[]).unwrap();
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
        let model = crate::validated_aarch64_physical_register_model();
        let slots =
            [("x19", 65_536), ("d8", 65_544), ("x30", 65_552)].map(|(name, offset_bytes)| {
                frame_slot(model.model().view_named(name).unwrap().id, offset_bytes, 8)
            });
        let EncodedFrameProtocol {
            prologue, epilogue, ..
        } = encode_aapcs64_frame_protocol(
            model,
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
    fn steps_split_address_words_from_the_slot_transfer() {
        let model = crate::validated_aarch64_physical_register_model();
        let slots = [("x19", 8), ("x30", 65_552)].map(|(name, offset_bytes)| {
            frame_slot(model.model().view_named(name).unwrap().id, offset_bytes, 8)
        });
        let encoded = encode_aapcs64_frame_protocol(
            model,
            65_568,
            Aarch64StackProbe {
                touches: 5,
                ..DARWIN_PROBE
            },
            &slots,
        )
        .unwrap();
        let step = |end_offset, effect| FrameProtocolStep { end_offset, effect };
        let granule = FrameProtocolEffect::AdjustStack { bytes: 16_384 };
        let probe = FrameProtocolEffect::Unchanged;
        let saved = |register, frame_offset_bytes| FrameProtocolEffect::Register {
            register,
            frame_offset_bytes,
        };
        assert_eq!(
            encoded.prologue_steps,
            [
                step(4, granule),
                step(8, probe),
                step(12, granule),
                step(16, probe),
                step(20, granule),
                step(24, probe),
                step(28, granule),
                step(32, probe),
                step(36, FrameProtocolEffect::AdjustStack { bytes: 32 }),
                step(40, probe),
                step(44, saved(MachineRegister::Aarch64X(19), 8)),
                step(48, FrameProtocolEffect::Unchanged),
                step(52, saved(MachineRegister::Aarch64X(30), 65_552)),
            ]
        );
        assert_eq!(encoded.prologue.len(), 52);
        assert_eq!(
            encoded.epilogue_steps,
            [
                step(4, FrameProtocolEffect::Unchanged),
                step(8, saved(MachineRegister::Aarch64X(30), 65_552)),
                step(12, saved(MachineRegister::Aarch64X(19), 8)),
                step(16, FrameProtocolEffect::AdjustStack { bytes: 65_536 }),
                step(20, FrameProtocolEffect::AdjustStack { bytes: 32 }),
            ]
        );
    }

    #[test]
    fn frame_past_sixteen_mebibytes_steps_its_release_and_slot_addresses() {
        let model = crate::validated_aarch64_physical_register_model();
        let slots = [("x19", 33_554_448), ("x30", 33_554_456)].map(|(name, offset_bytes)| {
            frame_slot(model.model().view_named(name).unwrap().id, offset_bytes, 8)
        });
        let EncodedFrameProtocol {
            prologue, epilogue, ..
        } = encode_aapcs64_frame_protocol(
            model,
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
        let model = crate::validated_aarch64_physical_register_model();
        let x16 = model.model().view_named("x16").unwrap().id;
        let x17 = model.model().view_named("x17").unwrap().id;
        let x19 = model.model().view_named("x19").unwrap().id;
        let probe = Aarch64StackProbe {
            touches: 5,
            ..DARWIN_PROBE
        };
        let slots = [frame_slot(x16, 0, 8), frame_slot(x19, 65_536, 8)];
        let plan = plan_aapcs64_frame_protocol(model, 65_552, probe, &slots).unwrap();
        assert_eq!(
            plan.address_scratch(),
            Some(17),
            "physical planning reserves the free scratch before encoding"
        );
        let EncodedFrameProtocol {
            prologue, epilogue, ..
        } = encode_planned_aapcs64_frame_protocol(model, &plan).unwrap();
        let save = [0x9140_43f1_u32.to_le_bytes(), 0xf900_0233_u32.to_le_bytes()].concat();
        assert!(prologue.ends_with(&save));
        assert_eq!(
            &epilogue[..8],
            &[0x9140_43f1_u32.to_le_bytes(), 0xf940_0233_u32.to_le_bytes()].concat()
        );
        let occupied = [slots[0], frame_slot(x17, 8, 8), slots[1]];
        assert_eq!(
            plan_aapcs64_frame_protocol(model, 65_552, probe, &occupied),
            Err(Aarch64FrameProtocolError::NonCanonicalSlots),
        );
        assert_eq!(
            encode_aapcs64_frame_protocol(model, 65_552, probe, &occupied),
            Err(Aarch64FrameProtocolError::NonCanonicalSlots),
        );
    }

    #[test]
    fn large_frames_still_reject_invalid_slot_geometry() {
        let model = crate::validated_aarch64_physical_register_model();
        let x19 = model.model().view_named("x19").unwrap().id;
        let x30 = model.model().view_named("x30").unwrap().id;
        let first = frame_slot(x19, 65_536, 8);
        for (offset_bytes, size_bytes) in [(65_536, 8), (65_540, 8), (65_552, 8), (65_544, 4)] {
            assert_eq!(
                encode_aapcs64_frame_protocol(
                    model,
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
        let model = crate::validated_aarch64_physical_register_model();
        for (frame_size, touches) in [(8_192_u64, 0), (8_192, 1), (8_192, 3), (4_096, 1), (16, 1)] {
            let probe = Aarch64StackProbe {
                interval_bytes: 4_096,
                touches,
            };
            assert_eq!(
                encode_aapcs64_frame_protocol(model, frame_size, probe, &[]),
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
                encode_aapcs64_frame_protocol(model, 8_192, probe, &[]),
                Err(Aarch64FrameProtocolError::NonCanonicalProbe),
                "interval {interval}"
            );
        }
    }
}
