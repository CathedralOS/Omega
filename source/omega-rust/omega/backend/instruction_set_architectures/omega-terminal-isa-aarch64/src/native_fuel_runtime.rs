//! Linux AArch64 opaque native-fuel transfer and resume entry encoding.
//!
//! The target encoder leaves one `BL` instruction and one `ADRP`/`ADD` pair
//! ready for typed object relocation. A later object/image owner binds and
//! independently replays those fields. This product is structural target
//! evidence only; it grants neither installed sponsor-route nor executable
//! transfer authority.

use omega_calling_conventions::{
    MachineRegister, MachineState, MachineStateSet, RegisterSet, StateFootprintEvidence,
};
use omega_target::{NativeTarget, TargetProfile};
use omega_terminal_installation_evidence::{
    NativeFuelSavedValue, NativeFuelTransferRuntimePlanProjection, SponsorContextTransport,
};
use psi_diagnostics::Diagnostic;

const CONTEXT_REGISTER: u8 = 28;
const RETRY_REGISTER: u8 = 16;
const STACK_SCRATCH_REGISTER: u8 = 17;
const LINK_REGISTER: u8 = 30;
const REALIZED_SPONSOR_STACK_PEAK_BYTES: u64 = 16;

/// Canonical compiler-owned transfer/resume bodies for one exact structural
/// plan. The transfer body ends immediately before the resume body, so return
/// from the sponsor call falls through to the separately symbolized resume
/// entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aarch64NativeFuelTransferRuntimeEncoding {
    transfer_bytes: Vec<u8>,
    resume_bytes: Vec<u8>,
    sponsor_call_branch26_offset: usize,
    retry_text_page21_offset: usize,
    retry_text_page_offset12_offset: usize,
    physical_state_footprint: StateFootprintEvidence,
    realized_sponsor_stack_peak_bytes: u64,
}

impl Aarch64NativeFuelTransferRuntimeEncoding {
    pub fn transfer_bytes(&self) -> &[u8] {
        &self.transfer_bytes
    }

    pub fn resume_bytes(&self) -> &[u8] {
        &self.resume_bytes
    }

    /// Transfer-entry-relative offset of the `BL imm26` instruction. Its
    /// immediate bits remain zero for an `Aarch64Branch26` relocation.
    pub const fn sponsor_call_branch26_offset(&self) -> usize {
        self.sponsor_call_branch26_offset
    }

    /// Resume-entry-relative offset of the `ADRP` instruction used to derive
    /// the final image's `.text` base.
    pub const fn retry_text_page21_offset(&self) -> usize {
        self.retry_text_page21_offset
    }

    /// Resume-entry-relative offset of the paired `ADD` instruction's low-12
    /// immediate field.
    pub const fn retry_text_page_offset12_offset(&self) -> usize {
        self.retry_text_page_offset12_offset
    }

    pub const fn physical_state_footprint(&self) -> &StateFootprintEvidence {
        &self.physical_state_footprint
    }

    pub const fn realized_sponsor_stack_peak_bytes(&self) -> u64 {
        self.realized_sponsor_stack_peak_bytes
    }
}

/// Encode the Linux AArch64 runtime entries for one validated structural plan.
///
/// X28 is the live context transport. X16/X17 are the already reserved private
/// dispatch scratch registers; X30 is modified by `BL` and is restored only
/// when the exact activation inventory contains it. SP remains a distinct
/// saved value and is never represented as X31/ZR.
pub fn encode_native_fuel_transfer_runtime(
    plan: &NativeFuelTransferRuntimePlanProjection,
) -> Result<Aarch64NativeFuelTransferRuntimeEncoding, Diagnostic> {
    validate_plan(plan)?;

    let mut transfer_bytes = Vec::new();
    let mut resume_bytes = Vec::new();
    let mut footprint_registers = vec![
        MachineRegister::Aarch64X(RETRY_REGISTER),
        MachineRegister::Aarch64X(STACK_SCRATCH_REGISTER),
        MachineRegister::Aarch64X(CONTEXT_REGISTER),
        MachineRegister::Aarch64X(LINK_REGISTER),
    ];
    let mut flags_offset = None;
    let mut stack_pointer_offset = None;

    for slot in plan.activation_state_slots() {
        match slot.value {
            NativeFuelSavedValue::Register(register) => {
                append_register_store(&mut transfer_bytes, register, slot.context_offset)?;
                footprint_registers.push(register);
            }
            NativeFuelSavedValue::Flags => {
                transfer_bytes.extend(encode_read_nzcv(RETRY_REGISTER));
                transfer_bytes.extend(encode_store_x_to_x(
                    RETRY_REGISTER,
                    CONTEXT_REGISTER,
                    slot.context_offset,
                )?);
                flags_offset = Some(slot.context_offset);
            }
            NativeFuelSavedValue::StackPointer => {
                transfer_bytes.extend(encode_move_x_from_sp(STACK_SCRATCH_REGISTER));
                transfer_bytes.extend(encode_store_x_to_x(
                    STACK_SCRATCH_REGISTER,
                    CONTEXT_REGISTER,
                    slot.context_offset,
                )?);
                stack_pointer_offset = Some(slot.context_offset);
            }
        }
    }

    let stack_pointer_offset = stack_pointer_offset.ok_or_else(|| {
        Diagnostic::error("AArch64 native fuel transfer plan lacks its exact SP save slot")
    })?;
    transfer_bytes.extend(encode_load_x_from_x(
        STACK_SCRATCH_REGISTER,
        CONTEXT_REGISTER,
        plan.context().sponsor_stack_top_offset,
    )?);
    transfer_bytes.extend(encode_move_sp_from_x(STACK_SCRATCH_REGISTER));
    transfer_bytes.extend(encode_sub_sp_immediate(16));
    transfer_bytes.extend(encode_store_x_to_sp(CONTEXT_REGISTER, 0)?);
    let sponsor_call_branch26_offset = transfer_bytes.len();
    transfer_bytes.extend(0x9400_0000_u32.to_le_bytes()); // bl sponsor (Branch26)
    transfer_bytes.extend(encode_load_x_from_sp(CONTEXT_REGISTER, 0)?);
    transfer_bytes.extend(encode_add_sp_immediate(16));

    let retry_text_page21_offset = resume_bytes.len();
    resume_bytes.extend((0x9000_0000_u32 | u32::from(RETRY_REGISTER)).to_le_bytes()); // adrp x16
    let retry_text_page_offset12_offset = resume_bytes.len();
    resume_bytes.extend(encode_add_x_immediate(RETRY_REGISTER, RETRY_REGISTER, 0)?);
    resume_bytes.extend(encode_load_x_from_x(
        STACK_SCRATCH_REGISTER,
        CONTEXT_REGISTER,
        plan.context().retry_code_offset_offset,
    )?);
    resume_bytes.extend(encode_add_x_register(
        RETRY_REGISTER,
        RETRY_REGISTER,
        STACK_SCRATCH_REGISTER,
    ));
    for slot in plan.activation_state_slots() {
        if let NativeFuelSavedValue::Register(register) = slot.value {
            append_register_load(&mut resume_bytes, register, slot.context_offset)?;
        }
    }
    if let Some(offset) = flags_offset {
        resume_bytes.extend(encode_load_x_from_x(
            STACK_SCRATCH_REGISTER,
            CONTEXT_REGISTER,
            offset,
        )?);
        resume_bytes.extend(encode_write_nzcv(STACK_SCRATCH_REGISTER));
    }
    resume_bytes.extend(encode_load_x_from_x(
        STACK_SCRATCH_REGISTER,
        CONTEXT_REGISTER,
        stack_pointer_offset,
    )?);
    resume_bytes.extend(encode_move_sp_from_x(STACK_SCRATCH_REGISTER));
    resume_bytes.extend(encode_branch_register(RETRY_REGISTER));

    let mut realized_state = vec![MachineState::InstructionPointer, MachineState::StackPointer];
    realized_state.extend(plan.activation_state_slots().iter().filter_map(
        |slot| match slot.value {
            NativeFuelSavedValue::Register(MachineRegister::Aarch64V(_)) => {
                Some(MachineState::VectorRegisters)
            }
            NativeFuelSavedValue::Register(_) => Some(MachineState::GeneralRegisters),
            NativeFuelSavedValue::Flags => Some(MachineState::Flags),
            NativeFuelSavedValue::StackPointer => None,
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
            "AArch64 native fuel runtime footprint does not cover the admitted saved state",
        ));
    }

    Ok(Aarch64NativeFuelTransferRuntimeEncoding {
        transfer_bytes,
        resume_bytes,
        sponsor_call_branch26_offset,
        retry_text_page21_offset,
        retry_text_page_offset12_offset,
        physical_state_footprint,
        realized_sponsor_stack_peak_bytes: REALIZED_SPONSOR_STACK_PEAK_BYTES,
    })
}

fn validate_plan(plan: &NativeFuelTransferRuntimePlanProjection) -> Result<(), Diagnostic> {
    if plan.profile() != TargetProfile::LinuxArm64
        || plan.target() != NativeTarget::linux_arm64()
        || !matches!(
            plan.transport(),
            SponsorContextTransport::ReservedNonvolatileRegister {
                register: MachineRegister::Aarch64X(CONTEXT_REGISTER),
            }
        )
    {
        return Err(Diagnostic::error(
            "native fuel transfer runtime encoder requires the exact Linux AArch64 X28 plan",
        ));
    }
    if plan.sponsor_stack().alignment < 16 {
        return Err(Diagnostic::error(
            "Linux AArch64 native fuel sponsor stack must retain 16-byte call alignment",
        ));
    }
    if plan.sponsor_stack().byte_ceiling < REALIZED_SPONSOR_STACK_PEAK_BYTES {
        return Err(Diagnostic::error(format!(
            "Linux AArch64 native fuel runtime needs {REALIZED_SPONSOR_STACK_PEAK_BYTES} sponsor-stack bytes but the plan admits {}",
            plan.sponsor_stack().byte_ceiling
        )));
    }

    encode_load_x_from_x(
        STACK_SCRATCH_REGISTER,
        CONTEXT_REGISTER,
        plan.context().retry_code_offset_offset,
    )?;
    encode_load_x_from_x(
        STACK_SCRATCH_REGISTER,
        CONTEXT_REGISTER,
        plan.context().sponsor_stack_top_offset,
    )?;
    for slot in plan.activation_state_slots() {
        match slot.value {
            NativeFuelSavedValue::Register(register) => {
                validate_saved_register(register)?;
                match register {
                    MachineRegister::Aarch64X(index) => {
                        encode_store_x_to_x(index, CONTEXT_REGISTER, slot.context_offset)?;
                    }
                    MachineRegister::Aarch64V(index) => {
                        encode_store_q_to_x(index, CONTEXT_REGISTER, slot.context_offset)?;
                    }
                    _ => unreachable!("saved-register validation is architecture exact"),
                }
            }
            NativeFuelSavedValue::Flags | NativeFuelSavedValue::StackPointer => {
                encode_store_x_to_x(
                    STACK_SCRATCH_REGISTER,
                    CONTEXT_REGISTER,
                    slot.context_offset,
                )?;
            }
        }
    }
    Ok(())
}

fn validate_saved_register(register: MachineRegister) -> Result<(), Diagnostic> {
    match register {
        MachineRegister::Aarch64X(index)
            if [RETRY_REGISTER, STACK_SCRATCH_REGISTER, CONTEXT_REGISTER].contains(&index) =>
        {
            Err(Diagnostic::error(format!(
                "AArch64 native fuel activation state cannot save reserved register {register:?}"
            )))
        }
        MachineRegister::Aarch64X(0..=30) | MachineRegister::Aarch64V(0..=31) => Ok(()),
        MachineRegister::Aarch64X(index) => Err(Diagnostic::error(format!(
            "AArch64 native fuel activation X{index} must use the distinct stack-pointer slot or is out of range"
        ))),
        MachineRegister::Aarch64V(index) => Err(Diagnostic::error(format!(
            "AArch64 native fuel runtime cannot encode V{index}"
        ))),
        _ => Err(Diagnostic::error(
            "AArch64 native fuel runtime rejects a foreign-architecture activation register",
        )),
    }
}

fn append_register_store(
    bytes: &mut Vec<u8>,
    register: MachineRegister,
    offset: u32,
) -> Result<(), Diagnostic> {
    match register {
        MachineRegister::Aarch64X(index) => {
            bytes.extend(encode_store_x_to_x(index, CONTEXT_REGISTER, offset)?);
        }
        MachineRegister::Aarch64V(index) => {
            bytes.extend(encode_store_q_to_x(index, CONTEXT_REGISTER, offset)?);
        }
        _ => {
            return Err(Diagnostic::error(
                "AArch64 native fuel runtime rejects a foreign-architecture activation register",
            ));
        }
    }
    Ok(())
}

fn append_register_load(
    bytes: &mut Vec<u8>,
    register: MachineRegister,
    offset: u32,
) -> Result<(), Diagnostic> {
    match register {
        MachineRegister::Aarch64X(index) => {
            bytes.extend(encode_load_x_from_x(index, CONTEXT_REGISTER, offset)?);
        }
        MachineRegister::Aarch64V(index) => {
            bytes.extend(encode_load_q_from_x(index, CONTEXT_REGISTER, offset)?);
        }
        _ => {
            return Err(Diagnostic::error(
                "AArch64 native fuel runtime rejects a foreign-architecture activation register",
            ));
        }
    }
    Ok(())
}

fn encode_store_x_to_x(source: u8, base: u8, byte_offset: u32) -> Result<[u8; 4], Diagnostic> {
    encode_scaled_memory(0xf900_0000, source, base, byte_offset, 8, "store X")
}

fn encode_load_x_from_x(
    destination: u8,
    base: u8,
    byte_offset: u32,
) -> Result<[u8; 4], Diagnostic> {
    encode_scaled_memory(0xf940_0000, destination, base, byte_offset, 8, "load X")
}

fn encode_store_q_to_x(source: u8, base: u8, byte_offset: u32) -> Result<[u8; 4], Diagnostic> {
    encode_scaled_memory(0x3d80_0000, source, base, byte_offset, 16, "store Q")
}

fn encode_load_q_from_x(
    destination: u8,
    base: u8,
    byte_offset: u32,
) -> Result<[u8; 4], Diagnostic> {
    encode_scaled_memory(0x3dc0_0000, destination, base, byte_offset, 16, "load Q")
}

fn encode_scaled_memory(
    opcode: u32,
    register: u8,
    base: u8,
    byte_offset: u32,
    scale: u32,
    operation: &str,
) -> Result<[u8; 4], Diagnostic> {
    if register > 31
        || base > 31
        || !byte_offset.is_multiple_of(scale)
        || byte_offset / scale > 4095
    {
        return Err(Diagnostic::error(format!(
            "AArch64 native fuel runtime cannot {operation} at context offset {byte_offset}"
        )));
    }
    Ok(
        (opcode | ((byte_offset / scale) << 10) | (u32::from(base) << 5) | u32::from(register))
            .to_le_bytes(),
    )
}

fn encode_store_x_to_sp(source: u8, byte_offset: u32) -> Result<[u8; 4], Diagnostic> {
    encode_store_x_to_x(source, 31, byte_offset)
}

fn encode_load_x_from_sp(destination: u8, byte_offset: u32) -> Result<[u8; 4], Diagnostic> {
    encode_load_x_from_x(destination, 31, byte_offset)
}

fn encode_read_nzcv(destination: u8) -> [u8; 4] {
    (0xd53b_4200_u32 | u32::from(destination)).to_le_bytes()
}

fn encode_write_nzcv(source: u8) -> [u8; 4] {
    (0xd51b_4200_u32 | u32::from(source)).to_le_bytes()
}

fn encode_move_x_from_sp(destination: u8) -> [u8; 4] {
    (0x9100_03e0_u32 | u32::from(destination)).to_le_bytes()
}

fn encode_move_sp_from_x(source: u8) -> [u8; 4] {
    (0x9100_001f_u32 | (u32::from(source) << 5)).to_le_bytes()
}

fn encode_sub_sp_immediate(value: u16) -> [u8; 4] {
    (0xd100_03ff_u32 | (u32::from(value) << 10)).to_le_bytes()
}

fn encode_add_sp_immediate(value: u16) -> [u8; 4] {
    (0x9100_03ff_u32 | (u32::from(value) << 10)).to_le_bytes()
}

fn encode_add_x_immediate(destination: u8, source: u8, value: u16) -> Result<[u8; 4], Diagnostic> {
    if destination > 30 || source > 30 || value > 4095 {
        return Err(Diagnostic::error(
            "AArch64 native fuel ADD immediate is not encodable",
        ));
    }
    Ok((0x9100_0000_u32
        | (u32::from(value) << 10)
        | (u32::from(source) << 5)
        | u32::from(destination))
    .to_le_bytes())
}

fn encode_add_x_register(destination: u8, left: u8, right: u8) -> [u8; 4] {
    (0x8b00_0000_u32 | (u32::from(right) << 16) | (u32::from(left) << 5) | u32::from(destination))
        .to_le_bytes()
}

fn encode_branch_register(register: u8) -> [u8; 4] {
    (0xd61f_0000_u32 | (u32::from(register) << 5)).to_le_bytes()
}

#[cfg(test)]
mod tests;
