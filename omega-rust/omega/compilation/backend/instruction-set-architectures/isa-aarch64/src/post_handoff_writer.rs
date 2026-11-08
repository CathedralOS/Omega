/*
We lower validated `PostHandoffWriterPlan` fragment operations into position-
independent, address-free AArch64 instruction words
(`encode_generated_post_handoff_writer_bytes`) and report the helper's exact
clobber set (`x9`..`x13`), byte width, and additional machine state.
*/

mod fragment {
    use super::writer_encoding::{
        encode_load_w_from_x, encode_load_x_from_x, encode_store_w_to_x, encode_store_x_to_x,
    };
    use layout_plans::LayoutPlacementReport;

    pub(crate) fn append_fragment(
        bytes: &mut Vec<u8>,
        source: u8,
        root: u8,
        fragment: u8,
        container: u8,
        placement: LayoutPlacementReport,
    ) -> Option<()> {
        let registers = [source, root, fragment, container];
        if registers.iter().any(|register| *register > 30)
            || registers
                .iter()
                .enumerate()
                .any(|(index, register)| registers[..index].contains(register))
        {
            return None;
        }
        let (offset, container_width, destination_lsb, source_lsb, width) = match placement {
            LayoutPlacementReport::At { offset } => (offset, 64, 0, 0, 64),
            LayoutPlacementReport::Bits {
                container,
                container_width,
                destination_lsb,
                source_lsb,
                width,
            } => (
                container,
                container_width,
                destination_lsb,
                source_lsb,
                width,
            ),
            LayoutPlacementReport::IntegerAt { .. } => return None,
        };
        layout_plans::validate_fragment(
            usize::MAX,
            "private address",
            offset,
            u16::try_from(container_width).ok()?,
            u16::try_from(destination_lsb).ok()?,
            u16::try_from(source_lsb).ok()?,
            u16::try_from(width).ok()?,
        )
        .ok()?;
        let offset = usize::try_from(offset).ok()?;
        let count = usize::try_from(container_width / 8).ok()?;
        let whole = destination_lsb == 0 && width == container_width;
        if !whole {
            load(bytes, container, fragment, root, offset, count)?;
        }
        instruction(
            bytes,
            0xd340_0000
                | ((source_lsb as u32) << 16)
                | (((source_lsb + width - 1) as u32) << 10)
                | (u32::from(source) << 5)
                | u32::from(fragment),
        );
        let result = if whole {
            fragment
        } else {
            instruction(
                bytes,
                0xb340_0000
                    | (((64 - destination_lsb) as u32 % 64) << 16)
                    | (((width - 1) as u32) << 10)
                    | (u32::from(fragment) << 5)
                    | u32::from(container),
            );
            container
        };
        store(bytes, result, root, offset, count)?;
        Some(())
    }

    fn instruction(bytes: &mut Vec<u8>, word: u32) {
        bytes.extend_from_slice(&word.to_le_bytes());
    }

    fn load(
        bytes: &mut Vec<u8>,
        result: u8,
        temporary: u8,
        root: u8,
        offset: usize,
        count: usize,
    ) -> Option<()> {
        match count {
            1 | 2 | 4 => bytes.extend(encode_load_w_from_x(result, root, offset, count).ok()?),
            8 => bytes.extend(encode_load_x_from_x(result, root, offset).ok()?),
            _ => {
                instruction(bytes, 0xd280_0000 | u32::from(result));
                for index in 0..count {
                    bytes.extend(encode_load_w_from_x(temporary, root, offset + index, 1).ok()?);
                    instruction(
                        bytes,
                        0xaa00_0000
                            | (u32::from(temporary) << 16)
                            | ((index as u32 * 8) << 10)
                            | (u32::from(result) << 5)
                            | u32::from(result),
                    );
                }
            }
        }
        Some(())
    }

    fn store(bytes: &mut Vec<u8>, source: u8, root: u8, offset: usize, count: usize) -> Option<()> {
        match count {
            1 | 2 | 4 => bytes.extend(encode_store_w_to_x(source, root, offset, count).ok()?),
            8 => bytes.extend(encode_store_x_to_x(source, root, offset).ok()?),
            _ => {
                for index in 0..count {
                    bytes.extend(encode_store_w_to_x(source, root, offset + index, 1).ok()?);
                    if index + 1 < count {
                        instruction(
                            bytes,
                            0xd340_fc00 | (8 << 16) | (u32::from(source) << 5) | u32::from(source),
                        );
                    }
                }
            }
        }
        Some(())
    }
}
pub(crate) use fragment::append_fragment;
mod generated_writer {
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
                .ok_or_else(|| {
                    Diagnostic::error("generated writer source-slot offset overflows")
                })?;

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
            DataSymbolId, MaterializationWrite, PlacementConstraints, PlacementPhase,
            RelocationTarget,
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
}
mod writer_encoding {
    use diagnostics::Diagnostic;

    fn instruction(value: u32) -> [u8; 4] {
        value.to_le_bytes()
    }

    pub(super) fn encode_move_x_register(destination: u8, source: u8) -> [u8; 4] {
        instruction(0xAA00_03E0 | (u32::from(source) << 16) | u32::from(destination))
    }

    fn encode_load_byte_w(destination: u8, base: u8, offset: usize) -> Result<[u8; 4], Diagnostic> {
        if offset > 4095 {
            return Err(Diagnostic::error(format!(
                "AArch64 writer cannot load a byte at offset `{offset}`"
            )));
        }
        Ok(instruction(
            0x3940_0000 | ((offset as u32) << 10) | (u32::from(base) << 5) | u32::from(destination),
        ))
    }

    pub(super) fn encode_load_w_from_x(
        destination: u8,
        base: u8,
        offset: usize,
        byte_size: usize,
    ) -> Result<[u8; 4], Diagnostic> {
        match byte_size {
            1 => encode_load_byte_w(destination, base, offset),
            2 if offset.is_multiple_of(2) && offset / 2 <= 4095 => Ok(instruction(
                0x7940_0000
                    | (((offset / 2) as u32) << 10)
                    | (u32::from(base) << 5)
                    | u32::from(destination),
            )),
            4 if offset.is_multiple_of(4) && offset / 4 <= 4095 => Ok(instruction(
                0xB940_0000
                    | (((offset / 4) as u32) << 10)
                    | (u32::from(base) << 5)
                    | u32::from(destination),
            )),
            _ => Err(Diagnostic::error(format!(
                "AArch64 writer cannot load a {byte_size}-byte value at offset `{offset}`"
            ))),
        }
    }

    pub(super) fn encode_load_x_from_x(
        destination: u8,
        base: u8,
        offset: usize,
    ) -> Result<[u8; 4], Diagnostic> {
        if offset.is_multiple_of(8) && offset / 8 <= 4095 {
            return Ok(instruction(
                0xF940_0000
                    | (((offset / 8) as u32) << 10)
                    | (u32::from(base) << 5)
                    | u32::from(destination),
            ));
        }
        if offset <= 255 {
            return Ok(instruction(
                0xF840_0000
                    | ((offset as u32) << 12)
                    | (u32::from(base) << 5)
                    | u32::from(destination),
            ));
        }
        Err(Diagnostic::error(format!(
            "AArch64 writer cannot load a u64 at offset `{offset}`"
        )))
    }

    fn encode_store_byte_w(source: u8, base: u8, offset: usize) -> Result<[u8; 4], Diagnostic> {
        if offset > 4095 {
            return Err(Diagnostic::error(format!(
                "AArch64 writer cannot store a byte at offset `{offset}`"
            )));
        }
        Ok(instruction(
            0x3900_0000 | ((offset as u32) << 10) | (u32::from(base) << 5) | u32::from(source),
        ))
    }

    pub(super) fn encode_store_w_to_x(
        source: u8,
        base: u8,
        offset: usize,
        byte_size: usize,
    ) -> Result<[u8; 4], Diagnostic> {
        match byte_size {
            1 => encode_store_byte_w(source, base, offset),
            2 if offset.is_multiple_of(2) && offset / 2 <= 4095 => Ok(instruction(
                0x7900_0000
                    | (((offset / 2) as u32) << 10)
                    | (u32::from(base) << 5)
                    | u32::from(source),
            )),
            4 if offset.is_multiple_of(4) && offset / 4 <= 4095 => Ok(instruction(
                0xB900_0000
                    | (((offset / 4) as u32) << 10)
                    | (u32::from(base) << 5)
                    | u32::from(source),
            )),
            _ => Err(Diagnostic::error(format!(
                "AArch64 writer cannot store a {byte_size}-byte value at offset `{offset}`"
            ))),
        }
    }

    pub(super) fn encode_store_x_to_x(
        source: u8,
        base: u8,
        offset: usize,
    ) -> Result<[u8; 4], Diagnostic> {
        if !offset.is_multiple_of(8) || offset / 8 > 4095 {
            return Err(Diagnostic::error(format!(
                "AArch64 writer cannot store a u64 at offset `{offset}`"
            )));
        }
        Ok(instruction(
            0xF900_0000
                | (((offset / 8) as u32) << 10)
                | (u32::from(base) << 5)
                | u32::from(source),
        ))
    }
}
pub use generated_writer::{
    encode_generated_post_handoff_writer_bytes,
    generated_post_handoff_writer_additional_machine_state, generated_post_handoff_writer_clobbers,
    generated_post_handoff_writer_width,
};
