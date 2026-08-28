//! Canonical format-36 scalar-type codec for boundary result evidence.
//!
//! Result presence, value/edge identity, placement, and settlement sequencing
//! remain in the installation parent. This child owns only the six-byte scalar
//! type row and its established validation errors.

use psi_core::{IntegerSign, IntegerType, ScalarType};

use super::{InstallationError, Reader, decode_boolean, push_u16};

pub(super) fn encode_boundary_result_scalar_type(bytes: &mut Vec<u8>, scalar_type: ScalarType) {
    match scalar_type {
        ScalarType::Boolean => bytes.extend_from_slice(&[1, 0, 0, 0, 0, 0]),
        ScalarType::Integer(integer) => {
            bytes.push(2);
            bytes.push(u8::from(integer.is_address()));
            bytes.push(u8::from(matches!(integer.sign(), IntegerSign::Signed)));
            bytes.push(0);
            push_u16(bytes, integer.bits());
        }
    }
}

pub(super) fn decode_boundary_result_scalar_type(
    reader: &mut Reader<'_>,
) -> Result<ScalarType, InstallationError> {
    let tag = reader.u8()?;
    let is_address = decode_boolean(reader.u8()?)?;
    let signed = decode_boolean(reader.u8()?)?;
    if reader.u8()? != 0 {
        return Err(InstallationError::NonzeroReservedField);
    }
    let bits = reader.u16()?;
    match tag {
        1 if !is_address && !signed && bits == 0 => Ok(ScalarType::Boolean),
        2 if is_address && !signed => IntegerType::address(bits)
            .map(ScalarType::Integer)
            .map_err(|_| InstallationError::InvalidBoundaryResult),
        2 if !is_address => IntegerType::new(
            if signed {
                IntegerSign::Signed
            } else {
                IntegerSign::Unsigned
            },
            bits,
        )
        .map(ScalarType::Integer)
        .map_err(|_| InstallationError::InvalidBoundaryResult),
        _ => Err(InstallationError::InvalidBoundaryResult),
    }
}
