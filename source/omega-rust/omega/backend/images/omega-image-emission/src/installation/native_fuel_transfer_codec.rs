//! Canonical transfer-runtime evidence codec.
//!
//! The native-fuel section owns semantic-to-metered coordinates. This
//! independent section owns only the compiler/runtime transfer extension,
//! including both relocation sides and its physical resource evidence.

use omega_calling_conventions::{
    MachineRegister, MachineState, MachineStateSet, RegisterSet, StateFootprintEvidence,
};
use omega_installation_evidence::{
    NativeFuelActivationStateSlot, NativeFuelRuntimeEntryIdentity, NativeFuelRuntimeTextEvidence,
    NativeFuelRuntimeTextSpan, NativeFuelSavedValue, NativeFuelSponsorStackPlan,
    NativeFuelTransferRuntimeEvidence, NativeFuelTransferRuntimePlanProjection,
};

use super::native_fuel_codec::{decode_profile, decode_transport, profile_tag, transport_tag};
use super::{
    InstallationError, InstalledNativeFuelTransferRuntime, NativeFuelTransferTextFingerprint,
    Reader, decode_boolean, push_u16, push_u32, push_u64,
};

pub(super) fn encode_native_fuel_transfer(
    bytes: &mut Vec<u8>,
    installed: Option<&InstalledNativeFuelTransferRuntime>,
) -> Result<(), InstallationError> {
    bytes.push(u8::from(installed.is_some()));
    bytes.extend_from_slice(&[0; 3]);
    let Some(installed) = installed else {
        return Ok(());
    };

    bytes.extend_from_slice(installed.unrelocated_text_fingerprint.as_bytes());
    push_u64(bytes, encode_offset(installed.unrelocated_text_byte_count)?);
    bytes.extend_from_slice(installed.final_text_fingerprint.as_bytes());
    push_u64(bytes, encode_offset(installed.final_text_byte_count)?);
    push_u64(bytes, encode_offset(installed.sponsor_text_offset)?);

    let evidence = &installed.evidence;
    let plan = evidence.plan();
    bytes.push(profile_tag(plan.profile()));
    bytes.push(transport_tag(plan.transport()));
    push_u16(bytes, 0);
    let context = plan.context();
    for value in [
        context.byte_size,
        context.alignment,
        context.remaining_units_offset,
        context.unpaid_site_kind_offset,
        context.unpaid_site_identity_offset,
        context.required_units_offset,
        context.transfer_entry_offset,
        context.retry_code_offset_offset,
        context.sponsor_stack_top_offset,
        context.activation_state_offset,
        context.activation_state_byte_count,
    ] {
        push_u32(bytes, value);
    }
    push_u32(
        bytes,
        u32::try_from(plan.activation_state_slots().len())
            .map_err(|_| InstallationError::TooManyNativeFuelTransferStateSlots)?,
    );
    for slot in plan.activation_state_slots() {
        encode_saved_value(bytes, slot.value)?;
        push_u16(bytes, 0);
        push_u32(bytes, slot.context_offset);
        push_u32(bytes, slot.byte_count);
    }
    push_u32(bytes, plan.sponsor_stack().alignment);
    push_u32(bytes, 0);
    push_u64(bytes, plan.sponsor_stack().byte_ceiling);
    push_u16(bytes, plan.interrupted_state().bits());
    push_u16(bytes, plan.saved_state().bits());
    push_u16(bytes, plan.restored_state().bits());
    push_u16(bytes, 0);
    encode_entry(bytes, plan.transfer_entry());
    encode_entry(bytes, plan.resume_entry());
    push_u64(bytes, plan.normalized_identity());

    encode_text_evidence(bytes, evidence.transfer_text())?;
    encode_text_evidence(bytes, evidence.resume_text())?;
    let footprint = evidence.physical_state_footprint();
    push_u32(
        bytes,
        u32::try_from(footprint.registers().as_slice().len())
            .map_err(|_| InstallationError::TooManyNativeFuelTransferRegisters)?,
    );
    for register in footprint.registers().as_slice() {
        encode_register(bytes, *register)?;
    }
    push_u16(bytes, footprint.machine_state().bits());
    push_u16(bytes, 0);
    push_u64(bytes, evidence.sponsor_stack_peak_bytes());
    push_u64(bytes, evidence.fingerprint());
    Ok(())
}

pub(super) fn decode_native_fuel_transfer(
    reader: &mut Reader<'_>,
    target: omega_target::NativeTarget,
) -> Result<Option<InstalledNativeFuelTransferRuntime>, InstallationError> {
    let present = decode_boolean(reader.u8()?)?;
    if reader.take(3)? != [0; 3] {
        return Err(InstallationError::NonzeroReservedField);
    }
    if !present {
        return Ok(None);
    }

    let unrelocated_text_fingerprint = NativeFuelTransferTextFingerprint(reader.array()?);
    let unrelocated_text_byte_count = decode_offset(reader)?;
    let final_text_fingerprint = NativeFuelTransferTextFingerprint(reader.array()?);
    let final_text_byte_count = decode_offset(reader)?;
    let sponsor_text_offset = decode_offset(reader)?;

    let profile = decode_profile(reader.u8()?)?;
    let transport = decode_transport(reader.u8()?)?;
    if reader.u16()? != 0 {
        return Err(InstallationError::NonzeroReservedField);
    }
    let context = omega_installation_evidence::NativeFuelContextLayout {
        byte_size: reader.u32()?,
        alignment: reader.u32()?,
        remaining_units_offset: reader.u32()?,
        unpaid_site_kind_offset: reader.u32()?,
        unpaid_site_identity_offset: reader.u32()?,
        required_units_offset: reader.u32()?,
        transfer_entry_offset: reader.u32()?,
        retry_code_offset_offset: reader.u32()?,
        sponsor_stack_top_offset: reader.u32()?,
        activation_state_offset: reader.u32()?,
        activation_state_byte_count: reader.u32()?,
    };
    let slot_count = usize::try_from(reader.u32()?)
        .map_err(|_| InstallationError::TooManyNativeFuelTransferStateSlots)?;
    if slot_count > reader.remaining() / 12 {
        return Err(InstallationError::UnexpectedEnd);
    }
    let mut activation_state_slots = Vec::with_capacity(slot_count);
    for _ in 0..slot_count {
        let value = decode_saved_value(reader)?;
        if reader.u16()? != 0 {
            return Err(InstallationError::NonzeroReservedField);
        }
        activation_state_slots.push(NativeFuelActivationStateSlot {
            value,
            context_offset: reader.u32()?,
            byte_count: reader.u32()?,
        });
    }
    let sponsor_alignment = reader.u32()?;
    if reader.u32()? != 0 {
        return Err(InstallationError::NonzeroReservedField);
    }
    let sponsor_stack = NativeFuelSponsorStackPlan {
        alignment: sponsor_alignment,
        byte_ceiling: reader.u64()?,
    };
    let interrupted_state = decode_machine_state_set(reader.u16()?)?;
    let saved_state = decode_machine_state_set(reader.u16()?)?;
    let restored_state = decode_machine_state_set(reader.u16()?)?;
    if reader.u16()? != 0 {
        return Err(InstallationError::NonzeroReservedField);
    }
    let transfer_entry = decode_entry(reader)?;
    let resume_entry = decode_entry(reader)?;
    let encoded_plan_identity = reader.u64()?;
    let plan = NativeFuelTransferRuntimePlanProjection::new(
        profile,
        target,
        transport,
        context,
        activation_state_slots,
        sponsor_stack,
        interrupted_state,
        saved_state,
        restored_state,
        transfer_entry,
        resume_entry,
    )
    .map_err(|_| InstallationError::InvalidNativeFuelTransferPlan)?;
    if plan.normalized_identity() != encoded_plan_identity {
        return Err(InstallationError::InvalidNativeFuelTransferPlan);
    }

    let transfer_text = decode_text_evidence(reader)?;
    let resume_text = decode_text_evidence(reader)?;
    let register_count = usize::try_from(reader.u32()?)
        .map_err(|_| InstallationError::TooManyNativeFuelTransferRegisters)?;
    if register_count > reader.remaining() / 2 {
        return Err(InstallationError::UnexpectedEnd);
    }
    let mut registers = Vec::with_capacity(register_count);
    for _ in 0..register_count {
        registers.push(decode_register(reader)?);
    }
    let footprint_state = decode_machine_state_set(reader.u16()?)?;
    if reader.u16()? != 0 {
        return Err(InstallationError::NonzeroReservedField);
    }
    let sponsor_stack_peak_bytes = reader.u64()?;
    let encoded_evidence_fingerprint = reader.u64()?;
    let evidence = NativeFuelTransferRuntimeEvidence::new(
        plan,
        transfer_text,
        resume_text,
        StateFootprintEvidence::new(RegisterSet::new(registers), footprint_state),
        sponsor_stack_peak_bytes,
    )
    .map_err(|_| InstallationError::InvalidNativeFuelTransferEvidence)?;
    if evidence.physical_state_footprint().machine_state() != footprint_state
        || evidence.fingerprint() != encoded_evidence_fingerprint
    {
        return Err(InstallationError::InvalidNativeFuelTransferEvidence);
    }

    Ok(Some(InstalledNativeFuelTransferRuntime {
        unrelocated_text_fingerprint,
        unrelocated_text_byte_count,
        final_text_fingerprint,
        final_text_byte_count,
        sponsor_text_offset,
        evidence,
    }))
}

fn encode_text_evidence(
    bytes: &mut Vec<u8>,
    evidence: &NativeFuelRuntimeTextEvidence,
) -> Result<(), InstallationError> {
    encode_entry(bytes, evidence.entry());
    push_u64(bytes, encode_offset(evidence.span().text_offset)?);
    push_u64(bytes, encode_offset(evidence.span().byte_count)?);
    push_u32(
        bytes,
        u32::try_from(evidence.unrelocated_bytes().len())
            .map_err(|_| InstallationError::NativeFuelTransferBytesTooLong)?,
    );
    bytes.extend_from_slice(evidence.unrelocated_bytes());
    push_u32(
        bytes,
        u32::try_from(evidence.final_bytes().len())
            .map_err(|_| InstallationError::NativeFuelTransferBytesTooLong)?,
    );
    bytes.extend_from_slice(evidence.final_bytes());
    Ok(())
}

fn decode_text_evidence(
    reader: &mut Reader<'_>,
) -> Result<NativeFuelRuntimeTextEvidence, InstallationError> {
    let entry = decode_entry(reader)?;
    let span = NativeFuelRuntimeTextSpan {
        text_offset: decode_offset(reader)?,
        byte_count: decode_offset(reader)?,
    };
    let unrelocated_len = usize::try_from(reader.u32()?)
        .map_err(|_| InstallationError::NativeFuelTransferBytesTooLong)?;
    let unrelocated_bytes = reader.take(unrelocated_len)?.to_vec();
    let final_len = usize::try_from(reader.u32()?)
        .map_err(|_| InstallationError::NativeFuelTransferBytesTooLong)?;
    let final_bytes = reader.take(final_len)?.to_vec();
    NativeFuelRuntimeTextEvidence::new(entry, span, unrelocated_bytes, final_bytes)
        .map_err(|_| InstallationError::InvalidNativeFuelTransferEvidence)
}

fn encode_entry(bytes: &mut Vec<u8>, entry: NativeFuelRuntimeEntryIdentity) {
    push_u64(bytes, entry.section_identity);
    push_u64(bytes, entry.symbol_identity);
}

fn decode_entry(
    reader: &mut Reader<'_>,
) -> Result<NativeFuelRuntimeEntryIdentity, InstallationError> {
    Ok(NativeFuelRuntimeEntryIdentity {
        section_identity: reader.u64()?,
        symbol_identity: reader.u64()?,
    })
}

fn encode_saved_value(
    bytes: &mut Vec<u8>,
    value: NativeFuelSavedValue,
) -> Result<(), InstallationError> {
    match value {
        NativeFuelSavedValue::Register(register) => {
            bytes.push(0);
            encode_register(bytes, register)?;
        }
        NativeFuelSavedValue::Flags => bytes.extend_from_slice(&[1, 0, 0]),
        NativeFuelSavedValue::StackPointer => bytes.extend_from_slice(&[2, 0, 0]),
    }
    Ok(())
}

fn decode_saved_value(reader: &mut Reader<'_>) -> Result<NativeFuelSavedValue, InstallationError> {
    match reader.u8()? {
        0 => Ok(NativeFuelSavedValue::Register(decode_register(reader)?)),
        1 => {
            if reader.u16()? != 0 {
                return Err(InstallationError::NonzeroReservedField);
            }
            Ok(NativeFuelSavedValue::Flags)
        }
        2 => {
            if reader.u16()? != 0 {
                return Err(InstallationError::NonzeroReservedField);
            }
            Ok(NativeFuelSavedValue::StackPointer)
        }
        tag => Err(InstallationError::InvalidNativeFuelSavedValueTag(tag)),
    }
}

fn encode_register(
    bytes: &mut Vec<u8>,
    register: MachineRegister,
) -> Result<(), InstallationError> {
    let (class, index) = match register {
        MachineRegister::X86Rax => (0, 0),
        MachineRegister::X86Rcx => (0, 1),
        MachineRegister::X86Rdx => (0, 2),
        MachineRegister::X86Rbx => (0, 3),
        MachineRegister::X86Rsp => (0, 4),
        MachineRegister::X86Rbp => (0, 5),
        MachineRegister::X86Rsi => (0, 6),
        MachineRegister::X86Rdi => (0, 7),
        MachineRegister::X86R8 => (0, 8),
        MachineRegister::X86R9 => (0, 9),
        MachineRegister::X86R10 => (0, 10),
        MachineRegister::X86R11 => (0, 11),
        MachineRegister::X86R12 => (0, 12),
        MachineRegister::X86R13 => (0, 13),
        MachineRegister::X86R14 => (0, 14),
        MachineRegister::X86R15 => (0, 15),
        MachineRegister::X86Xmm(index) if index <= 31 => (1, index),
        MachineRegister::Aarch64X(index) if index <= 30 => (2, index),
        MachineRegister::Aarch64V(index) if index <= 31 => (3, index),
        _ => return Err(InstallationError::InvalidNativeFuelTransferRegister),
    };
    bytes.extend_from_slice(&[class, index]);
    Ok(())
}

fn decode_register(reader: &mut Reader<'_>) -> Result<MachineRegister, InstallationError> {
    let class = reader.u8()?;
    let index = reader.u8()?;
    let register = match class {
        0 => match index {
            0 => MachineRegister::X86Rax,
            1 => MachineRegister::X86Rcx,
            2 => MachineRegister::X86Rdx,
            3 => MachineRegister::X86Rbx,
            4 => MachineRegister::X86Rsp,
            5 => MachineRegister::X86Rbp,
            6 => MachineRegister::X86Rsi,
            7 => MachineRegister::X86Rdi,
            8 => MachineRegister::X86R8,
            9 => MachineRegister::X86R9,
            10 => MachineRegister::X86R10,
            11 => MachineRegister::X86R11,
            12 => MachineRegister::X86R12,
            13 => MachineRegister::X86R13,
            14 => MachineRegister::X86R14,
            15 => MachineRegister::X86R15,
            _ => return Err(InstallationError::InvalidNativeFuelTransferRegister),
        },
        1 if index <= 31 => MachineRegister::X86Xmm(index),
        2 if index <= 30 => MachineRegister::Aarch64X(index),
        3 if index <= 31 => MachineRegister::Aarch64V(index),
        _ => return Err(InstallationError::InvalidNativeFuelTransferRegister),
    };
    Ok(register)
}

fn decode_machine_state_set(bits: u16) -> Result<MachineStateSet, InstallationError> {
    if bits & !0x01ff != 0 {
        return Err(InstallationError::InvalidNativeFuelTransferMachineState);
    }
    let states = [
        MachineState::GeneralRegisters,
        MachineState::VectorRegisters,
        MachineState::Flags,
        MachineState::InstructionPointer,
        MachineState::StackPointer,
        MachineState::SegmentState,
        MachineState::ControlState,
        MachineState::DebugState,
        MachineState::ExtendedState,
    ];
    Ok(MachineStateSet::new(
        states
            .into_iter()
            .filter(|state| bits & (1 << *state as u8) != 0),
    ))
}

fn encode_offset(value: usize) -> Result<u64, InstallationError> {
    u64::try_from(value).map_err(|_| InstallationError::NativeFuelOffsetNotRepresentable)
}

fn decode_offset(reader: &mut Reader<'_>) -> Result<usize, InstallationError> {
    usize::try_from(reader.u64()?).map_err(|_| InstallationError::NativeFuelOffsetNotRepresentable)
}
