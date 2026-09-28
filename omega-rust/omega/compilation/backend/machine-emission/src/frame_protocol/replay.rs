//! Validate each retained span and byte against the admitted frame and ISA codec.
//! No producer arena packing or function-row construction participates here.

use isa_aarch64::Aarch64FrameSlot;
use isa_x86_64::X86_64FrameSlot;

use crate::frame_layout::frame_unwind_policy;

use super::codec::{FrameProtocolCodec, frame_protocol_codec};
use super::{
    FrameProtocolByteSpan, TargetFrameProtocolEncodingError as Error,
    TargetFrameProtocolEncodingPlan, TargetFrameProtocolEncodingPolicy, ValidatedTargetFrameLayout,
    ValidatedTargetRegisterEnvironment,
};

pub(super) fn validate_bytes(
    frame: &ValidatedTargetFrameLayout,
    environment: &ValidatedTargetRegisterEnvironment,
    candidate: &TargetFrameProtocolEncodingPlan,
) -> Result<(), Error> {
    if candidate.policy != TargetFrameProtocolEncodingPolicy::CanonicalFixedFrameV1 {
        return Err(Error::UnsupportedPolicy);
    }
    if frame.plan().target != environment.target()
        || frame.plan().register_environment != environment.identity()
        || frame.plan().physical_register_model != environment.physical().identity()
    {
        return Err(Error::RootMismatch);
    }
    if candidate.functions.len() != frame.plan().functions.len() {
        return Err(Error::NonCanonicalEncoding);
    }
    // The replay selects the ISA codec through the same declared
    // (architecture, object-format) codec row the producer used, and the
    // pair's declared unwind row still supplies the continuation mechanism
    // the validated custody must be an instance of. A missing row is
    // fail-closed defense — the validated layout implies a declared pair.
    let codec = frame_protocol_codec(environment.target()).ok_or(Error::UnsupportedTarget)?;
    let continuation = frame_unwind_policy(environment.target())
        .ok_or(Error::UnsupportedTarget)?
        .continuation;
    let mut cursor = 0;
    for (row, function) in candidate.functions.iter().zip(&frame.plan().functions) {
        if row.machine != function.machine {
            return Err(Error::NonCanonicalEncoding);
        }
        // ISA encoding is the shared primitive. The submitted row, role,
        // offsets, lengths, ordering and exact arena extent are checked here.
        // The commit schedule covers only the non-red-zone extent, and a
        // resident frame can never carry preservation storage or a call.
        let committed_bytes = function
            .frame_size_bytes
            .checked_sub(function.red_zone_resident_bytes)
            .ok_or(Error::NonCanonicalEncoding)?;
        if function.red_zone_resident_bytes != 0
            && (function.contains_call || !function.callee_save_slots.is_empty())
        {
            return Err(Error::NonCanonicalEncoding);
        }
        // The recorded return-address custody must be an instance of the
        // mechanism the pair's declared unwind row continues through; a
        // foreign custody fails closed before the codec runs.
        if !continuation.admits(function.return_address) {
            return Err(Error::UnsupportedReturnAddressCustody);
        }
        // The submitted bytes must be the encoding of exactly the validated
        // unwind roster: the declared codec's save list runs in reverse
        // roster order so its epilogue restores in roster order — a saved
        // link register first, then preservation slots in descending frame
        // offset.
        let (prologue, epilogue) = match codec {
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
                .map_err(Error::X86)?
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
                .map_err(Error::Aarch64)?
            }
        };
        cursor = validate_span(&candidate.bytes, cursor, row.prologue, &prologue)?;
        cursor = validate_span(&candidate.bytes, cursor, row.epilogue, &epilogue)?;
    }
    if cursor != candidate.bytes.len() {
        return Err(Error::NonCanonicalEncoding);
    }
    Ok(())
}

fn validate_span(
    arena: &[u8],
    cursor: usize,
    span: FrameProtocolByteSpan,
    expected: &[u8],
) -> Result<usize, Error> {
    let end = cursor
        .checked_add(expected.len())
        .ok_or(Error::ByteArenaOverflow)?;
    if usize::try_from(span.offset).ok() != Some(cursor)
        || usize::try_from(span.length).ok() != Some(expected.len())
        || arena.get(cursor..end) != Some(expected)
    {
        return Err(Error::NonCanonicalEncoding);
    }
    Ok(end)
}

#[cfg(test)]
mod tests {
    use super::{FrameProtocolByteSpan, validate_span};

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
}
