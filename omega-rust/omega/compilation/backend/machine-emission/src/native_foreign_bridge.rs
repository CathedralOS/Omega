//! A fixed host frame entering one admitted native C calling plan.
//!
//! The host owns a two-pointer C frame: an array of pointers to complete native
//! argument values, followed by the result storage pointer. Each pointed-to
//! buffer has its calling-plan extent and alignment. Borrowed native arguments
//! contain a pointer supplied by their backing owner, not captured referent
//! bytes. This physical adapter establishes no backing, lifetime or authority.
//!
//! Snapshot every argument before distributing registers or outgoing stack
//! homes. The one unresolved call names the selected actual foreign import;
//! the resulting bytes can be replayed before a host installs the adapter.
//! A stable backing owner and actual completion remain separate obligations.

use calling_conventions::{
    CallSignature, CallingPolicy, EntryControl, MachineRegister, ValidatedBoundaryEntryPlan,
    ValueLocation, ValueShape, evaluate_call_plan,
};
use register_model::validate_physical_register_model;
use target::{Architecture, NativeTarget};

use crate::native_bridge_encoding::{
    AlignedHome, Encoding, NativeBridgeEncodingError, ValueStorage, WordLocation, allocate_home,
    allocate_word, is_borrowed, value_storage,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeForeignBridgeRequest {
    pub target: NativeTarget,
    pub foreign: ValidatedBoundaryEntryPlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeForeignBridge {
    request: NativeForeignBridgeRequest,
    bytes: Vec<u8>,
    frame_bytes: u32,
    call_field_offset: usize,
}

impl NativeForeignBridge {
    pub fn request(&self) -> &NativeForeignBridgeRequest {
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeForeignBridgeError {
    IncompatiblePlan,
    UnsupportedValueTransport,
    FrameOverflow,
    Encoding,
    ReplayMismatch,
}
diagnostics::failed_display!(NativeForeignBridgeError, "native foreign bridge failed");
impl std::error::Error for NativeForeignBridgeError {}
type Error = NativeForeignBridgeError;

impl From<NativeBridgeEncodingError> for Error {
    fn from(error: NativeBridgeEncodingError) -> Self {
        match error {
            NativeBridgeEncodingError::UnsupportedValueTransport => Self::UnsupportedValueTransport,
            NativeBridgeEncodingError::FrameOverflow => Self::FrameOverflow,
            NativeBridgeEncodingError::Encoding => Self::Encoding,
        }
    }
}

struct Argument {
    placement: ValueStorage,
    extent: u32,
    snapshot: u32,
}

fn extent(shape: ValueShape) -> u32 {
    if is_borrowed(shape.class) {
        8
    } else {
        shape.byte_size
    }
}

pub fn emit_native_foreign_bridge(
    request: NativeForeignBridgeRequest,
) -> Result<NativeForeignBridge, Error> {
    let policy = CallingPolicy::native_for_target(request.target);
    let foreign = &request.foreign.plan().call;
    // Consume the producer's admitted plan rather than reevaluating its policy.
    // This physical adapter supports the host's ordinary C call regime only.
    if foreign.policy != policy
        || foreign.entry_control != EntryControl::CallReturn
        || foreign.stack_alignment != 16
        || !foreign.callback_materializations.is_empty()
    {
        return Err(Error::IncompatiblePlan);
    }
    let entry = evaluate_call_plan(
        policy,
        &CallSignature {
            parameters: vec![ValueShape::integer(8, 8)],
            result: None,
        },
    )
    .map_err(|_| Error::IncompatiblePlan)?;
    let [
        ValueLocation::Register {
            register: frame_register,
            byte_size: 8,
            value_byte_offset: 0,
        },
    ] = entry.parameters[0].locations.as_slice()
    else {
        return Err(Error::IncompatiblePlan);
    };
    let architecture = request.target.architecture;
    let (scratch, address_scratch) = match architecture {
        Architecture::Aarch64 => (MachineRegister::Aarch64X(16), MachineRegister::Aarch64X(17)),
        Architecture::X86_64 => (MachineRegister::X86R10, MachineRegister::X86R11),
    };
    let model = match architecture {
        Architecture::Aarch64 => {
            validate_physical_register_model(isa_aarch64::aarch64_physical_register_model())
        }
        Architecture::X86_64 => {
            validate_physical_register_model(isa_x86_64::x86_64_physical_register_model())
        }
    }
    .map_err(|_| Error::Encoding)?;
    let mut encoding = Encoding {
        architecture,
        model,
        scratch,
        address_scratch,
        aligned_homes: Vec::new(),
    };
    let mut cursor = u32::from(foreign.shadow_bytes);
    let mut placements = Vec::with_capacity(foreign.parameters.len());
    for parameter in &foreign.parameters {
        let size = extent(parameter.shape);
        let placement = value_storage(parameter, size)?;
        for piece in placement.pieces() {
            if let WordLocation::Stack(offset) = piece.location {
                cursor = cursor.max(
                    offset
                        .checked_add(u32::from(piece.byte_size))
                        .ok_or(Error::FrameOverflow)?,
                );
            }
        }
        if let ValueStorage::Indirect {
            copy_offset: Some(offset),
            ..
        } = &placement
        {
            let alignment = parameter.shape.alignment;
            cursor = cursor.max(
                offset
                    .checked_add(size)
                    .and_then(|end| end.checked_add(u32::from(alignment.saturating_sub(16))))
                    .ok_or(Error::FrameOverflow)?,
            );
            if alignment > 16 {
                if !offset.is_multiple_of(16) {
                    return Err(Error::UnsupportedValueTransport);
                }
                encoding.aligned_homes.push(AlignedHome {
                    offset: *offset,
                    byte_size: size,
                    alignment,
                });
            }
        }
        placements.push((placement, size));
    }
    let frame_pointer = allocate_word(&mut cursor)?;
    let input_array = allocate_word(&mut cursor)?;
    let input_pointer = allocate_word(&mut cursor)?;
    let mut arguments = Vec::with_capacity(placements.len());
    for ((placement, size), parameter) in placements.into_iter().zip(&foreign.parameters) {
        let snapshot = allocate_home(
            &mut cursor,
            size,
            parameter.shape.alignment.max(8),
            &mut encoding.aligned_homes,
        )?;
        arguments.push(Argument {
            placement,
            extent: size,
            snapshot,
        });
    }
    let result = foreign
        .result
        .as_ref()
        .map(|value| value_storage(value, extent(value.shape)))
        .transpose()?;
    let output_pointer = result
        .as_ref()
        .map(|_| allocate_word(&mut cursor))
        .transpose()?;
    let result_snapshot = match (&result, &foreign.result) {
        (Some(ValueStorage::Direct(pieces)), Some(value)) => {
            if pieces
                .iter()
                .any(|piece| matches!(piece.location, WordLocation::Stack(_)))
            {
                return Err(Error::UnsupportedValueTransport);
            }
            Some(allocate_home(
                &mut cursor,
                extent(value.shape),
                value.shape.alignment.max(8),
                &mut encoding.aligned_homes,
            )?)
        }
        _ => None,
    };
    let continuation = (architecture == Architecture::Aarch64)
        .then(|| allocate_word(&mut cursor))
        .transpose()?;
    let frame_bytes = cursor
        .checked_add(15)
        .map(|size| size & !15)
        .and_then(|size| {
            size.checked_add(if architecture == Architecture::X86_64 {
                8
            } else {
                0
            })
        })
        .filter(|size| *size <= i32::MAX as u32)
        .ok_or(Error::FrameOverflow)?;
    let interval = crate::frame_layout::stack_commit::stack_commit_granule_bytes(request.target)
        .ok_or(Error::IncompatiblePlan)?;
    let touches = if u64::from(frame_bytes) > interval {
        u32::try_from(u64::from(frame_bytes).div_ceil(interval))
            .map_err(|_| Error::FrameOverflow)?
    } else {
        0
    };
    let (mut bytes, epilogue) = match architecture {
        Architecture::Aarch64 => isa_aarch64::encode_aapcs64_frame_protocol(
            &encoding.model,
            u64::from(frame_bytes),
            isa_aarch64::Aarch64StackProbe {
                interval_bytes: interval,
                touches,
            },
            &[],
        )
        .map_err(|_| Error::Encoding),
        Architecture::X86_64 => isa_x86_64::encode_system_v_amd64_frame_protocol(
            &encoding.model,
            u64::from(frame_bytes),
            isa_x86_64::X86_64StackProbe {
                interval_bytes: interval,
                touches,
            },
            &[],
        )
        .map_err(|_| Error::Encoding),
    }?;
    if let Some(offset) = continuation {
        encoding.memory(&mut bytes, MachineRegister::Aarch64X(30), offset, 8, false)?;
    }
    encoding.memory(&mut bytes, *frame_register, frame_pointer, 8, false)?;
    encoding.memory(&mut bytes, address_scratch, frame_pointer, 8, true)?;
    encoding.pointer_memory(&mut bytes, 0, 8, true)?;
    encoding.memory(&mut bytes, scratch, input_array, 8, false)?;
    if let Some(offset) = output_pointer {
        encoding.memory(&mut bytes, address_scratch, frame_pointer, 8, true)?;
        encoding.pointer_memory(&mut bytes, 8, 8, true)?;
        encoding.memory(&mut bytes, scratch, offset, 8, false)?;
    }
    for (ordinal, argument) in arguments.iter().enumerate() {
        let offset = u32::try_from(ordinal)
            .ok()
            .and_then(|n| n.checked_mul(8))
            .ok_or(Error::FrameOverflow)?;
        encoding.memory(&mut bytes, address_scratch, input_array, 8, true)?;
        encoding.pointer_memory(&mut bytes, offset, 8, true)?;
        encoding.memory(&mut bytes, scratch, input_pointer, 8, false)?;
        encoding.copy_indirect(
            &mut bytes,
            argument.snapshot,
            input_pointer,
            argument.extent,
            true,
        )?;
    }
    for argument in &arguments {
        encoding.distribute_argument(
            &mut bytes,
            &argument.placement,
            argument.snapshot,
            argument.extent,
        )?;
    }
    if let (Some(ValueStorage::Indirect { pointer, .. }), Some(output)) = (&result, output_pointer)
    {
        match pointer.location {
            WordLocation::Register(register) => {
                encoding.memory(&mut bytes, register, output, 8, true)?
            }
            WordLocation::Stack(offset) => encoding.copy_word(&mut bytes, output, offset, 8)?,
        }
    }
    let call_field_offset = bytes.len()
        + if architecture == Architecture::X86_64 {
            1
        } else {
            0
        };
    bytes.extend_from_slice(match architecture {
        Architecture::Aarch64 => &isa_aarch64::ORDINARY_DIRECT_CALL_BYTES,
        Architecture::X86_64 => &isa_x86_64::ORDINARY_DIRECT_CALL_BYTES,
    });
    if let (Some(ValueStorage::Direct(pieces)), Some(snapshot), Some(output), Some(value)) =
        (&result, result_snapshot, output_pointer, &foreign.result)
    {
        encoding.capture_registers(&mut bytes, pieces, snapshot)?;
        encoding.copy_indirect(&mut bytes, snapshot, output, extent(value.shape), false)?;
    }
    if let Some(offset) = continuation {
        encoding.memory(&mut bytes, MachineRegister::Aarch64X(30), offset, 8, true)?;
    }
    bytes.extend_from_slice(&epilogue);
    bytes.extend_from_slice(match architecture {
        Architecture::Aarch64 => &isa_aarch64::ORDINARY_RETURN_BYTES,
        Architecture::X86_64 => &isa_x86_64::ORDINARY_RETURN_BYTES,
    });
    Ok(NativeForeignBridge {
        request,
        bytes,
        frame_bytes,
        call_field_offset,
    })
}

pub fn validate_native_foreign_bridge(bridge: &NativeForeignBridge) -> Result<(), Error> {
    if emit_native_foreign_bridge(bridge.request.clone())? != *bridge {
        return Err(Error::ReplayMismatch);
    }
    Ok(())
}
