//! Canonical structural function-result and operation-result wire rows.

use terminal_psi::{
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

pub(super) fn validate_reference_sources(
    module: &terminal_psi::TerminalModule,
    machine: &terminal_psi::TerminalMachine,
) -> Result<(), CodecError> {
    let Some(result) = machine.result.structural() else {
        return Ok(());
    };
    if result
        .reference_sources
        .windows(2)
        .any(|pair| pair[0].path >= pair[1].path)
    {
        return super::malformed("reference result sources must have unique ordered paths");
    }
    for reference in &result.reference_sources {
        let reference_type =
            super::validate_structural_path(module, result.structural_type, &reference.path)?;
        let Some(terminal_psi::StructuralTypeShape::Reference { referent, access }) = module
            .structural_types
            .iter()
            .find(|row| row.id == reference_type)
            .map(|row| &row.shape)
        else {
            return super::malformed("reference result source must identify a reference leaf");
        };
        let Some(parameter) = machine
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == reference.source.place)
        else {
            return super::malformed("reference result source must identify a formal ingress");
        };
        if *access != reference.source.access
            || *referent
                != super::validate_structural_path(
                    module,
                    parameter.structural_type,
                    &reference.source.path,
                )?
        {
            return super::malformed("reference result source has mismatched referent or access");
        }
    }
    Ok(())
}

pub(super) fn encode_function_result(
    writer: &mut Writer,
    result: &StructuralResultDeclaration,
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
    encode_projected_qualifications(writer, &result.projected_qualifications)?;
    writer.len(
        "structural reference result sources",
        result.reference_sources.len(),
    )?;
    for reference in &result.reference_sources {
        encode_structural_path(writer, "reference result path", &reference.path)?;
        writer.id(reference.source.place);
        super::structural_signature_wire::encode_structural_access(writer, reference.source.access);
        encode_structural_path(writer, "reference source path", &reference.source.path)?;
    }
    Ok(())
}

pub(super) fn decode_function_result(
    reader: &mut Reader<'_>,
) -> Result<StructuralResultDeclaration, CodecError> {
    Ok(StructuralResultDeclaration {
        place: reader.id("PlaceId")?,
        structural_type: reader.id("StructuralTypeId")?,
        multiplicity: decode_multiplicity(reader)?,
        qualifications: decode_ids(reader, "StructuralDomainId")?,
        projected_qualifications: decode_projected_qualifications(reader)?,
        reference_sources: decode_counted(reader, |reader| {
            Ok(terminal_psi::StructuralReferenceResultSource {
                path: decode_structural_path(reader)?,
                source: terminal_psi::StructuralArgument {
                    place: reader.id("PlaceId")?,
                    access: super::structural_signature_wire::decode_structural_access(reader)?,
                    path: decode_structural_path(reader)?,
                },
            })
        })?,
    })
}

pub(super) fn encode_operation_result(
    writer: &mut Writer,
    result: &StructuralOperationResult,
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
    encode_projected_qualifications(writer, &result.projected_qualifications)?;
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
) -> Result<StructuralOperationResult, CodecError> {
    Ok(StructuralOperationResult {
        place: reader.id("PlaceId")?,
        structural_type: reader.id("StructuralTypeId")?,
        multiplicity: decode_multiplicity(reader)?,
        qualifications: decode_ids(reader, "StructuralDomainId")?,
        projected_qualifications: decode_projected_qualifications(reader)?,
        claims: decode_counted(reader, |reader| {
            Ok(StructuralResultClaimBinding {
                claim: reader.id("ClaimId")?,
                path: decode_structural_path(reader)?,
            })
        })?,
    })
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

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{PlaceId, StructuralTypeId};
    use terminal_psi::{
        StructuralAccess, StructuralArgument, StructuralPathSegment,
        StructuralReferenceResultSource,
    };

    #[test]
    fn reference_result_wire_retains_leaf_source_path_and_access() {
        let result = StructuralResultDeclaration {
            place: PlaceId::new(9).unwrap(),
            structural_type: StructuralTypeId::new(3).unwrap(),
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            reference_sources: vec![StructuralReferenceResultSource {
                path: vec![StructuralPathSegment::Field("body".into())],
                source: StructuralArgument {
                    place: PlaceId::new(1).unwrap(),
                    path: vec![
                        StructuralPathSegment::Referent,
                        StructuralPathSegment::FixedIndex(2),
                    ],
                    access: StructuralAccess::MutableBorrow,
                },
            }],
        };
        let mut writer = Writer::default();
        encode_function_result(&mut writer, &result).unwrap();
        let bytes = writer.finish();
        let mut reader = Reader::new(&bytes);
        assert_eq!(decode_function_result(&mut reader).unwrap(), result);
        assert_eq!(reader.remaining(), 0);

        for change in 0..4 {
            let mut changed = result.clone();
            let source = &mut changed.reference_sources[0];
            match change {
                0 => source.path.clear(),
                1 => source.source.place = PlaceId::new(2).unwrap(),
                2 => source.source.path.clear(),
                _ => source.source.access = StructuralAccess::SharedBorrow,
            }
            let mut writer = Writer::default();
            encode_function_result(&mut writer, &changed).unwrap();
            assert_ne!(
                writer.finish(),
                bytes,
                "reference custody cannot erase from wire identity"
            );
        }
    }
}
