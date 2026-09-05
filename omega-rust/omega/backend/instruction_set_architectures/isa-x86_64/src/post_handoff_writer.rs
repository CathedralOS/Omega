//! Reusable post-handoff writer lowering for x86-64.
//!
//! The input is the target-neutral, address-free fragment plan derived from a
//! validated `PostHandoffWriterPlan`. R10 receives the provider-private packed
//! context pointer; no destination or symbolic address is embedded in code.

use calling_conventions::{MachineRegister, MachineState, MachineStateSet, RegisterSet};
use diagnostics::Diagnostic;
use layout_plans::{
    ByteOrder, GeneratedPostHandoffWriterFragmentPlan, GeneratedPostHandoffWriterStep,
    POST_HANDOFF_WRITER_CONTEXT_ABI_V1, POST_HANDOFF_WRITER_SOURCE_SLOT_WIDTH,
    POST_HANDOFF_WRITER_SOURCE_SLOTS_OFFSET, post_handoff_writer_context_byte_len,
};

/// Exact registers written by the generated helper. R10 holds the packed
/// context, R11 the destination, RAX the source fragment, RCX the destination
/// container, and RDX the masks.
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

/// Emit a complete direct-destination writer. Every memory access uses pinned
/// disp32 geometry and the complete plan is revalidated before any bytes are
/// returned.
pub fn encode_generated_post_handoff_writer_bytes(
    pointer_register: MachineRegister,
    plan: &GeneratedPostHandoffWriterFragmentPlan,
) -> Result<Vec<u8>, Diagnostic> {
    validate_generated_post_handoff_writer(plan)?;

    let mut bytes = encode_private_pointer_to_r10(pointer_register)?.to_vec();
    bytes.extend([0x4d, 0x8b, 0x1a]); // mov r11, [r10]
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
        let destination_displacement = i32::try_from(step.container_byte_offset)
            .map_err(|_| Diagnostic::error("generated writer destination offset exceeds disp32"))?;

        bytes.extend([0x49, 0x8b, 0x82]); // mov rax, [r10+disp32]
        bytes.extend(source_displacement.to_le_bytes());
        if step.source_lsb != 0 {
            bytes.extend([0x48, 0xc1, 0xe8, step.source_lsb as u8]); // shr rax, imm8
        }
        let fragment_mask = low_mask(step.width);
        bytes.extend([0x48, 0xba]); // mov rdx, imm64
        bytes.extend(fragment_mask.to_le_bytes());
        bytes.extend([0x48, 0x21, 0xd0]); // and rax, rdx
        if step.destination_lsb != 0 {
            bytes.extend([0x48, 0xc1, 0xe0, step.destination_lsb as u8]); // shl rax, imm8
        }

        match step.container_width_bits {
            8 => bytes.extend([0x41, 0x0f, 0xb6, 0x8b]), // movzx ecx, byte [r11+disp32]
            16 => bytes.extend([0x41, 0x0f, 0xb7, 0x8b]), // movzx ecx, word [r11+disp32]
            32 => bytes.extend([0x41, 0x8b, 0x8b]),      // mov ecx, [r11+disp32]
            64 => bytes.extend([0x49, 0x8b, 0x8b]),      // mov rcx, [r11+disp32]
            _ => unreachable!("generated writer container width validated"),
        }
        bytes.extend(destination_displacement.to_le_bytes());

        let destination_mask = fragment_mask << step.destination_lsb;
        bytes.extend([0x48, 0xba]); // mov rdx, imm64
        bytes.extend((!destination_mask).to_le_bytes());
        bytes.extend([0x48, 0x21, 0xd1]); // and rcx, rdx
        bytes.extend([0x48, 0x09, 0xc1]); // or rcx, rax

        match step.container_width_bits {
            8 => bytes.extend([0x41, 0x88, 0x8b]), // mov byte [r11+disp32], cl
            16 => bytes.extend([0x66, 0x41, 0x89, 0x8b]), // mov word [r11+disp32], cx
            32 => bytes.extend([0x41, 0x89, 0x8b]), // mov dword [r11+disp32], ecx
            64 => bytes.extend([0x49, 0x89, 0x8b]), // mov qword [r11+disp32], rcx
            _ => unreachable!("generated writer container width validated"),
        }
        bytes.extend(destination_displacement.to_le_bytes());
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

fn encode_private_pointer_to_r10(source: MachineRegister) -> Result<[u8; 3], Diagnostic> {
    let code = match source {
        MachineRegister::X86Rax => 0,
        MachineRegister::X86Rcx => 1,
        MachineRegister::X86Rdx => 2,
        MachineRegister::X86Rbx => 3,
        MachineRegister::X86Rsp => 4,
        MachineRegister::X86Rbp => 5,
        MachineRegister::X86Rsi => 6,
        MachineRegister::X86Rdi => 7,
        MachineRegister::X86R8 => 8,
        MachineRegister::X86R9 => 9,
        MachineRegister::X86R10 => 10,
        MachineRegister::X86R11 => 11,
        MachineRegister::X86R12 => 12,
        MachineRegister::X86R13 => 13,
        MachineRegister::X86R14 => 14,
        MachineRegister::X86R15 => 15,
        MachineRegister::X86Xmm(_)
        | MachineRegister::Aarch64X(_)
        | MachineRegister::Aarch64V(_) => {
            return Err(Diagnostic::error(format!(
                "generated x86-64 writer context cannot arrive in {source:?}"
            )));
        }
    };
    Ok([0x4c | u8::from(code >= 8), 0x8b, 0xd0 | (code & 7)])
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
        EntryStubId, MaterializationWrite, PlacementConstraints, PlacementPhase,
        PostHandoffWriterPlan, PostHandoffWriterSource, PostHandoffWriterStep, RelocationTarget,
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
