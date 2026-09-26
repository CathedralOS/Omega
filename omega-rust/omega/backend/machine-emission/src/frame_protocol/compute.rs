use isa_aarch64::Aarch64FrameSlot;
use isa_x86_64::X86_64FrameSlot;

use crate::frame_layout::frame_unwind_policy;
use crate::frame_protocol::{ValidatedTargetFrameLayout, ValidatedTargetRegisterEnvironment};

use super::codec::{FrameProtocolCodec, frame_protocol_codec};
use super::{
    FrameProtocolByteSpan, FunctionTargetFrameProtocolEncoding, TargetFrameProtocolEncodingError,
    TargetFrameProtocolEncodingPlan, TargetFrameProtocolEncodingPolicy,
};

pub(super) fn derive(
    frame: &ValidatedTargetFrameLayout,
    environment: &ValidatedTargetRegisterEnvironment,
    policy: TargetFrameProtocolEncodingPolicy,
) -> Result<TargetFrameProtocolEncodingPlan, TargetFrameProtocolEncodingError> {
    if policy != TargetFrameProtocolEncodingPolicy::CanonicalFixedFrameV1 {
        return Err(TargetFrameProtocolEncodingError::UnsupportedPolicy);
    }
    if frame.plan().target != environment.target()
        || frame.plan().register_environment != environment.identity()
        || frame.plan().physical_register_model != environment.physical().identity()
    {
        return Err(TargetFrameProtocolEncodingError::RootMismatch);
    }
    // The pair's declared codec row selects the ISA codec the protocol is
    // encoded with: selection is an (architecture, object-format)
    // declaration, not an inference from the pair's unwind mechanism. The
    // pair's declared unwind row still supplies the continuation mechanism
    // each function's recorded custody must be an instance of. The
    // validated layout could only exist on a declared pair, so a missing
    // row is fail-closed defense, not a reachable target.
    let codec = frame_protocol_codec(environment.target())
        .ok_or(TargetFrameProtocolEncodingError::UnsupportedTarget)?;
    let continuation = frame_unwind_policy(environment.target())
        .ok_or(TargetFrameProtocolEncodingError::UnsupportedTarget)?
        .continuation;
    let mut bytes = Vec::new();
    let mut functions = Vec::with_capacity(frame.plan().functions.len());
    for function in &frame.plan().functions {
        // The prologue commits only the non-red-zone extent; resident bytes
        // already live below the unadjusted entry stack pointer. Residency is
        // a leaf-only storage shape: preservation slots and call frames can
        // never observe a below-RSP coordinate, so a resident row carrying
        // either is rejected here even though layout replay also refuses it.
        let committed_bytes = function
            .frame_size_bytes
            .checked_sub(function.red_zone_resident_bytes)
            .ok_or(TargetFrameProtocolEncodingError::NonCanonicalEncoding)?;
        if function.red_zone_resident_bytes != 0
            && (function.contains_call || !function.callee_save_slots.is_empty())
        {
            return Err(TargetFrameProtocolEncodingError::NonCanonicalEncoding);
        }
        // The recorded return-address custody must be an instance of the
        // mechanism the pair's declared unwind row continues through; a
        // foreign custody fails closed before the codec runs.
        if !continuation.admits(function.return_address) {
            return Err(TargetFrameProtocolEncodingError::UnsupportedReturnAddressCustody);
        }
        // The encoded protocol performs exactly the validated unwind roster:
        // the declared codec's save list runs in reverse roster order so its
        // epilogue restores in roster order — a saved link register first,
        // then preservation slots in descending frame offset.
        let (prologue_bytes, epilogue_bytes) = match codec {
            FrameProtocolCodec::X86_64 => {
                let slots = function
                    .unwind
                    .restores
                    .iter()
                    .rev()
                    .map(|restore| X86_64FrameSlot {
                        view: restore.view,
                        offset_bytes: restore.frame_offset_bytes,
                        size_bytes: restore.size_bytes,
                    })
                    .collect::<Vec<_>>();
                isa_x86_64::encode_system_v_amd64_frame_protocol(
                    environment.physical(),
                    committed_bytes,
                    isa_x86_64::X86_64StackProbe {
                        interval_bytes: function.stack_probe.interval_bytes,
                        touches: function.stack_probe.touches,
                    },
                    &slots,
                )
                .map_err(TargetFrameProtocolEncodingError::X86)?
            }
            FrameProtocolCodec::Aarch64 => {
                let slots = function
                    .unwind
                    .restores
                    .iter()
                    .rev()
                    .map(|restore| Aarch64FrameSlot {
                        view: restore.view,
                        offset_bytes: restore.frame_offset_bytes,
                        size_bytes: restore.size_bytes,
                    })
                    .collect::<Vec<_>>();
                isa_aarch64::encode_aapcs64_frame_protocol(
                    environment.physical(),
                    committed_bytes,
                    isa_aarch64::Aarch64StackProbe {
                        interval_bytes: function.stack_probe.interval_bytes,
                        touches: function.stack_probe.touches,
                    },
                    &slots,
                )
                .map_err(TargetFrameProtocolEncodingError::Aarch64)?
            }
        };
        let prologue = append(&mut bytes, &prologue_bytes)?;
        let epilogue = append(&mut bytes, &epilogue_bytes)?;
        functions.push(FunctionTargetFrameProtocolEncoding {
            machine: function.machine,
            prologue,
            epilogue,
        });
    }
    Ok(TargetFrameProtocolEncodingPlan {
        frame_layout: frame.receipt().identity(),
        register_environment: environment.identity(),
        physical_register_model: environment.physical().identity(),
        target: environment.target(),
        policy,
        functions,
        bytes,
    })
}

fn append(
    arena: &mut Vec<u8>,
    value: &[u8],
) -> Result<FrameProtocolByteSpan, TargetFrameProtocolEncodingError> {
    let offset = u32::try_from(arena.len())
        .map_err(|_| TargetFrameProtocolEncodingError::ByteArenaOverflow)?;
    let length = u32::try_from(value.len())
        .map_err(|_| TargetFrameProtocolEncodingError::ByteArenaOverflow)?;
    arena
        .len()
        .checked_add(value.len())
        .ok_or(TargetFrameProtocolEncodingError::ByteArenaOverflow)?;
    arena.extend_from_slice(value);
    Ok(FrameProtocolByteSpan { offset, length })
}
