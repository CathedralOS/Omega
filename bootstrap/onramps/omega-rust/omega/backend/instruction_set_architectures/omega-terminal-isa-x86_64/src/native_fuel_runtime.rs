//! Linux x86-64 opaque native-fuel transfer and resume entry encoding.
//!
//! The target encoder emits two explicit relocation fields: one `CALL rel32`
//! for the sponsor policy and one RIP-relative `.text`-base derivation for the
//! section-offset retry coordinate. A later object/image owner binds those
//! fields and independently replays the resulting bytes. This product is
//! implementation data, not installation authority.

use omega_calling_conventions::{
    MachineRegister, MachineState, MachineStateSet, RegisterSet, StateFootprintEvidence,
};
use omega_target::{NativeTarget, TargetProfile};
use omega_terminal_installation_evidence::{
    NativeFuelSavedValue, NativeFuelTransferRuntimePlanProjection, SponsorContextTransport,
};
use psi_diagnostics::Diagnostic;

const CONTEXT_REGISTER: MachineRegister = MachineRegister::X86Rbx;
const RETRY_REGISTER: MachineRegister = MachineRegister::X86R10;
const STACK_REGISTER: MachineRegister = MachineRegister::X86R11;
const SPONSOR_FRAME_BYTES: u64 = 16;
const CALL_RETURN_ADDRESS_BYTES: u64 = 8;
const REALIZED_SPONSOR_STACK_PEAK_BYTES: u64 = SPONSOR_FRAME_BYTES + CALL_RETURN_ADDRESS_BYTES;

/// Canonical compiler-owned transfer/resume bodies for one exact structural
/// plan. The transfer body ends immediately before the resume body, so return
/// from the sponsor call falls through to the separately symbolized resume
/// entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X86NativeFuelTransferRuntimeEncoding {
    transfer_bytes: Vec<u8>,
    resume_bytes: Vec<u8>,
    sponsor_call_rel32_field_offset: usize,
    retry_text_base_rel32_field_offset: usize,
    physical_state_footprint: StateFootprintEvidence,
    realized_sponsor_stack_peak_bytes: u64,
}

impl X86NativeFuelTransferRuntimeEncoding {
    pub fn transfer_bytes(&self) -> &[u8] {
        &self.transfer_bytes
    }

    pub fn resume_bytes(&self) -> &[u8] {
        &self.resume_bytes
    }

    /// Transfer-entry-relative offset of the four-byte `CALL rel32`
    /// displacement. The encoder leaves this field zero for typed object
    /// relocation and final-image replay.
    pub const fn sponsor_call_rel32_field_offset(&self) -> usize {
        self.sponsor_call_rel32_field_offset
    }

    /// Resume-entry-relative offset of the four-byte RIP-relative displacement
    /// used to derive the final image's `.text` base. The retry value retained
    /// in context is an absolute offset in that section, never a raw address.
    pub const fn retry_text_base_rel32_field_offset(&self) -> usize {
        self.retry_text_base_rel32_field_offset
    }

    pub const fn physical_state_footprint(&self) -> &StateFootprintEvidence {
        &self.physical_state_footprint
    }

    pub const fn realized_sponsor_stack_peak_bytes(&self) -> u64 {
        self.realized_sponsor_stack_peak_bytes
    }
}

/// Encode the Linux x86-64 runtime entries for one validated structural plan.
///
/// The activation slots are the complete save/restore inventory for this
/// entry. RBX is the live context transport, while R10/R11 were already chosen
/// as private dispatch scratch and remain private resume scratch; none can
/// masquerade as restorable activation state.
pub fn encode_native_fuel_transfer_runtime(
    plan: &NativeFuelTransferRuntimePlanProjection,
) -> Result<X86NativeFuelTransferRuntimeEncoding, Diagnostic> {
    validate_plan(plan)?;

    let mut transfer_bytes = Vec::new();
    let mut resume_bytes = Vec::new();
    let mut footprint_registers = vec![
        CONTEXT_REGISTER,
        MachineRegister::X86Rsp,
        RETRY_REGISTER,
        STACK_REGISTER,
    ];
    let mut flags_offset = None;
    let mut stack_pointer_offset = None;

    // Register stores and loads do not change RFLAGS. Save the original RSP
    // before switching stacks; RFLAGS is captured only after the switch, so
    // PUSHFQ never writes the suspended activation's stack.
    for slot in plan.activation_state_slots() {
        match slot.value {
            NativeFuelSavedValue::Register(register) => {
                append_register_store(&mut transfer_bytes, register, slot.context_offset)?;
                footprint_registers.push(register);
            }
            NativeFuelSavedValue::Flags => flags_offset = Some(slot.context_offset),
            NativeFuelSavedValue::StackPointer => {
                append_gpr_store(
                    &mut transfer_bytes,
                    MachineRegister::X86Rsp,
                    slot.context_offset,
                )?;
                stack_pointer_offset = Some(slot.context_offset);
            }
        }
    }

    let stack_pointer_offset = stack_pointer_offset.ok_or_else(|| {
        Diagnostic::error("x86-64 native fuel transfer plan lacks its exact RSP save slot")
    })?;
    append_gpr_load(
        &mut transfer_bytes,
        MachineRegister::X86Rsp,
        plan.context().sponsor_stack_top_offset,
    )?;
    if let Some(offset) = flags_offset {
        append_flags_store(&mut transfer_bytes, offset)?;
    }

    // Keep the context pointer recoverable even if the sponsor body clobbers
    // it. Sixteen frame bytes retain SysV call-site alignment; CALL adds the
    // separately accounted eight-byte return address.
    transfer_bytes.extend([0x48, 0x83, 0xec, 0x10]); // sub rsp, 16
    transfer_bytes.extend([0x48, 0x89, 0x1c, 0x24]); // mov [rsp], rbx
    transfer_bytes.push(0xe8); // call rel32
    let sponsor_call_rel32_field_offset = transfer_bytes.len();
    transfer_bytes.extend([0; 4]);
    transfer_bytes.extend([0x48, 0x8b, 0x1c, 0x24]); // mov rbx, [rsp]
    transfer_bytes.extend([0x48, 0x83, 0xc4, 0x10]); // add rsp, 16

    resume_bytes.extend([0x4c, 0x8d, 0x15]); // lea r10, [rip + text-base rel32]
    let retry_text_base_rel32_field_offset = resume_bytes.len();
    resume_bytes.extend([0; 4]);
    resume_bytes.extend([0x4c, 0x03, 0x93]); // add r10, [rbx + retry offset]
    resume_bytes.extend(displacement(plan.context().retry_code_offset_offset)?.to_le_bytes());
    append_gpr_load(&mut resume_bytes, STACK_REGISTER, stack_pointer_offset)?;
    for slot in plan.activation_state_slots() {
        if let NativeFuelSavedValue::Register(register) = slot.value {
            append_register_load(&mut resume_bytes, register, slot.context_offset)?;
        }
    }
    if let Some(offset) = flags_offset {
        append_flags_load(&mut resume_bytes, offset)?;
    }
    resume_bytes.extend([0x4c, 0x89, 0xdc]); // mov rsp, r11
    resume_bytes.extend([0x41, 0xff, 0xe2]); // jmp r10

    let mut realized_state = vec![
        MachineState::Flags,
        MachineState::InstructionPointer,
        MachineState::StackPointer,
    ];
    realized_state.extend(plan.activation_state_slots().iter().filter_map(
        |slot| match slot.value {
            NativeFuelSavedValue::Register(MachineRegister::X86Xmm(_)) => {
                Some(MachineState::VectorRegisters)
            }
            NativeFuelSavedValue::Register(_) => Some(MachineState::GeneralRegisters),
            NativeFuelSavedValue::Flags | NativeFuelSavedValue::StackPointer => None,
        },
    ));
    let physical_state_footprint = StateFootprintEvidence::new(
        RegisterSet::new(footprint_registers),
        MachineStateSet::new(realized_state),
    );
    if !physical_state_footprint
        .machine_state()
        .contains_all(plan.saved_state())
    {
        return Err(Diagnostic::error(
            "x86-64 native fuel runtime footprint does not cover the admitted saved state",
        ));
    }

    Ok(X86NativeFuelTransferRuntimeEncoding {
        transfer_bytes,
        resume_bytes,
        sponsor_call_rel32_field_offset,
        retry_text_base_rel32_field_offset,
        physical_state_footprint,
        realized_sponsor_stack_peak_bytes: REALIZED_SPONSOR_STACK_PEAK_BYTES,
    })
}

fn validate_plan(plan: &NativeFuelTransferRuntimePlanProjection) -> Result<(), Diagnostic> {
    if plan.profile() != TargetProfile::LinuxX64
        || plan.target() != NativeTarget::linux_x64()
        || !matches!(
            plan.transport(),
            SponsorContextTransport::ReservedNonvolatileRegister {
                register: MachineRegister::X86Rbx,
            }
        )
    {
        return Err(Diagnostic::error(
            "native fuel transfer runtime encoder requires the exact Linux x86-64 RBX plan",
        ));
    }
    if plan.sponsor_stack().alignment < 16 {
        return Err(Diagnostic::error(
            "Linux x86-64 native fuel sponsor stack must retain 16-byte call alignment",
        ));
    }
    if plan.sponsor_stack().byte_ceiling < REALIZED_SPONSOR_STACK_PEAK_BYTES {
        return Err(Diagnostic::error(format!(
            "Linux x86-64 native fuel runtime needs {REALIZED_SPONSOR_STACK_PEAK_BYTES} sponsor-stack bytes but the plan admits {}",
            plan.sponsor_stack().byte_ceiling
        )));
    }

    // Validate every displacement before returning any partial byte product.
    displacement(plan.context().retry_code_offset_offset)?;
    displacement(plan.context().sponsor_stack_top_offset)?;
    for slot in plan.activation_state_slots() {
        displacement(slot.context_offset)?;
        match slot.value {
            NativeFuelSavedValue::Register(register) => validate_saved_register(register)?,
            NativeFuelSavedValue::Flags | NativeFuelSavedValue::StackPointer => {}
        }
    }
    Ok(())
}

fn validate_saved_register(register: MachineRegister) -> Result<(), Diagnostic> {
    match register {
        MachineRegister::X86Rbx | MachineRegister::X86R10 | MachineRegister::X86R11 => {
            Err(Diagnostic::error(format!(
                "x86-64 native fuel activation state cannot save reserved register {register:?}"
            )))
        }
        MachineRegister::X86Rsp => Err(Diagnostic::error(
            "x86-64 native fuel activation RSP must use the distinct stack-pointer slot",
        )),
        MachineRegister::X86Xmm(index) if index > 15 => Err(Diagnostic::error(format!(
            "x86-64 native fuel runtime does not encode XMM{index} without an admitted extended-state plan"
        ))),
        MachineRegister::X86Rax
        | MachineRegister::X86Rcx
        | MachineRegister::X86Rdx
        | MachineRegister::X86Rbp
        | MachineRegister::X86Rsi
        | MachineRegister::X86Rdi
        | MachineRegister::X86R8
        | MachineRegister::X86R9
        | MachineRegister::X86R12
        | MachineRegister::X86R13
        | MachineRegister::X86R14
        | MachineRegister::X86R15
        | MachineRegister::X86Xmm(_) => Ok(()),
        MachineRegister::Aarch64X(_) | MachineRegister::Aarch64V(_) => Err(Diagnostic::error(
            "x86-64 native fuel runtime rejects a foreign-architecture activation register",
        )),
    }
}

fn append_register_store(
    bytes: &mut Vec<u8>,
    register: MachineRegister,
    offset: u32,
) -> Result<(), Diagnostic> {
    match register {
        MachineRegister::X86Xmm(index) => append_xmm_memory(bytes, index, offset, 0x7f),
        _ => append_gpr_store(bytes, register, offset),
    }
}

fn append_register_load(
    bytes: &mut Vec<u8>,
    register: MachineRegister,
    offset: u32,
) -> Result<(), Diagnostic> {
    match register {
        MachineRegister::X86Xmm(index) => append_xmm_memory(bytes, index, offset, 0x6f),
        _ => append_gpr_load(bytes, register, offset),
    }
}

fn append_gpr_store(
    bytes: &mut Vec<u8>,
    register: MachineRegister,
    offset: u32,
) -> Result<(), Diagnostic> {
    append_gpr_memory(bytes, register, offset, 0x89)
}

fn append_gpr_load(
    bytes: &mut Vec<u8>,
    register: MachineRegister,
    offset: u32,
) -> Result<(), Diagnostic> {
    append_gpr_memory(bytes, register, offset, 0x8b)
}

fn append_gpr_memory(
    bytes: &mut Vec<u8>,
    register: MachineRegister,
    offset: u32,
    opcode: u8,
) -> Result<(), Diagnostic> {
    let code = gpr_code(register).ok_or_else(|| {
        Diagnostic::error(format!(
            "x86-64 native fuel runtime cannot encode register {register:?}"
        ))
    })?;
    bytes.push(0x48 | (((code >= 8) as u8) << 2)); // REX.W + optional REX.R
    bytes.push(opcode);
    bytes.push(0x80 | ((code & 7) << 3) | 3); // mod=disp32, base=RBX
    bytes.extend(displacement(offset)?.to_le_bytes());
    Ok(())
}

fn append_xmm_memory(
    bytes: &mut Vec<u8>,
    index: u8,
    offset: u32,
    opcode: u8,
) -> Result<(), Diagnostic> {
    if index > 15 {
        return Err(Diagnostic::error(format!(
            "x86-64 native fuel runtime cannot encode XMM{index}"
        )));
    }
    bytes.push(0xf3); // MOVDQU
    if index >= 8 {
        bytes.push(0x44); // REX.R
    }
    bytes.extend([0x0f, opcode, 0x80 | ((index & 7) << 3) | 3]);
    bytes.extend(displacement(offset)?.to_le_bytes());
    Ok(())
}

fn append_flags_store(bytes: &mut Vec<u8>, offset: u32) -> Result<(), Diagnostic> {
    bytes.push(0x9c); // pushfq on the sponsor stack
    bytes.extend([0x8f, 0x83]); // pop qword ptr [rbx + disp32]
    bytes.extend(displacement(offset)?.to_le_bytes());
    Ok(())
}

fn append_flags_load(bytes: &mut Vec<u8>, offset: u32) -> Result<(), Diagnostic> {
    bytes.extend([0xff, 0xb3]); // push qword ptr [rbx + disp32]
    bytes.extend(displacement(offset)?.to_le_bytes());
    bytes.push(0x9d); // popfq on the sponsor stack
    Ok(())
}

fn gpr_code(register: MachineRegister) -> Option<u8> {
    Some(match register {
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
        | MachineRegister::Aarch64V(_) => return None,
    })
}

fn displacement(offset: u32) -> Result<i32, Diagnostic> {
    i32::try_from(offset).map_err(|_| {
        Diagnostic::error(format!(
            "Linux x86-64 native fuel runtime cannot address context offset {offset}"
        ))
    })
}

#[cfg(test)]
mod tests;
