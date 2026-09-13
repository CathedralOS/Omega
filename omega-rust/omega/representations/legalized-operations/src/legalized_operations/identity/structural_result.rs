//! Canonical structural result identities shared by ordinary graph operations.
use super::shared::*;
use super::structural_types::{
    encode_multiplicity, encode_projected_qualifications, encode_structural_argument,
    encode_structural_path,
};
use terminal_psi::{StructuralOperationResult, StructuralResultDeclaration};

pub(super) fn encode_operation_result(bytes: &mut Vec<u8>, result: &StructuralOperationResult) {
    bytes.extend_from_slice(&result.place.get().to_le_bytes());
    bytes.extend_from_slice(&result.structural_type.get().to_le_bytes());
    encode_multiplicity(bytes, result.multiplicity);
    encode_ids(
        bytes,
        result.qualifications.iter().map(|domain| domain.get()),
    );
    encode_projected_qualifications(bytes, &result.projected_qualifications);
    encode_len(bytes, result.claims.len());
    for claim in &result.claims {
        bytes.extend_from_slice(&claim.claim.get().to_le_bytes());
        encode_structural_path(bytes, &claim.path);
    }
}

pub(super) fn encode_result(bytes: &mut Vec<u8>, result: &StructuralResultDeclaration) {
    bytes.extend_from_slice(&result.place.get().to_le_bytes());
    bytes.extend_from_slice(&result.structural_type.get().to_le_bytes());
    encode_multiplicity(bytes, result.multiplicity);
    encode_ids(
        bytes,
        result.qualifications.iter().map(|domain| domain.get()),
    );
    encode_projected_qualifications(bytes, &result.projected_qualifications);
    encode_len(bytes, result.reference_sources.len());
    for reference in &result.reference_sources {
        encode_structural_path(bytes, &reference.path);
        encode_structural_argument(bytes, &reference.source);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{PlaceId, StructuralTypeId};
    use terminal_psi::StructuralReferenceResultSource;

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
                source: StructuralArgument {
                    place: PlaceId::new(2).unwrap(),
                    path: Vec::new(),
                    access: StructuralAccess::MutableBorrow,
                },
            }],
        };
        let identity = |result: &StructuralResultDeclaration| {
            let mut bytes = Vec::new();
            encode_result(&mut bytes, result);
            bytes
        };
        let original = identity(&result);
        for mutation in 0..5 {
            let mut changed = result.clone();
            match mutation {
                0 => changed.reference_sources.clear(),
                1 => changed.reference_sources[0].source.place = PlaceId::new(3).unwrap(),
                2 => changed.reference_sources[0]
                    .source
                    .path
                    .push(StructuralPathSegment::Referent),
                3 => changed.reference_sources[0]
                    .path
                    .push(StructuralPathSegment::Field("held".into())),
                4 => changed.reference_sources[0].source.access = StructuralAccess::SharedBorrow,
                _ => unreachable!(),
            }
            assert_ne!(identity(&changed), original, "custody mutation {mutation}");
        }
    }
}
