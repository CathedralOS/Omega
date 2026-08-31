//! Canonical transport for the complete call plan retained by fixed-integer
//! scalar ABIs and attached-Unit scalar call custody.

use omega_calling_conventions::{
    CallPlan, CallingPolicy, EntryControl, MachineRegister, RegisterSet,
};

use super::{
    InstallationError, Reader, push_u16, push_u32,
    value_placement_codec::{decode_direct_placement, encode_direct_placement},
};

pub(super) fn encode_scalar_call_plan(
    bytes: &mut Vec<u8>,
    plan: &CallPlan,
) -> Result<(), InstallationError> {
    bytes.push(match plan.policy {
        CallingPolicy::MicrosoftX64 => 1,
        CallingPolicy::SystemVAMD64 => 2,
        CallingPolicy::Aapcs64 => 3,
        CallingPolicy::LinuxSyscallX86_64 => 4,
        CallingPolicy::LinuxSyscallAarch64 => 5,
    });
    bytes.push(0);
    push_u16(bytes, plan.stack_alignment);
    push_u16(bytes, plan.shadow_bytes);
    push_u16(bytes, 0);
    push_u32(
        bytes,
        u32::try_from(plan.parameters.len())
            .map_err(|_| InstallationError::TooManyScalarCallPlanValues)?,
    );
    for placement in &plan.parameters {
        encode_direct_placement(bytes, placement)?;
    }
    match &plan.result {
        Some(result) => {
            bytes.extend_from_slice(&[1, 0, 0, 0]);
            encode_direct_placement(bytes, result)?;
        }
        None => bytes.extend_from_slice(&[0; 4]),
    }
    // Fixed-integer scalar calls cannot carry callback materializations. The
    // explicit zero remains part of the canonical format so a future format
    // change cannot silently reinterpret this field.
    if !plan.callback_materializations.is_empty() {
        return Err(InstallationError::UnsupportedScalarCallPlan);
    }
    push_u32(bytes, 0);
    push_u32(
        bytes,
        u32::try_from(plan.ordinary_clobbers.as_slice().len())
            .map_err(|_| InstallationError::TooManyScalarCallPlanRegisters)?,
    );
    for register in plan.ordinary_clobbers.as_slice() {
        encode_machine_register(bytes, *register);
    }
    match plan.entry_control {
        EntryControl::CallReturn => bytes.extend_from_slice(&[1, 0, 0, 0]),
        EntryControl::SupervisorCall {
            number_register,
            immediate,
        } => {
            bytes.extend_from_slice(&[2, 0]);
            push_u16(bytes, immediate);
            encode_machine_register(bytes, number_register);
        }
        EntryControl::InterruptReturn => bytes.extend_from_slice(&[3, 0, 0, 0]),
    }
    Ok(())
}

pub(super) fn decode_scalar_call_plan(
    reader: &mut Reader<'_>,
) -> Result<CallPlan, InstallationError> {
    let policy = match reader.u8()? {
        1 => CallingPolicy::MicrosoftX64,
        2 => CallingPolicy::SystemVAMD64,
        3 => CallingPolicy::Aapcs64,
        4 => CallingPolicy::LinuxSyscallX86_64,
        5 => CallingPolicy::LinuxSyscallAarch64,
        tag => return Err(InstallationError::InvalidScalarCallingPolicyTag(tag)),
    };
    if reader.u8()? != 0 {
        return Err(InstallationError::NonzeroReservedField);
    }
    let stack_alignment = reader.u16()?;
    let shadow_bytes = reader.u16()?;
    if reader.u16()? != 0 {
        return Err(InstallationError::NonzeroReservedField);
    }
    let parameter_count = usize::try_from(reader.u32()?)
        .map_err(|_| InstallationError::TooManyScalarCallPlanValues)?;
    if parameter_count > reader.remaining() / 12 {
        return Err(InstallationError::UnexpectedEnd);
    }
    let mut parameters = Vec::with_capacity(parameter_count);
    for _ in 0..parameter_count {
        parameters.push(decode_direct_placement(reader)?);
    }
    let result = match reader.u8()? {
        0 => {
            if reader.take(3)? != [0; 3] {
                return Err(InstallationError::NonzeroReservedField);
            }
            None
        }
        1 => {
            if reader.take(3)? != [0; 3] {
                return Err(InstallationError::NonzeroReservedField);
            }
            Some(decode_direct_placement(reader)?)
        }
        tag => return Err(InstallationError::InvalidPresenceFlag(tag)),
    };
    if reader.u32()? != 0 {
        return Err(InstallationError::UnsupportedScalarCallPlan);
    }
    let clobber_count = usize::try_from(reader.u32()?)
        .map_err(|_| InstallationError::TooManyScalarCallPlanRegisters)?;
    if clobber_count > reader.remaining() / 2 {
        return Err(InstallationError::UnexpectedEnd);
    }
    let mut clobbers = Vec::with_capacity(clobber_count);
    for _ in 0..clobber_count {
        clobbers.push(decode_machine_register(reader)?);
    }
    let entry_tag = reader.u8()?;
    if reader.u8()? != 0 {
        return Err(InstallationError::NonzeroReservedField);
    }
    let entry_detail = reader.u16()?;
    let entry_control = match entry_tag {
        1 if entry_detail == 0 => EntryControl::CallReturn,
        2 => EntryControl::SupervisorCall {
            number_register: decode_machine_register(reader)?,
            immediate: entry_detail,
        },
        3 if entry_detail == 0 => EntryControl::InterruptReturn,
        1 | 3 => return Err(InstallationError::NonzeroReservedField),
        tag => return Err(InstallationError::InvalidScalarEntryControlTag(tag)),
    };
    Ok(CallPlan {
        policy,
        parameters,
        result,
        callback_materializations: Vec::new(),
        ordinary_clobbers: RegisterSet::new(clobbers),
        stack_alignment,
        shadow_bytes,
        entry_control,
    })
}

fn encode_machine_register(bytes: &mut Vec<u8>, register: MachineRegister) {
    let (class, index) = match register {
        MachineRegister::X86Rax => (1, 0),
        MachineRegister::X86Rcx => (1, 1),
        MachineRegister::X86Rdx => (1, 2),
        MachineRegister::X86Rbx => (1, 3),
        MachineRegister::X86Rsp => (1, 4),
        MachineRegister::X86Rbp => (1, 5),
        MachineRegister::X86Rsi => (1, 6),
        MachineRegister::X86Rdi => (1, 7),
        MachineRegister::X86R8 => (1, 8),
        MachineRegister::X86R9 => (1, 9),
        MachineRegister::X86R10 => (1, 10),
        MachineRegister::X86R11 => (1, 11),
        MachineRegister::X86R12 => (1, 12),
        MachineRegister::X86R13 => (1, 13),
        MachineRegister::X86R14 => (1, 14),
        MachineRegister::X86R15 => (1, 15),
        MachineRegister::X86Xmm(index) => (2, index),
        MachineRegister::Aarch64X(index) => (3, index),
        MachineRegister::Aarch64V(index) => (4, index),
    };
    bytes.extend_from_slice(&[class, index]);
}

fn decode_machine_register(reader: &mut Reader<'_>) -> Result<MachineRegister, InstallationError> {
    let class = reader.u8()?;
    let index = reader.u8()?;
    match (class, index) {
        (1, 0) => Ok(MachineRegister::X86Rax),
        (1, 1) => Ok(MachineRegister::X86Rcx),
        (1, 2) => Ok(MachineRegister::X86Rdx),
        (1, 3) => Ok(MachineRegister::X86Rbx),
        (1, 4) => Ok(MachineRegister::X86Rsp),
        (1, 5) => Ok(MachineRegister::X86Rbp),
        (1, 6) => Ok(MachineRegister::X86Rsi),
        (1, 7) => Ok(MachineRegister::X86Rdi),
        (1, 8) => Ok(MachineRegister::X86R8),
        (1, 9) => Ok(MachineRegister::X86R9),
        (1, 10) => Ok(MachineRegister::X86R10),
        (1, 11) => Ok(MachineRegister::X86R11),
        (1, 12) => Ok(MachineRegister::X86R12),
        (1, 13) => Ok(MachineRegister::X86R13),
        (1, 14) => Ok(MachineRegister::X86R14),
        (1, 15) => Ok(MachineRegister::X86R15),
        (2, index) => Ok(MachineRegister::X86Xmm(index)),
        (3, index) => Ok(MachineRegister::Aarch64X(index)),
        (4, index) => Ok(MachineRegister::Aarch64V(index)),
        _ => Err(InstallationError::InvalidScalarCallPlanRegister { class, index }),
    }
}
