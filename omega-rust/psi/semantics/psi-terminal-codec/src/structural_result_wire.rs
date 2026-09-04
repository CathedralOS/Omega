//! Canonical structural function-result and operation-result wire rows.

use psi_terminal::{
    StructuralMultiplicity, StructuralOperationResult, StructuralResultClaimBinding,
    StructuralResultDeclaration,
};

use super::structural_signature_wire::{
    decode_projected_qualifications, encode_projected_qualifications,
};
use super::wire::{Reader, Writer};
use super::{
    CodecError, decode_counted, decode_ids, decode_structural_path, encode_structural_path,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ResultPathWireFormat {
    LegacyWithoutResultPaths,
    Current,
}

impl ResultPathWireFormat {
    pub(super) const fn carries_result_paths(self) -> bool {
        matches!(self, Self::Current)
    }
}

pub(super) fn encode_function_result(
    writer: &mut Writer,
    result: &StructuralResultDeclaration,
    format: ResultPathWireFormat,
) -> Result<(), CodecError> {
    writer.id(result.place);
    writer.id(result.structural_type);
    encode_multiplicity(writer, result.multiplicity);
    writer.len(
        "structural result qualifications",
        result.qualifications.len(),
    )?;
    for qualification in &result.qualifications {
        writer.id(*qualification);
    }
    if format.carries_result_paths() {
        encode_projected_qualifications(writer, &result.projected_qualifications)?;
    }
    Ok(())
}

pub(super) fn decode_function_result(
    reader: &mut Reader<'_>,
    format: ResultPathWireFormat,
) -> Result<StructuralResultDeclaration, CodecError> {
    Ok(StructuralResultDeclaration {
        place: reader.id("PlaceId")?,
        structural_type: reader.id("StructuralTypeId")?,
        multiplicity: decode_multiplicity(reader)?,
        qualifications: decode_ids(reader, "StructuralDomainId")?,
        projected_qualifications: decode_result_paths(reader, format)?,
    })
}

pub(super) fn encode_operation_result(
    writer: &mut Writer,
    result: &StructuralOperationResult,
    format: ResultPathWireFormat,
) -> Result<(), CodecError> {
    writer.id(result.place);
    writer.id(result.structural_type);
    encode_multiplicity(writer, result.multiplicity);
    writer.len(
        "structural operation result qualifications",
        result.qualifications.len(),
    )?;
    for qualification in &result.qualifications {
        writer.id(*qualification);
    }
    if format.carries_result_paths() {
        encode_projected_qualifications(writer, &result.projected_qualifications)?;
    }
    writer.len("structural operation result claims", result.claims.len())?;
    for claim in &result.claims {
        writer.id(claim.claim);
        encode_structural_path(
            writer,
            "structural operation result claim path",
            &claim.path,
        )?;
    }
    Ok(())
}

pub(super) fn decode_operation_result(
    reader: &mut Reader<'_>,
    format: ResultPathWireFormat,
) -> Result<StructuralOperationResult, CodecError> {
    Ok(StructuralOperationResult {
        place: reader.id("PlaceId")?,
        structural_type: reader.id("StructuralTypeId")?,
        multiplicity: decode_multiplicity(reader)?,
        qualifications: decode_ids(reader, "StructuralDomainId")?,
        projected_qualifications: decode_result_paths(reader, format)?,
        claims: decode_counted(reader, |reader| {
            Ok(StructuralResultClaimBinding {
                claim: reader.id("ClaimId")?,
                path: decode_structural_path(reader)?,
            })
        })?,
    })
}

fn decode_result_paths(
    reader: &mut Reader<'_>,
    format: ResultPathWireFormat,
) -> Result<Vec<psi_terminal::StructuralPathQualification>, CodecError> {
    if format.carries_result_paths() {
        decode_projected_qualifications(reader)
    } else {
        Ok(Vec::new())
    }
}

fn encode_multiplicity(writer: &mut Writer, multiplicity: StructuralMultiplicity) {
    writer.u8(match multiplicity {
        StructuralMultiplicity::Unrestricted => 1,
        StructuralMultiplicity::Affine => 2,
        StructuralMultiplicity::Linear => 3,
    });
}

fn decode_multiplicity(reader: &mut Reader<'_>) -> Result<StructuralMultiplicity, CodecError> {
    match reader.u8()? {
        1 => Ok(StructuralMultiplicity::Unrestricted),
        2 => Ok(StructuralMultiplicity::Affine),
        3 => Ok(StructuralMultiplicity::Linear),
        tag => Err(CodecError::InvalidTag("StructuralMultiplicity", tag)),
    }
}
