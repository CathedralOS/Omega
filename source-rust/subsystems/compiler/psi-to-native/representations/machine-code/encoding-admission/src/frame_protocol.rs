//! Check frame byte arenas against exact target recipes and issued layouts.
use calling_conventions::frame_unwind::{EncodedFrameProtocol, FrameProtocolStep};
use machine_code::FrameProtocolStepSpan;
use register_environment::ValidatedTargetRegisterEnvironment;

pub use machine_code::{
    FrameProtocolByteSpan, FunctionTargetFrameProtocolEncoding,
    TargetFrameProtocolEncodingIdentity, TargetFrameProtocolEncodingPlan,
    TargetFrameProtocolEncodingPolicy, target_frame_protocol_encoding_identity,
};

use frame_admission::frame_layout::{
    ValidatedTargetFrameLayout, committed_extent, frame_unwind_policy,
};
pub use frame_admission::frame_protocol::{
    FrameProtocolCodec, TargetFrameProtocolEncodingError, frame_protocol_codec,
};

/// Reconstruct the prologue and epilogue from the admitted frame. Only the
/// digest of the exact submitted protocol passes to later audit stages.
pub fn validate_target_frame_protocol_encoding(
    frame: &ValidatedTargetFrameLayout,
    environment: &ValidatedTargetRegisterEnvironment,
    candidate: &TargetFrameProtocolEncodingPlan,
) -> Result<TargetFrameProtocolEncodingIdentity, TargetFrameProtocolEncodingError> {
    if candidate.frame_layout != frame.receipt().identity()
        || candidate.register_environment != environment.identity()
        || candidate.physical_register_model != environment.physical().identity()
        || candidate.target != environment.target()
    {
        return Err(TargetFrameProtocolEncodingError::RootMismatch);
    }
    validate_bytes(frame, environment, candidate)?;
    Ok(target_frame_protocol_encoding_identity(candidate))
}

fn encode_frame_function(
    codec: FrameProtocolCodec,
    environment: &ValidatedTargetRegisterEnvironment,
    function: &machine_code::FunctionTargetFrameLayout,
    committed_bytes: u64,
) -> Result<EncodedFrameProtocol, TargetFrameProtocolEncodingError> {
    Ok(match codec {
        FrameProtocolCodec::X86_64 => {
            let slots = function
                .unwind
                .restores
                .iter()
                .rev()
                .map(|restore| isa_x86_64::X86_64FrameSlot {
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
                .map(|restore| isa_aarch64::Aarch64FrameSlot {
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
    })
}

fn validate_bytes(
    frame: &ValidatedTargetFrameLayout,
    environment: &ValidatedTargetRegisterEnvironment,
    candidate: &TargetFrameProtocolEncodingPlan,
) -> Result<(), TargetFrameProtocolEncodingError> {
    if candidate.policy != TargetFrameProtocolEncodingPolicy::CanonicalFixedFrameV1 {
        return Err(TargetFrameProtocolEncodingError::UnsupportedPolicy);
    }
    if frame.plan().target != environment.target()
        || frame.plan().register_environment != environment.identity()
        || frame.plan().physical_register_model != environment.physical().identity()
    {
        return Err(TargetFrameProtocolEncodingError::RootMismatch);
    }
    if candidate.functions.len() != frame.plan().functions.len() {
        return Err(TargetFrameProtocolEncodingError::NonCanonicalEncoding);
    }
    let codec = frame_protocol_codec(environment.target())
        .ok_or(TargetFrameProtocolEncodingError::UnsupportedTarget)?;
    let continuation = frame_unwind_policy(environment.target())
        .ok_or(TargetFrameProtocolEncodingError::UnsupportedTarget)?
        .continuation;
    let mut cursor = 0;
    let mut step_cursor = 0;
    for (row, function) in candidate.functions.iter().zip(&frame.plan().functions) {
        if row.machine != function.machine {
            return Err(TargetFrameProtocolEncodingError::NonCanonicalEncoding);
        }
        let committed_bytes = committed_extent(function)
            .ok_or(TargetFrameProtocolEncodingError::NonCanonicalEncoding)?;
        if !continuation.admits(function.return_address) {
            return Err(TargetFrameProtocolEncodingError::UnsupportedReturnAddressCustody);
        }
        let encoded = encode_frame_function(codec, environment, function, committed_bytes)?;
        cursor = validate_span(&candidate.bytes, cursor, row.prologue, &encoded.prologue)?;
        cursor = validate_span(&candidate.bytes, cursor, row.epilogue, &encoded.epilogue)?;
        step_cursor = validate_step_span(
            &candidate.steps,
            step_cursor,
            row.prologue_steps,
            &encoded.prologue_steps,
        )?;
        step_cursor = validate_step_span(
            &candidate.steps,
            step_cursor,
            row.epilogue_steps,
            &encoded.epilogue_steps,
        )?;
    }
    if cursor != candidate.bytes.len() || step_cursor != candidate.steps.len() {
        return Err(TargetFrameProtocolEncodingError::NonCanonicalEncoding);
    }
    Ok(())
}

fn validate_span(
    arena: &[u8],
    cursor: usize,
    span: FrameProtocolByteSpan,
    expected: &[u8],
) -> Result<usize, TargetFrameProtocolEncodingError> {
    let end = cursor
        .checked_add(expected.len())
        .ok_or(TargetFrameProtocolEncodingError::ByteArenaOverflow)?;
    if usize::try_from(span.offset).ok() != Some(cursor)
        || usize::try_from(span.length).ok() != Some(expected.len())
        || arena.get(cursor..end) != Some(expected)
    {
        return Err(TargetFrameProtocolEncodingError::NonCanonicalEncoding);
    }
    Ok(end)
}

fn validate_step_span(
    arena: &[FrameProtocolStep],
    cursor: usize,
    span: FrameProtocolStepSpan,
    expected: &[FrameProtocolStep],
) -> Result<usize, TargetFrameProtocolEncodingError> {
    let end = cursor
        .checked_add(expected.len())
        .ok_or(TargetFrameProtocolEncodingError::ByteArenaOverflow)?;
    if usize::try_from(span.offset).ok() != Some(cursor)
        || usize::try_from(span.length).ok() != Some(expected.len())
        || arena.get(cursor..end) != Some(expected)
    {
        return Err(TargetFrameProtocolEncodingError::NonCanonicalEncoding);
    }
    Ok(end)
}

#[cfg(test)]
mod tests {
    use target::NativeTarget;
    #[cfg(test)]
    use target::{Architecture, ObjectFormat};

    use super::{FrameProtocolByteSpan, FrameProtocolCodec, frame_protocol_codec, validate_span};
    use frame_admission::frame_layout::{FrameContinuationCustody, frame_unwind_policy};

    #[test]
    fn span_replay_rejects_padding_aliases_truncation_and_substituted_bytes() {
        let arena = [1, 2, 3];
        let span = FrameProtocolByteSpan {
            offset: 1,
            length: 2,
        };
        assert_eq!(validate_span(&arena, 1, span, &[2, 3]), Ok(3));
        assert!(validate_span(&arena, 0, span, &[2, 3]).is_err());
        assert!(
            validate_span(
                &arena,
                1,
                FrameProtocolByteSpan {
                    offset: 0,
                    length: 2
                },
                &[2, 3]
            )
            .is_err()
        );
        assert!(
            validate_span(
                &arena,
                1,
                FrameProtocolByteSpan {
                    offset: 1,
                    length: 1
                },
                &[2, 3]
            )
            .is_err()
        );
        assert!(validate_span(&arena, 1, span, &[3, 2]).is_err());
        assert!(validate_span(&arena[..2], 1, span, &[2, 3]).is_err());
    }

    #[test]
    fn codec_matrix_declares_one_row_per_supported_pair() {
        for (target, expected) in [
            (NativeTarget::linux_x64(), FrameProtocolCodec::X86_64),
            (NativeTarget::macos_x64(), FrameProtocolCodec::X86_64),
            (NativeTarget::windows_x64(), FrameProtocolCodec::X86_64),
            (NativeTarget::uefi_x64(), FrameProtocolCodec::X86_64),
            (NativeTarget::linux_arm64(), FrameProtocolCodec::Aarch64),
            (NativeTarget::macos_arm64(), FrameProtocolCodec::Aarch64),
        ] {
            assert_eq!(frame_protocol_codec(target), Some(expected), "{target:?}");
        }
    }

    #[test]
    fn windows_arm64_frames_use_the_aarch64_codec() {
        assert_eq!(
            frame_protocol_codec(NativeTarget::windows_arm64()),
            Some(FrameProtocolCodec::Aarch64)
        );
    }

    #[test]
    fn codec_matrix_agrees_with_the_unwind_matrix_on_every_pair() {
        for architecture in [Architecture::X86_64, Architecture::Aarch64] {
            for object_format in [ObjectFormat::Elf, ObjectFormat::MachO, ObjectFormat::Coff] {
                let target = NativeTarget {
                    architecture,
                    object_format,
                    pointer_size: 8,
                    pointer_alignment: 8,
                };
                let codec = frame_protocol_codec(target);
                let unwind = frame_unwind_policy(target);
                assert_eq!(
                    codec.is_some(),
                    unwind.is_some(),
                    "{target:?}: codec and unwind matrices must declare the same pairs"
                );
                let serves = matches!(
                    (codec, unwind.map(|policy| policy.continuation)),
                    (
                        Some(FrameProtocolCodec::X86_64),
                        Some(FrameContinuationCustody::CallerActivationStack { .. }),
                    ) | (
                        Some(FrameProtocolCodec::Aarch64),
                        Some(FrameContinuationCustody::LinkRegister { .. }),
                    ) | (None, None)
                );
                assert!(
                    serves,
                    "{target:?}: the declared codec cannot serve the declared continuation"
                );
            }
        }
    }
}
