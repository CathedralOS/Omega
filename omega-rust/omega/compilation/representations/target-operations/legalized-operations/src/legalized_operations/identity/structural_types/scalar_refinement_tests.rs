use super::{
    BindingRelevance, StructuralFieldDeclaration, StructuralFieldType, encode_structural_fields,
};
use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, Proposition, ScalarRefinement, ScalarTerm, ScalarType,
    StructuralFieldId,
};

#[test]
fn refined_field_identity_retains_carrier_predicate_operator_and_literal() {
    let integer = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    let carrier = ScalarType::Integer(integer);
    let subject = ScalarTerm::value(ScalarRefinement::SUBJECT, carrier);
    let zero = ScalarTerm::integer(integer, IntegerValue::Signed(0)).unwrap();
    let one = ScalarTerm::integer(integer, IntegerValue::Signed(1)).unwrap();
    let fields = [
        (
            carrier,
            Proposition::LessThan(zero.clone(), subject.clone()),
        ),
        (carrier, Proposition::LessOrEqual(zero, subject.clone())),
        (carrier, Proposition::LessThan(one, subject)),
        (carrier, Proposition::Truth),
        (ScalarType::Boolean, Proposition::Truth),
    ];
    let identities = fields.map(|(carrier, predicate)| {
        let field = StructuralFieldDeclaration {
            id: StructuralFieldId::new(1).unwrap(),
            identity: "value".into(),
            relevance: BindingRelevance::Relevant,
            field_type: StructuralFieldType::RefinedScalar(
                ScalarRefinement::new(carrier, predicate).unwrap(),
            ),
        };
        let mut bytes = Vec::new();
        encode_structural_fields(&mut bytes, &[field]);
        bytes
    });
    for (position, identity) in identities.iter().enumerate() {
        assert!(!identities[..position].contains(identity));
    }
}
