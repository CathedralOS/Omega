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
        owns_linear_claim: false,
        no_code_affine_cleanup: true,
        copyable: false,
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
                length: 2.into(),
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
                value: Some(
                    ScalarTerm::integer(integer, IntegerValue::Unsigned(value.into())).unwrap(),
                ),
                entry: None,
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
    wrong_type.scalar_observations[1].value = Some(ScalarTerm::Boolean(true));
    assert_eq!(classify(&wrong_type), None);
    contract.scalar_observations[1].value =
        Some(ScalarTerm::integer(integer, IntegerValue::Unsigned(255)).unwrap());
    assert_eq!(classify(&contract), Some(false));
}

#[test]
fn closure_gate_keeps_normalization_without_an_eligible_domain() {
    use semantic_vocabulary::StructuralDomainId;
    let first = StructuralDomainId::new(1).unwrap();
    let second = StructuralDomainId::new(2).unwrap();
    let mut contract = PayloadMembershipContract {
        scalar_observations: Vec::new(),
        scalar_requirements: Vec::new(),
        carrier: StructuralTypeId::new(1).unwrap(),
        qualifications: vec![second, first, first],
        projected_qualifications: Vec::new(),
    };
    assert!(closure_may_change(&[], &[], &contract));
    close(&[], &[], &mut contract);
    assert_eq!(contract.qualifications, vec![first, second]);
    assert!(!closure_may_change(&[], &[], &contract));
    let unchanged = contract.clone();
    close(&[], &[], &mut contract);
    assert_eq!(contract, unchanged);
}

#[test]
fn closure_gate_preserves_eligible_and_opaque_domain_behavior() {
    use semantic_vocabulary::{DomainSemanticId, StructuralDomainId};
    use terminal_psi::StructuralDomainPredicate;

    let carrier = StructuralTypeId::new(1).unwrap();
    let domain_id = StructuralDomainId::new(1).unwrap();
    let types = vec![StructuralTypeDeclaration {
        quotient_carrier: None,
        id: carrier,
        identity: TerminalIdentityIndex(1),
        owns_linear_claim: false,
        no_code_affine_cleanup: true,
        copyable: false,
        descriptor_cleanup: true,
        shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
        scalar_restriction: None,
    }];
    let domain = StructuralDomainDeclaration {
        required_memberships: Some(Vec::new()),
        complete_predicate_theory: true,
        predicate: Some(StructuralDomainPredicate {
            subject: PlaceId::new(1).unwrap(),
            proposition: Proposition::Truth,
        }),
        id: domain_id,
        semantic_domain: DomainSemanticId::new(1).unwrap(),
        identity: TerminalIdentityIndex(2),
        carrier,
        content_projection: None,
        byte_predicate: None,
        establishment_routes: Vec::new(),
    };
    let empty = PayloadMembershipContract {
        scalar_observations: Vec::new(),
        scalar_requirements: Vec::new(),
        carrier,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let mut contract = empty.clone();
    assert!(closure_may_change(
        &types,
        std::slice::from_ref(&domain),
        &contract
    ));
    close(&types, std::slice::from_ref(&domain), &mut contract);
    assert_eq!(contract.qualifications, vec![domain_id]);
    assert!(!closure_may_change(
        &types,
        std::slice::from_ref(&domain),
        &contract
    ));

    let mut opaque = domain.clone();
    opaque.predicate = None;
    let mut unknown = domain.clone();
    unknown.required_memberships = None;
    for excluded in [opaque, unknown] {
        let mut contract = empty.clone();
        let domains = std::slice::from_ref(&excluded);
        assert!(!closure_may_change(&types, domains, &contract));
        close(&types, domains, &mut contract);
        assert_eq!(contract, empty);
    }
    // Eligibility must not substitute for proving the predicate.
    let mut unprovable = domain;
    unprovable.predicate.as_mut().unwrap().proposition = Proposition::Falsehood;
    let domains = std::slice::from_ref(&unprovable);
    let mut contract = empty.clone();
    assert!(closure_may_change(&types, domains, &contract));
    close(&types, domains, &mut contract);
    assert_eq!(contract, empty);
}
