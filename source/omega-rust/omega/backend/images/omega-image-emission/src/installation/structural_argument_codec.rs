//! Canonical format-36 codec for one structural boundary argument.
//!
//! The installation parent owns argument counts and settlement sequencing;
//! this child owns only the place/path row and its established decode errors.

use psi_core::PlaceId;
use psi_terminal::{StructuralArgument, StructuralPathSegment};

use super::{
    InstallationError, Reader, push_u32, push_u64,
    structural_scalar_codec::{access_tag, decode_access},
};

pub(super) fn encode_structural_argument(
    bytes: &mut Vec<u8>,
    argument: &StructuralArgument,
) -> Result<(), InstallationError> {
    push_u64(bytes, argument.place.get());
    bytes.push(access_tag(argument.access));
    bytes.extend_from_slice(&[0; 3]);
    push_u32(
        bytes,
        u32::try_from(argument.path.len())
            .map_err(|_| InstallationError::TooManySettlementArgumentPathSegments)?,
    );
    for segment in &argument.path {
        match segment {
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
        }
    }
    Ok(())
}

pub(super) fn decode_structural_argument(
    reader: &mut Reader<'_>,
) -> Result<StructuralArgument, InstallationError> {
    let place =
        PlaceId::new(reader.u64()?).ok_or(InstallationError::ZeroSettlementIdentity("PlaceId"))?;
    let access = decode_access(reader.u8()?)?;
    if reader.take(3)? != [0; 3] {
        return Err(InstallationError::NonzeroReservedField);
    }
    let path_count = usize::try_from(reader.u32()?)
        .map_err(|_| InstallationError::TooManySettlementArgumentPathSegments)?;
    if path_count > reader.remaining() / 8 {
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
            _ => {
                return Err(InstallationError::InvalidSettlementArgumentPathTag(tag));
            }
        });
    }
    Ok(StructuralArgument {
        place,
        access,
        path,
    })
}
