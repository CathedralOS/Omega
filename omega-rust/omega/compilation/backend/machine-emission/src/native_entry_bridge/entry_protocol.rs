//! Choose the entry's stack and restoration rules before moving its arguments.
//!
//! An interrupt adapter receives a normalized x86-64 return frame. Its native
//! argument area follows RIP, CS, RFLAGS, RSP and SS; those five words are never
//! semantic arguments. Installation must establish the argument values and their
//! qualifications separately. This transport creates neither an installed root
//! nor an interrupt acknowledgement.

use calling_conventions::{
    EntryControl, MachineRegister, MachineState, MachineStateSet, RegisterSet,
    StateFootprintEvidence,
};
use target::Architecture;

use super::{Error, NativeEntryBridgeRequest};

pub(super) struct EntryProtocol {
    pub incoming_argument_bias: u32,
    pub frame_alignment_padding: u32,
    pub body_clobbers: RegisterSet,
    pub preserved_registers: RegisterSet,
    pub before_body: &'static [u8],
    pub return_bytes: &'static [u8],
}

pub(super) fn select(request: &NativeEntryBridgeRequest) -> Result<EntryProtocol, Error> {
    match (
        request.target.architecture,
        request.inbound.call.entry_control,
    ) {
        (architecture, EntryControl::CallReturn) => Ok(EntryProtocol {
            incoming_argument_bias: super::call_stack_bias(architecture),
            frame_alignment_padding: super::call_stack_bias(architecture),
            body_clobbers: request.internal.ordinary_clobbers.clone(),
            preserved_registers: RegisterSet::default(),
            before_body: &[],
            return_bytes: match architecture {
                Architecture::X86_64 => &isa_x86_64::ORDINARY_RETURN_BYTES,
                Architecture::Aarch64 => &isa_aarch64::ORDINARY_RETURN_BYTES,
            },
        }),
        (Architecture::X86_64, EntryControl::InterruptReturn) => interrupt(request),
        _ => Err(Error::UnsupportedEntryControl),
    }
}

fn interrupt(request: &NativeEntryBridgeRequest) -> Result<EntryProtocol, Error> {
    let state = &request.inbound.state;
    let restored = MachineStateSet::new([
        MachineState::GeneralRegisters,
        MachineState::Flags,
        MachineState::InstructionPointer,
        MachineState::StackPointer,
    ]);
    // The CPU return frame restores flags, IP and SP. This adapter saves all
    // integer registers. Other state must remain untouched by the complete
    // handler closure; additional save protocols need their own realization.
    if state.initial_regime.architecture() != Architecture::X86_64
        || state.saved_state != restored
        || state.restored_state != restored
        || !state.interrupted_state.contains_all(restored)
        || !state.permitted_transitive_use.contains_all(restored)
        || request.inbound.call.stack_alignment != 16
        || request.inbound.call.result.is_some()
    {
        return Err(Error::UnsupportedInterruptState);
    }
    let footprint = interrupt_body_footprint(request)?;
    if !restored.contains_all(footprint.machine_state()) {
        return Err(Error::InterruptBodyStateExceedsPlan);
    }
    Ok(EntryProtocol {
        incoming_argument_bias: 40,
        // Hardware aligns the stack before pushing five eight-byte words.
        // The remaining eight-byte residue must be removed before CALL.
        frame_alignment_padding: 8,
        body_clobbers: footprint.registers().clone(),
        preserved_registers: RegisterSet::new([
            MachineRegister::X86Rax,
            MachineRegister::X86Rbx,
            MachineRegister::X86Rcx,
            MachineRegister::X86Rdx,
            MachineRegister::X86Rsi,
            MachineRegister::X86Rdi,
            MachineRegister::X86Rbp,
            MachineRegister::X86R8,
            MachineRegister::X86R9,
            MachineRegister::X86R10,
            MachineRegister::X86R11,
            MachineRegister::X86R12,
            MachineRegister::X86R13,
            MachineRegister::X86R14,
            MachineRegister::X86R15,
        ]),
        // An interrupted program may have DF set. Ordinary compiled bodies
        // require it clear; IRETQ later restores the interrupted RFLAGS.
        before_body: &isa_x86_64::CLEAR_DIRECTION_FLAG_BYTES,
        return_bytes: &isa_x86_64::INTERRUPT_RETURN_BYTES,
    })
}

/// A fatal installation admits an exact body with no normal completion. It
/// consumes the arrival without resuming its interrupted activation, so its
/// bridge must not borrow the returning adapter's restoration promise.
pub(super) fn validate_fatal(request: &NativeEntryBridgeRequest) -> Result<(), Error> {
    let state = &request.inbound.state;
    if !state.interrupted_state.is_empty()
        || !state.saved_state.is_empty()
        || !state.restored_state.is_empty()
    {
        return Err(Error::UnsupportedFatalInterruptPreservation);
    }
    if state.initial_regime.architecture() != Architecture::X86_64
        || request.inbound.call.stack_alignment != 16
        || request.inbound.call.result.is_some()
    {
        return Err(Error::UnsupportedInterruptState);
    }
    interrupt_body_footprint(request)?;
    Ok(())
}

fn interrupt_body_footprint(
    request: &NativeEntryBridgeRequest,
) -> Result<&StateFootprintEvidence, Error> {
    let footprint = request
        .body_footprint
        .as_ref()
        .ok_or(Error::MissingInterruptBodyFootprint)?;
    if !request
        .inbound
        .state
        .permitted_transitive_use
        .contains_all(footprint.machine_state())
    {
        return Err(Error::InterruptBodyStateExceedsPlan);
    }
    Ok(footprint)
}
