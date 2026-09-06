use super::*;
use semantic_vocabulary::{PlaceId, StructuralFieldId};

fn identity(value: u32) -> StructuralTypeId {
    StructuralTypeId::new(u64::from(value)).unwrap()
}

fn field(value: u32, name: &str, nested: u32) -> terminal_psi::StructuralFieldDeclaration {
    terminal_psi::StructuralFieldDeclaration {
        id: StructuralFieldId::new(u64::from(value)).unwrap(),
        identity: name.into(),
        relevance: terminal_psi::BindingRelevance::Relevant,
        field_type: StructuralFieldType::Structural(identity(nested)),
    }
}

fn declaration(value: u32, shape: StructuralTypeShape) -> StructuralTypeDeclaration {
    StructuralTypeDeclaration {
        id: identity(value),
        identity: format!("Type{value}"),
        shape,
    }
}

fn discard(path: Vec<StructuralPathSegment>, structural_type: u32) -> StructuralAffineDiscard {
    StructuralAffineDiscard {
        place: PlaceId::new(1).unwrap(),
        path,
        structural_type: identity(structural_type),
    }
}

#[test]
fn finite_arrays_accept_every_nonempty_move_frontier_and_exact_empty_complements() {
    for length in [1, 2, 3, 4, 5, 8] {
        let declarations = [
            declaration(
                1,
                StructuralTypeShape::FixedArray {
                    element: identity(2),
                    length,
                },
            ),
            declaration(2, StructuralTypeShape::Record { fields: Vec::new() }),
        ];
        for mask in 1_u64..(1 << length) {
            let paths = (0..length)
                .filter(|index| mask & (1 << index) != 0)
                .map(|index| vec![StructuralPathSegment::FixedIndex(index)])
                .collect::<Vec<_>>();
            let moved = paths
                .iter()
                .rev()
                .map(|path| (path.as_slice(), identity(2)))
                .collect::<Vec<_>>();
            let residuals = (0..length)
                .rev()
                .filter(|index| mask & (1 << index) == 0)
                .map(|index| discard(vec![StructuralPathSegment::FixedIndex(index)], 2))
                .collect::<Vec<_>>();
            let references = residuals.iter().collect::<Vec<_>>();
            assert!(exact_partial_cleanup_partition(
                &declarations,
                identity(1),
                &moved,
                &references
            ));
            if !references.is_empty() {
                assert!(!exact_partial_cleanup_partition(
                    &declarations,
                    identity(1),
                    &moved,
                    &references[1..]
                ));
            }
            if references.len() > 1 {
                let mut reordered = references.clone();
                reordered.swap(0, 1);
                assert!(!exact_partial_cleanup_partition(
                    &declarations,
                    identity(1),
                    &moved,
                    &reordered
                ));
            }
        }
    }
}

#[test]
fn mixed_paths_retain_maximal_rows_and_reverse_field_order() {
    use StructuralPathSegment::{Field, FixedIndex};
    let declarations = [
        declaration(
            1,
            StructuralTypeShape::Record {
                fields: vec![field(1, "rows", 2), field(2, "tail", 4)],
            },
        ),
        declaration(
            2,
            StructuralTypeShape::FixedArray {
                element: identity(3),
                length: 3,
            },
        ),
        declaration(
            3,
            StructuralTypeShape::Record {
                fields: vec![field(3, "head", 4), field(4, "tail", 5)],
            },
        ),
        declaration(4, StructuralTypeShape::Record { fields: Vec::new() }),
        declaration(
            5,
            StructuralTypeShape::FixedArray {
                element: identity(4),
                length: 3,
            },
        ),
    ];
    let first = vec![
        Field("rows".into()),
        FixedIndex(1),
        Field("tail".into()),
        FixedIndex(1),
    ];
    let second = vec![Field("rows".into()), FixedIndex(1), Field("head".into())];
    let moved = [
        (first.as_slice(), identity(4)),
        (second.as_slice(), identity(4)),
    ];
    let residuals = [
        discard(vec![Field("tail".into())], 4),
        discard(vec![Field("rows".into()), FixedIndex(2)], 3),
        discard(
            vec![
                Field("rows".into()),
                FixedIndex(1),
                Field("tail".into()),
                FixedIndex(2),
            ],
            4,
        ),
        discard(
            vec![
                Field("rows".into()),
                FixedIndex(1),
                Field("tail".into()),
                FixedIndex(0),
            ],
            4,
        ),
        discard(vec![Field("rows".into()), FixedIndex(0)], 3),
    ];
    let references = residuals.iter().collect::<Vec<_>>();
    assert!(exact_partial_cleanup_partition(
        &declarations,
        identity(1),
        &moved,
        &references
    ));
    let ancestor = vec![Field("rows".into()), FixedIndex(1)];
    for invalid in [
        vec![moved[0], moved[0]],
        vec![moved[0], (ancestor.as_slice(), identity(3))],
        vec![(first.as_slice(), identity(3)), moved[1]],
    ] {
        assert!(!exact_partial_cleanup_partition(
            &declarations,
            identity(1),
            &invalid,
            &references
        ));
    }
    let mut expanded = residuals.to_vec();
    expanded[1] = discard(
        vec![Field("rows".into()), FixedIndex(2), Field("head".into())],
        4,
    );
    assert!(!exact_partial_cleanup_partition(
        &declarations,
        identity(1),
        &moved,
        &expanded.iter().collect::<Vec<_>>()
    ));
    assert!(!exact_partial_cleanup_partition(
        &declarations,
        identity(1),
        &moved,
        &[]
    ));
}

#[test]
fn short_evidence_rejects_huge_dimensions_before_enumeration() {
    use StructuralPathSegment::FixedIndex;
    let declarations = [
        declaration(
            1,
            StructuralTypeShape::FixedArray {
                element: identity(2),
                length: 3,
            },
        ),
        declaration(
            2,
            StructuralTypeShape::FixedArray {
                element: identity(3),
                length: u64::MAX,
            },
        ),
        declaration(3, StructuralTypeShape::Record { fields: Vec::new() }),
    ];
    let path = [FixedIndex(2), FixedIndex(0)];
    let residual = discard(vec![FixedIndex(2), FixedIndex(1)], 3);
    assert!(!exact_partial_cleanup_partition(
        &declarations,
        identity(1),
        &[(&path, identity(3))],
        &[&residual]
    ));
    assert!(!exact_partial_cleanup_partition(
        &declarations,
        identity(2),
        &[(&path[1..], identity(3))],
        &[]
    ));
}

#[test]
fn scalar_record_leaves_need_no_cleanup_but_structural_leaves_must_move() {
    let mut fields = vec![field(1, "first", 2), field(2, "second", 2)];
    fields.push(terminal_psi::StructuralFieldDeclaration {
        id: StructuralFieldId::new(3).unwrap(),
        identity: "flag".into(),
        relevance: terminal_psi::BindingRelevance::Relevant,
        field_type: StructuralFieldType::Scalar(semantic_vocabulary::ScalarType::Boolean),
    });
    let declarations = [
        declaration(1, StructuralTypeShape::Record { fields }),
        declaration(2, StructuralTypeShape::Record { fields: Vec::new() }),
    ];
    let first = [StructuralPathSegment::Field("first".into())];
    let second = [StructuralPathSegment::Field("second".into())];
    let moved = [(&first[..], identity(2)), (&second[..], identity(2))];
    assert!(exact_partial_cleanup_partition(
        &declarations,
        identity(1),
        &moved,
        &[]
    ));
    assert!(!exact_partial_cleanup_partition(
        &declarations,
        identity(1),
        &moved[..1],
        &[]
    ));
}

#[test]
fn finite_closure_rejects_cycles_zero_arrays_unknown_types_and_duplicate_fields() {
    let root = declaration(
        1,
        StructuralTypeShape::FixedArray {
            element: identity(2),
            length: 2,
        },
    );
    let leaf = declaration(2, StructuralTypeShape::Record { fields: Vec::new() });
    let path = [StructuralPathSegment::FixedIndex(0)];
    let residual = discard(vec![StructuralPathSegment::FixedIndex(1)], 2);
    for invalid in [
        vec![
            declaration(
                1,
                StructuralTypeShape::FixedArray {
                    element: identity(2),
                    length: 0,
                },
            ),
            leaf.clone(),
        ],
        vec![
            declaration(
                1,
                StructuralTypeShape::FixedArray {
                    element: identity(1),
                    length: 2,
                },
            ),
            leaf.clone(),
        ],
        vec![root.clone()],
        vec![
            root,
            declaration(
                2,
                StructuralTypeShape::Record {
                    fields: vec![field(1, "same", 3), field(2, "same", 3)],
                },
            ),
            declaration(3, StructuralTypeShape::Record { fields: Vec::new() }),
        ],
    ] {
        assert!(!exact_partial_cleanup_partition(
            &invalid,
            identity(1),
            &[(&path, identity(2))],
            &[&residual]
        ));
    }
}
