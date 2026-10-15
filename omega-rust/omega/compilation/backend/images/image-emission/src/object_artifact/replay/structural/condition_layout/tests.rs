use super::{
    replay_runtime_projection, replay_structural_projection, replay_structural_value_shape,
};
use calling_conventions::ValueShape;
use semantic_vocabulary::{
    IeeeFloatFormat, ObligationId, ScalarType, StructuralCaseId, StructuralFieldId,
    StructuralTypeId, ValueId,
};
use terminal_psi::{
    BindingRelevance, StructuralCaseDeclaration, StructuralFieldDeclaration, StructuralFieldType,
    StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape,
};

#[test]
fn case_projection_replay_preserves_sum_and_mixed_backing_geometry() {
    let root = StructuralTypeId::new(1).unwrap();
    let sum = StructuralTypeId::new(2).unwrap();
    let leaf = StructuralTypeId::new(3).unwrap();
    let present = StructuralCaseId::new(21).unwrap();
    let absent = StructuralCaseId::new(22).unwrap();
    let field = |raw, identity: &str, field_type| StructuralFieldDeclaration {
        storage: None,
        id: StructuralFieldId::new(raw).unwrap(),
        identity: terminal_psi::TerminalIdentityIndex(match identity {
            "common" => 1,
            "item" => 2,
            "value" => 3,
            _ => 0,
        }),
        relevance: BindingRelevance::Relevant,
        field_type,
    };
    let scalar = StructuralFieldType::Scalar(ScalarType::IeeeFloat(IeeeFloatFormat::Binary64));
    for (mixed, stride, payload_offset) in [(false, 16, 8), (true, 24, 16)] {
        let cases = vec![
            StructuralCaseDeclaration {
                id: present,
                identity: terminal_psi::TerminalIdentityIndex(1),
                fields: vec![field(31, "item", StructuralFieldType::Structural(leaf))],
            },
            StructuralCaseDeclaration {
                id: absent,
                identity: terminal_psi::TerminalIdentityIndex(2),
                fields: Vec::new(),
            },
        ];
        let mut types = vec![
            StructuralTypeDeclaration {
                quotient_carrier: None,
                no_code_affine_cleanup: false,
                descriptor_cleanup: false,
                scalar_restriction: None,
                id: root,
                identity: terminal_psi::TerminalIdentityIndex(3),
                shape: StructuralTypeShape::FixedArray {
                    element: sum,
                    length: 2,
                },
            },
            StructuralTypeDeclaration {
                quotient_carrier: None,
                no_code_affine_cleanup: false,
                descriptor_cleanup: false,
                scalar_restriction: None,
                id: sum,
                identity: terminal_psi::TerminalIdentityIndex(4),
                shape: if mixed {
                    StructuralTypeShape::Mixed {
                        fields: vec![field(33, "common", scalar.clone())],
                        cases,
                    }
                } else {
                    StructuralTypeShape::Sum { cases }
                },
            },
            StructuralTypeDeclaration {
                quotient_carrier: None,
                no_code_affine_cleanup: false,
                descriptor_cleanup: false,
                scalar_restriction: None,
                id: leaf,
                identity: terminal_psi::TerminalIdentityIndex(5),
                shape: StructuralTypeShape::Record {
                    storage: terminal_psi::StructuralRecordStorage::Packed,
                    fields: vec![field(32, "value", scalar.clone())],
                },
            },
        ];
        let path = vec![
            StructuralPathSegment::FixedIndex(1),
            StructuralPathSegment::Case(present),
            StructuralPathSegment::Field(terminal_psi::TerminalIdentityIndex(2)),
        ];
        assert_eq!(
            replay_structural_value_shape(root, &types),
            Some(ValueShape::integer(stride * 2, 8))
        );
        assert_eq!(
            replay_structural_projection(root, &path, &types),
            Some((leaf, ValueShape::integer(8, 8), stride + payload_offset))
        );
        let mut runtime = path.clone();
        runtime[0] = StructuralPathSegment::RuntimeIndex {
            index: ValueId::new(41).unwrap(),
            obligation: ObligationId::new(42).unwrap(),
        };
        assert_eq!(
            replay_runtime_projection(root, &runtime, &types),
            Some((
                leaf,
                ValueShape::integer(8, 8),
                payload_offset,
                vec![(2, stride)]
            ))
        );
        for invalid in [
            vec![
                StructuralPathSegment::FixedIndex(2),
                StructuralPathSegment::Case(present),
                StructuralPathSegment::Field(terminal_psi::TerminalIdentityIndex(2)),
            ],
            vec![
                StructuralPathSegment::FixedIndex(1),
                StructuralPathSegment::Case(absent),
                StructuralPathSegment::Field(terminal_psi::TerminalIdentityIndex(2)),
            ],
            vec![
                StructuralPathSegment::FixedIndex(1),
                StructuralPathSegment::Case(StructuralCaseId::new(99).unwrap()),
                StructuralPathSegment::Field(terminal_psi::TerminalIdentityIndex(2)),
            ],
            vec![
                StructuralPathSegment::FixedIndex(1),
                StructuralPathSegment::Case(present),
            ],
            vec![
                StructuralPathSegment::FixedIndex(1),
                StructuralPathSegment::Case(present),
                StructuralPathSegment::FixedIndex(0),
            ],
            vec![
                StructuralPathSegment::FixedIndex(1),
                StructuralPathSegment::Case(present),
                StructuralPathSegment::Field(terminal_psi::TerminalIdentityIndex(7)),
            ],
        ] {
            assert!(replay_structural_projection(root, &invalid, &types).is_none());
        }
        assert!(replay_structural_projection(leaf, &path[1..], &types).is_none());
        assert!(replay_structural_projection(root, &path[1..], &types).is_none());
        let mut duplicate = types.clone();
        duplicate.push(types[1].clone());
        assert!(replay_structural_projection(root, &path, &duplicate).is_none());
        let cases = match &mut types[1].shape {
            StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } => cases,
            _ => unreachable!(),
        };
        cases[0].fields[0].relevance = BindingRelevance::Erased;
        assert!(replay_structural_projection(root, &path, &types).is_none());
        assert!(replay_runtime_projection(root, &runtime, &types).is_none());
    }
}
