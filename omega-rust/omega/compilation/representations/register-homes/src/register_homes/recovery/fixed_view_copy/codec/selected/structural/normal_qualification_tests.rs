//! A quantified qualification scope is retained metadata, never an address.

use super::declarations::{decode_path, decode_qualification_path, decode_reference_origin_path};
use super::provider;
use crate::FixedViewCopyDecodeError;
use crate::register_homes::recovery::fixed_view_copy::codec::primitives::Cursor;
use selected_instructions::effects::program::identity::encode_path;
use semantic_vocabulary::{BoundaryMachineId, MachineId, StructuralDomainId, StructuralTypeId};
use terminal_psi::{
    NominalDeclarationIdentity, ProviderCandidateConformance, ProviderParameterRefinement,
    ProviderRefinement, ProviderSignature, ProviderSignatureParameter, StructuralAccess,
    StructuralDomainRequirement, StructuralMultiplicity, StructuralPathQualification,
    StructuralPathSegment, TerminalIdentityIndex,
};

fn provider() -> ProviderCandidateConformance {
    let path = vec![
        StructuralPathSegment::Field(TerminalIdentityIndex(7)),
        StructuralPathSegment::Referent,
        StructuralPathSegment::AllElements,
    ];
    let domain = StructuralDomainId::new(3).unwrap();
    ProviderCandidateConformance {
        boundary: BoundaryMachineId::new(1).unwrap(),
        requirement_identity: TerminalIdentityIndex(1),
        source_requirement_identity: TerminalIdentityIndex(2),
        receiving_application: TerminalIdentityIndex(3),
        receiving_region_arguments: vec![TerminalIdentityIndex(4)],
        provider_identity: TerminalIdentityIndex(5),
        provider_declaration: NominalDeclarationIdentity::new(TerminalIdentityIndex(6)),
        candidate_identity: TerminalIdentityIndex(8),
        source_candidate_identity: TerminalIdentityIndex(9),
        source_candidate_package_identity: None,
        candidate: MachineId::new(2).unwrap(),
        signature: ProviderSignature {
            parameters: vec![ProviderSignatureParameter {
                position: 0,
                structural_type: StructuralTypeId::new(2).unwrap(),
                multiplicity: StructuralMultiplicity::Affine,
                access: StructuralAccess::MutableBorrow,
                qualifications: Vec::new(),
                projected_qualifications: vec![StructuralPathQualification {
                    path: path.clone(),
                    domain,
                }],
            }],
        },
        refinement: ProviderRefinement {
            positional_parameters: vec![ProviderParameterRefinement {
                boundary_index: 0,
                candidate_index: 0,
            }],
            required_domains: vec![StructuralDomainRequirement {
                argument_index: 0,
                path,
                domain,
            }],
            realized_service_ceiling: Vec::new(),
        },
    }
}

#[test]
fn provider_required_domain_round_trip_retains_quantified_formal_scope() {
    let expected = provider();
    let mut bytes = Vec::new();
    provider::encode(&mut bytes, &expected, true);
    let mut cursor = Cursor::new(&bytes);
    assert_eq!(provider::decode(&mut cursor, true).unwrap(), expected);
    assert_eq!(cursor.remaining(), 0);

    // Different exact scopes cannot collide in a retained provider envelope.
    let mut other = expected;
    other.refinement.required_domains[0].path[2] = StructuralPathSegment::FixedIndex(0);
    let mut other_bytes = Vec::new();
    provider::encode(&mut other_bytes, &other, true);
    assert_ne!(other_bytes, bytes);
    assert_eq!(
        provider::decode(&mut Cursor::new(&other_bytes), true).unwrap(),
        other
    );
}

#[test]
fn qualification_wildcard_cannot_decode_as_address_or_reference_origin() {
    let path = vec![StructuralPathSegment::AllElements];
    let mut bytes = Vec::new();
    encode_path(&mut bytes, &path);
    assert_eq!(
        decode_qualification_path(&mut Cursor::new(&bytes)).unwrap(),
        path
    );
    assert_eq!(
        decode_path(&mut Cursor::new(&bytes)),
        Err(FixedViewCopyDecodeError::UnknownStructuralPathSegment(10)),
    );
    assert_eq!(
        decode_reference_origin_path(&mut Cursor::new(&bytes)),
        Err(FixedViewCopyDecodeError::UnknownStructuralPathSegment(10)),
    );

    // Exact addresses retain their ordinary interpretation at both entrances.
    let path = vec![StructuralPathSegment::FixedIndex(2)];
    let mut bytes = Vec::new();
    encode_path(&mut bytes, &path);
    assert_eq!(decode_path(&mut Cursor::new(&bytes)).unwrap(), path);
    assert_eq!(
        decode_reference_origin_path(&mut Cursor::new(&bytes)).unwrap(),
        vec![terminal_psi::ReferenceOriginProjection::Exact(
            path[0].clone()
        )],
    );
}

#[test]
fn provider_required_domain_rejects_truncated_quantified_metadata() {
    let mut bytes = Vec::new();
    provider::encode(&mut bytes, &provider(), true);
    for length in 0..bytes.len() {
        assert!(
            provider::decode(&mut Cursor::new(&bytes[..length]), true).is_err(),
            "provider metadata prefix of {length} bytes must not be accepted"
        );
    }
}
