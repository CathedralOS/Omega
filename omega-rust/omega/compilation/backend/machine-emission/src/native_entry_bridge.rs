//! Physical native entry transport, separate from an ordinary semantic body.
//!
//! Incoming values are snapshotted before any destination register is written:
//! nominal parameter permutations are parallel moves, not a sequence of copies.
//! A scalar loan constructs address/codec storage in this activation and passes
//! its address to the unchanged internal convention. The codec interprets bytes;
//! it supplies neither borrow permission nor referent extent. Admission of the
//! exact callback signature remains with the realization owner.
//!
//! This is a physical adapter, not a Terminal machine or another semantic stage.
//! Its one unresolved ordinary call targets the real body entry symbol. Body
//! helpers and recursion retain their ordinary convention and artifact namespace.
//! The retained request is replayed by artifact readers, off the production path.

use arena::Arena;
use calling_conventions::{
    CallPlan, EntryControl, IndirectPointerLocation, MachineRegister, ValueClass, ValueLocation,
    ValuePlacement, ValueShape,
};
use register_model::{
    RegisterViewId, ValidatedPhysicalRegisterModel, validate_physical_register_model,
};
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
    NoVolatileScratch,
    FrameOverflow,
    Encoding,
    ReplayMismatch,
}
diagnostics::failed_display!(NativeEntryBridgeError, "native entry bridge failed");
impl std::error::Error for NativeEntryBridgeError {}
type Error = NativeEntryBridgeError;

#[derive(Clone, Copy)]
enum WordLocation {
    Register(MachineRegister),
    Stack(u32),
}

fn word_location(placement: &ValuePlacement) -> Result<WordLocation, Error> {
    match placement.locations.as_slice() {
        [
            ValueLocation::Register {
                register,
                value_byte_offset: 0,
                byte_size: 1 | 2 | 4 | 8,
            },
        ] => Ok(WordLocation::Register(*register)),
        [
            ValueLocation::Stack {
                stack_byte_offset,
                value_byte_offset: 0,
                byte_size: 1 | 2 | 4 | 8,
                ..
            },
        ] => Ok(WordLocation::Stack(*stack_byte_offset)),
        [
            ValueLocation::Indirect {
                pointer,
                copy_stack_byte_offset: None,
                ..
            },
        ] if matches!(
            placement.shape.class,
            ValueClass::BorrowedReference | ValueClass::BorrowedScalarReference
        ) =>
        {
            Ok(match pointer {
                IndirectPointerLocation::Register(register) => WordLocation::Register(*register),
                IndirectPointerLocation::Stack {
                    stack_byte_offset, ..
                } => WordLocation::Stack(*stack_byte_offset),
            })
        }
        _ => Err(Error::UnsupportedValueTransport),
    }
}

fn word_size(placement: &ValuePlacement) -> Result<u8, Error> {
    if matches!(
        placement.shape.class,
        ValueClass::BorrowedReference | ValueClass::BorrowedScalarReference
    ) {
        return Ok(8);
    }
    u8::try_from(placement.shape.byte_size)
        .ok()
        .filter(|size| matches!(size, 1 | 2 | 4 | 8))
        .ok_or(Error::UnsupportedValueTransport)
}

fn allocate_word(cursor: &mut u32) -> Result<u32, Error> {
    *cursor = cursor
        .checked_add(7)
        .map(|value| value & !7)
        .ok_or(Error::FrameOverflow)?;
    let offset = *cursor;
    *cursor = cursor.checked_add(8).ok_or(Error::FrameOverflow)?;
    Ok(offset)
}

struct Encoding {
    architecture: Architecture,
    model: ValidatedPhysicalRegisterModel,
    scratch: MachineRegister,
    address_scratch: MachineRegister,
}

impl Encoding {
    fn view(&self, register: MachineRegister) -> Result<RegisterViewId, Error> {
        let name = match register {
            MachineRegister::Aarch64X(number) if number < 31 => format!("x{number}"),
            MachineRegister::Aarch64V(number) if number < 32 => format!("d{number}"),
            MachineRegister::X86Xmm(number) if number < 16 => format!("xmm{number}"),
            register if register.architecture() == Architecture::X86_64 => {
                let names = [
                    "rax", "rcx", "rdx", "rbx", "rsp", "rbp", "rsi", "rdi", "r8", "r9", "r10",
                    "r11", "r12", "r13", "r14", "r15",
                ];
                let name = names
                    .get(usize::from(register.codec_index()))
                    .ok_or(Error::Encoding)?;
                if *name == "rsp" {
                    return Err(Error::Encoding);
                }
                (*name).to_owned()
            }
            _ => return Err(Error::Encoding),
        };
        self.model
            .model()
            .view_named(&name)
            .map(|view| view.id)
            .ok_or(Error::Encoding)
    }

    fn memory(
        &self,
        bytes: &mut Vec<u8>,
        register: MachineRegister,
        offset: u32,
        size: u8,
        load: bool,
    ) -> Result<(), Error> {
        let view = self.view(register)?;
        let encoded = match self.architecture {
            Architecture::Aarch64 => isa_aarch64::encode_aarch64_frame_memory(
                &self.model,
                view,
                offset,
                size,
                load,
                self.view(self.address_scratch)?,
            )
            .map_err(|_| Error::Encoding)?,
            Architecture::X86_64 => {
                isa_x86_64::encode_x86_64_frame_memory(&self.model, view, offset, size, load)
                    .map_err(|_| Error::Encoding)?
            }
        };
        bytes.extend_from_slice(&encoded);
        Ok(())
    }

    fn address(&self, bytes: &mut Vec<u8>, offset: u32) -> Result<(), Error> {
        let view = self.view(self.scratch)?;
        let encoded = match self.architecture {
            Architecture::Aarch64 => {
                isa_aarch64::encode_aarch64_frame_address(&self.model, view, offset)
                    .map_err(|_| Error::Encoding)?
            }
            Architecture::X86_64 => {
                isa_x86_64::encode_x86_64_frame_address(&self.model, view, offset)
                    .map_err(|_| Error::Encoding)?
            }
        };
        bytes.extend_from_slice(&encoded);
        Ok(())
    }

    fn constant(&self, bytes: &mut Vec<u8>, value: u64) -> Result<(), Error> {
        let view = self.view(self.scratch)?;
        let encoded = match self.architecture {
            Architecture::Aarch64 => {
                isa_aarch64::encode_aarch64_integer_constant(&self.model, view, value)
                    .map_err(|_| Error::Encoding)?
            }
            Architecture::X86_64 => {
                isa_x86_64::encode_x86_64_integer_constant(&self.model, view, value)
                    .map_err(|_| Error::Encoding)?
            }
        };
        bytes.extend_from_slice(&encoded);
        Ok(())
    }

    fn copy_word(
        &self,
        bytes: &mut Vec<u8>,
        source: u32,
        destination: u32,
        size: u8,
    ) -> Result<(), Error> {
        self.memory(bytes, self.scratch, source, size, true)?;
        self.memory(bytes, self.scratch, destination, size, false)
    }
}

/// Build an adapter from two already admitted calling plans. Unsupported owned
/// aggregates and non-call entry regimes reject, rather than borrowing/copying
/// storage by inference. Equal thin-pointer geometry never admits a scalar loan.
pub fn emit_native_entry_bridge(
    request: NativeEntryBridgeRequest,
) -> Result<NativeEntryBridge, Error> {
    let internal = &request.internal;
    let inbound = &request.inbound;
    if internal.policy != inbound.policy
        || internal.policy.architecture() != request.target.architecture
        || internal.entry_control != EntryControl::CallReturn
        || inbound.entry_control != EntryControl::CallReturn
        || internal.stack_alignment != 16
        || inbound.stack_alignment != 16
        || internal.ordinary_clobbers != inbound.ordinary_clobbers
        || !internal.callback_materializations.is_empty()
        || !inbound.callback_materializations.is_empty()
        || internal.parameters.len() != inbound.parameters.len()
        || request.parameters.len() != internal.parameters.len()
    {
        return Err(Error::IncompatiblePlan);
    }
    let mut parameter_pairs = Vec::with_capacity(internal.parameters.len());
    let mut used_inbound = Vec::with_capacity(inbound.parameters.len());
    let mut cursor = u32::from(internal.shadow_bytes);
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
        match destination.shape.class {
            ValueClass::Integer | ValueClass::Float
                if destination.shape == source.shape && parameter.scalar_codec == 0 => {}
            ValueClass::BorrowedReference
                if source.shape == ValueShape::integer(8, 8) && parameter.scalar_codec == 0 => {}
            ValueClass::BorrowedScalarReference
                if source.shape == ValueShape::integer(8, 8)
                    && runtime_abi::ScalarStorageCodec::decode(parameter.scalar_codec)
                        .is_some() => {}
            ValueClass::BorrowedScalarReference => return Err(Error::MissingScalarCodec),
            _ => return Err(Error::UnsupportedValueTransport),
        }
        let source_location = word_location(source)?;
        let destination_location = word_location(destination)?;
        if let WordLocation::Stack(offset) = destination_location {
            cursor = cursor.max(
                offset
                    .checked_add(u32::from(word_size(destination)?))
                    .ok_or(Error::FrameOverflow)?,
            );
        }
        parameter_pairs.push((
            source_location,
            destination_location,
            word_size(source)?,
            word_size(destination)?,
            parameter.scalar_codec,
        ));
    }
    let result = match (&internal.result, &inbound.result) {
        (None, None) => None,
        (Some(source), Some(destination))
            if source.shape == destination.shape
                && matches!(source.shape.class, ValueClass::Integer | ValueClass::Float) =>
        {
            let (
                WordLocation::Register(source_register),
                WordLocation::Register(destination_register),
            ) = (word_location(source)?, word_location(destination)?)
            else {
                return Err(Error::UnsupportedValueTransport);
            };
            Some((source_register, destination_register, word_size(source)?))
        }
        _ => return Err(Error::UnsupportedValueTransport),
    };
    // Scratch never aliases an argument or result on either side. This also
    // protects inputs while large SP-relative transfers form their addresses.
    let occupied = |register| {
        parameter_pairs.iter().any(|(source, destination, ..)| matches!(source, WordLocation::Register(candidate) if *candidate == register) || matches!(destination, WordLocation::Register(candidate) if *candidate == register))
        || result.is_some_and(|(source, destination, _)| source == register || destination == register)
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
    let mut scratch = candidates
        .iter()
        .copied()
        .filter(|register| inbound.ordinary_clobbers.contains(*register) && !occupied(*register));
    let scratch_register = scratch.next().ok_or(Error::NoVolatileScratch)?;
    let address_scratch = if request.target.architecture == Architecture::Aarch64 {
        scratch.next().ok_or(Error::NoVolatileScratch)?
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
    let encoding = Encoding {
        architecture: request.target.architecture,
        model,
        scratch: scratch_register,
        address_scratch,
    };
    let mut snapshots = Vec::with_capacity(parameter_pairs.len());
    for (_, _, _, _, codec) in &parameter_pairs {
        let snapshot = allocate_word(&mut cursor)?;
        let descriptor = if *codec != 0 {
            let address = allocate_word(&mut cursor)?;
            allocate_word(&mut cursor)?;
            Some(address)
        } else {
            None
        };
        snapshots.push((snapshot, descriptor));
    }
    let result_snapshot = if result.is_some() {
        Some(allocate_word(&mut cursor)?)
    } else {
        None
    };
    let continuation = if request.target.architecture == Architecture::Aarch64 {
        Some(allocate_word(&mut cursor)?)
    } else {
        None
    };
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
    // Save register inputs first. Stack copies use only unoccupied scratches.
    for ((source, _, _, _, _), (snapshot, _)) in parameter_pairs.iter().zip(&snapshots) {
        if let WordLocation::Register(register) = source {
            encoding.memory(&mut bytes, *register, *snapshot, 8, false)?;
        }
    }
    let incoming_bias = frame_bytes
        .checked_add(if request.target.architecture == Architecture::X86_64 {
            8
        } else {
            0
        })
        .ok_or(Error::FrameOverflow)?;
    for ((source, _, size, _, _), (snapshot, _)) in parameter_pairs.iter().zip(&snapshots) {
        if let WordLocation::Stack(offset) = source {
            encoding.copy_word(
                &mut bytes,
                incoming_bias
                    .checked_add(*offset)
                    .ok_or(Error::FrameOverflow)?,
                *snapshot,
                *size,
            )?;
        }
    }
    for ((_, destination, _, size, codec), (snapshot, descriptor)) in
        parameter_pairs.iter().zip(&snapshots)
    {
        if let Some(descriptor) = descriptor {
            encoding.copy_word(&mut bytes, *snapshot, *descriptor, 8)?;
            encoding.constant(&mut bytes, *codec)?;
            encoding.memory(
                &mut bytes,
                encoding.scratch,
                descriptor.checked_add(8).ok_or(Error::FrameOverflow)?,
                8,
                false,
            )?;
            encoding.address(&mut bytes, *descriptor)?;
            encoding.memory(&mut bytes, encoding.scratch, *snapshot, 8, false)?;
        }
        match destination {
            WordLocation::Register(register) => {
                encoding.memory(&mut bytes, *register, *snapshot, *size, true)?
            }
            WordLocation::Stack(offset) => {
                encoding.copy_word(&mut bytes, *snapshot, *offset, *size)?
            }
        }
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
    if let (Some((source, destination, size)), Some(snapshot)) = (result, result_snapshot) {
        encoding.memory(&mut bytes, source, snapshot, size, false)?;
        encoding.memory(&mut bytes, destination, snapshot, size, true)?;
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
    use calling_conventions::{CallSignature, CallingPolicy, evaluate_call_plan};

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
}
