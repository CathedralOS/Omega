//! Scalar and structural results carried by calls and dynamic dispatch slots.
use crate::CodecError;
use crate::fields::{Field, Id, Ids, List, Scalar, wire_enum, wire_record};
use crate::wire::{Reader, Writer};
use semantic_vocabulary::QualifiedScalarType;
use terminal_psi::{
    BoundaryMachineResult, BoundaryStructuralResultDeclaration, StructuralMultiplicity,
    StructuralPathQualification, StructuralPathSegment,
};

pub struct QualificationPath;
impl Field<Vec<StructuralPathSegment>> for QualificationPath {
    fn write(writer: &mut Writer, value: &Vec<StructuralPathSegment>) -> Result<(), CodecError> {
        crate::places::encode_qualification_path(writer, value)
    }
    fn read(reader: &mut Reader<'_>) -> Result<Vec<StructuralPathSegment>, CodecError> {
        crate::places::decode_qualification_path(reader)
    }
}

pub struct QualificationSet;
impl Field<semantic_vocabulary::ScalarQualificationSetId> for QualificationSet {
    fn write(
        writer: &mut Writer,
        value: &semantic_vocabulary::ScalarQualificationSetId,
    ) -> Result<(), CodecError> {
        writer.u64(value.get());
        Ok(())
    }
    fn read(
        reader: &mut Reader<'_>,
    ) -> Result<semantic_vocabulary::ScalarQualificationSetId, CodecError> {
        Ok(semantic_vocabulary::ScalarQualificationSetId::new(
            reader.u64()?,
        ))
    }
}

wire_record! {
    PathQualification: StructuralPathQualification { path: QualificationPath, domain: Id }
}

wire_record! {
        StructuralResult: BoundaryStructuralResultDeclaration {
            structural_type: Id,
            multiplicity: Multiplicity,
            qualifications: Ids,
            projected_qualifications: List<PathQualification>,
        }
        QualifiedScalar: QualifiedScalarType { scalar_type: Scalar, qualifications: QualificationSet }
}

wire_enum! {
        CallResult: BoundaryMachineResult, "BoundaryMachineResult" {
            0 => Unit,
            1 => Scalar(result: QualifiedScalar),
            2 => Structural(result: StructuralResult),
        }
}

pub struct Multiplicity;

impl Field<StructuralMultiplicity> for Multiplicity {
    fn write(writer: &mut Writer, value: &StructuralMultiplicity) -> Result<(), CodecError> {
        encode_multiplicity(writer, *value);
        Ok(())
    }

    fn read(reader: &mut Reader<'_>) -> Result<StructuralMultiplicity, CodecError> {
        decode_multiplicity(reader)
    }
}

pub fn encode_multiplicity(writer: &mut Writer, multiplicity: StructuralMultiplicity) {
    writer.u8(match multiplicity {
        StructuralMultiplicity::Unrestricted => 1,
        StructuralMultiplicity::Affine => 2,
        StructuralMultiplicity::Linear => 3,
    });
}

pub fn decode_multiplicity(reader: &mut Reader<'_>) -> Result<StructuralMultiplicity, CodecError> {
    match reader.u8()? {
        1 => Ok(StructuralMultiplicity::Unrestricted),
        2 => Ok(StructuralMultiplicity::Affine),
        3 => Ok(StructuralMultiplicity::Linear),
        tag => Err(CodecError::InvalidTag("StructuralMultiplicity", tag)),
    }
}
