use super::{substitute_proposition_structural_places, substitute_result_place};
use semantic_vocabulary::{
    CanonicalStructuralPathSegment as Path, ContentAlgebra, ContentAlgebraKind,
    ContentConservation, ContentDomainId, ContentPlaceSegment as Segment, ContentPlaceVersion,
    ContentProjectionIdentity, ContentStructuralPlace, ContentTerm, PlaceId, Proposition,
    StructuralFieldId, StructuralTypeId,
};
use std::collections::BTreeMap;
use terminal_psi::{
    StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
    StructuralTypeDeclaration, StructuralTypeShape, TerminalIdentityTableBuilder, TerminalModule,
};

fn place(id: u64) -> PlaceId {
    PlaceId::new(id).unwrap()
}

fn projection(root: PlaceId, version: ContentPlaceVersion, segments: Vec<Segment>) -> ContentTerm {
    ContentTerm::Projection {
        projection: ContentProjectionIdentity {
            domain: ContentDomainId::new(1).unwrap(),
            projection_report_fingerprint: 7,
        },
        subject: ContentStructuralPlace {
            root,
            version,
            segments,
        },
    }
}

fn conservation(left: ContentTerm, right: ContentTerm) -> Proposition {
    Proposition::ContentConservation(ContentConservation::new(
        ContentAlgebra {
            kind: ContentAlgebraKind::IntervalSet,
            parameter: "space".into(),
        },
        left,
        right,
    ))
}

#[test]
fn projected_content_keeps_its_sibling_field_prefix_revision_and_suffix() {
    let root = StructuralTypeId::new(1).unwrap();
    let leaf = StructuralTypeId::new(2).unwrap();
    let mut identities = TerminalIdentityTableBuilder::default();
    let field_id = |i| StructuralFieldId::new(i).unwrap();
    let mut field = |id, name, carrier| StructuralFieldDeclaration {
        declaration_identity: Default::default(),
        inline_carrier: None,
        id: field_id(id),
        identity: identities.intern(name),
        relevance: terminal_psi::BindingRelevance::Relevant,
        field_type: StructuralFieldType::Structural(carrier),
        storage: None,
    };
    let mut module = TerminalModule::declarations();
    let type_decl = |id, shape| StructuralTypeDeclaration {
        declaration_identity: Default::default(),
        identity: Default::default(),
        id,
        shape,
        multiplicity: StructuralMultiplicity::Unrestricted,
        owns_linear_claim: false,
        no_code_affine_cleanup: false,
        descriptor_cleanup: false,
        nominal_cleanup: None,
        requires: Vec::new(),
        quotient_carrier: None,
        scalar_restriction: None,
    };
    module.types.structural_types = vec![
        type_decl(
            root,
            StructuralTypeShape::Record {
                fields: vec![field(17, "left", leaf), field(18, "right", leaf)],
                storage: terminal_psi::StructuralRecordStorage::Packed,
            },
        ),
        type_decl(
            leaf,
            StructuralTypeShape::Record {
                fields: vec![field(1, "payload", StructuralTypeId::new(3).unwrap())],
                storage: terminal_psi::StructuralRecordStorage::Packed,
            },
        ),
        type_decl(
            StructuralTypeId::new(3).unwrap(),
            StructuralTypeShape::Opaque,
        ),
    ];
    module.identities = identities.build();
    let suffix = Segment::Field("payload".into());
    let after = projection(place(2), ContentPlaceVersion::Current, vec![suffix.clone()]);
    let template = conservation(
        projection(place(1), ContentPlaceVersion::Entry, vec![suffix.clone()]),
        after.clone(),
    );
    for (id, name) in [(17, "left"), (18, "right")] {
        let actual = (place(4), vec![Path::Field(field_id(id))]);
        assert_eq!(
            substitute_result_place(&module, root, &template, place(1), &actual).unwrap(),
            conservation(
                projection(
                    place(4),
                    ContentPlaceVersion::Entry,
                    vec![Segment::Field(name.into()), suffix.clone()]
                ),
                after.clone()
            )
        );
        assert!(
            substitute_proposition_structural_places(
                &template,
                &BTreeMap::from([(place(1), actual)])
            )
            .is_err()
        );
    }
    for step in [Path::Field(field_id(19)), Path::Referent] {
        assert!(
            substitute_result_place(&module, root, &template, place(1), &(place(4), vec![step]))
                .is_err()
        );
    }
}

#[test]
fn separated_content_recovers_canonical_order_and_refuses_aliased_actuals() {
    let current = |root, segments| projection(place(root), ContentPlaceVersion::Current, segments);
    let input = ContentTerm::separate([current(1, vec![]), current(2, vec![])]).unwrap();
    let output = current(3, vec![]);
    let template = conservation(input, output.clone());
    let mut inputs = BTreeMap::from([
        (place(1), (place(5), vec![Path::FixedIndex(4)])),
        (place(2), (place(5), vec![Path::FixedIndex(3)])),
    ]);
    let expected = ContentTerm::separate([
        current(5, vec![Segment::FixedIndex(3)]),
        current(5, vec![Segment::FixedIndex(4)]),
    ])
    .unwrap();
    assert_eq!(
        substitute_proposition_structural_places(&template, &inputs).unwrap(),
        conservation(expected, output)
    );
    inputs.insert(place(2), (place(5), vec![Path::FixedIndex(4)]));
    let nested = Proposition::Implication {
        premise: Box::new(template),
        conclusion: Box::new(Proposition::Truth),
    };
    assert!(substitute_proposition_structural_places(&nested, &inputs).is_err());
}
