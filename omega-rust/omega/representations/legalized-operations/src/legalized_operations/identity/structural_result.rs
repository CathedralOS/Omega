//! Canonical structural result identities shared by ordinary graph operations.
use super::shared::*;
use super::structural_types::{
    encode_multiplicity, encode_projected_qualifications, encode_structural_path,
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
}
