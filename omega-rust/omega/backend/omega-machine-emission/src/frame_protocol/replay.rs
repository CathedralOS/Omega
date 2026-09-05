//! Validate each retained span and byte against the admitted frame and ISA codec.
//! No producer arena packing or function-row construction participates here.

use omega_isa_aarch64::Aarch64FrameSlot;
use omega_isa_x86_64::X86_64FrameSlot;
use omega_target::Architecture;

use super::{
    FrameProtocolByteSpan, ReturnAddressFrameCustody, TargetFrameProtocolEncodingError as Error,
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
    let mut cursor = 0;
    for (row, function) in candidate.functions.iter().zip(&frame.plan().functions) {
        if row.machine != function.machine {
            return Err(Error::NonCanonicalEncoding);
        }
        // ISA encoding is the shared primitive. The submitted row, role,
        // offsets, lengths, ordering and exact arena extent are checked here.
        let (prologue, epilogue) = match environment.target().architecture {
            Architecture::X86_64 => {
                if !matches!(
                    function.return_address,
                    ReturnAddressFrameCustody::CallerActivationStack { .. }
                ) {
                    return Err(Error::UnsupportedReturnAddressCustody);
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
                .map_err(Error::X86)?
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
                    ReturnAddressFrameCustody::SavedLinkRegister {
                        view,
                        frame_offset_bytes,
                        size_bytes,
                    } => {
                        slots.push(Aarch64FrameSlot {
                            view,
                            offset_bytes: frame_offset_bytes,
                            size_bytes: u64::from(size_bytes),
                        });
                    }
                    ReturnAddressFrameCustody::LiveLinkRegister { .. } => {}
                    ReturnAddressFrameCustody::CallerActivationStack { .. } => {
                        return Err(Error::UnsupportedReturnAddressCustody);
                    }
                }
                omega_isa_aarch64::encode_aapcs64_frame_protocol(
                    environment.physical(),
                    function.frame_size_bytes,
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
    use super::*;

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
