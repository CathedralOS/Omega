use omega_isa_aarch64::Aarch64FrameSlot;
use omega_isa_x86_64::X86_64FrameSlot;
use omega_target::Architecture;

use crate::frame_protocol::{
    ReturnAddressFrameCustody, ValidatedTargetFrameLayout, ValidatedTargetRegisterEnvironment,
};

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
    let mut bytes = Vec::new();
    let mut functions = Vec::with_capacity(frame.plan().functions.len());
    for function in &frame.plan().functions {
        let (prologue_bytes, epilogue_bytes) = match environment.target().architecture {
            Architecture::X86_64 => {
                if !matches!(
                    function.return_address,
                    ReturnAddressFrameCustody::CallerActivationStack { .. }
                ) {
                    return Err(TargetFrameProtocolEncodingError::UnsupportedReturnAddressCustody);
                }
                let slots = function
                    .callee_save_slots
                    .iter()
                    .map(|slot| X86_64FrameSlot {
                        view: slot.storage_view,
                        offset_bytes: slot.frame_offset_bytes,
                        size_bytes: slot.size_bytes,
                    })
                    .collect::<Vec<_>>();
                omega_isa_x86_64::encode_system_v_amd64_frame_protocol(
                    environment.physical(),
                    function.frame_size_bytes,
                    &slots,
                )
                .map_err(TargetFrameProtocolEncodingError::X86)?
            }
            Architecture::Aarch64 => {
                let mut slots = function
                    .callee_save_slots
                    .iter()
                    .map(|slot| Aarch64FrameSlot {
                        view: slot.storage_view,
                        offset_bytes: slot.frame_offset_bytes,
                        size_bytes: slot.size_bytes,
                    })
                    .collect::<Vec<_>>();
                match function.return_address {
                    ReturnAddressFrameCustody::LiveLinkRegister { .. } => {}
                    ReturnAddressFrameCustody::SavedLinkRegister {
                        view,
                        frame_offset_bytes,
                        size_bytes,
                    } => slots.push(Aarch64FrameSlot {
                        view,
                        offset_bytes: frame_offset_bytes,
                        size_bytes: u64::from(size_bytes),
                    }),
                    ReturnAddressFrameCustody::CallerActivationStack { .. } => {
                        return Err(
                            TargetFrameProtocolEncodingError::UnsupportedReturnAddressCustody,
                        );
                    }
                }
                omega_isa_aarch64::encode_aapcs64_frame_protocol(
                    environment.physical(),
                    function.frame_size_bytes,
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
