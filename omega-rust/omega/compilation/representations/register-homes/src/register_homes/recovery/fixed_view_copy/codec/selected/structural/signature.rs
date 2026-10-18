//! Ordinary structural signature data; no separate executable function payload.
use crate::FixedViewCopyDecodeError;
use crate::register_homes::recovery::fixed_view_copy::codec::primitives::{
    Cursor, decode_id, decode_ids, encode_ids, length,
};
use crate::register_homes::recovery::fixed_view_copy::codec::selected::structural::declarations::{
    decode_access, decode_entry_claim, decode_multiplicity, decode_parameter, decode_path,
    decode_place, decode_reference_origin_path, decode_type, encode_access, encode_entry_claim,
    encode_multiplicity, encode_parameter, encode_place, encode_type,
};
use legalized_operations::LegalizedStructuralContract;
use selected_instructions::effects::program::identity::{
    encode_path, encode_reference_origin_path,
};
use semantic_vocabulary::{PlaceId, ServiceId, StructuralDomainId, StructuralTypeId};
pub(super) fn encode_signature(
    bytes: &mut Vec<u8>,
    value: &LegalizedStructuralContract,
) -> Result<(), terminal_codec::CodecError> {
    length(bytes, value.structural_types.len());
    for row in &value.structural_types {
        encode_type(bytes, row)?;
    }
    length(bytes, value.structural_types.referents().len());
    for referent in value.structural_types.referents() {
        crate::codec_bytes::push_encoded(bytes, referent.id.get().to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, referent.identity.0.to_le_bytes());
    }
    let quotients = terminal_codec::encode_quotient_catalog(value.structural_types.quotients())?;
    length(bytes, quotients.len());
    bytes.extend_from_slice(&quotients);
    length(bytes, value.parameters.len());
    for row in &value.parameters {
        encode_parameter(bytes, row, true);
    }
    match &value.result {
        None => bytes.push(0),
        Some(result) => {
            bytes.push(1);
            crate::codec_bytes::push_encoded(bytes, result.place.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, result.structural_type.get().to_le_bytes());
            encode_multiplicity(bytes, result.multiplicity);
            encode_ids(
                bytes,
                result.qualifications.iter().map(|domain| domain.get()),
            );
            super::projected_qualifications::encode_projected(
                bytes,
                &result.projected_qualifications,
                true,
            );
            length(bytes, result.reference_sources.len());
            for reference in &result.reference_sources {
                encode_path(bytes, &reference.path);
                length(bytes, reference.sources.len());
                for source in &reference.sources {
                    match source {
                        terminal_psi::StructuralReferenceOrigin::Parameter(source) => {
                            bytes.push(1);
                            crate::codec_bytes::push_encoded(
                                bytes,
                                source.place.get().to_le_bytes(),
                            );
                        }
                        terminal_psi::StructuralReferenceOrigin::Invocation {
                            carrier_type,
                            ..
                        } => {
                            bytes.push(2);
                            crate::codec_bytes::push_encoded(
                                bytes,
                                carrier_type.get().to_le_bytes(),
                            );
                        }
                    }
                    encode_reference_origin_path(bytes, source.path());
                    encode_access(bytes, source.access());
                    bytes.push(match source.extent() {
                        semantic_vocabulary::ReferenceOriginExtent::Projected => 1,
                        semantic_vocabulary::ReferenceOriginExtent::WithinRegion => 2,
                    });
                }
            }
        }
    }
    length(bytes, value.structural_places.len());
    for row in &value.structural_places {
        encode_place(bytes, *row);
    }
    length(bytes, value.entry_claims.len());
    for row in &value.entry_claims {
        encode_entry_claim(bytes, row);
    }
    encode_ids(
        bytes,
        value.published_service_ceiling.iter().map(|id| id.get()),
    );
    Ok(())
}
pub(super) fn decode_signature(
    cursor: &mut Cursor<'_>,
) -> Result<LegalizedStructuralContract, FixedViewCopyDecodeError> {
    let count = cursor.length()?;
    let mut structural_types = Vec::new();
    for _ in 0..count {
        structural_types.push(decode_type(cursor)?);
    }
    let count = cursor.length()?;
    let mut referents = Vec::with_capacity(count);
    for _ in 0..count {
        referents.push(terminal_psi::StructuralReferentIdentity {
            id: decode_id(cursor, StructuralTypeId::new)?,
            identity: terminal_psi::TerminalIdentityIndex(cursor.u32()?),
        });
    }
    let count = cursor.length()?;
    let quotients = terminal_codec::decode_quotient_catalog(cursor.take(count)?)
        .map_err(|_| FixedViewCopyDecodeError::InvalidQuotientCatalog)?;
    terminal_semantics::quotients::validate_quotient_representations(&structural_types, &quotients)
        .map_err(|_| FixedViewCopyDecodeError::InvalidQuotientCatalog)?;
    let count = cursor.length()?;
    let mut parameters = Vec::new();
    for _ in 0..count {
        parameters.push(decode_parameter(cursor, true)?);
    }
    let result = crate::cursor::optional(
        cursor,
        |c| {
            Ok(terminal_psi::StructuralResultDeclaration {
                place: decode_id(c, PlaceId::new)?,
                structural_type: decode_id(c, StructuralTypeId::new)?,
                multiplicity: decode_multiplicity(c)?,
                qualifications: decode_ids(c, StructuralDomainId::new)?,
                projected_qualifications: super::projected_qualifications::decode_projected(
                    c, true,
                )?,
                reference_sources: {
                    let count = c.length()?;
                    let mut sources = Vec::new();
                    for _ in 0..count {
                        sources.push(terminal_psi::StructuralReferenceResultSource {
                            path: decode_path(c)?,
                            sources: {
                                let count = c.length()?;
                                let mut candidates = Vec::new();
                                for _ in 0..count {
                                    let (place, carrier_type) = match c.byte()? {
                                        1 => (Some(decode_id(c, PlaceId::new)?), None),
                                        2 => (None, Some(decode_id(c, StructuralTypeId::new)?)),
                                        tag => {
                                            return Err(FixedViewCopyDecodeError::UnknownOption(
                                                tag,
                                            ));
                                        }
                                    };
                                    let path = decode_reference_origin_path(c)?;
                                    let access = decode_access(c)?;
                                    let extent = match c.byte()? {
                                        1 => semantic_vocabulary::ReferenceOriginExtent::Projected,
                                        2 => {
                                            semantic_vocabulary::ReferenceOriginExtent::WithinRegion
                                        }
                                        tag => return Err(
                                            FixedViewCopyDecodeError::UnknownReferenceOriginExtent(
                                                tag,
                                            ),
                                        ),
                                    };
                                    candidates.push(match (place, carrier_type) {
                                        (Some(place), None) => {
                                            terminal_psi::StructuralReferenceOrigin::Parameter(
                                                terminal_psi::ReferenceOriginSource {
                                                    place,
                                                    path,
                                                    access,
                                                    extent,
                                                },
                                            )
                                        }
                                        (None, Some(carrier_type)) => {
                                            terminal_psi::StructuralReferenceOrigin::Invocation {
                                                carrier_type,
                                                path,
                                                access,
                                                extent,
                                            }
                                        }
                                        _ => {
                                            return Err(FixedViewCopyDecodeError::UnknownOption(0));
                                        }
                                    });
                                }
                                candidates
                            },
                        });
                    }
                    sources
                },
            })
        },
        FixedViewCopyDecodeError::UnknownOption,
    )?;
    let count = cursor.length()?;
    let mut structural_places = Vec::new();
    for _ in 0..count {
        structural_places.push(decode_place(cursor)?);
    }
    let count = cursor.length()?;
    let mut entry_claims = Vec::new();
    for _ in 0..count {
        entry_claims.push(decode_entry_claim(cursor)?);
    }
    Ok(LegalizedStructuralContract {
        structural_types: abstract_operations::StructuralTypeCatalog::new(
            structural_types,
            referents,
        )
        .with_quotients(quotients)
        .map_err(|_| FixedViewCopyDecodeError::InvalidQuotientCatalog)?,
        parameters,
        result,
        structural_places,
        entry_claims,
        published_service_ceiling: decode_ids(cursor, ServiceId::new)?,
    })
}

#[cfg(test)]
mod tests {
    use super::{Cursor, LegalizedStructuralContract, decode_signature, encode_signature};

    fn parameter(
        source: &mut terminal_psi::StructuralReferenceOrigin,
    ) -> &mut terminal_psi::ReferenceOriginSource {
        match source {
            terminal_psi::StructuralReferenceOrigin::Parameter(source) => source,
            terminal_psi::StructuralReferenceOrigin::Invocation { .. } => {
                panic!("expected parameter origin")
            }
        }
    }

    #[test]
    fn reference_signature_round_trip_retains_origins_and_referent_identity() {
        let signature = LegalizedStructuralContract {
            result: Some(terminal_psi::StructuralResultDeclaration {
                place: semantic_vocabulary::PlaceId::new(1).unwrap(),
                structural_type: semantic_vocabulary::StructuralTypeId::new(2).unwrap(),
                multiplicity: terminal_psi::StructuralMultiplicity::Affine,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                reference_sources: vec![terminal_psi::StructuralReferenceResultSource {
                    path: Vec::new(),
                    sources: [2, 3]
                        .into_iter()
                        .map(|place| terminal_psi::ReferenceOriginSource {
                            place: semantic_vocabulary::PlaceId::new(place).unwrap(),
                            access: terminal_psi::StructuralAccess::MutableBorrow,
                            extent: semantic_vocabulary::ReferenceOriginExtent::Projected,
                            path: Vec::new(),
                        })
                        .map(terminal_psi::StructuralReferenceOrigin::Parameter)
                        .collect(),
                }],
            }),
            structural_types: abstract_operations::StructuralTypeCatalog::new(
                vec![terminal_psi::StructuralTypeDeclaration {
                    quotient_carrier: None,
                    no_code_affine_cleanup: false,
                    descriptor_cleanup: false,
                    scalar_restriction: None,
                    id: semantic_vocabulary::StructuralTypeId::new(2).unwrap(),
                    identity: terminal_psi::TerminalIdentityIndex(1),
                    shape: terminal_psi::StructuralTypeShape::Reference {
                        referent: semantic_vocabulary::StructuralTypeId::new(1).unwrap(),
                        access: terminal_psi::StructuralAccess::MutableBorrow,
                    },
                }],
                vec![terminal_psi::StructuralReferentIdentity {
                    id: semantic_vocabulary::StructuralTypeId::new(1).unwrap(),
                    identity: terminal_psi::TerminalIdentityIndex(2),
                }],
            ),
            parameters: Vec::new(),
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
        };
        let mut signature = signature;
        signature.result.as_mut().unwrap().reference_sources[0]
            .sources
            .push(terminal_psi::StructuralReferenceOrigin::Invocation {
                carrier_type: semantic_vocabulary::StructuralTypeId::new(2).unwrap(),
                path: vec![terminal_psi::ReferenceOriginProjection::Exact(
                    terminal_psi::StructuralPathSegment::Referent,
                )],
                extent: semantic_vocabulary::ReferenceOriginExtent::Projected,
                access: terminal_psi::StructuralAccess::MutableBorrow,
            });
        let mut bytes = Vec::new();
        encode_signature(&mut bytes, &signature).unwrap();
        let mut cursor = Cursor::new(&bytes);
        assert_eq!(decode_signature(&mut cursor).unwrap(), signature);
        assert_eq!(cursor.remaining(), 0);
        let mut region = signature.clone();
        parameter(&mut region.result.as_mut().unwrap().reference_sources[0].sources[0]).extent =
            semantic_vocabulary::ReferenceOriginExtent::WithinRegion;
        let mut region_bytes = Vec::new();
        encode_signature(&mut region_bytes, &region).unwrap();
        assert_ne!(region_bytes, bytes);
        let mut region_cursor = Cursor::new(&region_bytes);
        assert_eq!(decode_signature(&mut region_cursor).unwrap(), region);
        assert_eq!(region_cursor.remaining(), 0);
        let mut family = signature.clone();
        parameter(&mut family.result.as_mut().unwrap().reference_sources[0].sources[0]).path = vec![
            terminal_psi::ReferenceOriginProjection::Exact(
                terminal_psi::StructuralPathSegment::RepresentationView {
                    structural_type: semantic_vocabulary::StructuralTypeId::new(2).unwrap(),
                    stored_scalar: None,
                },
            ),
            terminal_psi::ReferenceOriginProjection::Element,
            terminal_psi::ReferenceOriginProjection::Exact(
                terminal_psi::StructuralPathSegment::Field(terminal_psi::TerminalIdentityIndex(9)),
            ),
        ];
        let mut family_bytes = Vec::new();
        encode_signature(&mut family_bytes, &family).unwrap();
        assert_ne!(family_bytes, bytes);
        let mut family_cursor = Cursor::new(&family_bytes);
        assert_eq!(decode_signature(&mut family_cursor).unwrap(), family);
        assert_eq!(family_cursor.remaining(), 0);
        let mut renamed = signature.clone();
        renamed.structural_types = abstract_operations::StructuralTypeCatalog::new(
            signature.structural_types.to_vec(),
            vec![terminal_psi::StructuralReferentIdentity {
                id: semantic_vocabulary::StructuralTypeId::new(1).unwrap(),
                identity: terminal_psi::TerminalIdentityIndex(3),
            }],
        );
        let mut changed = Vec::new();
        encode_signature(&mut changed, &renamed).unwrap();
        assert_ne!(bytes, changed);
        let mut reordered = signature.clone();
        reordered.result.as_mut().unwrap().reference_sources[0]
            .sources
            .swap(0, 1);
        changed.clear();
        encode_signature(&mut changed, &reordered).unwrap();
        assert_ne!(bytes, changed);
        reordered.result.as_mut().unwrap().reference_sources[0]
            .sources
            .pop();
        changed.clear();
        encode_signature(&mut changed, &reordered).unwrap();
        assert_ne!(bytes, changed);
    }
}
