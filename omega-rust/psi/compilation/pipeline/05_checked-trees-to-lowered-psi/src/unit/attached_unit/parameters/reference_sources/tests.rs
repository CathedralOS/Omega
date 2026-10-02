use super::lower_reference_source;
use crate::terminal_identities::{place_id, structural_field_id, structural_type_id};
use checked_trees::{
    CheckedStructuralAccess, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralPathSegment,
};
use terminal_psi::{
    BindingRelevance, StructuralAccess, StructuralFieldDeclaration, StructuralFieldType,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralTypeDeclaration, StructuralTypeShape,
};

#[test]
fn reference_sources_preserve_leaf_marker_without_widening_ordinary_arguments() {
    let root = structural_type_id(1);
    let leaf = structural_type_id(2);
    let reference = structural_type_id(3);
    let mut types = vec![
        StructuralTypeDeclaration {
            id: root,
            identity: "Container".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: structural_field_id(1),
                    identity: "node".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(leaf),
                }],
            },
        },
        StructuralTypeDeclaration {
            id: leaf,
            identity: "Node".into(),
            shape: StructuralTypeShape::Record { fields: Vec::new() },
        },
        StructuralTypeDeclaration {
            id: reference,
            identity: "Reference<Node>".into(),
            shape: StructuralTypeShape::Reference {
                referent: leaf,
                access: StructuralAccess::SharedBorrow,
            },
        },
    ];
    let parameter = StructuralParameterDeclaration {
        place: place_id(1),
        position: 0,
        is_self: false,
        structural_type: root,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::SharedBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let source = CheckedUnitStructuralArgumentPlan {
        source: CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index: 0 },
        path: vec![
            CheckedUnitStructuralPathSegment::Field("node".into()),
            CheckedUnitStructuralPathSegment::Referent,
        ],
        type_identity: "Node".into(),
        access: CheckedStructuralAccess::SharedBorrow,
    };
    let lower = |source: &CheckedUnitStructuralArgumentPlan,
                 parameter: &StructuralParameterDeclaration,
                 types: &[StructuralTypeDeclaration]| {
        lower_reference_source(
            source,
            types,
            std::slice::from_ref(parameter),
            &[],
            &[],
            &[],
        )
    };
    let ordinary = |source: &CheckedUnitStructuralArgumentPlan,
                    types: &[StructuralTypeDeclaration]| {
        super::super::lower_structural_arguments(
            std::slice::from_ref(source),
            types,
            std::slice::from_ref(&parameter),
            &[],
            &[],
            &[],
            &[],
            None,
            &mut std::iter::empty(),
            None,
        )
    };
    let lowered =
        lower(&source, &parameter, &types).expect("owned record member is a reference origin");
    assert_eq!(lowered.place, parameter.place);
    assert_eq!(lowered.access, StructuralAccess::SharedBorrow);
    assert_eq!(
        lowered.path,
        vec![
            StructuralPathSegment::Field("node".into()),
            StructuralPathSegment::Referent
        ]
    );
    assert!(
        ordinary(&source, &types).is_err(),
        "ordinary arguments cannot dereference owned record storage"
    );
    let mut wrong_field = source.clone();
    wrong_field.path[0] = CheckedUnitStructuralPathSegment::Field("missing".into());
    assert!(lower(&wrong_field, &parameter, &types).is_err());
    let mut foreign_root = parameter.clone();
    foreign_root.structural_type = leaf;
    assert!(lower(&source, &foreign_root, &types).is_err());
    let mut owned = source.clone();
    owned.access = CheckedStructuralAccess::Owned;
    assert!(lower(&owned, &parameter, &types).is_err());
    let StructuralTypeShape::Record { fields } = &mut types[0].shape else {
        unreachable!()
    };
    fields[0].relevance = BindingRelevance::Erased;
    assert!(lower(&source, &parameter, &types).is_err());
    let StructuralTypeShape::Record { fields } = &mut types[0].shape else {
        unreachable!()
    };
    fields[0].relevance = BindingRelevance::Relevant;
    fields[0].field_type = StructuralFieldType::Structural(reference);
    assert_eq!(lower(&source, &parameter, &types).unwrap(), lowered);
    assert!(
        ordinary(&source, &types).is_ok(),
        "an actual Reference carrier permits the ordinary crossing"
    );
}
