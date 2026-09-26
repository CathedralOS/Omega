//! Canonical format-36 codec for one structural boundary argument.
//!
//! The installation parent owns argument counts and settlement sequencing;
//! this child owns only the place/path row and its established decode errors.

use semantic_vocabulary::PlaceId;
use terminal_psi::{StructuralArgument, StructuralPathSegment};

use crate::installation_record::codec::structural::scalar::{access_tag, decode_access};
use crate::installation_record::{InstallationError, Reader, push_u32, push_u64};

pub(crate) fn encode_structural_argument(
    bytes: &mut Vec<u8>,
    argument: &StructuralArgument,
) -> Result<(), InstallationError> {
    push_u64(bytes, argument.place.get());
    bytes.push(access_tag(argument.access));
    bytes.extend_from_slice(&[0; 3]);
    encode_path(bytes, &argument.path)
}

/// One exact structural path: segment count, then each segment's canonical
/// tag and payload. `Referent` carries no bytes — the segment records only
/// that the path crosses a reference carrier into its referent.
pub(crate) fn encode_path(
    bytes: &mut Vec<u8>,
    path: &[StructuralPathSegment],
) -> Result<(), InstallationError> {
    push_u32(
        bytes,
        u32::try_from(path.len())
            .map_err(|_| InstallationError::TooManySettlementArgumentPathSegments)?,
    );
    for segment in path {
        match segment {
            StructuralPathSegment::Referent => {
                bytes.push(3);
                bytes.extend_from_slice(&[0; 3]);
            }
            StructuralPathSegment::Field(identity) => {
                if identity.is_empty() {
                    return Err(InstallationError::InvalidSettlementArgumentField);
                }
                bytes.push(1);
                bytes.extend_from_slice(&[0; 3]);
                push_u32(
                    bytes,
                    u32::try_from(identity.len())
                        .map_err(|_| InstallationError::SettlementArgumentFieldTooLong)?,
                );
                bytes.extend_from_slice(identity.as_bytes());
            }
            StructuralPathSegment::FixedIndex(index) => {
                bytes.push(2);
                bytes.extend_from_slice(&[0; 3]);
                push_u64(bytes, *index);
            }
            StructuralPathSegment::FixedByteRange { start, end } => {
                bytes.extend_from_slice(&[4, 0, 0, 0]);
                push_u64(bytes, *start);
                push_u64(bytes, *end);
            }
            StructuralPathSegment::RuntimeIndex { index, obligation } => {
                bytes.extend_from_slice(&[6, 0, 0, 0]);
                push_u64(bytes, index.get());
                push_u64(bytes, obligation.get());
            }
        }
    }
    Ok(())
}

pub(crate) fn decode_path(
    reader: &mut Reader<'_>,
) -> Result<Vec<StructuralPathSegment>, InstallationError> {
    let path_count = usize::try_from(reader.u32()?)
        .map_err(|_| InstallationError::TooManySettlementArgumentPathSegments)?;
    if path_count > reader.remaining() / 4 {
        return Err(InstallationError::UnexpectedEnd);
    }
    let mut path = Vec::with_capacity(path_count);
    for _ in 0..path_count {
        let tag = reader.u8()?;
        if reader.take(3)? != [0; 3] {
            return Err(InstallationError::NonzeroReservedField);
        }
        path.push(match tag {
            1 => {
                let identity_len = usize::try_from(reader.u32()?)
                    .map_err(|_| InstallationError::SettlementArgumentFieldTooLong)?;
                let identity = std::str::from_utf8(reader.take(identity_len)?)
                    .map_err(|_| InstallationError::InvalidSettlementArgumentField)?
                    .to_owned();
                if identity.is_empty() {
                    return Err(InstallationError::InvalidSettlementArgumentField);
                }
                StructuralPathSegment::Field(identity)
            }
            2 => StructuralPathSegment::FixedIndex(reader.u64()?),
            3 => StructuralPathSegment::Referent,
            4 => StructuralPathSegment::FixedByteRange {
                start: reader.u64()?,
                end: reader.u64()?,
            },
            6 => StructuralPathSegment::RuntimeIndex {
                index: semantic_vocabulary::ValueId::new(reader.u64()?)
                    .ok_or(InstallationError::ZeroSettlementIdentity("ValueId"))?,
                obligation: semantic_vocabulary::ObligationId::new(reader.u64()?)
                    .ok_or(InstallationError::ZeroSettlementIdentity("ObligationId"))?,
            },
            _ => {
                return Err(InstallationError::InvalidSettlementArgumentPathTag(tag));
            }
        });
    }
    Ok(path)
}

pub(crate) fn decode_structural_argument(
    reader: &mut Reader<'_>,
) -> Result<StructuralArgument, InstallationError> {
    let place =
        PlaceId::new(reader.u64()?).ok_or(InstallationError::ZeroSettlementIdentity("PlaceId"))?;
    let access = decode_access(reader.u8()?)?;
    if reader.take(3)? != [0; 3] {
        return Err(InstallationError::NonzeroReservedField);
    }
    let path = decode_path(reader)?;
    Ok(StructuralArgument {
        place,
        access,
        path,
    })
}
