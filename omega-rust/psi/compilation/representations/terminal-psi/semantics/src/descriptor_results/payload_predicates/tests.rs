use super::*;
use semantic_vocabulary::{
    ByteSequencePredicate, IntegerSign, IntegerType, IntegerValue, PlaceId, StructuralTypeId,
};
use terminal_psi::{StructuralTypeShape, TerminalIdentityIndex};

#[test]
fn byte_predicate_observations_require_complete_current_typed_contents() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let scalar = StructuralTypeId::new(1).unwrap();
    let array = StructuralTypeId::new(2).unwrap();
    let subject = PlaceId::new(1).unwrap();
    let declaration = |id, shape| StructuralTypeDeclaration {
        quotient_carrier: None,
        id,
        identity: TerminalIdentityIndex(id.get() as u32),
        no_code_affine_cleanup: true,
        descriptor_cleanup: true,
        shape,
        scalar_restriction: None,
    };
    let types = vec![
        declaration(
            scalar,
            StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer)),
        ),
        declaration(
            array,
            StructuralTypeShape::FixedArray {
                element: scalar,
                length: 2,
            },
        ),
    ];
    let term = ScalarTerm::ByteSequencePredicate {
        predicate: ByteSequencePredicate::AsciiOnly,
        root: subject,
        path: Vec::new(),
    };
    let mut contract = PayloadMembershipContract {
        scalar_observations: [65u8, 66]
            .into_iter()
            .enumerate()
            .map(|(index, value)| PayloadScalarObservation {
                path: vec![DescriptorPathSegment::FixedIndex(index as u64)],
                value: ScalarTerm::integer(integer, IntegerValue::Unsigned(value.into())).unwrap(),
            })
            .collect(),
        scalar_requirements: Vec::new(),
        carrier: array,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let classify = |contract: &PayloadMembershipContract| {
        observed_byte_predicate(&types, contract, subject, &term)
    };
    assert_eq!(classify(&contract), Some(true));
    assert_eq!(
        observed_byte_predicate(&types, &contract, PlaceId::new(2).unwrap(), &term),
        None
    );
    let mut missing = contract.clone();
    missing.scalar_observations.pop();
    assert_eq!(classify(&missing), None);
    let mut duplicate = contract.clone();
    duplicate
        .scalar_observations
        .push(duplicate.scalar_observations[0].clone());
    assert_eq!(classify(&duplicate), None);
    let mut wrong_type = contract.clone();
    wrong_type.scalar_observations[1].value = ScalarTerm::Boolean(true);
    assert_eq!(classify(&wrong_type), None);
    contract.scalar_observations[1].value =
        ScalarTerm::integer(integer, IntegerValue::Unsigned(255)).unwrap();
    assert_eq!(classify(&contract), Some(false));
}
