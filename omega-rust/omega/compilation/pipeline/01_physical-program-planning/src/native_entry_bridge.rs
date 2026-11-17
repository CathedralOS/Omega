//! Plan the physical adapter from the external arrival ABI to the body ABI.
use arena::Arena;
use calling_conventions::{
    BoundaryEntryPlan, CallPlan, EntryControl, EntryStack, MachineRegister, MachineState,
    MachineStateSet, Preemption, RegisterSet, StateEffectsEvidence, ValueClass, ValuePlacement,
    ValueShape,
};
use post_allocation_machine::adapters::{
    AdapterInstructions, AdapterOperation as Op, X86AdapterTemplate as Template,
};
use register_model::validate_physical_register_model;
pub use runtime_abi::static_interrupts::{InstalledInterruptEntryPlan, InstalledInterruptFrame};
use runtime_abi::static_interrupts::{InstalledInterruptProtocol, acknowledgement as ack, root};
use target::{Architecture, NativeTarget};

use crate::bridge_values::{
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
pub struct NativeEntryBridgePlan {
    request: NativeEntryBridgeRequest,
    instructions: AdapterInstructions,
    effects: StateEffectsEvidence,
    complete_effects: StateEffectsEvidence,
    frame_bytes: u32,
    installed_interrupt: Option<InstalledInterruptEntryPlan>,
}

impl NativeEntryBridgePlan {
    pub fn instructions(&self) -> &AdapterInstructions {
        &self.instructions
    }
    pub fn into_contract(
        self,
    ) -> (
        NativeEntryBridgeRequest,
        StateEffectsEvidence,
        StateEffectsEvidence,
    ) {
        (self.request, self.effects, self.complete_effects)
    }

    pub fn effects(&self) -> &StateEffectsEvidence {
        &self.effects
    }
    pub fn complete_effects(&self) -> &StateEffectsEvidence {
        &self.complete_effects
    }
    pub fn request(&self) -> &NativeEntryBridgeRequest {
        &self.request
    }
    pub fn frame_bytes(&self) -> u32 {
        self.frame_bytes
    }
    pub fn call_stack_bias(&self) -> u32 {
        call_stack_bias(self.request.target.architecture)
    }
    pub fn installed_interrupt(&self) -> Option<InstalledInterruptEntryPlan> {
        self.installed_interrupt
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
    instructions: AdapterInstructions,
    frame_bytes: u32,
    installed_interrupt: Option<InstalledInterruptEntryPlan>,
) -> Result<NativeEntryBridgePlan, Error> {
    let effects = instructions.effects();
    let complete_effects = request.checked_complete_effects(&effects)?;
    Ok(NativeEntryBridgePlan {
        request,
        instructions,
        effects,
        complete_effects,
        frame_bytes,
        installed_interrupt,
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
    before_body: Option<Op>,
    return_operation: Op,
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
            before_body: None,
            return_operation: Op::Return,
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
        before_body: Some(Op::ClearDirection),
        return_operation: Op::InterruptReturn,
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

pub fn call_stack_bias(architecture: Architecture) -> u32 {
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

pub fn plan_native_entry_bridge(
    request: NativeEntryBridgeRequest,
) -> Result<NativeEntryBridgePlan, Error> {
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
    let interval = frame_admission::frame_layout::stack_commit_granule_bytes(request.target)
        .ok_or(Error::IncompatiblePlan)?;
    let touches = if u64::from(frame_bytes) > interval {
        u32::try_from(u64::from(frame_bytes).div_ceil(interval))
            .map_err(|_| Error::FrameOverflow)?
    } else {
        0
    };
    let mut bytes = AdapterInstructions::new(request.target.architecture);
    bytes.frame(frame_bytes, interval, touches, true);
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
    if let Some(operation) = protocol.before_body {
        bytes.push(operation);
    }
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
    bytes.push(Op::Call);
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
    bytes.frame(frame_bytes, interval, touches, false);
    bytes.push(protocol.return_operation);
    finish_native_entry(request, bytes, frame_bytes, None)
}

pub fn plan_installed_interrupt_entry_bridge(
    request: NativeEntryBridgeRequest,
    plan: InstalledInterruptEntryPlan,
) -> Result<NativeEntryBridgePlan, NativeEntryBridgeError> {
    if !plan.has_canonical_geometry() {
        return Err(Error::InvalidInstalledInterruptGeometry);
    }
    if matches!(
        plan.protocol,
        InstalledInterruptProtocol::FatalContext { .. }
    ) {
        return plan_installed_fatal_entry_bridge(request, plan);
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
    let interval = frame_admission::frame_layout::stack_commit_granule_bytes(request.target)
        .ok_or(Error::IncompatiblePlan)?;
    let touches = if u64::from(local_bytes) > interval {
        u32::try_from(u64::from(local_bytes).div_ceil(interval))
            .map_err(|_| Error::FrameOverflow)?
    } else {
        0
    };
    let mut bytes = AdapterInstructions::new(Architecture::X86_64);
    let gap = u8::try_from(plan.root_slot_from_top - plan.hardware_frame_bytes())
        .map_err(|_| Error::InvalidInstalledInterruptGeometry)?;
    bytes.push(Op::X86AdjustStack {
        amount: gap,
        subtract: true,
    });
    for register in SAVED_GPRS {
        push_or_pop(&mut bytes, register, false);
    }
    if let Some(operation) = protocol.before_body {
        bytes.push(operation);
    }
    bytes.frame(local_bytes, interval, touches, true);
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
    bytes.push(Op::Call);
    encoding.memory(&mut bytes, address_scratch, saved_root, 8, true)?;
    encoding.pointer_memory(&mut bytes, root::DEBT, 8, true)?;
    require_scratch_equal(&mut bytes, scratch, 0);
    bytes.frame(local_bytes, interval, touches, false);
    for register in SAVED_GPRS.into_iter().rev() {
        push_or_pop(&mut bytes, register, true);
    }
    bytes.push(Op::X86AdjustStack {
        amount: 16,
        subtract: false,
    });
    bytes.push(protocol.return_operation);
    finish_native_entry(request, bytes, frame_bytes, Some(plan))
}

fn plan_installed_fatal_entry_bridge(
    request: NativeEntryBridgeRequest,
    plan: InstalledInterruptEntryPlan,
) -> Result<NativeEntryBridgePlan, Error> {
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
    let interval = frame_admission::frame_layout::stack_commit_granule_bytes(request.target)
        .ok_or(Error::IncompatiblePlan)?;
    let touches = if u64::from(local_bytes) > interval {
        u32::try_from(u64::from(local_bytes).div_ceil(interval))
            .map_err(|_| Error::FrameOverflow)?
    } else {
        0
    };
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
    let mut bytes = AdapterInstructions::new(Architecture::X86_64);
    for register in SAVED_GPRS {
        push_or_pop(&mut bytes, register, false);
    }
    bytes.push(Op::ClearDirection);
    bytes.push(Op::X86Fixed(Template::AlignStack));
    bytes.frame(local_bytes, interval, touches, true);
    descriptor_store(&mut bytes, idtr, 1);
    descriptor_store(&mut bytes, gdtr, 0);
    encoding.memory(&mut bytes, MachineRegister::X86Rax, idtr, 2, true)?;
    compare_eax(&mut bytes, table_bytes - 1);
    require(&mut bytes, 0x74);
    encoding.memory(&mut bytes, MachineRegister::X86Rdx, idtr + 2, 8, true)?;
    encoding.pointer_memory(&mut bytes, anchor_gate_byte_offset + 4, 1, true)?;
    bytes.push(Op::X86Fixed(Template::MaskAndTestIst));
    require(&mut bytes, 0x75);
    encoding.memory(&mut bytes, MachineRegister::X86Rax, ist, 8, false)?;
    encoding.pointer_memory(&mut bytes, anchor_gate_byte_offset + 8, 4, true)?;
    bytes.push(Op::X86Fixed(Template::ShiftHighGateAddress));
    encoding.pointer_memory(&mut bytes, anchor_gate_byte_offset + 6, 2, true)?;
    bytes.push(Op::X86Fixed(Template::MergeMiddleGateAddress));
    encoding.pointer_memory(&mut bytes, anchor_gate_byte_offset, 2, true)?;
    bytes.push(Op::X86Fixed(Template::MergeLowGateAddress));
    encoding.memory(&mut bytes, MachineRegister::X86Rax, provider, 8, false)?;
    bytes.push(Op::X86Fixed(Template::ReadTaskSelector));
    bytes.push(Op::X86Fixed(Template::TestSelectorAlignment));
    require(&mut bytes, 0x74);
    bytes.push(Op::X86Fixed(Template::TestSelectorNonzero));
    require(&mut bytes, 0x75);
    bytes.push(Op::X86Fixed(Template::SelectorLimit));
    encoding.memory(&mut bytes, MachineRegister::X86Rdx, gdtr, 2, true)?;
    bytes.push(Op::X86Fixed(Template::CompareDescriptorLimit));
    require(&mut bytes, 0x76);
    encoding.memory(&mut bytes, MachineRegister::X86Rdx, gdtr + 2, 8, true)?;
    bytes.push(Op::X86Fixed(Template::AddDescriptorBase));
    encoding.pointer_memory(&mut bytes, 4, 4, true)?;
    bytes.push(Op::X86Fixed(Template::HighDescriptorBase));
    encoding.pointer_memory(&mut bytes, 2, 2, true)?;
    bytes.push(Op::X86Fixed(Template::MergeDescriptorBase));
    encoding.pointer_memory(&mut bytes, 4, 1, true)?;
    bytes.push(Op::X86Fixed(Template::MergeMiddleGateAddress));
    encoding.pointer_memory(&mut bytes, 8, 4, true)?;
    bytes.push(Op::X86Fixed(Template::FinishDescriptorBase));
    encoding.memory(&mut bytes, MachineRegister::X86Rcx, ist, 8, true)?;
    bytes.push(Op::X86Fixed(Template::LoadIstTop));
    bytes.push(Op::X86Fixed(Template::LoadIstRoot));
    encoding.pointer_memory(&mut bytes, root::STATE, 8, true)?;
    bytes.push(Op::X86Fixed(Template::CompareActiveRoot));
    require(&mut bytes, 0x74);
    encoding.pointer_memory(&mut bytes, root::PROVIDER, 8, true)?;
    encoding.memory(&mut bytes, MachineRegister::X86Rcx, provider, 8, true)?;
    bytes.push(Op::X86Fixed(Template::CompareProvider));
    require(&mut bytes, 0x74);
    encoding.pointer_memory(&mut bytes, root::CONTEXT, 8, true)?;
    encoding.memory(&mut bytes, MachineRegister::X86Rax, context, 8, false)?;
    encoding.distribute_argument(&mut bytes, &transport.destination, context, 8)?;
    bytes.push(Op::Call);
    halt(&mut bytes);
    finish_native_entry(request, bytes, frame_bytes, Some(plan))
}

fn push_or_pop(bytes: &mut AdapterInstructions, register: u8, pop: bool) {
    bytes.push(Op::X86PushPop { register, pop });
}

fn require_scratch_equal(bytes: &mut AdapterInstructions, register: MachineRegister, value: u8) {
    bytes.push(Op::X86RequireEqual { register, value });
}

fn add_one_without_overflow(bytes: &mut AdapterInstructions, register: MachineRegister) {
    bytes.push(Op::X86IncrementChecked { register });
}

fn descriptor_store(bytes: &mut AdapterInstructions, offset: u32, operation: u8) {
    bytes.push(Op::X86DescriptorStore { offset, operation });
}

fn compare_eax(bytes: &mut AdapterInstructions, value: u32) {
    bytes.push(Op::X86CompareEax { value });
}

fn require(bytes: &mut AdapterInstructions, condition: u8) {
    bytes.push(Op::X86Require { condition });
}

fn halt(bytes: &mut AdapterInstructions) {
    bytes.push(Op::X86Stop);
}

#[cfg(test)]
mod value_tests {
    use super::*;
    use crate::bridge_values::value_pieces;
    use calling_conventions::{CallSignature, CallingPolicy, ValueLocation, evaluate_call_plan};
    #[test]
    fn darwin_record_uses_its_inbound_stack_slot_and_internal_scalar_slot() {
        let policy = CallingPolicy::native_for_target(NativeTarget::macos_arm64());
        let mut parameters = vec![ValueShape::integer(8, 8); 8];
        parameters.extend([ValueShape::integer(1, 1), ValueShape::integer(4, 2), ValueShape::integer(1, 1)]);
        let internal = evaluate_call_plan(policy, &CallSignature { parameters: parameters.clone(), result: None }).unwrap();
        parameters[9] = ValueShape::integer_aggregate(4, 2);
        let inbound = calling_conventions::evaluate_ordinary_boundary_entry_plan(policy, &CallSignature { parameters, result: None }).unwrap();
        let transport = value_transport(&inbound.plan().call.parameters[9], &internal.parameters[9], 0).unwrap();
        assert_eq!(transport.byte_size, 4);
        assert!(matches!(transport.source.pieces()[0].location, WordLocation::Stack(8)));
        assert!(matches!(transport.destination.pieces()[0].location, WordLocation::Stack(2)));
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
