use psi_declaration_encoding::{
    access::lifecycles::decode_access_lifecycle_catalog, domains::Domain,
    encode_access_lifecycle_catalog, encode_quotient_catalog, encode_structural_domain_declaration,
    fields::Field, quotients::decode_quotient_catalog, wire::Reader,
};
use semantic_vocabulary::{DomainSemanticId, StructuralTypeId};

// The Psi artifact writer at 296ef7ad0bf produced these records. Native program
// identities must retain their bytes when the declarations are written again.
#[test]
fn structural_domain_identity_keeps_its_predicate_and_membership() {
    let bytes = include_bytes!("fixtures/identity/domain.bin");
    let mut reader = Reader::new(bytes);
    let domain = Domain::read(&mut reader).unwrap();
    assert_eq!(reader.remaining(), 0);
    assert!(domain.predicate.is_some());
    assert_eq!(
        domain.required_memberships,
        Some(vec![DomainSemanticId::new(6).unwrap()])
    );
    assert_eq!(
        encode_structural_domain_declaration(&domain).unwrap(),
        bytes
    );
}

#[test]
fn quotient_identity_keeps_its_representation_and_relation() {
    let bytes = include_bytes!("fixtures/identity/quotients.bin");
    let catalog = decode_quotient_catalog(bytes).unwrap();
    assert_eq!(catalog.representations[0].formation.relation, "=");
    assert_eq!(encode_quotient_catalog(&catalog).unwrap(), bytes);
}

#[test]
fn access_identity_keeps_its_placement_and_storage() {
    let bytes = include_bytes!("fixtures/identity/access.bin");
    let mut plans = decode_access_lifecycle_catalog(bytes).unwrap();
    assert_eq!(plans[0].storage.carrier, StructuralTypeId::new(13).unwrap());
    assert_eq!(encode_access_lifecycle_catalog(&plans).unwrap(), bytes);
    plans[0].storage.vacant = None;
    assert_eq!(
        encode_access_lifecycle_catalog(&plans),
        Err(psi_declaration_encoding::CodecError::NonCanonicalEncoding),
    );
}
