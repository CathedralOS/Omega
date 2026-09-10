use super::*;
use semantic_vocabulary::{DomainSemanticId, ScalarDomainId, ScalarQualificationSetId};
use terminal_psi::{ScalarDomainDeclaration, ScalarQualificationSet};

#[test]
fn scalar_qualification_catalog_round_trips_and_commits_semantic_identity() {
    let mut module = unit_fixture();
    module
        .scalar_qualifications
        .domains
        .push(ScalarDomainDeclaration {
            id: ScalarDomainId::new(1).unwrap(),
            semantic_domain: DomainSemanticId::new(7).unwrap(),
            identity: "bool::Tag<7>".into(),
            carrier: ScalarType::Boolean,
        });
    module
        .scalar_qualifications
        .sets
        .push(ScalarQualificationSet {
            id: ScalarQualificationSetId::new(1),
            domains: vec![ScalarDomainId::new(1).unwrap()],
        });
    let bytes = encode_module(&module).unwrap();
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    let original = semantic_fingerprint(&module).unwrap();
    module.scalar_qualifications.domains[0].identity = "bool::Tag<8>".into();
    assert_ne!(semantic_fingerprint(&module).unwrap(), original);
    module.scalar_qualifications.domains[0].semantic_domain = DomainSemanticId::new(8).unwrap();
    assert_ne!(encode_module(&module).unwrap(), bytes);
}

#[test]
fn scalar_qualification_catalog_rejects_previous_vocabulary() {
    let mut bytes = encode_module(&unit_fixture()).unwrap();
    bytes[10..12].copy_from_slice(&94_u16.to_le_bytes());
    assert_eq!(
        decode_module(&bytes),
        Err(CodecError::UnsupportedVocabularyMarker(94))
    );
}
