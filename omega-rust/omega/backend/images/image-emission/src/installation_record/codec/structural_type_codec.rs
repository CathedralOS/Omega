//! Wire codec for the structural type declarations carried by installation
//! record rows.

use super::boundary_result_scalar_codec;
use super::structural_scalar_codec::{access_tag, decode_access};
use crate::installation_record::{
    InstallationError, Reader, StructuralTypeId, decode_identity, decode_structural_cases,
    decode_structural_fields, encode_identity, encode_structural_cases, encode_structural_fields,
    push_u32, push_u64,
};
pub(crate) fn encode_structural_types(
    bytes: &mut Vec<u8>,
    declarations: &[terminal_psi::StructuralTypeDeclaration],
) -> Result<(), InstallationError> {
    push_u32(
        bytes,
        u32::try_from(declarations.len()).map_err(|_| InstallationError::TooManyStructuralTypes)?,
    );
    for declaration in declarations {
        push_u64(bytes, declaration.id.get());
        encode_identity(bytes, &declaration.identity)?;
        match &declaration.shape {
            // A reference is semantic custody, not native pointer layout:
            // the wire row preserves referent identity and access mode and
            // no placement bytes accompany the declaration.
            terminal_psi::StructuralTypeShape::Reference { referent, access } => {
                bytes.extend_from_slice(&[7, 0, 0, 0]);
                push_u64(bytes, referent.get());
                bytes.push(access_tag(*access));
                bytes.extend_from_slice(&[0; 3]);
            }
            terminal_psi::StructuralTypeShape::PrimitiveScalar(scalar_type) => {
                bytes.extend_from_slice(&[6, 0, 0, 0]);
                boundary_result_scalar_codec::encode_boundary_result_scalar_type(
                    bytes,
                    *scalar_type,
                );
            }
            terminal_psi::StructuralTypeShape::ByteSequence(carrier) => {
                bytes.extend_from_slice(&[4, 0, 0, 0]);
                match carrier {
                    terminal_psi::ByteSequenceCarrier::BorrowedView => {
                        bytes.extend_from_slice(&[1, 0, 0, 0]);
                        push_u64(bytes, 0);
                    }
                    terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity } => {
                        bytes.extend_from_slice(&[2, 0, 0, 0]);
                        push_u64(bytes, *capacity);
                    }
                }
            }
            terminal_psi::StructuralTypeShape::Record { fields } => {
                bytes.extend_from_slice(&[1, 0, 0, 0]);
                encode_structural_fields(bytes, fields)?;
            }
            terminal_psi::StructuralTypeShape::FixedArray { element, length } => {
                bytes.extend_from_slice(&[2, 0, 0, 0]);
                push_u64(bytes, element.get());
                push_u64(bytes, *length);
            }
            terminal_psi::StructuralTypeShape::Sum { cases } => {
                bytes.extend_from_slice(&[3, 0, 0, 0]);
                encode_structural_cases(bytes, cases)?;
            }
            terminal_psi::StructuralTypeShape::Mixed { fields, cases } => {
                bytes.extend_from_slice(&[5, 0, 0, 0]);
                encode_structural_fields(bytes, fields)?;
                encode_structural_cases(bytes, cases)?;
            }
            // Tag 8 matches the other structural-shape encoders
            // (terminal-codec, optimization-unit, legalized-operations,
            // register-homes all use tag 8 for ElementView).
            terminal_psi::StructuralTypeShape::ElementView { element } => {
                bytes.extend_from_slice(&[8, 0, 0, 0]);
                push_u64(bytes, element.get());
            }
        }
    }
    Ok(())
}

pub(crate) fn decode_structural_types(
    reader: &mut Reader<'_>,
) -> Result<Vec<terminal_psi::StructuralTypeDeclaration>, InstallationError> {
    let count =
        usize::try_from(reader.u32()?).map_err(|_| InstallationError::TooManyStructuralTypes)?;
    if count > reader.remaining() {
        return Err(InstallationError::UnexpectedEnd);
    }
    let mut declarations = Vec::with_capacity(count);
    for _ in 0..count {
        let id = StructuralTypeId::new(reader.u64()?).ok_or(
            InstallationError::ZeroStructuralReturnIdentity("structural type"),
        )?;
        let identity = decode_identity(reader)?;
        let shape_tag = reader.u8()?;
        if reader.u8()? != 0 || reader.u8()? != 0 || reader.u8()? != 0 {
            return Err(InstallationError::NonzeroReservedField);
        }
        let shape = match shape_tag {
            1 => terminal_psi::StructuralTypeShape::Record {
                fields: decode_structural_fields(reader)?,
            },
            2 => terminal_psi::StructuralTypeShape::FixedArray {
                element: StructuralTypeId::new(reader.u64()?).ok_or(
                    InstallationError::ZeroStructuralReturnIdentity("fixed-array element type"),
                )?,
                length: reader.u64()?,
            },
            3 => terminal_psi::StructuralTypeShape::Sum {
                cases: decode_structural_cases(reader)?,
            },
            4 => {
                let carrier_tag = reader.u8()?;
                if reader.u8()? != 0 || reader.u8()? != 0 || reader.u8()? != 0 {
                    return Err(InstallationError::NonzeroReservedField);
                }
                let capacity = reader.u64()?;
                terminal_psi::StructuralTypeShape::ByteSequence(match carrier_tag {
                    1 if capacity == 0 => terminal_psi::ByteSequenceCarrier::BorrowedView,
                    2 => terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity },
                    tag => {
                        return Err(InstallationError::InvalidStructuralTypeShapeTag(tag));
                    }
                })
            }
            5 => terminal_psi::StructuralTypeShape::Mixed {
                fields: decode_structural_fields(reader)?,
                cases: decode_structural_cases(reader)?,
            },
            6 => terminal_psi::StructuralTypeShape::PrimitiveScalar(
                boundary_result_scalar_codec::decode_boundary_result_scalar_type(reader)?,
            ),
            7 => {
                let referent = StructuralTypeId::new(reader.u64()?).ok_or(
                    InstallationError::ZeroStructuralReturnIdentity("reference referent type"),
                )?;
                let access = decode_access(reader.u8()?)?;
                if reader.take(3)? != [0; 3] {
                    return Err(InstallationError::NonzeroReservedField);
                }
                terminal_psi::StructuralTypeShape::Reference { referent, access }
            }
            8 => terminal_psi::StructuralTypeShape::ElementView {
                element: StructuralTypeId::new(reader.u64()?).ok_or(
                    InstallationError::ZeroStructuralReturnIdentity("element-view element type"),
                )?,
            },
            tag => {
                return Err(InstallationError::InvalidStructuralTypeShapeTag(tag));
            }
        };
        declarations.push(terminal_psi::StructuralTypeDeclaration {
            id,
            identity,
            shape,
        });
    }
    Ok(declarations)
}
