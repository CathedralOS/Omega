use super::structural_catalog::validate_function_structural_catalog;
use abstract_operations::AbstractFunctionResult;
use arena::NumericId;
use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::{
    ContentAlgebra, ContentAlgebraKind, ContentPlaceSegment, ContentPlaceVersion,
    ContentProjectionExpression, ContentProjectionIdentity, ContentProjectionScalar,
    ContentStructuralPlace, StructuralPlaceKind,
};
use terminal_psi::{
    ClaimContentProjection, ContentEntryClaim, MachineContract, StructuralAccess,
    StructuralContentProjection, StructuralDomainDeclaration, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPlaceDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalIdentityIndex,
};

fn id<T: NumericId>(value: u64) -> T {
    T::new(value).unwrap()
}

#[test]
fn content_entry_claims_use_machine_claim_ids() {
    let linear = StructuralTypeDeclaration {
        declaration_identity: Default::default(),
        identity: TerminalIdentityIndex(1),
        id: id(1),
        shape: StructuralTypeShape::Opaque,
        multiplicity: StructuralMultiplicity::Linear,
        owns_linear_claim: true,
        no_code_affine_cleanup: false,
        descriptor_cleanup: false,
        nominal_cleanup: None,
        requires: Vec::new(),
        quotient_carrier: None,
        scalar_restriction: None,
    };
    let plain = StructuralTypeDeclaration {
        identity: TerminalIdentityIndex(2),
        id: id(2),
        multiplicity: StructuralMultiplicity::Affine,
        owns_linear_claim: false,
        ..linear.clone()
    };
    let types = [linear, plain];
    let mut contract = MachineContract::empty(id(1));
    for (position, ty) in [0, 0, 1].into_iter().enumerate() {
        contract
            .structural_parameters
            .push(StructuralParameterDeclaration {
                place: id(position as u64 + 1),
                position: position as u32,
                structural_type: types[ty].id,
                multiplicity: types[ty].multiplicity,
                access: StructuralAccess::Owned,
                qualifications: if position == 0 {
                    Vec::new()
                } else {
                    vec![id(types[ty].id.get())]
                },
                projected_qualifications: Vec::new(),
                additional_input_qualifications: Vec::new(),
            });
    }
    terminal_semantics::plain_affine::publish_contract_claims(&mut contract, &types).unwrap();
    assert_eq!(
        contract
            .entry_claims
            .iter()
            .map(|entry| entry.claim)
            .collect::<Vec<_>>(),
        [id(1), id(2)]
    );

    let algebra = ContentAlgebra {
        kind: ContentAlgebraKind::CountedQuantity,
        parameter: "Bytes".into(),
    };
    let expression =
        ContentProjectionExpression::CountedQuantity(ContentProjectionScalar::Natural("1".into()));
    let projection = |ty: u64| ContentProjectionIdentity {
        domain: id(ty),
        projection_report_fingerprint: expression.report_fingerprint(&algebra),
    };
    let owners = types.each_ref().map(|ty| StructuralDomainDeclaration {
        required_memberships: None,
        complete_predicate_theory: false,
        predicate: None,
        id: id(ty.id.get()),
        semantic_domain: id(ty.id.get()),
        identity: TerminalIdentityIndex(ty.identity.0 + 2),
        carrier: ty.id,
        subject_identity: ty.identity,
        byte_predicate: None,
        establishment_routes: None,
        access_policy: None,
        content_projection: Some(StructuralContentProjection {
            identity: projection(ty.id.get()),
            algebra: algebra.clone(),
            expression: expression.clone(),
        }),
    });
    let content = |claim, root| ContentEntryClaim {
        claim: id(claim),
        input: ContentStructuralPlace {
            version: ContentPlaceVersion::Entry,
            root: id(root),
            segments: Vec::new(),
        },
        projections: vec![ClaimContentProjection {
            projection: projection(if root == 3 { 2 } else { 1 }),
            algebra: algebra.clone(),
        }],
    };
    let mut function = PsiOptimizationFunction {
        machine: id(1),
        attachment: None,
        entry: id(1),
        parameters: Vec::new(),
        structural_places: contract
            .structural_parameters
            .iter()
            .map(|parameter| StructuralPlaceDeclaration {
                id: parameter.place,
                kind: StructuralPlaceKind::Parameter {
                    position: parameter.position,
                },
            })
            .collect(),
        structural_parameters: contract.structural_parameters,
        result: AbstractFunctionResult::Unit,
        declared_places: Default::default(),
        entry_claims: contract
            .entry_claims
            .iter()
            .map(|entry| entry.claim)
            .collect(),
        entry_claim_declarations: contract.entry_claims,
        block_entry_claims: Vec::new(),
        content_entry_claims: Vec::new(),
        verified_contract: None,
        evidence_contract_lanes: Vec::new(),
        published_service_ceiling: Vec::new(),
        declared_service_reach: Vec::new(),
        facts: Vec::new(),
        blocks: Vec::new(),
    };
    let types = types.iter().map(|ty| (ty.id, ty)).collect();
    let domains = owners.iter().map(|domain| (domain.id, domain)).collect();
    let accepts = |function: &PsiOptimizationFunction| {
        validate_function_structural_catalog(function, &Default::default(), &types, &domains)
            .is_ok()
    };

    function.content_entry_claims = vec![content(2, 2)];
    assert!(
        accepts(&function),
        "only the second structural claim carries content"
    );
    function.content_entry_claims[0].input.root = id(1);
    assert!(!accepts(&function), "a sibling cannot supply this claim");
    function.content_entry_claims[0] = content(2, 2);
    function.content_entry_claims[0]
        .input
        .segments
        .push(ContentPlaceSegment::FixedIndex(0));
    assert!(!accepts(&function), "the structural path must agree");
    function.content_entry_claims[0] = content(2, 2);
    function.content_entry_claims[0].input.version = ContentPlaceVersion::Current;
    assert!(
        !accepts(&function),
        "a current revision cannot supply entry custody"
    );
    function.content_entry_claims = vec![content(2, 2), content(2, 2)];
    assert!(!accepts(&function), "duplicate content row");
    function.content_entry_claims = vec![content(2, 2), content(1, 1)];
    assert!(!accepts(&function), "out-of-order content rows");

    function.content_entry_claims = vec![content(3, 3)];
    assert!(
        accepts(&function),
        "content-only claims follow ordinary entries"
    );
    function.content_entry_claims[0].claim = id(4);
    assert!(!accepts(&function), "undeclared claim 3 would leave a gap");
}
