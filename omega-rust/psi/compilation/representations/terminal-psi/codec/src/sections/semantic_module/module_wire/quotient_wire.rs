//! Canonical quotient catalog encoding. Field order and variant tags are explicit.
use super::super::{
    CodecError,
    wire::{Reader, Writer, decode_canonical, decode_counted},
};
use language_semantics::quotient_correspondence::*;
use semantic_vocabulary::{MachineId, OperationId, PlaceId, StructuralTypeId};
use terminal_psi::terminal_module::quotients::*;

trait Wire: Sized {
    fn write(&self, w: &mut Writer) -> Result<(), CodecError>;
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError>;
}
impl Wire for String {
    fn write(&self, w: &mut Writer) -> Result<(), CodecError> {
        w.string("quotient identity", self)
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        r.string("quotient identity")
    }
}
impl Wire for u32 {
    fn write(&self, w: &mut Writer) -> Result<(), CodecError> {
        w.u32(*self);
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        r.u32()
    }
}
impl<T: Wire> Wire for Vec<T> {
    fn write(&self, w: &mut Writer) -> Result<(), CodecError> {
        w.len("quotient rows", self.len())?;
        for x in self {
            x.write(w)?;
        }
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        decode_counted(r, T::read)
    }
}

impl Wire for MachineId {
    fn write(&self, w: &mut Writer) -> Result<(), CodecError> {
        w.id(*self);
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        r.id("quotient MachineId")
    }
}

impl Wire for OperationId {
    fn write(&self, w: &mut Writer) -> Result<(), CodecError> {
        w.id(*self);
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        r.id("quotient OperationId")
    }
}

impl Wire for PlaceId {
    fn write(&self, w: &mut Writer) -> Result<(), CodecError> {
        w.id(*self);
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        r.id("quotient PlaceId")
    }
}

impl Wire for StructuralTypeId {
    fn write(&self, w: &mut Writer) -> Result<(), CodecError> {
        w.id(*self);
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        r.id("quotient StructuralTypeId")
    }
}

impl Wire for QuotientRelationIdentity {
    fn write(&self, w: &mut Writer) -> Result<(), CodecError> {
        self.quotient_declaration.write(w)?;
        self.quotient_type.write(w)?;
        self.carrier_type.write(w)?;
        self.relation.write(w)?;
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        Ok(Self {
            quotient_declaration: Wire::read(r)?,
            quotient_type: Wire::read(r)?,
            carrier_type: Wire::read(r)?,
            relation: Wire::read(r)?,
        })
    }
}

impl Wire for QuotientPositionalRelation {
    fn write(&self, w: &mut Writer) -> Result<(), CodecError> {
        match self {
            Self::Quotient(value) => {
                w.u8(1);
                value.write(w)?;
            }
            Self::ExactEquality {
                public_type,
                representative_type,
            } => {
                w.u8(2);
                public_type.write(w)?;
                representative_type.write(w)?;
            }
        };
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        match r.u8()? {
            1 => Ok(Self::Quotient(Wire::read(r)?)),
            2 => Ok(Self::ExactEquality {
                public_type: Wire::read(r)?,
                representative_type: Wire::read(r)?,
            }),
            tag => Err(CodecError::InvalidTag("QuotientPositionalRelation", tag)),
        }
    }
}

impl Wire for TerminalQuotientCatalog {
    fn write(&self, w: &mut Writer) -> Result<(), CodecError> {
        self.representations.write(w)?;
        self.operations.write(w)?;
        self.adaptations.write(w)?;
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        Ok(Self {
            representations: Wire::read(r)?,
            operations: Wire::read(r)?,
            adaptations: Wire::read(r)?,
        })
    }
}

impl Wire for TerminalQuotientRepresentation {
    fn write(&self, w: &mut Writer) -> Result<(), CodecError> {
        self.quotient.write(w)?;
        self.carrier.write(w)?;
        self.quotient_storage_identity.0.write(w)?;
        self.carrier_storage_identity.0.write(w)?;
        self.formation.write(w)?;
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        Ok(Self {
            quotient: Wire::read(r)?,
            carrier: Wire::read(r)?,
            quotient_storage_identity: terminal_psi::TerminalIdentityIndex(Wire::read(r)?),
            carrier_storage_identity: terminal_psi::TerminalIdentityIndex(Wire::read(r)?),
            formation: Wire::read(r)?,
        })
    }
}

impl Wire for TerminalQuotientOperation {
    fn write(&self, w: &mut Writer) -> Result<(), CodecError> {
        self.public_machine.write(w)?;
        self.representative_machine.write(w)?;
        self.call.write(w)?;
        self.input_relations.write(w)?;
        self.result_relation.write(w)?;
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        Ok(Self {
            public_machine: Wire::read(r)?,
            representative_machine: Wire::read(r)?,
            call: Wire::read(r)?,
            input_relations: Wire::read(r)?,
            result_relation: Wire::read(r)?,
        })
    }
}

impl Wire for TerminalQuotientAdaptationLicense {
    fn write(&self, w: &mut Writer) -> Result<(), CodecError> {
        match self {
            Self::Mint => {
                w.u8(1);
            }
            Self::Input {
                operation,
                position,
            } => {
                w.u8(2);
                operation.write(w)?;
                position.write(w)?;
            }
            Self::Result { operation } => {
                w.u8(3);
                operation.write(w)?;
            }
        };
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        match r.u8()? {
            1 => Ok(Self::Mint),
            2 => Ok(Self::Input {
                operation: Wire::read(r)?,
                position: Wire::read(r)?,
            }),
            3 => Ok(Self::Result {
                operation: Wire::read(r)?,
            }),
            tag => Err(CodecError::InvalidTag(
                "TerminalQuotientAdaptationLicense",
                tag,
            )),
        }
    }
}

impl Wire for TerminalQuotientAdaptation {
    fn write(&self, w: &mut Writer) -> Result<(), CodecError> {
        self.machine.write(w)?;
        self.copy.write(w)?;
        self.source.write(w)?;
        self.result.write(w)?;
        self.source_type.write(w)?;
        self.result_type.write(w)?;
        self.representation.write(w)?;
        self.license.write(w)?;
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        Ok(Self {
            machine: Wire::read(r)?,
            copy: Wire::read(r)?,
            source: Wire::read(r)?,
            result: Wire::read(r)?,
            source_type: Wire::read(r)?,
            result_type: Wire::read(r)?,
            representation: Wire::read(r)?,
            license: Wire::read(r)?,
        })
    }
}

pub(super) fn encode(w: &mut Writer, catalog: &TerminalQuotientCatalog) -> Result<(), CodecError> {
    catalog.write(w)
}
pub(super) fn decode(r: &mut Reader<'_>) -> Result<TerminalQuotientCatalog, CodecError> {
    Wire::read(r)
}

/// Encode retained identity data; this does not admit its mathematical claims.
pub fn encode_quotient_catalog(catalog: &TerminalQuotientCatalog) -> Result<Vec<u8>, CodecError> {
    let mut writer = Writer::default();
    catalog.write(&mut writer)?;
    Ok(writer.finish())
}
pub fn decode_quotient_catalog(bytes: &[u8]) -> Result<TerminalQuotientCatalog, CodecError> {
    decode_canonical(
        bytes,
        TerminalQuotientCatalog::read,
        encode_quotient_catalog,
    )
}
