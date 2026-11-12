/*
We emit physical native entry bridges that adapt an external calling or interrupt arrival plan to a function's unchanged internal calling plan.

Before any destination register is written, we snapshot incoming values into our bridge activation so nominal parameter permutations execute as parallel moves rather than sequential overwrites. Scalar loans construct address-and-codec descriptor storage in this activation and pass the descriptor address to the internal convention; owned indirect values copy their complete byte extent into our frame so the caller's ABI pointer never becomes a loan. Over-aligned owned homes reserve alignment padding inside the fixed ABI frame, and any internal clobbers excluded from the inbound promise—including all 128 bits of clobbered vector registers and borrowed scratch registers—are saved before redistribution and restored before return, except a register class the body's checked closure never overwrites. For returning and fatal installed interrupt entries, we validate the hardware return frame, machine-state ceiling, and active root/gate geometry before dispatching to the internal handler body.
*/

use arena::Arena;
#[cfg(test)]
use calling_conventions::StateFootprintEvidence;
use calling_conventions::{
    BoundaryEntryPlan, CallPlan, EntryControl, EntryStack, GeneratedEntryEvidence, MachineRegister,
    MachineState, MachineStateSet, Preemption, RegisterSet, StateEffectsEvidence, ValueClass,
    ValuePlacement, ValueShape,
};
use register_model::validate_physical_register_model;
pub use runtime_abi::static_interrupts::{InstalledInterruptEntryPlan, InstalledInterruptFrame};
use runtime_abi::static_interrupts::{InstalledInterruptProtocol, acknowledgement as ack, root};
use target::{Architecture, NativeTarget};

use crate::native_bridge_encoding::{
    AlignedHome, Encoding, NativeBridgeEncodingError, ValueStorage, WordLocation, allocate_home,
    allocate_value, allocate_word, is_borrowed, value_storage,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NativeEntryParameter {
    pub inbound_ordinal: u32,
    pub scalar_codec: u64, // Zero means no descriptor conversion; nonzero is a closed scalar codec.
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeEntryBridgeRequest {
    pub target: NativeTarget,
    pub internal: CallPlan,
    pub inbound: BoundaryEntryPlan, // Owns both argument placement and interrupted-state obligations.
    pub body_footprint: Option<StateEffectsEvidence>, // Checked body closure required for asynchronous entry ceilings.
    pub parameters: Arena<NativeEntryParameter>, // Internal scalar-then-structural order joined through nominal origins.
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeEntryBridge {
    request: NativeEntryBridgeRequest,
    bytes: Vec<u8>,
    frame_bytes: u32,
    call_field_offset: usize,
    installed_interrupt: Option<InstalledInterruptEntryPlan>,
    generated: GeneratedEntryEvidence,
    complete_effects: StateEffectsEvidence,
}

impl NativeEntryBridge {
    pub fn generated(&self) -> &GeneratedEntryEvidence {
        &self.generated
    }
    pub fn complete_effects(&self) -> &StateEffectsEvidence {
        &self.complete_effects
    }
    pub fn installed_interrupt(&self) -> Option<InstalledInterruptEntryPlan> {
        self.installed_interrupt
    }
    pub fn request(&self) -> &NativeEntryBridgeRequest {
        &self.request
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn frame_bytes(&self) -> u32 {
        self.frame_bytes
    }
    pub fn call_field_offset(&self) -> usize {
        self.call_field_offset
    }
    pub fn call_stack_bias(&self) -> u32 {
        call_stack_bias(self.request.target.architecture)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeEntryBridgeError {
    IncompatiblePlan,
    UnsupportedEntryControl,
    UnsupportedInstalledInterruptSignature,
    UnsupportedFatalInterruptSignature,
    UnsupportedFatalInterruptPreservation,
    InvalidInstalledInterruptGeometry,
    UnsupportedInterruptState,
    MissingInterruptBodyFootprint,
    InterruptBodyStateExceedsPlan,
    FatalInterruptBodyStateExceedsPlan,
    InvalidParameterJoin,
    UnsupportedValueTransport,
    MissingScalarCodec,
    NoUnoccupiedScratch,
    FrameOverflow,
    Encoding,
    ReplayMismatch,
    GeneratedEntryEffects(String),
}

impl std::fmt::Display for NativeEntryBridgeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let reason = match self {
            Self::GeneratedEntryEffects(reason) => {
                return write!(formatter, "native entry bridge failed: {reason}");
            }
            Self::UnsupportedInstalledInterruptSignature => {
                "installed timer arrival requires exactly one owned 40-byte acknowledgement and no result"
            }
            Self::UnsupportedFatalInterruptSignature => {
                "installed fatal arrival requires exactly one shared context reference and no result"
            }
            Self::UnsupportedFatalInterruptPreservation => {
                "nonreturning fatal entry cannot promise to preserve or restore an interrupted continuation"
            }
            Self::InvalidInstalledInterruptGeometry => {
                "installed interrupt entry changed its root, gate or hardware-frame geometry"
            }
            Self::UnsupportedEntryControl => "this target cannot emit the requested entry return",
            Self::UnsupportedInterruptState => {
                "interrupt entry requires the supported x86-64 integer-register save/restore plan and 16-byte stack alignment"
            }
            Self::MissingInterruptBodyFootprint => {
                "interrupt entry lacks the checked handler's machine-state effects"
            }
            Self::InterruptBodyStateExceedsPlan => {
                "interrupt handler uses machine state its entry does not permit, save and restore"
            }
            Self::FatalInterruptBodyStateExceedsPlan => {
                "fatal interrupt handler exceeds its entry's machine-state ceiling"
            }
            _ => return write!(formatter, "native entry bridge failed: {self:?}"),
        };
        write!(formatter, "native entry bridge failed: {reason}")
    }
}

impl std::error::Error for NativeEntryBridgeError {}
type Error = NativeEntryBridgeError;

impl NativeEntryBridgeRequest {
    pub fn checked_complete_effects(
        &self,
        adapter: &StateEffectsEvidence,
    ) -> Result<StateEffectsEvidence, Error> {
        let body = self
            .body_footprint
            .as_ref()
            .ok_or(Error::MissingInterruptBodyFootprint)?;
        let complete = adapter.compose(body);
        complete.validate(&self.inbound.state).map_err(|reason| {
            Error::GeneratedEntryEffects(format!(
                "{reason}: used {:?}, permitted {:?}, clobbered {:?}, saved {:?}",
                complete.machine_state(),
                self.inbound.state.permitted_transitive_use,
                complete.clobbered_state(),
                self.inbound.state.saved_state
            ))
        })?;
        Ok(complete)
    }
}

fn finish_native_entry(
    request: NativeEntryBridgeRequest,
    bytes: Vec<u8>,
    frame_bytes: u32,
    call_field_offset: usize,
    installed_interrupt: Option<InstalledInterruptEntryPlan>,
) -> Result<NativeEntryBridge, Error> {
    let call = Some(u32::try_from(call_field_offset).map_err(|_| Error::FrameOverflow)?);
    let generated = match request.target.architecture {
        Architecture::X86_64 => isa_x86_64::decode_x86_64_generated_entry(&bytes, call),
        Architecture::Aarch64 => isa_aarch64::decode_aarch64_generated_entry(&bytes, call),
    }
    .map_err(Error::GeneratedEntryEffects)?;
    let complete_effects = request.checked_complete_effects(&generated.effects)?;
    Ok(NativeEntryBridge {
        request,
        bytes,
        frame_bytes,
        call_field_offset,
        installed_interrupt,
        generated,
        complete_effects,
    })
}

impl From<NativeBridgeEncodingError> for NativeEntryBridgeError {
    fn from(error: NativeBridgeEncodingError) -> Self {
        match error {
            NativeBridgeEncodingError::UnsupportedValueTransport => Self::UnsupportedValueTransport,
            NativeBridgeEncodingError::FrameOverflow => Self::FrameOverflow,
            NativeBridgeEncodingError::Encoding => Self::Encoding,
        }
    }
}

struct ValueTransport {
    source: ValueStorage,
    destination: ValueStorage,
    byte_size: u32,
    alignment: u16,
    scalar_codec: u64,
}

struct ValueSnapshot {
    value_offset: u32,
    source_pointer_offset: Option<u32>,
    descriptor_offset: Option<u32>,
}

struct PreservedRegister {
    register: MachineRegister,
    offset: u32,
    byte_size: u8,
}

struct EntryProtocol {
    incoming_argument_bias: u32,
    frame_alignment_padding: u32,
    body_clobbers: RegisterSet,
    preserved_registers: RegisterSet,
    before_body: &'static [u8],
    return_bytes: &'static [u8],
}

const SAVED_GPRS: [u8; 15] = [0, 1, 2, 3, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];

fn select_entry_protocol(request: &NativeEntryBridgeRequest) -> Result<EntryProtocol, Error> {
    match (
        request.target.architecture,
        request.inbound.call.entry_control,
    ) {
        (architecture, EntryControl::CallReturn) => Ok(EntryProtocol {
            incoming_argument_bias: call_stack_bias(architecture),
            frame_alignment_padding: call_stack_bias(architecture),
            body_clobbers: checked_body_clobbers(request),
            preserved_registers: RegisterSet::default(),
            before_body: &[],
            return_bytes: match architecture {
                Architecture::X86_64 => &isa_x86_64::ORDINARY_RETURN_BYTES,
                Architecture::Aarch64 => &isa_aarch64::ORDINARY_RETURN_BYTES,
            },
        }),
        (Architecture::X86_64, EntryControl::InterruptReturn) => select_interrupt_protocol(request),
        _ => Err(Error::UnsupportedEntryControl),
    }
}

// The internal convention lists every register a body may overwrite, and the
// inbound promise may keep some of them. Saving all of those would make the
// bridge itself touch state the entry's ceiling excludes: a handler that never
// writes a vector register would still have each one spilled around it. The
// checked footprint of the body's whole synchronous closure says which register
// classes it overwrites, so a class it leaves alone needs no save. Without that
// evidence the convention's full list stands.
fn checked_body_clobbers(request: &NativeEntryBridgeRequest) -> RegisterSet {
    let convention = &request.internal.ordinary_clobbers;
    let Some(footprint) = request.body_footprint.as_ref() else {
        return convention.clone();
    };
    let overwritten = footprint.clobbered_state();
    RegisterSet::new(convention.as_slice().iter().copied().filter(|register| {
        let class = match register {
            MachineRegister::X86Xmm(_) | MachineRegister::Aarch64V(_) => {
                MachineState::VectorRegisters
            }
            _ => MachineState::GeneralRegisters,
        };
        overwritten.contains_all(MachineStateSet::new([class]))
    }))
}

fn select_interrupt_protocol(request: &NativeEntryBridgeRequest) -> Result<EntryProtocol, Error> {
    let state = &request.inbound.state;
    let restored = MachineStateSet::new([
        MachineState::GeneralRegisters,
        MachineState::Flags,
        MachineState::InstructionPointer,
        MachineState::StackPointer,
    ]);
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
    if !restored.contains_all(footprint.clobbered_state()) {
        return Err(Error::InterruptBodyStateExceedsPlan);
    }
    Ok(EntryProtocol {
        incoming_argument_bias: 40,
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
        before_body: &isa_x86_64::CLEAR_DIRECTION_FLAG_BYTES,
        return_bytes: &isa_x86_64::INTERRUPT_RETURN_BYTES,
    })
}

fn validate_fatal_entry_protocol(request: &NativeEntryBridgeRequest) -> Result<(), Error> {
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
    interrupt_body_footprint(request).map_err(|error| match error {
        Error::InterruptBodyStateExceedsPlan => Error::FatalInterruptBodyStateExceedsPlan,
        error => error,
    })?;
    Ok(())
}

fn interrupt_body_footprint(
    request: &NativeEntryBridgeRequest,
) -> Result<&StateEffectsEvidence, Error> {
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

fn call_stack_bias(architecture: Architecture) -> u32 {
    u32::from(architecture == Architecture::X86_64) * 8
}

fn value_transport(
    source: &ValuePlacement,
    destination: &ValuePlacement,
    scalar_codec: u64,
) -> Result<ValueTransport, Error> {
    let byte_size = match destination.shape.class {
        ValueClass::BorrowedReference
            if source.shape == ValueShape::integer(8, 8) && scalar_codec == 0 =>
        {
            8
        }
        ValueClass::BorrowedScalarReference
            if source.shape == ValueShape::integer(8, 8)
                && runtime_abi::ScalarStorageCodec::decode(scalar_codec).is_some() =>
        {
            8
        }
        ValueClass::BorrowedScalarReference => return Err(Error::MissingScalarCodec),
        _ if !is_borrowed(source.shape.class)
            && !is_borrowed(destination.shape.class)
            && source.shape.byte_size == destination.shape.byte_size
            && source.shape.alignment == destination.shape.alignment
            && scalar_codec == 0 =>
        {
            source.shape.byte_size
        }
        _ => return Err(Error::UnsupportedValueTransport),
    };
    Ok(ValueTransport {
        source: value_storage(source, byte_size)?,
        destination: value_storage(destination, byte_size)?,
        byte_size,
        alignment: if is_borrowed(destination.shape.class) {
            8
        } else {
            destination.shape.alignment
        },
        scalar_codec,
    })
}

pub fn emit_native_entry_bridge(
    request: NativeEntryBridgeRequest,
) -> Result<NativeEntryBridge, Error> {
    let protocol = select_entry_protocol(&request)?;
    let internal = &request.internal;
    let inbound = &request.inbound.call;
    if internal.policy.architecture() != request.target.architecture
        || inbound.policy.architecture() != request.target.architecture
        || internal.entry_control != EntryControl::CallReturn
        || internal.stack_alignment != 16
        || inbound.stack_alignment < 16
        || !inbound.stack_alignment.is_power_of_two()
        || !internal.callback_materializations.is_empty()
        || !inbound.callback_materializations.is_empty()
        || internal.parameters.len() != inbound.parameters.len()
        || request.parameters.len() != internal.parameters.len()
    {
        return Err(Error::IncompatiblePlan);
    }
    let mut transports = Vec::with_capacity(internal.parameters.len());
    let mut used_inbound = Vec::with_capacity(inbound.parameters.len());
    let mut cursor = u32::from(internal.shadow_bytes);
    let mut aligned_homes = Vec::new();
    for (destination, (_, parameter)) in internal.parameters.iter().zip(request.parameters.iter()) {
        let ordinal =
            usize::try_from(parameter.inbound_ordinal).map_err(|_| Error::InvalidParameterJoin)?;
        let source = inbound
            .parameters
            .get(ordinal)
            .ok_or(Error::InvalidParameterJoin)?;
        if used_inbound.contains(&ordinal) {
            return Err(Error::InvalidParameterJoin);
        }
        used_inbound.push(ordinal);
        let transport = value_transport(source, destination, parameter.scalar_codec)?;
        if let ValueStorage::Indirect {
            copy_offset: Some(offset),
            ..
        } = &transport.destination
        {
            if transport.alignment > 16 {
                if !offset.is_multiple_of(16) {
                    return Err(Error::UnsupportedValueTransport);
                }
                aligned_homes.push(AlignedHome {
                    offset: *offset,
                    byte_size: transport.byte_size,
                    alignment: transport.alignment,
                });
            }
            cursor = cursor.max(
                offset
                    .checked_add(transport.byte_size)
                    .and_then(|end| {
                        end.checked_add(u32::from(transport.alignment.saturating_sub(16)))
                    })
                    .ok_or(Error::FrameOverflow)?,
            );
        }
        for piece in transport.destination.pieces() {
            if let WordLocation::Stack(offset) = piece.location {
                cursor = cursor.max(
                    offset
                        .checked_add(u32::from(piece.byte_size))
                        .ok_or(Error::FrameOverflow)?,
                );
            }
        }
        transports.push(transport);
    }
    let result = match (&internal.result, &inbound.result) {
        (None, None) => None,
        (Some(source), Some(destination)) => {
            let transport = value_transport(source, destination, 0)?;
            if transport
                .source
                .pieces()
                .iter()
                .chain(transport.destination.pieces())
                .any(|piece| matches!(piece.location, WordLocation::Stack(_)))
            {
                return Err(Error::UnsupportedValueTransport);
            }
            Some(transport)
        }
        _ => return Err(Error::UnsupportedValueTransport),
    };
    let occupied = |register| {
        transports.iter().chain(result.iter()).any(|transport| {
            transport.source.pieces().iter().chain(transport.destination.pieces()).any(|piece|
                matches!(piece.location, WordLocation::Register(candidate) if candidate == register))
        })
    };
    let candidates: &[MachineRegister] = match request.target.architecture {
        Architecture::Aarch64 => &[
            MachineRegister::Aarch64X(16),
            MachineRegister::Aarch64X(17),
            MachineRegister::Aarch64X(9),
            MachineRegister::Aarch64X(10),
            MachineRegister::Aarch64X(11),
            MachineRegister::Aarch64X(12),
            MachineRegister::Aarch64X(13),
            MachineRegister::Aarch64X(14),
            MachineRegister::Aarch64X(15),
        ],
        Architecture::X86_64 => &[
            MachineRegister::X86R11,
            MachineRegister::X86R10,
            MachineRegister::X86Rax,
            MachineRegister::X86Rcx,
            MachineRegister::X86Rdx,
            MachineRegister::X86R8,
            MachineRegister::X86R9,
        ],
    };
    let volatile = |register| inbound.ordinary_clobbers.contains(register);
    let mut scratch = candidates
        .iter()
        .copied()
        .filter(|register| volatile(*register) && !occupied(*register))
        .chain(
            candidates
                .iter()
                .copied()
                .filter(|register| !volatile(*register) && !occupied(*register)),
        );
    let scratch_register = scratch.next().ok_or(Error::NoUnoccupiedScratch)?;
    let indirect = transports
        .iter()
        .chain(result.iter())
        .any(|transport| transport.source.is_indirect() || transport.destination.is_indirect());
    let address_scratch = if request.target.architecture == Architecture::Aarch64 || indirect {
        scratch.next().ok_or(Error::NoUnoccupiedScratch)?
    } else {
        scratch_register
    };
    let model = match request.target.architecture {
        Architecture::Aarch64 => {
            validate_physical_register_model(isa_aarch64::aarch64_physical_register_model())
        }
        Architecture::X86_64 => {
            validate_physical_register_model(isa_x86_64::x86_64_physical_register_model())
        }
    }
    .map_err(|_| Error::Encoding)?;
    let mut encoding = Encoding {
        architecture: request.target.architecture,
        model,
        scratch: scratch_register,
        address_scratch,
        aligned_homes,
    };
    let mut snapshots = Vec::with_capacity(transports.len());
    for transport in &transports {
        let alignment = if transport.source.is_indirect() || transport.destination.is_indirect() {
            transport.alignment.max(16)
        } else {
            8
        };
        let snapshot = allocate_home(
            &mut cursor,
            transport.byte_size,
            alignment,
            &mut encoding.aligned_homes,
        )?;
        let source_pointer = transport
            .source
            .is_indirect()
            .then(|| allocate_word(&mut cursor))
            .transpose()?;
        let descriptor = if transport.scalar_codec != 0 {
            let address = allocate_word(&mut cursor)?;
            allocate_word(&mut cursor)?;
            Some(address)
        } else {
            None
        };
        snapshots.push(ValueSnapshot {
            value_offset: snapshot,
            source_pointer_offset: source_pointer,
            descriptor_offset: descriptor,
        });
    }
    let result_snapshot = result
        .as_ref()
        .map(|transport| {
            let alignment = if transport.source.is_indirect() {
                transport.alignment.max(16)
            } else {
                8
            };
            allocate_home(
                &mut cursor,
                transport.byte_size,
                alignment,
                &mut encoding.aligned_homes,
            )
        })
        .transpose()?;
    let caller_result_pointer = result
        .as_ref()
        .is_some_and(|transport| transport.destination.is_indirect())
        .then(|| allocate_word(&mut cursor))
        .transpose()?;
    let continuation = if request.target.architecture == Architecture::Aarch64 {
        Some(allocate_word(&mut cursor)?)
    } else {
        None
    };
    let internal_writes = RegisterSet::new(
        protocol
            .body_clobbers
            .as_slice()
            .iter()
            .copied()
            .chain(protocol.preserved_registers.as_slice().iter().copied())
            .chain([scratch_register, address_scratch])
            .chain(
                transports
                    .iter()
                    .flat_map(|transport| transport.destination.pieces())
                    .chain(
                        result
                            .iter()
                            .flat_map(|transport| transport.source.pieces()),
                    )
                    .filter_map(|piece| match piece.location {
                        WordLocation::Register(register) => Some(register),
                        WordLocation::Stack(_) => None,
                    }),
            ),
    );
    if inbound.entry_control == EntryControl::InterruptReturn
        && internal_writes
            .as_slice()
            .iter()
            .copied()
            .chain(
                transports
                    .iter()
                    .flat_map(|transport| transport.source.pieces())
                    .filter_map(|piece| match piece.location {
                        WordLocation::Register(register) => Some(register),
                        WordLocation::Stack(_) => None,
                    }),
            )
            .any(|register| {
                matches!(
                    register,
                    MachineRegister::X86Xmm(_) | MachineRegister::Aarch64V(_)
                )
            })
    {
        return Err(Error::UnsupportedInterruptState);
    }
    let result_output = |register| match result.as_ref().map(|transport| &transport.destination) {
        Some(ValueStorage::Direct(pieces)) => pieces.iter().any(
            |piece| matches!(piece.location, WordLocation::Register(output) if output == register),
        ),
        Some(ValueStorage::Indirect { .. }) => {
            request.target.architecture == Architecture::X86_64
                && register == MachineRegister::X86Rax
        }
        None => false,
    };
    let mut preserved = Vec::new();
    for register in internal_writes
        .as_slice()
        .iter()
        .copied()
        .filter(|register| {
            *register != MachineRegister::X86Rsp
                && (protocol.preserved_registers.contains(*register)
                    || (!inbound.ordinary_clobbers.contains(*register)
                        && !result_output(*register)))
        })
    {
        let byte_size = if matches!(
            register,
            MachineRegister::Aarch64V(_) | MachineRegister::X86Xmm(_)
        ) {
            cursor = cursor
                .checked_add(15)
                .map(|value| value & !15)
                .ok_or(Error::FrameOverflow)?;
            16
        } else {
            8
        };
        preserved.push(PreservedRegister {
            register,
            offset: allocate_value(&mut cursor, u32::from(byte_size))?,
            byte_size,
        });
    }
    let frame_bytes = cursor
        .checked_add(15)
        .map(|value| value & !15)
        .and_then(|value| value.checked_add(protocol.frame_alignment_padding))
        .filter(|value| *value <= i32::MAX as u32)
        .ok_or(Error::FrameOverflow)?;
    let interval = crate::frame_layout::stack_commit::stack_commit_granule_bytes(request.target)
        .ok_or(Error::IncompatiblePlan)?;
    let touches = if u64::from(frame_bytes) > interval {
        u32::try_from(u64::from(frame_bytes).div_ceil(interval))
            .map_err(|_| Error::FrameOverflow)?
    } else {
        0
    };
    let (mut bytes, epilogue) = match request.target.architecture {
        Architecture::Aarch64 => isa_aarch64::encode_aapcs64_frame_protocol(
            &encoding.model,
            u64::from(frame_bytes),
            isa_aarch64::Aarch64StackProbe {
                interval_bytes: interval,
                touches,
            },
            &[],
        )
        .map_err(|_| Error::Encoding)?,
        Architecture::X86_64 => isa_x86_64::encode_system_v_amd64_frame_protocol(
            &encoding.model,
            u64::from(frame_bytes),
            isa_x86_64::X86_64StackProbe {
                interval_bytes: interval,
                touches,
            },
            &[],
        )
        .map_err(|_| Error::Encoding)?,
    };
    if let Some(offset) = continuation {
        encoding.memory(&mut bytes, MachineRegister::Aarch64X(30), offset, 8, false)?;
    }
    for saved in &preserved {
        encoding.memory(
            &mut bytes,
            saved.register,
            saved.offset,
            saved.byte_size,
            false,
        )?;
    }
    bytes.extend_from_slice(protocol.before_body);
    for (transport, snapshot) in transports.iter().zip(&snapshots) {
        encoding.capture_registers(
            &mut bytes,
            transport.source.pieces(),
            snapshot
                .source_pointer_offset
                .unwrap_or(snapshot.value_offset),
        )?;
    }
    if let (Some(transport), Some(pointer)) = (&result, caller_result_pointer) {
        encoding.capture_registers(&mut bytes, transport.destination.pieces(), pointer)?;
    }
    let incoming_bias = frame_bytes
        .checked_add(protocol.incoming_argument_bias)
        .ok_or(Error::FrameOverflow)?;
    for (transport, snapshot) in transports.iter().zip(&snapshots) {
        encoding.capture_stack(
            &mut bytes,
            transport.source.pieces(),
            snapshot
                .source_pointer_offset
                .unwrap_or(snapshot.value_offset),
            incoming_bias,
        )?;
        if let Some(pointer) = snapshot.source_pointer_offset {
            encoding.copy_indirect(
                &mut bytes,
                snapshot.value_offset,
                pointer,
                transport.byte_size,
                true,
            )?;
        }
    }
    for (transport, snapshot) in transports.iter().zip(&snapshots) {
        if let Some(descriptor) = snapshot.descriptor_offset {
            encoding.copy_word(&mut bytes, snapshot.value_offset, descriptor, 8)?;
            encoding.constant(&mut bytes, transport.scalar_codec)?;
            encoding.memory(
                &mut bytes,
                encoding.scratch,
                descriptor.checked_add(8).ok_or(Error::FrameOverflow)?,
                8,
                false,
            )?;
            encoding.address(&mut bytes, descriptor)?;
            encoding.memory(
                &mut bytes,
                encoding.scratch,
                snapshot.value_offset,
                8,
                false,
            )?;
        }
        encoding.distribute_argument(
            &mut bytes,
            &transport.destination,
            snapshot.value_offset,
            transport.byte_size,
        )?;
    }
    if let (Some(transport), Some(snapshot)) = (&result, result_snapshot)
        && let ValueStorage::Indirect { pointer, .. } = &transport.source
    {
        encoding.place_address(&mut bytes, pointer.location, snapshot)?;
    }
    let call_field_offset = bytes.len()
        + if request.target.architecture == Architecture::X86_64 {
            1
        } else {
            0
        };
    match request.target.architecture {
        Architecture::Aarch64 => bytes.extend_from_slice(&isa_aarch64::ORDINARY_DIRECT_CALL_BYTES),
        Architecture::X86_64 => bytes.extend_from_slice(&isa_x86_64::ORDINARY_DIRECT_CALL_BYTES),
    }
    if let (Some(transport), Some(snapshot)) = (&result, result_snapshot) {
        if let ValueStorage::Direct(pieces) = &transport.source {
            encoding.capture_registers(&mut bytes, pieces, snapshot)?;
        }
        match &transport.destination {
            ValueStorage::Direct(pieces) => encoding.distribute(&mut bytes, pieces, snapshot)?,
            ValueStorage::Indirect { .. } => {
                let pointer = caller_result_pointer.ok_or(Error::UnsupportedValueTransport)?;
                encoding.copy_indirect(
                    &mut bytes,
                    snapshot,
                    pointer,
                    transport.byte_size,
                    false,
                )?;
                if request.target.architecture == Architecture::X86_64 {
                    encoding.memory(&mut bytes, MachineRegister::X86Rax, pointer, 8, true)?;
                }
            }
        }
    }
    for saved in preserved.iter().rev() {
        encoding.memory(
            &mut bytes,
            saved.register,
            saved.offset,
            saved.byte_size,
            true,
        )?;
    }
    if let Some(offset) = continuation {
        encoding.memory(&mut bytes, MachineRegister::Aarch64X(30), offset, 8, true)?;
    }
    bytes.extend_from_slice(&epilogue);
    bytes.extend_from_slice(protocol.return_bytes);
    finish_native_entry(request, bytes, frame_bytes, call_field_offset, None)
}

pub fn emit_installed_interrupt_entry_bridge(
    request: NativeEntryBridgeRequest,
    plan: InstalledInterruptEntryPlan,
) -> Result<NativeEntryBridge, NativeEntryBridgeError> {
    if !plan.has_canonical_geometry() {
        return Err(Error::InvalidInstalledInterruptGeometry);
    }
    if matches!(
        plan.protocol,
        InstalledInterruptProtocol::FatalContext { .. }
    ) {
        return emit_installed_fatal_entry_bridge(request, plan);
    }
    let protocol = select_entry_protocol(&request)?;
    let internal = &request.internal;
    let inbound = &request.inbound.call;
    if request.target.architecture != Architecture::X86_64
        || inbound.entry_control != EntryControl::InterruptReturn
        || internal.entry_control != EntryControl::CallReturn
        || internal.policy.architecture() != Architecture::X86_64
        || inbound.policy.architecture() != Architecture::X86_64
        || internal.stack_alignment != 16
        || !matches!(request.inbound.state.stack, EntryStack::Dedicated { .. })
        || request.inbound.state.preemption != Preemption::Masked
    {
        return Err(Error::UnsupportedInterruptState);
    }
    if internal.parameters.len() != 1
        || inbound.parameters.len() != 1
        || request.parameters.len() != 1
        || internal.result.is_some()
        || inbound.result.is_some()
        || !internal.callback_materializations.is_empty()
        || !inbound.callback_materializations.is_empty()
    {
        return Err(Error::UnsupportedInstalledInterruptSignature);
    }
    let (_, parameter) = request
        .parameters
        .iter()
        .next()
        .ok_or(Error::InvalidParameterJoin)?;
    if parameter.inbound_ordinal != 0 || parameter.scalar_codec != 0 {
        return Err(Error::UnsupportedInstalledInterruptSignature);
    }
    let source = &inbound.parameters[0];
    let destination = &internal.parameters[0];
    if source.shape.byte_size != ack::BYTES
        || u32::from(source.shape.alignment) != runtime_abi::static_interrupts::ALIGNMENT
        || is_borrowed(source.shape.class)
        || is_borrowed(destination.shape.class)
    {
        return Err(Error::UnsupportedInstalledInterruptSignature);
    }
    let transport = value_transport(source, destination, 0)?;
    if transport.destination.pieces().iter().any(|piece| {
        matches!(piece.location, WordLocation::Register(register) if register.codec_index() >= 16)
    }) {
        return Err(Error::UnsupportedInstalledInterruptSignature);
    }
    let mut cursor = u32::from(internal.shadow_bytes);
    if let ValueStorage::Indirect {
        copy_offset: Some(offset),
        ..
    } = &transport.destination
    {
        cursor = cursor.max(offset.checked_add(ack::BYTES).ok_or(Error::FrameOverflow)?);
    }
    for piece in transport.destination.pieces() {
        if let WordLocation::Stack(offset) = piece.location {
            cursor = cursor.max(
                offset
                    .checked_add(u32::from(piece.byte_size))
                    .ok_or(Error::FrameOverflow)?,
            );
        }
    }
    let occupied = |register| {
        transport.destination.pieces().iter().any(|piece| {
            matches!(piece.location, WordLocation::Register(candidate) if candidate == register)
        })
    };
    let mut candidates = [
        MachineRegister::X86R11,
        MachineRegister::X86R10,
        MachineRegister::X86Rax,
        MachineRegister::X86Rcx,
        MachineRegister::X86Rdx,
        MachineRegister::X86R8,
        MachineRegister::X86R9,
    ]
    .into_iter()
    .filter(|register| !occupied(*register));
    let scratch = candidates.next().ok_or(Error::NoUnoccupiedScratch)?;
    let address_scratch = candidates.next().ok_or(Error::NoUnoccupiedScratch)?;
    let mut encoding = Encoding {
        architecture: Architecture::X86_64,
        model: validate_physical_register_model(isa_x86_64::x86_64_physical_register_model())
            .map_err(|_| Error::Encoding)?,
        scratch,
        address_scratch,
        aligned_homes: Vec::new(),
    };
    let snapshot = allocate_home(&mut cursor, ack::BYTES, 8, &mut encoding.aligned_homes)?;
    let saved_root = allocate_word(&mut cursor)?;
    let local_bytes = cursor
        .checked_add(15)
        .map(|value| value & !15)
        .filter(|value| *value <= i32::MAX as u32)
        .ok_or(Error::FrameOverflow)?;
    let frame_bytes = plan
        .root_slot_from_top
        .checked_add(plan.saved_register_bytes)
        .and_then(|value| value.checked_add(local_bytes))
        .ok_or(Error::FrameOverflow)?;
    let root_offset = local_bytes
        .checked_add(plan.saved_register_bytes)
        .ok_or(Error::FrameOverflow)?;
    let interval = crate::frame_layout::stack_commit::stack_commit_granule_bytes(request.target)
        .ok_or(Error::IncompatiblePlan)?;
    let touches = if u64::from(local_bytes) > interval {
        u32::try_from(u64::from(local_bytes).div_ceil(interval))
            .map_err(|_| Error::FrameOverflow)?
    } else {
        0
    };
    let (prologue, epilogue) = isa_x86_64::encode_system_v_amd64_frame_protocol(
        &encoding.model,
        u64::from(local_bytes),
        isa_x86_64::X86_64StackProbe {
            interval_bytes: interval,
            touches,
        },
        &[],
    )
    .map_err(|_| Error::Encoding)?;

    let mut bytes = Vec::new();
    let gap = u8::try_from(plan.root_slot_from_top - plan.hardware_frame_bytes())
        .map_err(|_| Error::InvalidInstalledInterruptGeometry)?;
    bytes.extend_from_slice(&[0x48, 0x83, 0xec, gap]);
    for register in SAVED_GPRS {
        push_or_pop(&mut bytes, register, false);
    }
    bytes.extend_from_slice(protocol.before_body);
    bytes.extend_from_slice(&prologue);
    encoding.memory(&mut bytes, address_scratch, root_offset, 8, true)?;
    encoding.memory(&mut bytes, address_scratch, saved_root, 8, false)?;
    encoding.memory(&mut bytes, address_scratch, snapshot + ack::ROOT, 8, false)?;
    encoding.pointer_memory(&mut bytes, root::STATE, 8, true)?;
    require_scratch_equal(&mut bytes, scratch, root::ACTIVE as u8);
    encoding.pointer_memory(&mut bytes, root::DEBT, 8, true)?;
    require_scratch_equal(&mut bytes, scratch, 0);
    encoding.pointer_memory(&mut bytes, root::INVOCATION, 8, true)?;
    add_one_without_overflow(&mut bytes, scratch);
    encoding.pointer_memory(&mut bytes, root::INVOCATION, 8, false)?;
    encoding.pointer_memory(&mut bytes, root::DEBT, 8, false)?;
    encoding.memory(&mut bytes, scratch, snapshot + ack::INVOCATION, 8, false)?;
    encoding.memory(&mut bytes, scratch, snapshot + ack::DEBT, 8, false)?;
    for (root_field, token_field) in [
        (root::PROVIDER, ack::PROVIDER),
        (root::COMPLETION, ack::COMPLETION),
    ] {
        encoding.pointer_memory(&mut bytes, root_field, 8, true)?;
        encoding.memory(&mut bytes, scratch, snapshot + token_field, 8, false)?;
    }
    encoding.distribute_argument(&mut bytes, &transport.destination, snapshot, ack::BYTES)?;
    let call_field_offset = bytes.len() + 1;
    bytes.extend_from_slice(&isa_x86_64::ORDINARY_DIRECT_CALL_BYTES);
    encoding.memory(&mut bytes, address_scratch, saved_root, 8, true)?;
    encoding.pointer_memory(&mut bytes, root::DEBT, 8, true)?;
    require_scratch_equal(&mut bytes, scratch, 0);
    bytes.extend_from_slice(&epilogue);
    for register in SAVED_GPRS.into_iter().rev() {
        push_or_pop(&mut bytes, register, true);
    }
    bytes.extend_from_slice(&[0x48, 0x83, 0xc4, 16]);
    bytes.extend_from_slice(protocol.return_bytes);
    finish_native_entry(request, bytes, frame_bytes, call_field_offset, Some(plan))
}

fn emit_installed_fatal_entry_bridge(
    request: NativeEntryBridgeRequest,
    plan: InstalledInterruptEntryPlan,
) -> Result<NativeEntryBridge, Error> {
    let InstalledInterruptProtocol::FatalContext {
        anchor_gate_byte_offset,
        table_bytes,
        ..
    } = plan.protocol
    else {
        return Err(Error::InvalidInstalledInterruptGeometry);
    };
    if !plan.has_canonical_geometry() {
        return Err(Error::InvalidInstalledInterruptGeometry);
    }
    validate_fatal_entry_protocol(&request)?;
    let inbound = &request.inbound.call;
    let internal = &request.internal;
    if request.target.architecture != Architecture::X86_64
        || inbound.entry_control != EntryControl::InterruptReturn
        || internal.entry_control != EntryControl::CallReturn
        || internal.policy.architecture() != Architecture::X86_64
        || inbound.policy.architecture() != Architecture::X86_64
        || internal.stack_alignment != 16
        || !matches!(
            request.inbound.state.stack,
            EntryStack::Interrupted | EntryStack::Dedicated { .. }
        )
        || request.inbound.state.preemption != Preemption::Masked
    {
        return Err(Error::UnsupportedInterruptState);
    }
    if internal.parameters.len() != 1
        || inbound.parameters.len() != 1
        || request.parameters.len() != 1
        || internal.result.is_some()
        || inbound.result.is_some()
        || !internal.callback_materializations.is_empty()
        || !inbound.callback_materializations.is_empty()
    {
        return Err(Error::UnsupportedFatalInterruptSignature);
    }
    let (_, parameter) = request
        .parameters
        .iter()
        .next()
        .ok_or(Error::InvalidParameterJoin)?;
    if parameter.inbound_ordinal != 0
        || parameter.scalar_codec != 0
        || inbound.parameters[0].shape != ValueShape::integer(8, 8)
        || internal.parameters[0].shape.class != ValueClass::BorrowedReference
    {
        return Err(Error::UnsupportedFatalInterruptSignature);
    }
    let transport = value_transport(&inbound.parameters[0], &internal.parameters[0], 0)?;
    if transport.destination.pieces().iter().any(|piece| {
        matches!(piece.location, WordLocation::Register(register) if register.codec_index() >= 16)
    }) {
        return Err(Error::UnsupportedFatalInterruptSignature);
    }
    let mut cursor = u32::from(internal.shadow_bytes);
    for piece in transport.destination.pieces() {
        if let WordLocation::Stack(offset) = piece.location {
            cursor = cursor.max(
                offset
                    .checked_add(u32::from(piece.byte_size))
                    .ok_or(Error::FrameOverflow)?,
            );
        }
    }
    let encoding = Encoding {
        architecture: Architecture::X86_64,
        model: validate_physical_register_model(isa_x86_64::x86_64_physical_register_model())
            .map_err(|_| Error::Encoding)?,
        scratch: MachineRegister::X86Rax,
        address_scratch: MachineRegister::X86Rdx,
        aligned_homes: Vec::new(),
    };
    let idtr = allocate_value(&mut cursor, 16)?;
    let gdtr = allocate_value(&mut cursor, 16)?;
    let provider = allocate_word(&mut cursor)?;
    let ist = allocate_word(&mut cursor)?;
    let context = allocate_word(&mut cursor)?;
    let local_bytes = cursor
        .checked_add(15)
        .map(|value| value & !15)
        .filter(|value| *value <= i32::MAX as u32)
        .ok_or(Error::FrameOverflow)?;
    let interval = crate::frame_layout::stack_commit::stack_commit_granule_bytes(request.target)
        .ok_or(Error::IncompatiblePlan)?;
    let touches = if u64::from(local_bytes) > interval {
        u32::try_from(u64::from(local_bytes).div_ceil(interval))
            .map_err(|_| Error::FrameOverflow)?
    } else {
        0
    };
    let (prologue, _) = isa_x86_64::encode_system_v_amd64_frame_protocol(
        &encoding.model,
        u64::from(local_bytes),
        isa_x86_64::X86_64StackProbe {
            interval_bytes: interval,
            touches,
        },
        &[],
    )
    .map_err(|_| Error::Encoding)?;
    let cpu_alignment_bytes = match request.inbound.state.stack {
        EntryStack::Interrupted => 15,
        EntryStack::Dedicated { .. } => 0,
        _ => return Err(Error::UnsupportedInterruptState),
    };
    let frame_bytes = plan
        .hardware_frame_bytes()
        .checked_add(cpu_alignment_bytes)
        .and_then(|bytes| bytes.checked_add(plan.saved_register_bytes))
        .and_then(|bytes| bytes.checked_add(15))
        .and_then(|bytes| bytes.checked_add(local_bytes))
        .ok_or(Error::FrameOverflow)?;
    let mut bytes = Vec::new();
    for register in SAVED_GPRS {
        push_or_pop(&mut bytes, register, false);
    }
    bytes.extend_from_slice(&isa_x86_64::CLEAR_DIRECTION_FLAG_BYTES);
    bytes.extend_from_slice(&[0x48, 0x83, 0xe4, 0xf0]);
    bytes.extend_from_slice(&prologue);
    descriptor_store(&mut bytes, idtr, 1);
    descriptor_store(&mut bytes, gdtr, 0);
    encoding.memory(&mut bytes, MachineRegister::X86Rax, idtr, 2, true)?;
    compare_eax(&mut bytes, table_bytes - 1);
    require(&mut bytes, 0x74);
    encoding.memory(&mut bytes, MachineRegister::X86Rdx, idtr + 2, 8, true)?;
    encoding.pointer_memory(&mut bytes, anchor_gate_byte_offset + 4, 1, true)?;
    bytes.extend_from_slice(&[0x83, 0xe0, 7, 0x85, 0xc0]);
    require(&mut bytes, 0x75);
    encoding.memory(&mut bytes, MachineRegister::X86Rax, ist, 8, false)?;
    encoding.pointer_memory(&mut bytes, anchor_gate_byte_offset + 8, 4, true)?;
    bytes.extend_from_slice(&[0x48, 0xc1, 0xe0, 32, 0x48, 0x89, 0xc1]);
    encoding.pointer_memory(&mut bytes, anchor_gate_byte_offset + 6, 2, true)?;
    bytes.extend_from_slice(&[0x48, 0xc1, 0xe0, 16, 0x48, 0x09, 0xc1]);
    encoding.pointer_memory(&mut bytes, anchor_gate_byte_offset, 2, true)?;
    bytes.extend_from_slice(&[0x48, 0x09, 0xc8]);
    encoding.memory(&mut bytes, MachineRegister::X86Rax, provider, 8, false)?;
    bytes.extend_from_slice(&[0x0f, 0x00, 0xc8, 0x0f, 0xb7, 0xc0]);
    bytes.extend_from_slice(&[0xa9, 7, 0, 0, 0]);
    require(&mut bytes, 0x74);
    bytes.extend_from_slice(&[0x85, 0xc0]);
    require(&mut bytes, 0x75);
    bytes.extend_from_slice(&[0x48, 0x89, 0xc1, 0x83, 0xc0, 15]);
    encoding.memory(&mut bytes, MachineRegister::X86Rdx, gdtr, 2, true)?;
    bytes.extend_from_slice(&[0x39, 0xd0]);
    require(&mut bytes, 0x76);
    encoding.memory(&mut bytes, MachineRegister::X86Rdx, gdtr + 2, 8, true)?;
    bytes.extend_from_slice(&[0x48, 0x01, 0xca]);
    encoding.pointer_memory(&mut bytes, 4, 4, true)?;
    bytes.extend_from_slice(&[0x25, 0, 0, 0, 0xff, 0x48, 0x89, 0xc1]);
    encoding.pointer_memory(&mut bytes, 2, 2, true)?;
    bytes.extend_from_slice(&[0x48, 0x09, 0xc1]);
    encoding.pointer_memory(&mut bytes, 4, 1, true)?;
    bytes.extend_from_slice(&[0x48, 0xc1, 0xe0, 16, 0x48, 0x09, 0xc1]);
    encoding.pointer_memory(&mut bytes, 8, 4, true)?;
    bytes.extend_from_slice(&[0x48, 0xc1, 0xe0, 32, 0x48, 0x09, 0xc8, 0x48, 0x89, 0xc2]);
    encoding.memory(&mut bytes, MachineRegister::X86Rcx, ist, 8, true)?;
    bytes.extend_from_slice(&[0x48, 0x8b, 0x54, 0xca, 28]);
    bytes.extend_from_slice(&[0x48, 0x8b, 0x52, 0xc8]);
    encoding.pointer_memory(&mut bytes, root::STATE, 8, true)?;
    bytes.extend_from_slice(&[0x48, 0x83, 0xf8, root::ACTIVE as u8]);
    require(&mut bytes, 0x74);
    encoding.pointer_memory(&mut bytes, root::PROVIDER, 8, true)?;
    encoding.memory(&mut bytes, MachineRegister::X86Rcx, provider, 8, true)?;
    bytes.extend_from_slice(&[0x48, 0x39, 0xc8]);
    require(&mut bytes, 0x74);
    encoding.pointer_memory(&mut bytes, root::CONTEXT, 8, true)?;
    encoding.memory(&mut bytes, MachineRegister::X86Rax, context, 8, false)?;
    encoding.distribute_argument(&mut bytes, &transport.destination, context, 8)?;
    let call_field_offset = bytes.len() + 1;
    bytes.extend_from_slice(&isa_x86_64::ORDINARY_DIRECT_CALL_BYTES);
    halt(&mut bytes);
    finish_native_entry(request, bytes, frame_bytes, call_field_offset, Some(plan))
}

fn push_or_pop(bytes: &mut Vec<u8>, register: u8, pop: bool) {
    if register >= 8 {
        bytes.push(0x41);
    }
    bytes.push(if pop { 0x58 } else { 0x50 } + (register & 7));
}

fn require_scratch_equal(bytes: &mut Vec<u8>, register: MachineRegister, value: u8) {
    let register = register.codec_index();
    bytes.extend_from_slice(&[0x48 | (register >> 3), 0x83, 0xf8 | (register & 7), value]);
    bytes.extend_from_slice(&[0x74, 0x02, 0x0f, 0x0b]);
}

fn add_one_without_overflow(bytes: &mut Vec<u8>, register: MachineRegister) {
    let register = register.codec_index();
    bytes.extend_from_slice(&[0x48 | (register >> 3), 0x83, 0xc0 | (register & 7), 1]);
    bytes.extend_from_slice(&[0x73, 0x02, 0x0f, 0x0b]);
}

fn descriptor_store(bytes: &mut Vec<u8>, offset: u32, operation: u8) {
    bytes.extend_from_slice(&[0x0f, 0x01, 0x84 | (operation << 3), 0x24]);
    bytes.extend_from_slice(&offset.to_le_bytes());
}

fn compare_eax(bytes: &mut Vec<u8>, value: u32) {
    bytes.push(0x3d);
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn require(bytes: &mut Vec<u8>, condition: u8) {
    bytes.extend_from_slice(&[condition, 4]);
    halt(bytes);
}

fn halt(bytes: &mut Vec<u8>) {
    bytes.extend_from_slice(&isa_x86_64::PRIVILEGED_STOP_BYTES);
}

pub fn validate_native_entry_bridge(bridge: &NativeEntryBridge) -> Result<(), Error> {
    let replay = match bridge.installed_interrupt {
        Some(plan) => emit_installed_interrupt_entry_bridge(bridge.request.clone(), plan)?,
        None => emit_native_entry_bridge(bridge.request.clone())?,
    };
    if replay != *bridge {
        return Err(Error::ReplayMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_bridge_encoding::value_pieces;
    use calling_conventions::{
        CallSignature, CallingPolicy, ValueLocation, evaluate_call_plan,
        evaluate_ordinary_boundary_entry_plan,
    };

    fn request(
        target: NativeTarget,
        internal_shapes: Vec<ValueShape>,
        inbound_shapes: Vec<ValueShape>,
        ordinals: &[u32],
        codecs: &[u64],
        result: Option<ValueShape>,
    ) -> NativeEntryBridgeRequest {
        let policy = CallingPolicy::native_for_target(target);
        let internal = evaluate_call_plan(
            policy,
            &CallSignature {
                parameters: internal_shapes,
                result,
            },
        )
        .unwrap();
        let inbound = evaluate_ordinary_boundary_entry_plan(
            policy,
            &CallSignature {
                parameters: inbound_shapes,
                result,
            },
        )
        .unwrap();
        let mut parameters = Arena::with_capacity(ordinals.len());
        for (ordinal, codec) in ordinals.iter().zip(codecs) {
            parameters.insert(NativeEntryParameter {
                inbound_ordinal: *ordinal,
                scalar_codec: *codec,
            });
        }
        NativeEntryBridgeRequest {
            target,
            internal,
            inbound: inbound.plan().clone(),
            body_footprint: Some(full_convention_body()),
            parameters,
        }
    }

    // A body that overwrites both register classes, as the internal convention allows.
    fn full_convention_body() -> StateEffectsEvidence {
        let classes = MachineStateSet::new([
            MachineState::GeneralRegisters,
            MachineState::VectorRegisters,
        ]);
        StateEffectsEvidence::new(
            RegisterSet::default(),
            RegisterSet::default(),
            classes,
            classes,
        )
    }

    fn interrupt_request(shapes: Vec<ValueShape>) -> NativeEntryBridgeRequest {
        use calling_conventions::{
            EntryStack, MachineRegime, MachineState, MachineStateSet, Preemption, StatePlan,
        };
        let count = shapes.len();
        let mut request = request(
            NativeTarget::linux_x64(),
            shapes.clone(),
            shapes,
            &(0..count as u32).collect::<Vec<_>>(),
            &vec![0; count],
            None,
        );
        request.inbound.call.entry_control = EntryControl::InterruptReturn;
        let preserved = MachineStateSet::new([
            MachineState::GeneralRegisters,
            MachineState::Flags,
            MachineState::InstructionPointer,
            MachineState::StackPointer,
        ]);
        request.inbound.state = StatePlan {
            initial_regime: MachineRegime::X86Long64,
            interrupted_state: preserved
                .union(MachineStateSet::new([MachineState::VectorRegisters])),
            saved_state: preserved,
            restored_state: preserved,
            permitted_transitive_use: preserved
                .union(MachineStateSet::new([MachineState::SegmentState])),
            stack: EntryStack::Dedicated { class: 4 },
            preemption: Preemption::Masked,
        };
        request.body_footprint = Some(
            StateFootprintEvidence::new(
                RegisterSet::new([
                    MachineRegister::X86Rax,
                    MachineRegister::X86Rdx,
                    MachineRegister::X86Rsp,
                ]),
                MachineStateSet::new([
                    MachineState::Flags,
                    MachineState::InstructionPointer,
                    MachineState::StackPointer,
                ]),
            )
            .into(),
        );
        request
    }

    #[test]
    fn interrupt_entry_preserves_registers_and_returns_through_cpu_frame() {
        use iced_x86::{Decoder, DecoderOptions, Mnemonic, Register};
        let bridge = emit_native_entry_bridge(interrupt_request(vec![])).unwrap();
        let instructions = Decoder::new(64, bridge.bytes(), DecoderOptions::NONE)
            .into_iter()
            .collect::<Vec<_>>();
        assert!(
            instructions
                .iter()
                .all(|instruction| !instruction.is_invalid())
        );
        assert_eq!(instructions.last().unwrap().mnemonic(), Mnemonic::Iretq);
        let call = instructions
            .iter()
            .position(|instruction| instruction.mnemonic() == Mnemonic::Call)
            .unwrap();
        assert!(
            instructions[..call]
                .iter()
                .any(|instruction| instruction.mnemonic() == Mnemonic::Cld)
        );
        let stores = instructions[..call]
            .iter()
            .filter(|instruction| {
                instruction.mnemonic() == Mnemonic::Mov
                    && instruction.memory_base() == Register::RSP
                    && instruction.op1_register() != Register::None
            })
            .count();
        let loads = instructions[call + 1..]
            .iter()
            .filter(|instruction| {
                instruction.mnemonic() == Mnemonic::Mov
                    && instruction.memory_base() == Register::RSP
                    && instruction.op0_register() != Register::None
            })
            .count();
        assert_eq!((stores, loads), (15, 15));
        assert_eq!(bridge.frame_bytes() % 16, 8);
        validate_native_entry_bridge(&bridge).unwrap();

        if let Some(path) = std::env::var_os("OMEGA_INTERRUPT_BRIDGE_PROBE") {
            let mut payload = bridge.bytes().to_vec();
            let body_offset = payload.len();
            payload.extend_from_slice(&[0xb0, b'T', 0xe6, 0xe9, 0xc3]);
            let field = bridge.call_field_offset();
            let displacement =
                i32::try_from(body_offset).unwrap() - i32::try_from(field + 4).unwrap();
            payload[field..field + 4].copy_from_slice(&displacement.to_le_bytes());
            std::fs::write(path, payload).unwrap();
        }
    }

    #[test]
    fn interrupt_stack_arguments_follow_the_complete_return_frame() {
        use calling_conventions::ValueLocation;
        use iced_x86::{Decoder, DecoderOptions, Register};
        let mut request = interrupt_request(vec![ValueShape::integer(8, 8)]);
        request.inbound.call.parameters[0].locations = vec![ValueLocation::Stack {
            stack_byte_offset: 0,
            value_byte_offset: 0,
            byte_size: 8,
            alignment: 8,
        }];
        let bridge = emit_native_entry_bridge(request).unwrap();
        let instructions = Decoder::new(64, bridge.bytes(), DecoderOptions::NONE)
            .into_iter()
            .collect::<Vec<_>>();
        assert!(
            instructions
                .iter()
                .any(|instruction| instruction.memory_base() == Register::RSP
                    && instruction.memory_displacement64() == u64::from(bridge.frame_bytes()) + 40)
        );
        assert!(!instructions.iter().any(|instruction| {
            instruction.memory_base() == Register::RSP
                && (u64::from(bridge.frame_bytes())..u64::from(bridge.frame_bytes()) + 40)
                    .contains(&instruction.memory_displacement64())
        }));
    }

    #[test]
    fn interrupt_entry_refuses_vector_argument_reads() {
        use calling_conventions::ValueLocation;
        let mut request = interrupt_request(vec![ValueShape::integer(8, 8)]);
        request.inbound.call.parameters[0].locations = vec![ValueLocation::Register {
            register: MachineRegister::X86Xmm(0),
            value_byte_offset: 0,
            byte_size: 8,
        }];
        assert_eq!(
            emit_native_entry_bridge(request),
            Err(Error::UnsupportedInterruptState)
        );
    }

    #[test]
    fn interrupt_entry_refuses_unpreserved_or_unaccounted_body_state() {
        use calling_conventions::{MachineState, MachineStateSet};
        let mut missing = interrupt_request(vec![]);
        missing.body_footprint = None;
        assert_eq!(
            emit_native_entry_bridge(missing),
            Err(Error::MissingInterruptBodyFootprint)
        );
        let mut vector = interrupt_request(vec![]);
        vector.body_footprint = Some(
            StateFootprintEvidence::new(
                RegisterSet::new([MachineRegister::X86Xmm(0)]),
                MachineStateSet::empty(),
            )
            .into(),
        );
        assert_eq!(
            emit_native_entry_bridge(vector),
            Err(Error::InterruptBodyStateExceedsPlan)
        );
        let mut unsaved = interrupt_request(vec![]);
        unsaved.inbound.state.saved_state = MachineStateSet::new([MachineState::Flags]);
        assert_eq!(
            emit_native_entry_bridge(unsaved),
            Err(Error::UnsupportedInterruptState)
        );
    }

    fn narrow_inbound_clobbers(
        request: &mut NativeEntryBridgeRequest,
        keep: impl Fn(&MachineRegister) -> bool,
    ) {
        request.inbound.call.ordinary_clobbers = RegisterSet::new(
            request
                .internal
                .ordinary_clobbers
                .as_slice()
                .iter()
                .copied()
                .filter(|register| keep(register)),
        );
    }

    #[test]
    fn narrower_inbound_clobbers_save_integer_and_full_vector_state() {
        let mut request = request(NativeTarget::macos_arm64(), vec![], vec![], &[], &[], None);
        narrow_inbound_clobbers(&mut request, |register| {
            !matches!(
                register,
                MachineRegister::Aarch64X(9) | MachineRegister::Aarch64V(16)
            )
        });
        let bridge = emit_native_entry_bridge(request).unwrap();
        assert_eq!(bridge.frame_bytes(), 32);
        let before_call = &bridge.bytes()[..bridge.call_field_offset()];
        let after_call = &bridge.bytes()[bridge.call_field_offset() + 4..];
        for (save, restore) in [
            (0xf900_07e9_u32, 0xf940_07e9_u32),
            (0x3d80_07f0, 0x3dc0_07f0),
        ] {
            assert!(
                before_call
                    .windows(4)
                    .any(|bytes| bytes == save.to_le_bytes())
            );
            assert!(
                after_call
                    .windows(4)
                    .any(|bytes| bytes == restore.to_le_bytes())
            );
        }
        validate_native_entry_bridge(&bridge).unwrap();
    }

    #[test]
    fn body_that_overwrites_no_vector_register_gets_no_vector_saves() {
        let mut request = request(NativeTarget::macos_arm64(), vec![], vec![], &[], &[], None);
        narrow_inbound_clobbers(&mut request, |register| {
            !matches!(
                register,
                MachineRegister::Aarch64X(9) | MachineRegister::Aarch64V(16)
            )
        });
        let general = MachineStateSet::new([MachineState::GeneralRegisters]);
        request.body_footprint = Some(StateEffectsEvidence::new(
            RegisterSet::default(),
            RegisterSet::default(),
            general,
            general,
        ));
        let bridge = emit_native_entry_bridge(request).unwrap();
        let before_call = &bridge.bytes()[..bridge.call_field_offset()];
        assert!(
            before_call
                .windows(4)
                .any(|bytes| bytes == 0xf900_07e9_u32.to_le_bytes())
        );
        assert!(
            !bridge
                .complete_effects()
                .machine_state()
                .contains_all(MachineStateSet::new([MachineState::VectorRegisters]))
        );
        validate_native_entry_bridge(&bridge).unwrap();
    }

    #[test]
    fn returned_register_is_not_restored_over_the_result() {
        let mut request = request(
            NativeTarget::macos_arm64(),
            vec![],
            vec![],
            &[],
            &[],
            Some(ValueShape::integer(8, 8)),
        );
        narrow_inbound_clobbers(&mut request, |r| *r != MachineRegister::Aarch64X(0));
        assert_eq!(emit_native_entry_bridge(request).unwrap().frame_bytes(), 16);
    }

    #[test]
    fn hidden_result_pointer_remains_caller_state_not_a_direct_output() {
        let mut request = request(
            NativeTarget::macos_arm64(),
            vec![],
            vec![],
            &[],
            &[],
            Some(ValueShape::integer(24, 8)),
        );
        narrow_inbound_clobbers(&mut request, |r| *r != MachineRegister::Aarch64X(8));
        let bridge = emit_native_entry_bridge(request).unwrap();
        assert_eq!(bridge.frame_bytes(), 48);
        for word in [0xf900_17e8_u32, 0xf940_17e8] {
            assert!(
                bridge
                    .bytes()
                    .windows(4)
                    .any(|bytes| bytes == word.to_le_bytes())
            );
        }
    }

    #[test]
    fn microsoft_entry_preserves_sysv_additional_clobbers() {
        let target = NativeTarget::windows_x64();
        let mut request = request(target, vec![], vec![], &[], &[], None);
        request.internal =
            evaluate_call_plan(CallingPolicy::SystemVAMD64, &CallSignature::default()).unwrap();
        let bridge = emit_native_entry_bridge(request).unwrap();
        assert_eq!(bridge.frame_bytes(), 184);
        let before_call = &bridge.bytes()[..bridge.call_field_offset() - 1];
        let after_call = &bridge.bytes()[bridge.call_field_offset() + 4..];
        for (register_bits, offset) in [(0x34, 0), (0x7c, 8)] {
            let save = if offset == 0 {
                vec![0x48, 0x89, register_bits, 0x24]
            } else {
                vec![0x48, 0x89, register_bits, 0x24, offset]
            };
            let mut restore = save.clone();
            restore[1] = 0x8b;
            assert!(before_call.windows(save.len()).any(|bytes| bytes == save));
            assert!(
                after_call
                    .windows(restore.len())
                    .any(|bytes| bytes == restore)
            );
        }
        assert!(
            before_call
                .windows(8)
                .any(|bytes| bytes == [0x0f, 0x11, 0xb4, 0x24, 16, 0, 0, 0])
        );
        assert!(
            after_call
                .windows(9)
                .any(|bytes| bytes == [0x44, 0x0f, 0x10, 0xbc, 0x24, 160, 0, 0, 0])
        );
        validate_native_entry_bridge(&bridge).unwrap();
    }

    #[test]
    fn fully_preserving_inbound_promise_saves_borrowed_scratch() {
        let target = NativeTarget::linux_x64();
        let mut request = request(target, vec![], vec![], &[], &[], None);
        request.inbound.call.ordinary_clobbers = RegisterSet::default();
        request.internal.ordinary_clobbers = RegisterSet::default();
        let bridge = emit_native_entry_bridge(request).unwrap();
        let before_call = &bridge.bytes()[..bridge.call_field_offset() - 1];
        let after_call = &bridge.bytes()[bridge.call_field_offset() + 4..];
        let moves_r11 = |bytes: &[u8], opcode: u8| {
            bytes.windows(4).any(|window| {
                window[0] == 0x4c
                    && window[1] == opcode
                    && window[2] & 0x38 == 0x18
                    && window[2] & 0x07 == 0x04
                    && window[3] == 0x24
            })
        };
        assert!(moves_r11(before_call, 0x89));
        assert!(moves_r11(after_call, 0x8b));
        validate_native_entry_bridge(&bridge).unwrap();
    }

    #[test]
    fn different_conventions_transport_arguments_results_and_shadow_space() {
        let shapes = [
            vec![ValueShape::integer(8, 8); 7],
            vec![ValueShape::float(8); 7],
        ]
        .concat();
        for (internal_policy, inbound_policy) in [
            (CallingPolicy::SystemVAMD64, CallingPolicy::MicrosoftX64),
            (CallingPolicy::MicrosoftX64, CallingPolicy::SystemVAMD64),
        ] {
            let mut request = request(
                NativeTarget::windows_x64(),
                shapes.clone(),
                shapes.clone(),
                &(0..14).collect::<Vec<_>>(),
                &[0; 14],
                Some(ValueShape::float(8)),
            );
            let signature = CallSignature {
                parameters: shapes.clone(),
                result: Some(ValueShape::float(8)),
            };
            request.internal = evaluate_call_plan(internal_policy, &signature).unwrap();
            request.inbound = evaluate_ordinary_boundary_entry_plan(inbound_policy, &signature)
                .unwrap()
                .plan()
                .clone();
            validate_native_entry_bridge(&emit_native_entry_bridge(request).unwrap()).unwrap();
        }
    }

    #[test]
    fn word_permutations_cross_register_stack_and_shadow_areas() {
        for target in [
            NativeTarget::macos_arm64(),
            NativeTarget::linux_arm64(),
            NativeTarget::linux_x64(),
            NativeTarget::macos_x64(),
            NativeTarget::windows_x64(),
        ] {
            let bridge = emit_native_entry_bridge(request(
                target,
                vec![ValueShape::integer(8, 8); 9],
                vec![ValueShape::integer(8, 8); 9],
                &[8, 7, 6, 5, 4, 3, 2, 1, 0],
                &[0; 9],
                None,
            ))
            .unwrap();
            validate_native_entry_bridge(&bridge).unwrap();
            let bias = bridge.call_stack_bias();
            assert_eq!(
                bias,
                if target.architecture == Architecture::X86_64 {
                    8
                } else {
                    0
                }
            );
            assert_eq!((bridge.frame_bytes() + bias) % 16, 0);
            let offset = bridge.call_field_offset();
            match target.architecture {
                Architecture::Aarch64 => assert_eq!(
                    &bridge.bytes()[offset..offset + 4],
                    &isa_aarch64::ORDINARY_DIRECT_CALL_BYTES
                ),
                Architecture::X86_64 => assert_eq!(
                    &bridge.bytes()[offset - 1..offset + 4],
                    &isa_x86_64::ORDINARY_DIRECT_CALL_BYTES
                ),
            }
        }
    }

    #[test]
    fn descriptor_entry_retains_codec_and_parameter_permutation() {
        for target in [
            NativeTarget::macos_arm64(),
            NativeTarget::linux_x64(),
            NativeTarget::windows_x64(),
        ] {
            let bridge = emit_native_entry_bridge(request(
                target,
                vec![ValueShape::borrowed_scalar_reference(); 3],
                vec![ValueShape::integer(8, 8); 3],
                &[1, 0, 2],
                &[8, 258, 516],
                Some(ValueShape::integer(4, 4)),
            ))
            .unwrap();
            assert!(bridge.frame_bytes() >= 3 * 24);
            assert_eq!(
                bridge
                    .request
                    .parameters
                    .iter()
                    .map(|(_, parameter)| parameter.scalar_codec)
                    .collect::<Vec<_>>(),
                vec![8, 258, 516]
            );
            validate_native_entry_bridge(&bridge).unwrap();
        }
    }

    #[test]
    fn scalar_pointer_without_codec_does_not_become_a_descriptor() {
        let invalid = request(
            NativeTarget::macos_arm64(),
            vec![ValueShape::borrowed_scalar_reference()],
            vec![ValueShape::integer(8, 8)],
            &[0],
            &[0],
            None,
        );
        assert_eq!(
            emit_native_entry_bridge(invalid),
            Err(Error::MissingScalarCodec)
        );
    }

    #[test]
    fn duplicate_parameter_origin_is_not_a_parallel_move() {
        let invalid = request(
            NativeTarget::linux_x64(),
            vec![ValueShape::integer(8, 8); 2],
            vec![ValueShape::integer(8, 8); 2],
            &[0, 0],
            &[0; 2],
            None,
        );
        assert_eq!(
            emit_native_entry_bridge(invalid),
            Err(Error::InvalidParameterJoin)
        );
    }

    #[test]
    fn float_register_permutation_and_result_use_scalar_transfers() {
        for target in [
            NativeTarget::macos_arm64(),
            NativeTarget::linux_x64(),
            NativeTarget::windows_x64(),
        ] {
            let bridge = emit_native_entry_bridge(request(
                target,
                vec![ValueShape::float(4); 2],
                vec![ValueShape::float(4); 2],
                &[1, 0],
                &[0; 2],
                Some(ValueShape::float(4)),
            ))
            .unwrap();
            validate_native_entry_bridge(&bridge).unwrap();
        }
    }

    #[test]
    fn large_frame_uses_target_commit_probes_and_checked_addressing() {
        let mut request = request(
            NativeTarget::macos_arm64(),
            vec![ValueShape::integer(8, 8)],
            vec![ValueShape::integer(8, 8)],
            &[0],
            &[0],
            None,
        );
        request.internal.parameters[0].locations = vec![ValueLocation::Stack {
            stack_byte_offset: 65_536,
            value_byte_offset: 0,
            byte_size: 8,
            alignment: 8,
        }];
        let bridge = emit_native_entry_bridge(request).unwrap();
        assert!(bridge.frame_bytes() > 65_536);
        let touches = bridge
            .bytes()
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|word| **word == 0xf940_03ff_u32.to_le_bytes())
            .count();
        assert_eq!(touches, 5);
    }

    #[test]
    fn occupied_ip_registers_do_not_get_used_as_address_scratch() {
        let mut request = request(
            NativeTarget::macos_arm64(),
            vec![ValueShape::integer(8, 8); 2],
            vec![ValueShape::integer(8, 8); 2],
            &[1, 0],
            &[0; 2],
            None,
        );
        request.inbound.call.parameters[0].locations = vec![ValueLocation::Register {
            register: MachineRegister::Aarch64X(16),
            value_byte_offset: 0,
            byte_size: 8,
        }];
        request.inbound.call.parameters[1].locations = vec![ValueLocation::Register {
            register: MachineRegister::Aarch64X(17),
            value_byte_offset: 0,
            byte_size: 8,
        }];
        validate_native_entry_bridge(&emit_native_entry_bridge(request).unwrap()).unwrap();
    }

    #[test]
    fn aggregate_members_cross_integer_float_banks_in_both_directions() {
        for target in [NativeTarget::macos_arm64(), NativeTarget::linux_x64()] {
            for member_size in [4, 8] {
                let aggregate = ValueShape::homogeneous_float_aggregate(member_size, 2);
                let ordinary = ValueShape::integer(aggregate.byte_size, aggregate.alignment);
                let mut request = request(
                    target,
                    vec![ordinary],
                    vec![aggregate],
                    &[0],
                    &[0],
                    Some(ordinary),
                );
                request.inbound.call.result = evaluate_call_plan(
                    request.inbound.call.policy,
                    &CallSignature {
                        parameters: vec![aggregate],
                        result: Some(aggregate),
                    },
                )
                .unwrap()
                .result;
                let bridge = emit_native_entry_bridge(request).unwrap();
                validate_native_entry_bridge(&bridge).unwrap();
                assert!(bridge.frame_bytes() >= aggregate.byte_size * 2);
            }
        }
    }

    #[test]
    fn owned_indirect_arguments_and_results_keep_separate_frame_storage() {
        for target in [
            NativeTarget::macos_arm64(),
            NativeTarget::linux_arm64(),
            NativeTarget::linux_x64(),
            NativeTarget::macos_x64(),
            NativeTarget::windows_x64(),
        ] {
            let shape = ValueShape::integer(24, 8);
            let bridge = emit_native_entry_bridge(request(
                target,
                vec![shape; 9],
                vec![shape; 9],
                &[8, 7, 6, 5, 4, 3, 2, 1, 0],
                &[0; 9],
                Some(shape),
            ))
            .unwrap();
            validate_native_entry_bridge(&bridge).unwrap();
            assert!(bridge.frame_bytes() >= 10 * shape.byte_size);
        }
    }

    #[test]
    fn direct_float_entry_can_call_indirect_body_and_return_directly() {
        for target in [NativeTarget::macos_arm64(), NativeTarget::linux_arm64()] {
            let aggregate = ValueShape::homogeneous_float_aggregate(8, 4);
            let ordinary = ValueShape::integer(32, 8);
            let mut request = request(
                target,
                vec![ordinary],
                vec![aggregate],
                &[0],
                &[0],
                Some(ordinary),
            );
            request.inbound.call.result = evaluate_call_plan(
                request.inbound.call.policy,
                &CallSignature {
                    parameters: vec![aggregate],
                    result: Some(aggregate),
                },
            )
            .unwrap()
            .result;
            validate_native_entry_bridge(&emit_native_entry_bridge(request).unwrap()).unwrap();
        }
    }

    #[test]
    fn stack_aggregate_is_partitioned_without_widening_its_tail() {
        let placement = ValuePlacement {
            shape: ValueShape::integer(13, 1),
            locations: vec![ValueLocation::Stack {
                stack_byte_offset: 32,
                value_byte_offset: 0,
                byte_size: 13,
                alignment: 1,
            }],
        };
        let pieces = value_pieces(&placement, 13).unwrap();
        assert_eq!(
            pieces
                .iter()
                .map(|piece| (piece.value_byte_offset, piece.byte_size))
                .collect::<Vec<_>>(),
            vec![(0, 8), (8, 4), (12, 1)]
        );
    }

    #[test]
    fn x64_owned_homes_align_inside_fixed_bridge_frames() {
        for target in [NativeTarget::linux_x64(), NativeTarget::windows_x64()] {
            let bridge = emit_native_entry_bridge(request(
                target,
                vec![ValueShape::integer(32, 32), ValueShape::integer(64, 64)],
                vec![ValueShape::integer(32, 32), ValueShape::integer(64, 64)],
                &[0, 1],
                &[0, 0],
                Some(ValueShape::integer(64, 64)),
            ))
            .unwrap();
            assert_eq!(bridge.frame_bytes() % 16, 8);
            assert!(bridge.frame_bytes() > 128);
            assert!(
                bridge
                    .bytes()
                    .windows(4)
                    .any(|word| word == [0xc0, 0xff, 0xff, 0xff])
            );
            validate_native_entry_bridge(&bridge).unwrap();
            let mut changed = bridge.clone();
            let mask = changed
                .bytes
                .windows(4)
                .position(|word| word == [0xc0, 0xff, 0xff, 0xff])
                .unwrap();
            changed.bytes[mask] ^= 1;
            assert!(validate_native_entry_bridge(&changed).is_err());
        }
    }

    #[test]
    fn missing_or_overlapping_member_does_not_cover_the_value_extent() {
        let mut placement = evaluate_call_plan(
            CallingPolicy::Aapcs64,
            &CallSignature {
                parameters: vec![ValueShape::homogeneous_float_aggregate(8, 2)],
                result: None,
            },
        )
        .unwrap()
        .parameters
        .remove(0);
        placement.locations.pop();
        assert!(matches!(
            value_pieces(&placement, 16).map_err(Error::from),
            Err(Error::UnsupportedValueTransport)
        ));
        placement.locations.push(placement.locations[0]);
        assert!(matches!(
            value_pieces(&placement, 16).map_err(Error::from),
            Err(Error::UnsupportedValueTransport)
        ));
    }
}
