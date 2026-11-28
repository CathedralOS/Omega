//! Stored result types, including the input paths available to a borrowed result.
use crate::CodecError;
use crate::fields::{Field, List, PathQualification};
use crate::wire::{Reader, Writer, decode_counted, decode_ids, strictly_increasing};
use psi_declaration_encoding::places::{
    decode_reference_origin_path, decode_structural_path, encode_reference_origin_path,
    encode_structural_path,
};
use psi_declaration_encoding::results::{decode_multiplicity, encode_multiplicity};
use terminal_psi::{StructuralPathQualification, StructuralResultDeclaration};
pub fn validate_reference_source_order(
    result: &StructuralResultDeclaration,
) -> Result<(), CodecError> {
    if !strictly_increasing(result.reference_sources.iter().map(|source| &source.path)) {
        return psi_declaration_encoding::error::malformed(
            "reference result sources must have unique ordered paths",
        );
    }
    for reference in &result.reference_sources {
        if reference.sources.is_empty() || !strictly_increasing(reference.sources.iter()) {
            return psi_declaration_encoding::error::malformed(
                "reference result alternatives must be nonempty, unique and ordered",
            );
        }
    }
    Ok(())
}

pub fn encode_function_result(
    writer: &mut Writer,
    result: &StructuralResultDeclaration,
) -> Result<(), CodecError> {
    writer.id(result.place);
    writer.id(result.structural_type);
    encode_multiplicity(writer, result.multiplicity);
    writer.ids("structural result qualifications", &result.qualifications)?;
    encode_projected_qualifications(writer, &result.projected_qualifications)?;
    writer.len(
        "structural reference result sources",
        result.reference_sources.len(),
    )?;
    for reference in &result.reference_sources {
        encode_structural_path(writer, "reference result path", &reference.path)?;
        writer.len("reference source alternatives", reference.sources.len())?;
        for source in &reference.sources {
            match source {
                terminal_psi::StructuralReferenceOrigin::Parameter(source) => {
                    writer.u8(1);
                    writer.id(source.place);
                }
                terminal_psi::StructuralReferenceOrigin::UpgradedParameter(source) => {
                    if source.access != terminal_psi::StructuralAccess::SharedBorrow {
                        return Err(CodecError::NonCanonicalEncoding);
                    }
                    writer.u8(3);
                    writer.id(source.place);
                }
                terminal_psi::StructuralReferenceOrigin::Invocation { carrier_type, .. } => {
                    writer.u8(2);
                    writer.id(*carrier_type);
                }
            }
            psi_declaration_encoding::access::mode::encode_structural_access(
                writer,
                source
                    .parameter()
                    .map_or_else(|| source.access(), |parameter| parameter.access),
            );
            writer.u8(match source.extent() {
                semantic_vocabulary::ReferenceOriginExtent::Projected => 1,
                semantic_vocabulary::ReferenceOriginExtent::WithinRegion => 2,
            });
            encode_reference_origin_path(writer, "reference source path", source.path())?;
        }
    }
    Ok(())
}

pub fn decode_function_result(
    reader: &mut Reader<'_>,
) -> Result<StructuralResultDeclaration, CodecError> {
    let result = StructuralResultDeclaration {
        place: reader.id("PlaceId")?,
        structural_type: reader.id("StructuralTypeId")?,
        multiplicity: decode_multiplicity(reader)?,
        qualifications: decode_ids(reader, "StructuralDomainId")?,
        projected_qualifications: decode_projected_qualifications(reader)?,
        reference_sources: decode_counted(reader, |reader| {
            Ok(terminal_psi::StructuralReferenceResultSource {
                path: decode_structural_path(reader)?,
                sources: decode_counted(reader, |reader| {
                    let kind = reader.u8()?;
                    let (parameter, carrier) = match kind {
                        1 | 3 => (Some(reader.id("PlaceId")?), None),
                        2 => (None, Some(reader.id("StructuralTypeId")?)),
                        tag => {
                            return Err(CodecError::InvalidTag("StructuralReferenceOrigin", tag));
                        }
                    };
                    let access =
                        psi_declaration_encoding::access::mode::decode_structural_access(reader)?;
                    let extent = match reader.u8()? {
                        1 => semantic_vocabulary::ReferenceOriginExtent::Projected,
                        2 => semantic_vocabulary::ReferenceOriginExtent::WithinRegion,
                        tag => {
                            return Err(CodecError::InvalidTag("ReferenceOriginExtent", tag));
                        }
                    };
                    let path = decode_reference_origin_path(reader)?;
                    match kind {
                        1 | 3 => {
                            let source = terminal_psi::ReferenceOriginSource {
                                place: parameter.ok_or(CodecError::NonCanonicalEncoding)?,
                                access,
                                extent,
                                path,
                            };
                            if kind == 1 {
                                Ok(terminal_psi::StructuralReferenceOrigin::Parameter(source))
                            } else if access == terminal_psi::StructuralAccess::SharedBorrow {
                                Ok(terminal_psi::StructuralReferenceOrigin::UpgradedParameter(
                                    source,
                                ))
                            } else {
                                Err(CodecError::NonCanonicalEncoding)
                            }
                        }
                        2 => Ok(terminal_psi::StructuralReferenceOrigin::Invocation {
                            carrier_type: carrier.ok_or(CodecError::NonCanonicalEncoding)?,
                            access,
                            extent,
                            path,
                        }),
                        tag => Err(CodecError::InvalidTag("StructuralReferenceOrigin", tag)),
                    }
                })?,
            })
        })?,
    };
    validate_reference_source_order(&result)?;
    Ok(result)
}

pub fn encode_projected_qualifications(
    writer: &mut Writer,
    qualifications: &[StructuralPathQualification],
) -> Result<(), CodecError> {
    writer.counted(
        "projected structural parameter qualifications",
        qualifications,
        PathQualification::write,
    )
}

pub fn decode_projected_qualifications(
    reader: &mut Reader<'_>,
) -> Result<Vec<StructuralPathQualification>, CodecError> {
    List::<PathQualification>::read(reader)
}

#[cfg(test)]
mod native_borrow_tests {
    use super::*;
    use semantic_vocabulary::{PlaceId, ReferenceOriginExtent, StructuralTypeId};
    use terminal_psi::{
        ReferenceOriginSource, StructuralAccess, StructuralMultiplicity, StructuralReferenceOrigin,
        StructuralReferenceResultSource,
    };

    #[test]
    fn upgrade_origin_names_the_shared_input_and_round_trips_its_effective_access() {
        let source = ReferenceOriginSource {
            place: PlaceId::new(1).unwrap(),
            path: vec![terminal_psi::ReferenceOriginProjection::Exact(
                terminal_psi::StructuralPathSegment::Referent,
            )],
            extent: ReferenceOriginExtent::Projected,
            access: StructuralAccess::SharedBorrow,
        };
        let declaration = |origin| StructuralResultDeclaration {
            place: PlaceId::new(3).unwrap(),
            structural_type: StructuralTypeId::new(2).unwrap(),
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            reference_sources: vec![StructuralReferenceResultSource {
                path: Vec::new(),
                sources: vec![origin],
            }],
        };
        let encoded = |result: &StructuralResultDeclaration| {
            let mut writer = Writer::default();
            encode_function_result(&mut writer, result)?;
            Ok::<_, CodecError>(writer.finish())
        };

        let upgraded = StructuralReferenceOrigin::UpgradedParameter(source.clone());
        assert_eq!(upgraded.access(), StructuralAccess::MutableBorrow);
        assert_eq!(upgraded.parameter(), Some(&source));
        let result = declaration(upgraded);
        let bytes = encoded(&result).unwrap();
        let mut reader = Reader::new(&bytes);
        assert_eq!(decode_function_result(&mut reader).unwrap(), result);
        assert_eq!(reader.remaining(), 0);

        let ordinary = encoded(&declaration(StructuralReferenceOrigin::Parameter(
            source.clone(),
        )))
        .unwrap();
        let differences: Vec<_> = ordinary
            .iter()
            .zip(&bytes)
            .enumerate()
            .filter(|(_, (regular, upgrade))| regular != upgrade)
            .collect();
        assert_eq!(differences.len(), 1);
        let (tag, (&old, &new)) = differences[0];
        assert_eq!((old, new), (1, 3));

        let mutable = ReferenceOriginSource {
            access: StructuralAccess::MutableBorrow,
            ..source
        };
        assert!(
            encoded(&declaration(StructuralReferenceOrigin::UpgradedParameter(
                mutable.clone()
            )))
            .is_err()
        );
        let mut forged =
            encoded(&declaration(StructuralReferenceOrigin::Parameter(mutable))).unwrap();
        forged[tag] = 3;
        assert!(matches!(
            decode_function_result(&mut Reader::new(&forged)),
            Err(CodecError::NonCanonicalEncoding)
        ));
    }
}
