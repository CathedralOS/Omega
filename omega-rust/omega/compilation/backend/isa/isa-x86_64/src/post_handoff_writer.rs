/*
We lower validated `PostHandoffWriterPlan` fragment operations into position-
independent, address-free x86-64 instruction bytes
(`encode_generated_post_handoff_writer_bytes`) and report the helper's exact
clobber set (`rax`, `rcx`, `rdx`, `r10`, `r11`), byte width, and flag effects.
*/

mod fragment {
    use calling_conventions::{MachineRegister, gpr_code};
    use layout_plans::LayoutPlacementReport;

    pub(crate) fn append_fragment(
        bytes: &mut Vec<u8>,
        source: MachineRegister,
        root: MachineRegister,
        fragment: MachineRegister,
        container: MachineRegister,
        placement: LayoutPlacementReport,
    ) -> Option<()> {
        let source = gpr_code(source)?;
        let root = gpr_code(root)?;
        let fragment = gpr_code(fragment)?;
        let container = gpr_code(container)?;
        let registers = [source, root, fragment, container];
        if registers
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
            i32::MAX as usize,
            "private address",
            offset,
            u16::try_from(container_width).ok()?,
            u16::try_from(destination_lsb).ok()?,
            u16::try_from(source_lsb).ok()?,
            u16::try_from(width).ok()?,
        )
        .ok()?;
        let offset = u32::try_from(offset).ok()?;
        let container_bytes = (container_width / 8) as u8;
        let whole = destination_lsb == 0 && width == container_width;
        if !whole {
            load(bytes, container, fragment, root, offset, container_bytes);
            shift(bytes, container, 1, destination_lsb as u8);
            shift(bytes, container, 5, width as u8);
            shift(bytes, container, 4, width as u8);
            shift(bytes, container, 0, destination_lsb as u8);
        }
        register_binary(bytes, 0x89, source, fragment);
        shift(bytes, fragment, 5, source_lsb as u8);
        if width < 64 {
            shift(bytes, fragment, 4, (64 - width) as u8);
            shift(bytes, fragment, 5, (64 - width) as u8);
        }
        shift(bytes, fragment, 4, destination_lsb as u8);
        let result = if whole {
            fragment
        } else {
            register_binary(bytes, 0x09, fragment, container);
            container
        };
        store(bytes, result, root, offset, container_bytes);
        Some(())
    }

    fn shift(bytes: &mut Vec<u8>, register: u8, operation: u8, amount: u8) {
        if amount != 0 {
            bytes.extend([
                0x48 | u8::from(register >= 8),
                0xc1,
                0xc0 | (operation << 3) | (register & 7),
                amount,
            ]);
        }
    }

    fn register_binary(bytes: &mut Vec<u8>, opcode: u8, source: u8, destination: u8) {
        bytes.extend([
            0x48 | (u8::from(source >= 8) << 2) | u8::from(destination >= 8),
            opcode,
            0xc0 | ((source & 7) << 3) | (destination & 7),
        ]);
    }

    fn memory(bytes: &mut Vec<u8>, opcode: &[u8], wide: bool, register: u8, root: u8, offset: u32) {
        bytes.push(
            0x40 | (u8::from(wide) << 3) | (u8::from(register >= 8) << 2) | u8::from(root >= 8),
        );
        bytes.extend_from_slice(opcode);
        bytes.push(0x80 | ((register & 7) << 3) | (root & 7));
        if root & 7 == 4 {
            bytes.push(0x24);
        }
        bytes.extend_from_slice(&offset.to_le_bytes());
    }

    fn load(bytes: &mut Vec<u8>, result: u8, temporary: u8, root: u8, offset: u32, count: u8) {
        match count {
            1 => memory(bytes, &[0x0f, 0xb6], true, result, root, offset),
            2 => memory(bytes, &[0x0f, 0xb7], true, result, root, offset),
            4 => memory(bytes, &[0x8b], false, result, root, offset),
            8 => memory(bytes, &[0x8b], true, result, root, offset),
            _ => {
                register_binary(bytes, 0x31, result, result);
                for index in (0..count).rev() {
                    shift(bytes, result, 4, 8);
                    memory(
                        bytes,
                        &[0x0f, 0xb6],
                        true,
                        temporary,
                        root,
                        offset + u32::from(index),
                    );
                    register_binary(bytes, 0x09, temporary, result);
                }
            }
        }
    }

    fn store(bytes: &mut Vec<u8>, source: u8, root: u8, offset: u32, count: u8) {
        match count {
            1 => memory(bytes, &[0x88], false, source, root, offset),
            2 => {
                bytes.push(0x66);
                memory(bytes, &[0x89], false, source, root, offset);
            }
            4 => memory(bytes, &[0x89], false, source, root, offset),
            8 => memory(bytes, &[0x89], true, source, root, offset),
            _ => {
                for index in 0..count {
                    memory(
                        bytes,
                        &[0x88],
                        false,
                        source,
                        root,
                        offset + u32::from(index),
                    );
                    if index + 1 < count {
                        shift(bytes, source, 5, 8);
                    }
                }
            }
        }
    }
}
pub(crate) use fragment::append_fragment;

use calling_conventions::{MachineRegister, MachineState, MachineStateSet, RegisterSet};
use diagnostics::Diagnostic;
use layout_plans::ByteOrder;
use post_handoff_writer_plan::{
    GeneratedPostHandoffWriterFragmentPlan, POST_HANDOFF_WRITER_CONTEXT_ABI_V1,
    POST_HANDOFF_WRITER_SOURCE_SLOT_WIDTH, POST_HANDOFF_WRITER_SOURCE_SLOTS_OFFSET,
    post_handoff_writer_context_byte_len, validate_step,
};

pub fn generated_post_handoff_writer_clobbers() -> RegisterSet {
    RegisterSet::new([
        MachineRegister::X86Rax,
        MachineRegister::X86Rcx,
        MachineRegister::X86Rdx,
        MachineRegister::X86R10,
        MachineRegister::X86R11,
    ])
}

pub fn generated_post_handoff_writer_additional_machine_state() -> MachineStateSet {
    MachineStateSet::new([MachineState::Flags])
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
    validate_generated_post_handoff_writer(plan)?;

    let mut bytes = encode_private_pointer_to_r10(pointer_register)?.to_vec();
    bytes.extend([0x4d, 0x8b, 0x1a]);
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
        let source_displacement = i32::try_from(source_offset)
            .map_err(|_| Diagnostic::error("generated writer source offset exceeds disp32"))?;

        bytes.extend([0x49, 0x8b, 0x82]);
        bytes.extend(source_displacement.to_le_bytes());
        append_fragment(
            &mut bytes,
            MachineRegister::X86Rax,
            MachineRegister::X86R11,
            MachineRegister::X86Rdx,
            MachineRegister::X86Rcx,
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
    plan: &GeneratedPostHandoffWriterFragmentPlan,
) -> Result<(), Diagnostic> {
    if plan.context_abi() != POST_HANDOFF_WRITER_CONTEXT_ABI_V1 {
        return Err(Diagnostic::error(format!(
            "generated post-handoff writer context ABI {:016x} is not PHWRITR1",
            plan.context_abi()
        )));
    }
    if plan.byte_order() != ByteOrder::LittleEndian {
        return Err(Diagnostic::error(
            "generated x86-64 post-handoff writer requires little-endian containers",
        ));
    }
    validate_common_geometry(plan, i32::MAX as usize)
}

fn validate_common_geometry(
    plan: &GeneratedPostHandoffWriterFragmentPlan,
    maximum_context_width: usize,
) -> Result<(), Diagnostic> {
    if plan.steps().is_empty() || plan.source_slot_count() == 0 {
        return Err(Diagnostic::error(
            "generated post-handoff writer requires at least one fragment and source slot",
        ));
    }
    let context_width = post_handoff_writer_context_byte_len(plan.source_slot_count())
        .ok_or_else(|| Diagnostic::error("generated writer private context size overflows"))?;
    if context_width > maximum_context_width {
        return Err(Diagnostic::error(
            "generated writer private context exceeds target addressing",
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
    Ok(())
}

fn encode_private_pointer_to_r10(source: MachineRegister) -> Result<[u8; 3], Diagnostic> {
    let code = calling_conventions::gpr_code(source).ok_or_else(|| {
        Diagnostic::error(format!(
            "generated x86-64 writer context cannot arrive in {source:?}"
        ))
    })?;
    Ok([0x4c | u8::from(code >= 8), 0x8b, 0xd0 | (code & 7)])
}

#[cfg(test)]
mod tests {
    use super::{
        ByteOrder, GeneratedPostHandoffWriterFragmentPlan, MachineRegister, MachineState,
        MachineStateSet, encode_generated_post_handoff_writer_bytes,
        generated_post_handoff_writer_additional_machine_state,
        generated_post_handoff_writer_clobbers, generated_post_handoff_writer_width,
        post_handoff_writer_context_byte_len,
    };
    use layout_plans::{
        EntryStubId, MaterializationWrite, PlacementConstraints, PlacementPhase, RelocationTarget,
    };
    use post_handoff_writer_plan::{
        PostHandoffWriterPlan, PostHandoffWriterSource, PostHandoffWriterStep,
    };

    fn fragment() -> GeneratedPostHandoffWriterFragmentPlan {
        let target = RelocationTarget::Entry(
            EntryStubId::from_normalized_identity(7).expect("entry identity"),
        );
        PostHandoffWriterPlan {
            byte_len: 16,
            byte_order: ByteOrder::LittleEndian,
            placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
            steps: vec![
                PostHandoffWriterStep {
                    write: MaterializationWrite {
                        field: "entry".into(),
                        target,
                        container_byte_offset: 0,
                        container_width_bits: 64,
                        destination_lsb: 16,
                        source_lsb: 0,
                        width: 16,
                        stored_integer_fit: None,
                    },
                    source: PostHandoffWriterSource::Resolve(target),
                },
                PostHandoffWriterStep {
                    write: MaterializationWrite {
                        field: "entry".into(),
                        target,
                        container_byte_offset: 8,
                        container_width_bits: 64,
                        destination_lsb: 0,
                        source_lsb: 16,
                        width: 48,
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
    fn generated_writer_emits_complete_fragment_program() {
        let fragment = fragment();
        let bytes = encode_generated_post_handoff_writer_bytes(MachineRegister::X86Rdi, &fragment)
            .expect("x86-64 writer");
        assert!(!bytes.is_empty());
        assert_eq!(
            bytes.len(),
            generated_post_handoff_writer_width(MachineRegister::X86Rdi, &fragment)
                .expect("matching width")
        );
        assert_eq!(post_handoff_writer_context_byte_len(1), Some(16));
        assert_eq!(
            generated_post_handoff_writer_clobbers().as_slice(),
            &[
                MachineRegister::X86Rax,
                MachineRegister::X86Rcx,
                MachineRegister::X86Rdx,
                MachineRegister::X86R10,
                MachineRegister::X86R11,
            ]
        );
        assert!(
            generated_post_handoff_writer_additional_machine_state()
                .contains_all(MachineStateSet::new([MachineState::Flags]))
        );
    }

    #[test]
    fn generated_writer_rejects_non_gpr_context_register() {
        let error =
            encode_generated_post_handoff_writer_bytes(MachineRegister::Aarch64X(0), &fragment())
                .expect_err("wrong architecture register");
        assert!(error.message.contains("cannot arrive"));
    }
}
