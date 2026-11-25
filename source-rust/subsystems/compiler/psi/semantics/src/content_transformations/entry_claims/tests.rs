use super::*;
use arena::NumericId;
use semantic_vocabulary::{
    ContentAlgebra, ContentAlgebraKind, ContentProjectionExpression, ContentProjectionIdentity,
    ContentProjectionScalar,
};
use semantic_vocabulary::{ContentPlaceSegment, StructuralTypeId};
use terminal_psi::StructuralPathSegment;
use terminal_psi::{
    ArrayExtent, StructuralContentProjection, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralTypeShape, TerminalIdentityIndex,
};

fn id<T: NumericId>(raw: u64) -> T {
    T::new(raw).unwrap()
}

fn carrier(
    raw: u64,
    shape: StructuralTypeShape,
    owns_linear_claim: bool,
) -> StructuralTypeDeclaration {
    StructuralTypeDeclaration {
        declaration_identity: Default::default(),
        identity: TerminalIdentityIndex(raw as u32),
        id: id(raw),
        shape,
        multiplicity: StructuralMultiplicity::Linear,
        owns_linear_claim,
        no_code_affine_cleanup: false,
        descriptor_cleanup: false,
        nominal_cleanup: None,
        requires: Vec::new(),
        quotient_carrier: None,
        scalar_restriction: None,
    }
}

fn owner(raw: u64, carrier: StructuralTypeId) -> StructuralDomainDeclaration {
    let algebra = ContentAlgebra {
        kind: ContentAlgebraKind::CountedQuantity,
        parameter: "Bytes".into(),
    };
    let expression =
        ContentProjectionExpression::CountedQuantity(ContentProjectionScalar::Natural("1".into()));
    StructuralDomainDeclaration {
        required_memberships: None,
        complete_predicate_theory: false,
        predicate: None,
        id: id(raw),
        semantic_domain: id(raw),
        identity: TerminalIdentityIndex(raw as u32 + 10),
        carrier,
        subject_identity: TerminalIdentityIndex(carrier.get() as u32),
        byte_predicate: None,
        establishment_routes: None,
        access_policy: None,
        content_projection: Some(StructuralContentProjection {
            identity: ContentProjectionIdentity {
                domain: id(raw),
                projection_report_fingerprint: expression.report_fingerprint(&algebra),
            },
            algebra,
            expression,
        }),
    }
}

fn input(position: u32, carrier: StructuralTypeId) -> StructuralParameterDeclaration {
    StructuralParameterDeclaration {
        place: id(u64::from(position) + 1),
        position,
        structural_type: carrier,
        multiplicity: StructuralMultiplicity::Linear,
        access: StructuralAccess::Owned,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        additional_input_qualifications: Vec::new(),
    }
}

#[test]
fn qualified_entry_content_uses_the_actual_owned_input_and_revision() {
    let types = [carrier(1, StructuralTypeShape::Opaque, true)];
    let domains = [owner(1, types[0].id), owner(2, types[0].id)];
    let mut contract = MachineContract::empty(id(1));
    contract.structural_parameters = vec![input(0, types[0].id), input(1, types[0].id)];
    contract.structural_parameters[1].qualifications = vec![id(1), id(2)];
    crate::plain_affine::publish_contract_claims(&mut contract, &types).unwrap();
    let derive = |contract: &MachineContract| {
        declared_entry_contents(&types, &domains, &Default::default(), contract)
    };
    let verify = |contract: &MachineContract| {
        validate_declared_entry_contents(&types, &domains, &Default::default(), contract)
    };
    let rows = derive(&contract).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].claim, id(2));
    assert_eq!(
        rows[0].input,
        ContentStructuralPlace {
            version: ContentPlaceVersion::Entry,
            root: id(2),
            segments: Vec::new()
        }
    );
    assert_eq!(
        rows[0]
            .projections
            .iter()
            .map(|p| p.projection.domain)
            .collect::<Vec<_>>(),
        [id(1), id(2)]
    );
    let mut independent = rows[0].clone();
    independent.claim = id(3);
    contract.content_entry_claims.push(independent.clone());
    publish_declared_entry_contents(&types, &domains, &Default::default(), &mut contract).unwrap();
    assert_eq!(
        contract.content_entry_claims,
        [rows[0].clone(), independent]
    );
    assert!(verify(&contract).is_ok());
    let mut other_owner = domains.clone();
    other_owner[0]
        .content_projection
        .as_mut()
        .unwrap()
        .identity
        .domain = id(7);
    assert!(
        validate_declared_entry_contents(&types, &other_owner, &Default::default(), &contract)
            .is_err(),
        "owner differs from its declared domain"
    );
    contract.content_entry_claims = rows.clone();
    assert!(verify(&contract).is_ok());
    contract.content_entry_claims.clear();
    assert!(verify(&contract).is_err(), "omitted binding");
    contract.content_entry_claims = rows.clone();
    contract.content_entry_claims[0].input.root = id(1);
    assert!(verify(&contract).is_err(), "plain sibling");
    contract.content_entry_claims = rows.clone();
    contract.content_entry_claims[0].input.version = ContentPlaceVersion::Current;
    assert!(verify(&contract).is_err(), "wrong revision");
    contract.content_entry_claims = rows.clone();
    contract.content_entry_claims[0].projections.pop();
    assert!(verify(&contract).is_err(), "one of two owners omitted");
    contract.content_entry_claims = rows.clone();
    contract.content_entry_claims[0].projections[0]
        .algebra
        .parameter = "Words".into();
    assert!(verify(&contract).is_err(), "wrong unit");
    contract.entry_claims.pop();
    contract.content_entry_claims.clear();
    assert!(
        verify(&contract).is_err(),
        "omitting structural and content rows together"
    );
    contract.structural_parameters[1].access = StructuralAccess::SharedBorrow;
    assert!(
        derive(&contract).unwrap().is_empty(),
        "borrowed input carries no owned claim"
    );
}

#[test]
fn qualified_entry_content_matches_each_concrete_array_element() {
    let leaf = carrier(1, StructuralTypeShape::Opaque, true);
    let pair = carrier(
        2,
        StructuralTypeShape::FixedArray {
            element: leaf.id,
            length: ArrayExtent::Literal(2),
        },
        false,
    );
    let domains = [owner(1, leaf.id), owner(2, leaf.id)];
    let types = [leaf, pair];
    let mut contract = MachineContract::empty(id(1));
    let mut formal = input(0, types[1].id);
    formal
        .projected_qualifications
        .push(StructuralPathQualification {
            path: vec![StructuralPathSegment::FixedIndex(1)],
            domain: id(1),
        });
    formal
        .additional_input_qualifications
        .push(StructuralPathQualification {
            path: vec![StructuralPathSegment::AllElements],
            domain: id(2),
        });
    contract.structural_parameters.push(formal);
    crate::plain_affine::publish_contract_claims(&mut contract, &types).unwrap();
    let rows = declared_entry_contents(&types, &domains, &Default::default(), &contract).unwrap();
    assert_eq!(rows.len(), 2);
    for (i, row) in rows.iter().enumerate() {
        assert_eq!(row.claim, id(i as u64 + 1));
        assert_eq!(
            row.input.segments,
            [ContentPlaceSegment::FixedIndex(i as u64)]
        );
    }
    assert_eq!(rows[0].projections.len(), 1);
    assert_eq!(rows[0].projections[0].projection.domain, id(2));
    assert_eq!(rows[1].projections.len(), 2);
    contract.content_entry_claims = rows;
    contract.content_entry_claims[1].input.segments[0] = ContentPlaceSegment::FixedIndex(0);
    assert!(
        validate_declared_entry_contents(&types, &domains, &Default::default(), &contract).is_err()
    );
}

#[test]
fn qualified_entry_content_keeps_the_owning_case_for_a_payload() {
    let leaf = carrier(1, StructuralTypeShape::Opaque, true);
    let mut ids = terminal_psi::TerminalIdentityTableBuilder::default();
    let field_name = ids.intern("payload");
    let mut case = |n, name| terminal_psi::StructuralCaseDeclaration {
        declaration_identity: Default::default(),
        id: id(n),
        identity: ids.intern(name),
        fields: vec![terminal_psi::StructuralFieldDeclaration {
            declaration_identity: Default::default(),
            inline_carrier: None,
            id: id(1),
            identity: field_name,
            relevance: terminal_psi::BindingRelevance::Relevant,
            field_type: terminal_psi::StructuralFieldType::Structural(leaf.id),
            storage: None,
        }],
    };
    let tagged = carrier(
        2,
        StructuralTypeShape::Sum {
            cases: vec![case(1, "Left"), case(2, "Right")],
        },
        false,
    );
    let ids = ids.build();
    let domains = [owner(1, leaf.id)];
    let types = [leaf, tagged];
    let mut contract = MachineContract::empty(id(1));
    let mut formal = input(0, types[1].id);
    let declared = vec![
        StructuralPathSegment::Case(id(2)),
        StructuralPathSegment::Field(field_name),
    ];
    formal
        .projected_qualifications
        .push(StructuralPathQualification {
            path: declared.clone(),
            domain: id(1),
        });
    contract.structural_parameters.push(formal);
    crate::plain_affine::publish_contract_claims(&mut contract, &types).unwrap();
    publish_declared_entry_contents(&types, &domains, &ids, &mut contract).unwrap();
    let claim = &contract.content_entry_claims[0];
    assert_eq!(claim.claim, id(2));
    assert_eq!(
        claim.input.segments,
        [
            ContentPlaceSegment::Case("Right".into()),
            ContentPlaceSegment::Field("payload".into())
        ]
    );
    assert_eq!(
        super::super::structural_path(types.iter(), &ids, types[1].id, &claim.input.segments),
        Some(declared)
    );
    contract.content_entry_claims[0].input.segments[0] = ContentPlaceSegment::Case("Left".into());
    assert!(validate_declared_entry_contents(&types, &domains, &ids, &contract).is_err());
}
