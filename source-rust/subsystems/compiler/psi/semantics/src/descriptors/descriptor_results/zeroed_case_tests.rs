//! `zeroed_case_predicates` resolves each selected case path through a type
//! index. It must give the predicates that resolving the same paths over the
//! declaration list gives.

use super::*;
use semantic_vocabulary::{ScalarType, StructuralCaseId, StructuralFieldId};
use terminal_psi::{
    ArrayExtent, BindingRelevance, StructuralCaseDeclaration, StructuralFieldDeclaration,
    StructuralFieldType, StructuralMultiplicity, StructuralRecordStorage, TerminalIdentityIndex,
};

const SCALAR: u64 = 1;
const CHOICE: u64 = 2;
const HOLDER: u64 = 3;
const HOLDERS: u64 = 4;
const PLAIN: u64 = 5;
const PLAINS: u64 = 6;

fn id(number: u64) -> StructuralTypeId {
    StructuralTypeId::new(number).unwrap()
}

fn declaration(number: u64, shape: StructuralTypeShape) -> StructuralTypeDeclaration {
    StructuralTypeDeclaration {
        declaration_identity: Default::default(),
        requires: Vec::new(),
        quotient_carrier: None,
        id: id(number),
        identity: TerminalIdentityIndex(number as u32),
        owns_linear_claim: false,
        no_code_affine_cleanup: true,
        multiplicity: StructuralMultiplicity::Unrestricted,
        descriptor_cleanup: false,
        nominal_cleanup: None,
        scalar_restriction: None,
        shape,
    }
}

fn field(number: u32, member: u64) -> StructuralFieldDeclaration {
    StructuralFieldDeclaration {
        declaration_identity: Default::default(),
        inline_carrier: None,
        id: StructuralFieldId::new(u64::from(number)).unwrap(),
        identity: TerminalIdentityIndex(number),
        relevance: BindingRelevance::Relevant,
        field_type: StructuralFieldType::Structural(id(member)),
        storage: None,
    }
}

fn record(number: u64, members: &[u64]) -> StructuralTypeDeclaration {
    declaration(
        number,
        StructuralTypeShape::Record {
            fields: members
                .iter()
                .enumerate()
                .map(|(position, member)| field(position as u32 + 1, *member))
                .collect(),
            storage: StructuralRecordStorage::Packed,
        },
    )
}

fn array(number: u64, element: u64, length: u64) -> StructuralTypeDeclaration {
    declaration(
        number,
        StructuralTypeShape::FixedArray {
            element: id(element),
            length: ArrayExtent::Literal(length),
        },
    )
}

/// A choice inside a record inside an array, beside records and arrays that
/// hold only scalars.
fn types() -> Vec<StructuralTypeDeclaration> {
    vec![
        declaration(
            SCALAR,
            StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
        ),
        declaration(
            CHOICE,
            StructuralTypeShape::Sum {
                cases: (1..=2)
                    .map(|case| StructuralCaseDeclaration {
                        declaration_identity: Default::default(),
                        id: StructuralCaseId::new(case).unwrap(),
                        identity: TerminalIdentityIndex(case as u32),
                        fields: Vec::new(),
                    })
                    .collect(),
            },
        ),
        record(HOLDER, &[SCALAR, CHOICE]),
        array(HOLDERS, HOLDER, 3),
        record(PLAIN, &[SCALAR, SCALAR]),
        array(PLAINS, PLAIN, 4),
    ]
}

/// The predicates with each path resolved over the declaration list.
fn walked(
    types: &[StructuralTypeDeclaration],
    root: PlaceId,
    structural_type: StructuralTypeId,
) -> Vec<DescriptorCasePredicate> {
    let Some(leaves) = crate::zeroed_structural_leaves(types.iter(), structural_type) else {
        return Vec::new();
    };
    let mut predicates = Vec::new();
    for leaf in leaves {
        let crate::ZeroedStructuralLeaf::SumSelection { path, case, .. } = leaf else {
            continue;
        };
        let Some(subject) =
            descriptor_lifetimes::descriptor_location(types, root, structural_type, &path, None)
        else {
            return Vec::new();
        };
        predicates.push(DescriptorCasePredicate {
            subject: subject.into(),
            case,
        });
    }
    predicates
}

#[test]
fn zeroed_case_predicates_match_paths_resolved_over_the_list() {
    let types = types();
    let root = PlaceId::new(7).unwrap();
    for structural_type in [SCALAR, CHOICE, HOLDER, HOLDERS, PLAIN, PLAINS] {
        assert_eq!(
            zeroed_case_predicates(&types, root, id(structural_type)),
            walked(&types, root, id(structural_type)),
            "type {structural_type}"
        );
    }
    // Each array element's choice starts in its first case.
    assert_eq!(zeroed_case_predicates(&types, root, id(HOLDERS)).len(), 3);
    assert_eq!(zeroed_case_predicates(&types, root, id(CHOICE)).len(), 1);
}

#[test]
fn a_duplicated_declaration_selects_no_case() {
    // The list reader refuses an identity declared twice; the index must too.
    // Each selected path steps through the record `HOLDER`.
    let mut types = types();
    let mut second = types[2].clone();
    second.identity = TerminalIdentityIndex(20);
    types.push(second);
    let root = PlaceId::new(7).unwrap();
    assert!(walked(&types, root, id(HOLDERS)).is_empty());
    assert!(zeroed_case_predicates(&types, root, id(HOLDERS)).is_empty());
}
