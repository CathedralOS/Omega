use super::*;
use semantic_vocabulary::{DomainSemanticId, ScalarDomainId, ScalarType};

fn catalog() -> ScalarQualificationCatalog {
    ScalarQualificationCatalog {
        domains: vec![ScalarDomainDeclaration {
            id: ScalarDomainId::new(1).unwrap(),
            semantic_domain: DomainSemanticId::new(9).unwrap(),
            identity: "carrier::Tag<7>".into(),
            carrier: ScalarType::Boolean,
        }],
        sets: vec![ScalarQualificationSet {
            id: ScalarQualificationSetId::new(1),
            domains: vec![ScalarDomainId::new(1).unwrap()],
        }],
        coercions: Vec::new(),
    }
}

#[test]
fn scalar_qualification_catalog_round_trip_retains_exact_identity() {
    let catalog = catalog();
    let mut writer = Writer::default();
    encode(&mut writer, &catalog).unwrap();
    let bytes = writer.finish();
    let mut reader = Reader::new(&bytes);
    assert_eq!(decode(&mut reader), Ok(catalog));
    assert_eq!(reader.remaining(), 0);
}

#[test]
fn scalar_qualification_catalog_rejects_zero_empty_and_duplicate_sets() {
    let mut catalog = catalog();
    catalog.sets[0].id = ScalarQualificationSetId::ZERO;
    assert!(validate(&catalog).is_err());
    catalog.sets[0].id = ScalarQualificationSetId::new(1);
    let mut duplicate = catalog.sets[0].clone();
    duplicate.id = ScalarQualificationSetId::new(2);
    catalog.sets.push(duplicate);
    assert!(validate(&catalog).is_err());
    catalog.sets.pop();
    catalog.sets[0].domains.clear();
    assert!(validate(&catalog).is_err());
    assert_eq!(validate(&ScalarQualificationCatalog::default()), Ok(()));
}

#[test]
fn scalar_qualification_catalog_rejects_noncanonical_rows_and_members() {
    let mut catalog = catalog();
    catalog.domains.push(catalog.domains[0].clone());
    assert!(validate(&catalog).is_err());
    catalog.domains.pop();
    let domain = catalog.sets[0].domains[0];
    catalog.sets[0].domains.push(domain);
    assert!(validate(&catalog).is_err());
    catalog.sets[0].domains = vec![
        ScalarDomainId::new(2).unwrap(),
        ScalarDomainId::new(1).unwrap(),
    ];
    assert!(validate(&catalog).is_err());
}

#[test]
fn scalar_qualification_catalog_rejects_hostile_counts_and_zero_domain_identity() {
    assert_eq!(
        decode(&mut Reader::new(&u32::MAX.to_le_bytes())),
        Err(CodecError::UnexpectedEnd)
    );
    let mut writer = Writer::default();
    encode(&mut writer, &catalog()).unwrap();
    let mut bytes = writer.finish();
    bytes[4..12].fill(0);
    assert_eq!(
        decode(&mut Reader::new(&bytes)),
        Err(CodecError::ZeroIdentity("ScalarDomainId"))
    );
}

#[test]
fn scalar_qualification_catalog_rejects_duplicate_and_reversed_coercions() {
    use semantic_vocabulary::{EdgeId, MachineId, ValueId};
    let mut catalog = catalog();
    let coercion = ScalarQualificationCoercion {
        machine: MachineId::new(1).unwrap(),
        edge: EdgeId::new(1).unwrap(),
        argument_ordinal: 0,
        source: ValueId::new(1).unwrap(),
        destination: ValueId::new(2).unwrap(),
    };
    catalog.coercions = vec![coercion, coercion];
    assert!(validate(&catalog).is_err());
    catalog.coercions[0].argument_ordinal = 1;
    assert!(validate(&catalog).is_err());
    catalog.coercions.reverse();
    assert_eq!(validate(&catalog), Ok(()));
    let mut writer = Writer::default();
    encode(&mut writer, &catalog).unwrap();
    assert_eq!(decode(&mut Reader::new(&writer.finish())), Ok(catalog));
}
