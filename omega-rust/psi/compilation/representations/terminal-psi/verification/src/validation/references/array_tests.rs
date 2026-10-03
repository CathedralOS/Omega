use super::*;
use terminal_psi::StructuralTypeDeclaration;

#[test]
fn completed_structural_array_reference_enumeration_is_bounded_before_expansion() {
    let reference = StructuralTypeId::new(1).unwrap();
    let array = StructuralTypeId::new(2).unwrap();
    let mut module = TerminalModule::for_entry(MachineId::new(1).unwrap());
    module.structural_types = vec![
        StructuralTypeDeclaration {
            id: reference,
            identity: "Reference".into(),
            shape: StructuralTypeShape::Reference {
                referent: StructuralTypeId::new(3).unwrap(),
                access: StructuralAccess::MutableBorrow,
            },
        },
        StructuralTypeDeclaration {
            id: array,
            identity: "Array".into(),
            shape: StructuralTypeShape::FixedArray {
                element: reference,
                length: 2,
            },
        },
    ];
    assert_eq!(
        leaf_paths(&module, array, 2),
        Some(vec![
            vec![StructuralPathSegment::FixedIndex(0)],
            vec![StructuralPathSegment::FixedIndex(1)],
        ])
    );
    assert!(leaf_paths(&module, array, 1).is_none());
    module.structural_types[1].shape = StructuralTypeShape::FixedArray {
        element: reference,
        length: u64::MAX,
    };
    assert!(leaf_paths(&module, array, 2).is_none());
    module.structural_types[1].shape = StructuralTypeShape::FixedArray {
        element: reference,
        length: 0,
    };
    assert_eq!(leaf_paths(&module, array, 0), Some(Vec::new()));
    assert!(!contains_reference(&module, array));
}
