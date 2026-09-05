//! Reusable post-handoff writer lowering for AArch64.
//!
//! X9 receives the provider-private packed context pointer. X10 holds the
//! destination, X11 the source fragment, X12 masks, and X13 the destination
//! container. The helper embeds geometry only—never destination or source
//! addresses.

use super::writer_encoding::{
    append_unsigned_immediate_padded, encode_and_x_register, encode_load_w_from_x,
    encode_load_x_from_x, encode_lsl_x_immediate, encode_lsr_x_immediate, encode_move_x_register,
    encode_orr_x_register, encode_store_w_to_x, encode_store_x_to_x,
};
use calling_conventions::{MachineRegister, MachineStateSet, RegisterSet};
use diagnostics::Diagnostic;
use layout_plans::{
    ByteOrder, GeneratedPostHandoffWriterFragmentPlan, GeneratedPostHandoffWriterStep,
    POST_HANDOFF_WRITER_CONTEXT_ABI_V1, POST_HANDOFF_WRITER_SOURCE_SLOT_WIDTH,
    POST_HANDOFF_WRITER_SOURCE_SLOTS_OFFSET, post_handoff_writer_context_byte_len,
};

const CONTEXT_REGISTER: u8 = 9;
const DESTINATION_REGISTER: u8 = 10;
const SOURCE_REGISTER: u8 = 11;
const MASK_REGISTER: u8 = 12;
const CONTAINER_REGISTER: u8 = 13;

pub fn generated_post_handoff_writer_clobbers() -> RegisterSet {
    RegisterSet::new([
        MachineRegister::Aarch64X(CONTEXT_REGISTER),
        MachineRegister::Aarch64X(DESTINATION_REGISTER),
        MachineRegister::Aarch64X(SOURCE_REGISTER),
        MachineRegister::Aarch64X(MASK_REGISTER),
        MachineRegister::Aarch64X(CONTAINER_REGISTER),
    ])
}

pub const fn generated_post_handoff_writer_additional_machine_state() -> MachineStateSet {
    MachineStateSet::empty()
}

pub fn generated_post_handoff_writer_width(
    pointer_register: MachineRegister,
    plan: &GeneratedPostHandoffWriterFragmentPlan,
) -> Result<usize, Diagnostic> {
    Ok(encode_generated_post_handoff_writer_bytes(pointer_register, plan)?.len())
}

pub fn encode_generated_post_handoff_writer_bytes(
    pointer_register: MachineRegister,
    plan: &GeneratedPostHandoffWriterFragmentPlan,
) -> Result<Vec<u8>, Diagnostic> {
    let pointer_register = validate_generated_post_handoff_writer(pointer_register, plan)?;
    let mut bytes = Vec::new();
    bytes.extend(encode_move_x_register(CONTEXT_REGISTER, pointer_register));
    bytes.extend(encode_load_x_from_x(
        DESTINATION_REGISTER,
        CONTEXT_REGISTER,
        0,
    )?);

    for step in plan.steps() {
        let source_offset = POST_HANDOFF_WRITER_SOURCE_SLOTS_OFFSET
            .checked_add(
                step.source_slot
                    .checked_mul(POST_HANDOFF_WRITER_SOURCE_SLOT_WIDTH)
                    .ok_or_else(|| {
                        Diagnostic::error("generated writer source-slot offset overflows")
                    })?,
            )
            .ok_or_else(|| Diagnostic::error("generated writer source-slot offset overflows"))?;
        let destination_offset = usize::try_from(step.container_byte_offset).map_err(|_| {
            Diagnostic::error("generated writer destination offset does not fit the host")
        })?;

        bytes.extend(encode_load_x_from_x(
            SOURCE_REGISTER,
            CONTEXT_REGISTER,
            source_offset,
        )?);
        if step.source_lsb != 0 {
            bytes.extend(encode_lsr_x_immediate(
                SOURCE_REGISTER,
                SOURCE_REGISTER,
                step.source_lsb as u8,
            ));
        }
        let fragment_mask = low_mask(step.width);
        append_unsigned_immediate_padded(&mut bytes, MASK_REGISTER, fragment_mask);
        bytes.extend(encode_and_x_register(
            SOURCE_REGISTER,
            SOURCE_REGISTER,
            MASK_REGISTER,
        ));
        if step.destination_lsb != 0 {
            bytes.extend(encode_lsl_x_immediate(
                SOURCE_REGISTER,
                SOURCE_REGISTER,
                step.destination_lsb as u8,
            ));
        }

        let container_bytes = usize::from(step.container_width_bits / 8);
        match container_bytes {
            1 | 2 | 4 => bytes.extend(encode_load_w_from_x(
                CONTAINER_REGISTER,
                DESTINATION_REGISTER,
                destination_offset,
                container_bytes,
            )?),
            8 => bytes.extend(encode_load_x_from_x(
                CONTAINER_REGISTER,
                DESTINATION_REGISTER,
                destination_offset,
            )?),
            _ => unreachable!("generated writer container width validated"),
        }

        let destination_mask = fragment_mask << step.destination_lsb;
        append_unsigned_immediate_padded(&mut bytes, MASK_REGISTER, !destination_mask);
        bytes.extend(encode_and_x_register(
            CONTAINER_REGISTER,
            CONTAINER_REGISTER,
            MASK_REGISTER,
        ));
        bytes.extend(encode_orr_x_register(
            CONTAINER_REGISTER,
            CONTAINER_REGISTER,
            SOURCE_REGISTER,
        ));

        match container_bytes {
            1 | 2 | 4 => bytes.extend(encode_store_w_to_x(
                CONTAINER_REGISTER,
                DESTINATION_REGISTER,
                destination_offset,
                container_bytes,
            )?),
            8 => bytes.extend(encode_store_x_to_x(
                CONTAINER_REGISTER,
                DESTINATION_REGISTER,
                destination_offset,
            )?),
            _ => unreachable!("generated writer container width validated"),
        }
    }
    Ok(bytes)
}

fn validate_generated_post_handoff_writer(
    pointer_register: MachineRegister,
    plan: &GeneratedPostHandoffWriterFragmentPlan,
) -> Result<u8, Diagnostic> {
    let MachineRegister::Aarch64X(pointer_register) = pointer_register else {
        return Err(Diagnostic::error(format!(
            "generated AArch64 writer context cannot arrive in {pointer_register:?}"
        )));
    };
    if pointer_register >= 31 {
        return Err(Diagnostic::error(
            "generated AArch64 writer context requires a general X register",
        ));
    }
    if plan.context_abi() != POST_HANDOFF_WRITER_CONTEXT_ABI_V1 {
        return Err(Diagnostic::error(format!(
            "generated post-handoff writer context ABI {:016x} is not PHWRITR1",
            plan.context_abi()
        )));
    }
    if plan.byte_order() != ByteOrder::LittleEndian {
        return Err(Diagnostic::error(
            "generated AArch64 post-handoff writer requires little-endian containers",
        ));
    }
    if plan.steps().is_empty() || plan.source_slot_count() == 0 {
        return Err(Diagnostic::error(
            "generated post-handoff writer requires at least one fragment and source slot",
        ));
    }
    let context_width = post_handoff_writer_context_byte_len(plan.source_slot_count())
        .ok_or_else(|| Diagnostic::error("generated writer private context size overflows"))?;
    if context_width > 4096 * POST_HANDOFF_WRITER_SOURCE_SLOT_WIDTH {
        return Err(Diagnostic::error(
            "generated writer private context exceeds AArch64 scaled-load addressing",
        ));
    }
    let byte_len = u64::try_from(plan.byte_len())
        .map_err(|_| Diagnostic::error("generated writer destination size is too large"))?;
    let mut used_slots = vec![false; plan.source_slot_count()];
    for step in plan.steps() {
        validate_step(step, byte_len, &mut used_slots)?;
    }
    if used_slots.iter().any(|used| !used) {
        return Err(Diagnostic::error(
            "generated writer source slots are not a dense exact set",
        ));
    }
    Ok(pointer_register)
}

fn validate_step(
    step: &GeneratedPostHandoffWriterStep,
    byte_len: u64,
    used_slots: &mut [bool],
) -> Result<(), Diagnostic> {
    let slot_count = used_slots.len();
    let Some(used) = used_slots.get_mut(step.source_slot) else {
        return Err(Diagnostic::error(format!(
            "generated writer fragment names source slot {}, but the context has {slot_count}",
            step.source_slot
        )));
    };
    *used = true;
    if !matches!(step.container_width_bits, 8 | 16 | 32 | 64) {
        return Err(Diagnostic::error(format!(
            "generated writer has invalid {}-bit container",
            step.container_width_bits
        )));
    }
    if step.width == 0
        || step.width > 64
        || step
            .source_lsb
            .checked_add(step.width)
            .is_none_or(|end| end > 64)
        || step
            .destination_lsb
            .checked_add(step.width)
            .is_none_or(|end| end > step.container_width_bits)
    {
        return Err(Diagnostic::error(
            "generated writer has an invalid source or destination bit range",
        ));
    }
    let container_bytes = u64::from(step.container_width_bits / 8);
    if step
        .container_byte_offset
        .checked_add(container_bytes)
        .is_none_or(|end| end > byte_len)
    {
        return Err(Diagnostic::error(format!(
            "generated writer fragment at byte {} lies outside its {byte_len}-byte destination",
            step.container_byte_offset
        )));
    }
    Ok(())
}

const fn low_mask(width: u16) -> u64 {
    if width == 64 {
        u64::MAX
    } else {
        (1_u64 << width) - 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use layout_plans::{
        DataSymbolId, MaterializationWrite, PlacementConstraints, PlacementPhase,
        PostHandoffWriterPlan, PostHandoffWriterSource, PostHandoffWriterStep, RelocationTarget,
    };

    fn fragment() -> GeneratedPostHandoffWriterFragmentPlan {
        let target = RelocationTarget::Data(
            DataSymbolId::from_normalized_identity(9).expect("data identity"),
        );
        PostHandoffWriterPlan {
            byte_len: 16,
            byte_order: ByteOrder::LittleEndian,
            placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
            steps: vec![
                PostHandoffWriterStep {
                    write: MaterializationWrite {
                        field: "data".into(),
                        target,
                        container_byte_offset: 0,
                        container_width_bits: 32,
                        destination_lsb: 4,
                        source_lsb: 0,
                        width: 28,
                        stored_integer_fit: None,
                    },
                    source: PostHandoffWriterSource::Resolve(target),
                },
                PostHandoffWriterStep {
                    write: MaterializationWrite {
                        field: "data".into(),
                        target,
                        container_byte_offset: 8,
                        container_width_bits: 64,
                        destination_lsb: 0,
                        source_lsb: 28,
                        width: 36,
                        stored_integer_fit: None,
                    },
                    source: PostHandoffWriterSource::Resolve(target),
                },
            ],
        }
        .lower_reusable_fragment()
        .expect("reusable fragment")
        .fragment()
        .clone()
    }

    #[test]
    fn generated_writer_emits_fragment_program_for_data_symbols() {
        let fragment = fragment();
        let bytes =
            encode_generated_post_handoff_writer_bytes(MachineRegister::Aarch64X(0), &fragment)
                .expect("AArch64 writer");
        assert!(!bytes.is_empty());
        assert_eq!(
            bytes.len(),
            generated_post_handoff_writer_width(MachineRegister::Aarch64X(0), &fragment)
                .expect("matching width")
        );
        assert_eq!(
            generated_post_handoff_writer_clobbers().as_slice(),
            &[
                MachineRegister::Aarch64X(9),
                MachineRegister::Aarch64X(10),
                MachineRegister::Aarch64X(11),
                MachineRegister::Aarch64X(12),
                MachineRegister::Aarch64X(13),
            ]
        );
        assert!(generated_post_handoff_writer_additional_machine_state().is_empty());
    }

    #[test]
    fn generated_writer_rejects_non_aarch64_context_register() {
        let error =
            encode_generated_post_handoff_writer_bytes(MachineRegister::X86Rdi, &fragment())
                .expect_err("wrong architecture register");
        assert!(error.message.contains("cannot arrive"));
    }
}
