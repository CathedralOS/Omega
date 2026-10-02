use super::*;
use crate::custody::resolve_structural_arguments;
use terminal_psi::{
    BindingRelevance, StructuralAccess, StructuralArgument, StructuralCaseDeclaration,
    StructuralFieldDeclaration,
};

fn sum(identity: u64, first_case: u64, child: StructuralTypeId) -> StructuralTypeDeclaration {
    StructuralTypeDeclaration {
        id: StructuralTypeId::new(identity).unwrap(),
        identity: format!("Sum{identity}"),
        shape: StructuralTypeShape::Sum {
            // Both overlays intentionally have the same field spelling and
            // referent type. Layout/type agreement cannot choose the case.
            cases: [first_case, first_case + 1]
                .into_iter()
                .map(|case| StructuralCaseDeclaration {
                    id: StructuralCaseId::new(case).unwrap(),
                    identity: format!("Case{case}"),
                    fields: vec![StructuralFieldDeclaration {
                        id: semantic_vocabulary::StructuralFieldId::new(case).unwrap(),
                        identity: "payload".to_owned(),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Structural(child),
                    }],
                })
                .collect(),
        },
    }
}

#[test]
fn nested_case_loans_preserve_original_backing_and_require_current_discriminators() {
    let outer = StructuralTypeId::new(1).unwrap();
    let inner = StructuralTypeId::new(2).unwrap();
    let leaf = StructuralTypeId::new(3).unwrap();
    let first = StructuralCaseId::new(1).unwrap();
    let second = StructuralCaseId::new(3).unwrap();
    let source = PlaceId::new(1).unwrap();
    let types = [
        sum(1, 1, inner),
        sum(2, 3, leaf),
        StructuralTypeDeclaration {
            id: leaf,
            identity: "Payload".to_owned(),
            shape: StructuralTypeShape::Record { fields: vec![] },
        },
    ]
    .into_iter()
    .map(|declaration| (declaration.id, declaration))
    .collect();
    let value = TerminalStructuralValue {
        opaque_identity: 91,
        structural_type: outer,
        qualifications: vec![],
        // The parameter may itself be a projected caller value.
        path: vec![StructuralPathSegment::Field("caller_field".to_owned())],
    };
    let values = BTreeMap::from([(source, value.clone())]);
    let root_storage = StructuralRuntimePlace::from(&value);
    let mut inner_storage = root_storage.clone();
    inner_storage
        .path
        .push(StructuralPathSegment::Field("payload".to_owned()));
    let cases = BTreeMap::from([
        (
            root_storage.clone(),
            StructuralCaseContents {
                structural_type: outer,
                case: first,
            },
        ),
        (
            inner_storage.clone(),
            StructuralCaseContents {
                structural_type: inner,
                case: second,
            },
        ),
    ]);
    let argument = StructuralArgument {
        place: source,
        access: StructuralAccess::SharedBorrow,
        path: vec![
            StructuralPathSegment::Case(first),
            StructuralPathSegment::Field("payload".to_owned()),
            StructuralPathSegment::Case(second),
            StructuralPathSegment::Field("payload".to_owned()),
        ],
    };
    validate_case_projection(&types, &values, &cases, source, &argument.path).unwrap();
    let resolved =
        resolve_structural_arguments(&types, &values, std::slice::from_ref(&argument)).unwrap();
    let mut expected = value.clone();
    expected.structural_type = leaf;
    expected.path.extend([
        StructuralPathSegment::Field("payload".to_owned()),
        StructuralPathSegment::Field("payload".to_owned()),
    ]);
    assert_eq!(resolved, [expected]);
    assert_eq!(values[&source], value);

    for (storage, wrong_case) in [
        (root_storage.clone(), StructuralCaseId::new(2).unwrap()),
        (inner_storage.clone(), StructuralCaseId::new(4).unwrap()),
    ] {
        let mut changed = cases.clone();
        changed.get_mut(&storage).unwrap().case = wrong_case;
        assert!(
            validate_case_projection(&types, &values, &changed, source, &argument.path).is_err()
        );
        changed.remove(&storage);
        assert!(
            validate_case_projection(&types, &values, &changed, source, &argument.path).is_err()
        );
    }
    let mut changed = values.clone();
    changed.get_mut(&source).unwrap().opaque_identity += 1;
    assert!(validate_case_projection(&types, &changed, &cases, source, &argument.path).is_err());
    let mut changed = cases.clone();
    changed.get_mut(&inner_storage).unwrap().structural_type = outer;
    assert!(validate_case_projection(&types, &values, &changed, source, &argument.path).is_err());

    for path in [
        vec![StructuralPathSegment::Case(first)],
        vec![
            StructuralPathSegment::Case(second),
            StructuralPathSegment::Field("payload".to_owned()),
        ],
        vec![
            StructuralPathSegment::Case(first),
            StructuralPathSegment::Field("absent".to_owned()),
        ],
    ] {
        assert!(validate_case_projection(&types, &values, &cases, source, &path).is_err());
    }
}
