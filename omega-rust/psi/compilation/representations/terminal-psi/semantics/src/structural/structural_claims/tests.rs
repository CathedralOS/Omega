use super::structural_claim_paths_may_overlap;
use semantic_vocabulary::{
    ObligationId, StructuralCaseId, StructuralFieldId, StructuralTypeId, ValueId,
};
use terminal_psi::{
    BindingRelevance, StructuralCaseDeclaration, StructuralFieldDeclaration, StructuralFieldType,
    StructuralPathSegment as Path, StructuralTypeDeclaration, StructuralTypeShape,
    TerminalIdentityIndex,
};

fn id(value: u64) -> StructuralTypeId {
    StructuralTypeId::new(value).unwrap()
}
fn field(identity: u32, child: u64) -> StructuralFieldDeclaration {
    StructuralFieldDeclaration {
        inline_carrier: None,
        id: StructuralFieldId::new(u64::from(identity)).unwrap(),
        identity: TerminalIdentityIndex(identity),
        relevance: BindingRelevance::Relevant,
        field_type: StructuralFieldType::Structural(id(child)),
        storage: None,
    }
}
fn declaration(identity: u64, shape: StructuralTypeShape) -> StructuralTypeDeclaration {
    StructuralTypeDeclaration {
        quotient_carrier: None,
        id: id(identity),
        identity: TerminalIdentityIndex(identity as u32),
        owns_linear_claim: identity == 3,
        copyable: false,
        no_code_affine_cleanup: false,
        descriptor_cleanup: false,
        shape,
        scalar_restriction: None,
    }
}
fn types() -> Vec<StructuralTypeDeclaration> {
    vec![
        declaration(
            1,
            StructuralTypeShape::Record {
                fields: vec![field(10, 2), field(20, 2)],
                storage: terminal_psi::StructuralRecordStorage::Packed,
            },
        ),
        declaration(
            2,
            StructuralTypeShape::Sum {
                cases: [1, 2]
                    .into_iter()
                    .map(|case| StructuralCaseDeclaration {
                        id: StructuralCaseId::new(case).unwrap(),
                        identity: TerminalIdentityIndex(case as u32),
                        fields: vec![field(11, 3), field(12, 3)],
                    })
                    .collect(),
            },
        ),
        declaration(
            3,
            StructuralTypeShape::Record {
                fields: vec![],
                storage: terminal_psi::StructuralRecordStorage::Packed,
            },
        ),
        declaration(
            4,
            StructuralTypeShape::Reference {
                referent: id(1),
                access: terminal_psi::StructuralAccess::SharedBorrow,
            },
        ),
        declaration(
            5,
            StructuralTypeShape::FixedArray {
                element: id(2),
                length: terminal_psi::ArrayExtent::Literal(2),
            },
        ),
    ]
}
fn payload(case: u64, field: u32) -> Vec<Path> {
    vec![
        Path::Field(TerminalIdentityIndex(10)),
        Path::Case(StructuralCaseId::new(case).unwrap()),
        Path::Field(TerminalIdentityIndex(field)),
    ]
}

#[test]
fn ownership_catalogue_distinguishes_case_guards_from_borrowed_aliasing() {
    let types = types();
    let left = payload(1, 11);
    let right = payload(2, 11);
    assert_eq!(
        structural_claim_paths_may_overlap(types.iter(), id(1), &left, &right),
        Some(false)
    );
    assert!(crate::structural_paths_may_overlap(&left, &right));
}
#[test]
fn ownership_catalogue_rejects_duplicate_and_ancestor_claims() {
    let types = types();
    let path = payload(1, 11);
    for other in [&path[..], &path[..1], &[]] {
        assert_eq!(
            structural_claim_paths_may_overlap(types.iter(), id(1), &path, other),
            Some(true)
        );
    }
    assert_eq!(
        structural_claim_paths_may_overlap(types.iter(), id(1), &path, &payload(1, 12)),
        Some(false)
    );
}
#[test]
fn ownership_catalogue_checks_both_case_namespaces() {
    let types = types();
    let valid = payload(1, 11);
    let invalid = payload(3, 11);
    assert_eq!(
        structural_claim_paths_may_overlap(types.iter(), id(1), &valid, &invalid),
        None
    );
    assert_eq!(
        structural_claim_paths_may_overlap(types.iter(), id(1), &invalid, &valid),
        None
    );
    assert_eq!(
        structural_claim_paths_may_overlap(types.iter(), id(3), &valid, &payload(2, 11)),
        None
    );
}
#[test]
fn ownership_catalogue_requires_owned_static_storage() {
    let types = types();
    let left = payload(1, 11);
    let right = payload(2, 11);
    let borrowed = |path: &[Path]| {
        std::iter::once(Path::Referent)
            .chain(path.iter().cloned())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        structural_claim_paths_may_overlap(
            types.iter(),
            id(4),
            &borrowed(&left),
            &borrowed(&right)
        ),
        None
    );
    let indexed = |index| {
        vec![
            index,
            Path::Case(StructuralCaseId::new(1).unwrap()),
            Path::Field(TerminalIdentityIndex(11)),
        ]
    };
    let first = indexed(Path::FixedIndex(0));
    let second = indexed(Path::FixedIndex(1));
    assert_eq!(
        structural_claim_paths_may_overlap(types.iter(), id(5), &first, &second),
        Some(false)
    );
    let runtime = indexed(Path::RuntimeIndex {
        index: ValueId::new(1).unwrap(),
        obligation: ObligationId::new(1).unwrap(),
    });
    assert_eq!(
        structural_claim_paths_may_overlap(types.iter(), id(5), &first, &runtime),
        None
    );
}
