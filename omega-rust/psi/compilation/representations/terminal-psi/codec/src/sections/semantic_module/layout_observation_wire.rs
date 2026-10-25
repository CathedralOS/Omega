//! One exact geometry subject on executable operations and erased proof terms.
use super::CodecError;
use super::scalar_wire::{decode_scalar_type, encode_scalar_type};
use super::wire::{Reader, Writer};
use semantic_vocabulary::{LayoutObservationCarrier as Carrier, LayoutObservationKind as Kind};

pub(crate) fn encode_subject(writer: &mut Writer, kind: Kind, carrier: Carrier) {
    writer.u8(match kind {
        Kind::Size => 1,
        Kind::Alignment => 2,
    });
    match carrier {
        Carrier::Unit => writer.u8(1),
        Carrier::Scalar(scalar) => {
            writer.u8(2);
            encode_scalar_type(writer, scalar);
        }
        Carrier::Structural(identity) => {
            writer.u8(3);
            writer.id(identity);
        }
    }
}

pub(crate) fn decode_subject(reader: &mut Reader<'_>) -> Result<(Kind, Carrier), CodecError> {
    let kind = match reader.u8()? {
        1 => Kind::Size,
        2 => Kind::Alignment,
        tag => return Err(CodecError::InvalidTag("LayoutObservationKind", tag)),
    };
    let carrier = match reader.u8()? {
        1 => Carrier::Unit,
        2 => Carrier::Scalar(decode_scalar_type(reader)?),
        3 => Carrier::Structural(reader.id("StructuralTypeId")?),
        tag => return Err(CodecError::InvalidTag("LayoutObservationCarrier", tag)),
    };
    Ok((kind, carrier))
}
