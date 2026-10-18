//! Canonical structural result identities shared by ordinary graph operations.
use super::encoding::{encode_ids, encode_len};
use super::structural_types::{
    encode_multiplicity, encode_projected_qualifications, encode_reference_origin,
    encode_structural_path,
};
use terminal_psi::{StructuralOperationResult, StructuralResultDeclaration};

pub(super) fn encode_operation_result(bytes: &mut Vec<u8>, result: &StructuralOperationResult) {
    crate::codec_bytes::push_encoded(bytes, result.place.get().to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, result.structural_type.get().to_le_bytes());
    encode_multiplicity(bytes, result.multiplicity);
    encode_ids(
        bytes,
        result.qualifications.iter().map(|domain| domain.get()),
    );
    encode_projected_qualifications(bytes, &result.projected_qualifications);
    encode_len(bytes, result.claims.len());
    for claim in &result.claims {
        crate::codec_bytes::push_encoded(bytes, claim.claim.get().to_le_bytes());
        encode_structural_path(bytes, &claim.path);
    }
}

pub(super) fn encode_result(bytes: &mut Vec<u8>, result: &StructuralResultDeclaration) {
    crate::codec_bytes::push_encoded(bytes, result.place.get().to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, result.structural_type.get().to_le_bytes());
    encode_multiplicity(bytes, result.multiplicity);
    encode_ids(
        bytes,
        result.qualifications.iter().map(|domain| domain.get()),
    );
    encode_projected_qualifications(bytes, &result.projected_qualifications);
    encode_len(bytes, result.reference_sources.len());
    for reference in &result.reference_sources {
        encode_structural_path(bytes, &reference.path);
        encode_len(bytes, reference.sources.len());
        for source in &reference.sources {
            encode_reference_origin(bytes, source);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{StructuralResultDeclaration, encode_result};
    use semantic_vocabulary::{PlaceId, StructuralTypeId};
    use terminal_psi::{
        StructuralAccess, StructuralArgument, StructuralMultiplicity, StructuralPathSegment,
        StructuralReferenceResultSource,
    };

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
    fn reference_result_identity_retains_exact_origin_and_leaf_path() {
        let result = StructuralResultDeclaration {
            place: PlaceId::new(1).unwrap(),
            structural_type: StructuralTypeId::new(1).unwrap(),
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            reference_sources: vec![StructuralReferenceResultSource {
                path: Vec::new(),
                sources: vec![
                    StructuralArgument {
                        place: PlaceId::new(2).unwrap(),
                        path: Vec::new(),
                        access: StructuralAccess::MutableBorrow,
                    }
                    .into(),
                    StructuralArgument {
                        place: PlaceId::new(4).unwrap(),
                        path: Vec::new(),
                        access: StructuralAccess::MutableBorrow,
                    }
                    .into(),
                ],
            }],
        };
        let identity = |result: &StructuralResultDeclaration| {
            let mut bytes = Vec::new();
            encode_result(&mut bytes, result);
            bytes
        };
        let original = identity(&result);
        for mutation in 0..13 {
            let mut changed = result.clone();
            match mutation {
                0 => changed.reference_sources.clear(),
                1 => {
                    parameter(&mut changed.reference_sources[0].sources[0]).place =
                        PlaceId::new(3).unwrap()
                }
                2 => parameter(&mut changed.reference_sources[0].sources[0])
                    .path
                    .push(terminal_psi::ReferenceOriginProjection::Exact(
                        StructuralPathSegment::Referent,
                    )),
                3 => changed.reference_sources[0]
                    .path
                    .push(StructuralPathSegment::Field(
                        terminal_psi::TerminalIdentityIndex(2),
                    )),
                4 => {
                    parameter(&mut changed.reference_sources[0].sources[0]).access =
                        StructuralAccess::SharedBorrow
                }
                5 => {
                    parameter(&mut changed.reference_sources[0].sources[1]).place =
                        PlaceId::new(5).unwrap()
                }
                6 => changed.reference_sources[0].sources.swap(0, 1),
                7 => {
                    changed.reference_sources[0].sources.pop();
                }
                8 => parameter(&mut changed.reference_sources[0].sources[0])
                    .path
                    .push(terminal_psi::ReferenceOriginProjection::Element),
                9 => {
                    parameter(&mut changed.reference_sources[0].sources[0]).extent =
                        semantic_vocabulary::ReferenceOriginExtent::WithinRegion
                }
                10..13 => {
                    changed.reference_sources[0].sources[0] =
                        terminal_psi::StructuralReferenceOrigin::Invocation {
                            carrier_type: StructuralTypeId::new(if mutation == 11 { 3 } else { 2 })
                                .unwrap(),
                            path: if mutation == 12 {
                                vec![terminal_psi::ReferenceOriginProjection::Element]
                            } else {
                                Vec::new()
                            },
                            extent: semantic_vocabulary::ReferenceOriginExtent::Projected,
                            access: StructuralAccess::MutableBorrow,
                        };
                }
                _ => unreachable!(),
            }
            assert_ne!(identity(&changed), original, "custody mutation {mutation}");
        }
    }
}
