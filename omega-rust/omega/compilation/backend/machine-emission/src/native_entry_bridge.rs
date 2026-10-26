//! Physical native entry transport, separate from an ordinary semantic body.
//!
//! Incoming values are snapshotted before any destination register is written:
//! nominal parameter permutations are parallel moves, not a sequence of copies.
//! A scalar loan constructs address/codec storage in this activation and passes
//! its address to the unchanged internal convention. The codec interprets bytes;
//! it supplies neither borrow permission nor referent extent. Admission of the
//! exact callback signature remains with the realization owner.
//! Owned indirect values copy their complete bytes into this frame; their ABI
//! pointer never becomes a loan. The inbound caller's result pointer is saved
//! before argument redistribution, separately from the internal result storage.
//! Over-aligned owned homes reserve padding and align their absolute addresses
//! inside the fixed ABI frame. Every snapshot access uses that same locator;
//! offset alignment alone cannot establish a value's promised alignment.
//! A differing clobber promise saves the internal call's additional writes;
//! vector saves retain all 128 bits, not just the scalar transport lane. Published
//! direct result registers are outputs, not incoming state to restore over them.
//!
//! This is a physical adapter, not a Terminal machine or another semantic stage.
//! Its one unresolved ordinary call targets the real body entry symbol. Body
//! helpers and recursion retain their ordinary convention and artifact namespace.
//! The retained request is replayed by artifact readers, off the production path.

use arena::Arena;
use calling_conventions::{
    CallPlan, EntryControl, MachineRegister, RegisterSet, ValueClass, ValuePlacement, ValueShape,
};
use register_model::validate_physical_register_model;
use target::{Architecture, NativeTarget};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NativeEntryParameter {
    pub inbound_ordinal: u32,
    /// Zero means no descriptor conversion. Nonzero is a closed scalar codec.
    pub scalar_codec: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeEntryBridgeRequest {
    pub target: NativeTarget,
    pub internal: CallPlan,
    pub inbound: CallPlan,
    /// Internal scalar-then-structural order, joined through nominal origins.
    pub parameters: Arena<NativeEntryParameter>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeEntryBridge {
    request: NativeEntryBridgeRequest,
    bytes: Vec<u8>,
    frame_bytes: u32,
    call_field_offset: usize,
}

impl NativeEntryBridge {
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
        if self.request.target.architecture == Architecture::X86_64 {
            8
        } else {
            0
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeEntryBridgeError {
    IncompatiblePlan,
    InvalidParameterJoin,
    UnsupportedValueTransport,
    MissingScalarCodec,
    NoUnoccupiedScratch,
    FrameOverflow,
    Encoding,
    ReplayMismatch,
}
diagnostics::failed_display!(NativeEntryBridgeError, "native entry bridge failed");
impl std::error::Error for NativeEntryBridgeError {}
type Error = NativeEntryBridgeError;

use crate::native_bridge_encoding::{
    AlignedHome, Encoding, NativeBridgeEncodingError, ValueStorage, WordLocation, allocate_home,
    allocate_value, allocate_word, is_borrowed, value_storage,
};

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

/// Build an adapter from two already admitted calling plans. Owned ABI pointers
/// name complete byte extents, not borrows. Non-call entry regimes reject;
/// equal thin-pointer geometry never admits a scalar loan.
pub fn emit_native_entry_bridge(
    request: NativeEntryBridgeRequest,
) -> Result<NativeEntryBridge, Error> {
    let internal = &request.internal;
    let inbound = &request.inbound;
    if internal.policy.architecture() != request.target.architecture
        || inbound.policy.architecture() != request.target.architecture
        || internal.entry_control != EntryControl::CallReturn
        || inbound.entry_control != EntryControl::CallReturn
        || internal.stack_alignment != 16
        // A stronger incoming alignment still guarantees the bridge's
        // 16-byte frame base. Incoming stack offsets are entry-relative;
        // over-aligned owned homes are aligned independently below.
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
                // A stack return is a caller-owned result area, not this
                // activation's argument area. Keep its distinct protocol open.
                return Err(Error::UnsupportedValueTransport);
            }
            Some(transport)
        }
        _ => return Err(Error::UnsupportedValueTransport),
    };
    // Every piece contributes to scratch exclusion, including fragmented
    // results. Save all pieces before redistribution, so cycles across integer
    // and floating banks cannot overwrite another member.
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
    // Scratch registers are the bridge's own writes. Prefer those the inbound
    // promise already lets this entry clobber. A promise that preserves every
    // candidate still leaves an unoccupied one usable: it joins the saved set
    // below, so its caller value is stored before first use and restored last.
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
    // Argument redistribution is a write too, even when a custom internal
    // promise omits that register from its call clobbers. Hidden result pointers
    // are inputs; only direct result registers (and x64's result-pointer echo)
    // supersede the caller's old register value.
    let internal_writes = RegisterSet::new(
        internal
            .ordinary_clobbers
            .as_slice()
            .iter()
            .copied()
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
            !inbound.ordinary_clobbers.contains(*register) && !result_output(*register)
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
        .and_then(|value| {
            value.checked_add(if request.target.architecture == Architecture::X86_64 {
                8
            } else {
                0
            })
        })
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
    // Capture registers before any scratch-based stack transfer.
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
        .checked_add(if request.target.architecture == Architecture::X86_64 {
            8
        } else {
            0
        })
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
                    // SysV and Microsoft aggregate returns echo the original
                    // caller's destination in RAX, not the body's local area.
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
    match request.target.architecture {
        Architecture::Aarch64 => bytes.extend_from_slice(&isa_aarch64::ORDINARY_RETURN_BYTES),
        Architecture::X86_64 => bytes.extend_from_slice(&isa_x86_64::ORDINARY_RETURN_BYTES),
    }
    Ok(NativeEntryBridge {
        request,
        bytes,
        frame_bytes,
        call_field_offset,
    })
}

/// Replay physical bytes and frame geometry at an independent artifact boundary.
pub fn validate_native_entry_bridge(bridge: &NativeEntryBridge) -> Result<(), Error> {
    if emit_native_entry_bridge(bridge.request.clone())? != *bridge {
        return Err(Error::ReplayMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_bridge_encoding::value_pieces;
    use calling_conventions::{CallSignature, CallingPolicy, ValueLocation, evaluate_call_plan};

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
        let inbound = evaluate_call_plan(
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
            inbound,
            parameters,
        }
    }

    #[test]
    fn narrower_inbound_clobbers_save_integer_and_full_vector_state() {
        let mut request = request(NativeTarget::macos_arm64(), vec![], vec![], &[], &[], None);
        request.inbound.ordinary_clobbers = RegisterSet::new(
            request
                .internal
                .ordinary_clobbers
                .as_slice()
                .iter()
                .copied()
                .filter(|register| {
                    !matches!(
                        register,
                        MachineRegister::Aarch64X(9) | MachineRegister::Aarch64V(16)
                    )
                }),
        );
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
    fn returned_register_is_not_restored_over_the_result() {
        let mut request = request(
            NativeTarget::macos_arm64(),
            vec![],
            vec![],
            &[],
            &[],
            Some(ValueShape::integer(8, 8)),
        );
        request.inbound.ordinary_clobbers = RegisterSet::new(
            request
                .internal
                .ordinary_clobbers
                .as_slice()
                .iter()
                .copied()
                .filter(|register| *register != MachineRegister::Aarch64X(0)),
        );
        // Only result scratch and LR: preserving old X0 would allocate another
        // slot and, worse, overwrite the published result after distribution.
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
        request.inbound.ordinary_clobbers = RegisterSet::new(
            request
                .internal
                .ordinary_clobbers
                .as_slice()
                .iter()
                .copied()
                .filter(|register| *register != MachineRegister::Aarch64X(8)),
        );
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
        // XMM6 through XMM15 preserve both halves, never scalar MOVSD.
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
        // Neither promise clobbers anything, so only the bridge's own scratch
        // use can put a register in the saved set.
        request.inbound.ordinary_clobbers = RegisterSet::default();
        request.internal.ordinary_clobbers = RegisterSet::default();
        let bridge = emit_native_entry_bridge(request).unwrap();
        let before_call = &bridge.bytes()[..bridge.call_field_offset() - 1];
        let after_call = &bridge.bytes()[bridge.call_field_offset() + 4..];
        // R11 is the first scratch candidate. Like every promise-excluded
        // write it is stored to the frame before first use and loaded after
        // the call: REX.WR, MOV r/m64 (0x89 store, 0x8b load), ModRM reg=R11
        // with an RSP-based SIB address at any displacement.
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
            request.inbound = evaluate_call_plan(inbound_policy, &signature).unwrap();
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
        request.inbound.parameters[0].locations = vec![ValueLocation::Register {
            register: MachineRegister::Aarch64X(16),
            value_byte_offset: 0,
            byte_size: 8,
        }];
        request.inbound.parameters[1].locations = vec![ValueLocation::Register {
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
                request.inbound.result = evaluate_call_plan(
                    request.inbound.policy,
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
            request.inbound.result = evaluate_call_plan(
                request.inbound.policy,
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
