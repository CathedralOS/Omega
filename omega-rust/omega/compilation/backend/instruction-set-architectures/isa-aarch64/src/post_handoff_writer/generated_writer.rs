//! Reusable post-handoff writer lowering for AArch64.
//!
//! X9 receives the provider-private packed context pointer. X10 holds the
//! destination, X11 the unchanged source, X12 the extracted fragment, and X13 the destination
//! container. The helper embeds geometry only—never destination or source
//! addresses.

use super::writer_encoding::{encode_load_x_from_x, encode_move_x_register};
use calling_conventions::{MachineRegister, MachineStateSet, RegisterSet};
use diagnostics::Diagnostic;
use layout_plans::ByteOrder;
use post_handoff_writer_plan::{
    GeneratedPostHandoffWriterFragmentPlan, POST_HANDOFF_WRITER_CONTEXT_ABI_V1,
    POST_HANDOFF_WRITER_SOURCE_SLOT_WIDTH, POST_HANDOFF_WRITER_SOURCE_SLOTS_OFFSET,
    post_handoff_writer_context_byte_len, validate_step,
};

const CONTEXT_REGISTER: u8 = 9;
const DESTINATION_REGISTER: u8 = 10;
const SOURCE_REGISTER: u8 = 11;
const FRAGMENT_REGISTER: u8 = 12;
const CONTAINER_REGISTER: u8 = 13;

pub fn generated_post_handoff_writer_clobbers() -> RegisterSet {
    RegisterSet::new([
        MachineRegister::Aarch64X(CONTEXT_REGISTER),
        MachineRegister::Aarch64X(DESTINATION_REGISTER),
        MachineRegister::Aarch64X(SOURCE_REGISTER),
        MachineRegister::Aarch64X(FRAGMENT_REGISTER),
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

        bytes.extend(encode_load_x_from_x(
            SOURCE_REGISTER,
            CONTEXT_REGISTER,
            source_offset,
        )?);
        super::append_fragment(
            &mut bytes,
            SOURCE_REGISTER,
            DESTINATION_REGISTER,
            FRAGMENT_REGISTER,
            CONTAINER_REGISTER,
            layout_plans::LayoutPlacementReport::Bits {
                container: step.container_byte_offset,
                container_width: u64::from(step.container_width_bits),
                destination_lsb: u64::from(step.destination_lsb),
                source_lsb: u64::from(step.source_lsb),
                width: u64::from(step.width),
            },
        )
        .ok_or_else(|| Diagnostic::error("generated writer fragment cannot be encoded"))?;
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

#[cfg(test)]
mod tests {
    use super::{
        ByteOrder, GeneratedPostHandoffWriterFragmentPlan, MachineRegister,
        encode_generated_post_handoff_writer_bytes,
        generated_post_handoff_writer_additional_machine_state,
        generated_post_handoff_writer_clobbers, generated_post_handoff_writer_width,
    };
    use layout_plans::{
        DataSymbolId, MaterializationWrite, PlacementConstraints, PlacementPhase, RelocationTarget,
    };
    use post_handoff_writer_plan::{
        PostHandoffWriterPlan, PostHandoffWriterSource, PostHandoffWriterStep,
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
