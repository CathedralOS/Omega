//! Canonical structural-type declaration wire format.
//!
//! This module owns the exact record, fixed-array, and sum tags plus ordered
//! case payload envelopes. Field payloads remain delegated to the dedicated
//! structural-field codec.

use psi_terminal::{
    ByteSequenceCarrier, StructuralCaseDeclaration, StructuralTypeDeclaration, StructuralTypeShape,
};

use super::structural_field_wire::{decode_structural_field, encode_structural_field};
use super::wire::{Reader, Writer};
use super::{CodecError, decode_counted};

pub(super) fn encode_structural_type(
    writer: &mut Writer,
    declaration: &StructuralTypeDeclaration,
) -> Result<(), CodecError> {
    writer.id(declaration.id);
    writer.string("structural type identity", &declaration.identity)?;
    match &declaration.shape {
        StructuralTypeShape::ByteSequence(carrier) => {
            writer.u8(4);
            match carrier {
                ByteSequenceCarrier::BorrowedView => writer.u8(1),
                ByteSequenceCarrier::BoundedOwned { capacity } => {
                    writer.u8(2);
                    writer.u64(*capacity);
                }
            }
        }
        StructuralTypeShape::Record { fields } => {
            writer.u8(1);
            writer.len("structural fields", fields.len())?;
            for field in fields {
                encode_structural_field(writer, field)?;
            }
        }
        StructuralTypeShape::FixedArray { element, length } => {
            writer.u8(2);
            writer.id(*element);
            writer.u64(*length);
        }
        StructuralTypeShape::Sum { cases } => {
            writer.u8(3);
            writer.len("structural cases", cases.len())?;
            for case in cases {
                writer.id(case.id);
                writer.string("structural case identity", &case.identity)?;
                writer.len("structural case payload fields", case.fields.len())?;
                for field in &case.fields {
                    encode_structural_field(writer, field)?;
                }
            }
        }
        StructuralTypeShape::Mixed { fields, cases } => {
            writer.u8(5);
            writer.len("mixed structural fields", fields.len())?;
            for field in fields {
                encode_structural_field(writer, field)?;
            }
            writer.len("mixed structural cases", cases.len())?;
            for case in cases {
                writer.id(case.id);
                writer.string("mixed structural case identity", &case.identity)?;
                writer.len("mixed structural case payload fields", case.fields.len())?;
                for field in &case.fields {
                    encode_structural_field(writer, field)?;
                }
            }
        }
    }
    Ok(())
}

pub(super) fn decode_structural_type(
    reader: &mut Reader<'_>,
) -> Result<StructuralTypeDeclaration, CodecError> {
    let id = reader.id("StructuralTypeId")?;
    let identity = reader.string("structural type identity")?;
    let shape = match reader.u8()? {
        1 => StructuralTypeShape::Record {
            fields: decode_counted(reader, decode_structural_field)?,
        },
        2 => StructuralTypeShape::FixedArray {
            element: reader.id("StructuralTypeId")?,
            length: reader.u64()?,
        },
        3 => StructuralTypeShape::Sum {
            cases: decode_counted(reader, |reader| {
                Ok(StructuralCaseDeclaration {
                    id: reader.id("StructuralCaseId")?,
                    identity: reader.string("structural case identity")?,
                    fields: decode_counted(reader, decode_structural_field)?,
                })
            })?,
        },
        4 => StructuralTypeShape::ByteSequence(match reader.u8()? {
            1 => ByteSequenceCarrier::BorrowedView,
            2 => ByteSequenceCarrier::BoundedOwned {
                capacity: reader.u64()?,
            },
            tag => return Err(CodecError::InvalidTag("ByteSequenceCarrier", tag)),
        }),
        5 => StructuralTypeShape::Mixed {
            fields: decode_counted(reader, decode_structural_field)?,
            cases: decode_counted(reader, |reader| {
                Ok(StructuralCaseDeclaration {
                    id: reader.id("StructuralCaseId")?,
                    identity: reader.string("mixed structural case identity")?,
                    fields: decode_counted(reader, decode_structural_field)?,
                })
            })?,
        },
        tag => return Err(CodecError::InvalidTag("StructuralTypeShape", tag)),
    };
    Ok(StructuralTypeDeclaration {
        id,
        identity,
        shape,
    })
}
