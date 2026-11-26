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

#[test]
fn zeroed_case_table_reuses_paths_for_different_roots() {
    let types = types();
    let mut table = ZeroedCaseTable::new(&types);
    for root_id in [7, 19, 42] {
        let root = PlaceId::new(root_id).unwrap();
        for structural_type in [SCALAR, CHOICE, HOLDER, HOLDERS, PLAIN, PLAINS] {
            assert_eq!(
                table.predicates_for(root, id(structural_type)),
                walked(&types, root, id(structural_type)),
                "root {root_id}, type {structural_type}"
            );
        }
    }
}

/// One leaf stands for a whole array of empty arrays, so its length does not
/// count against the walk's bound. The choice beside it still starts in its
/// first case. Its twin repeats the choice itself past the bound, and the
/// walk refuses it.
#[test]
fn an_array_of_empty_arrays_is_one_zeroed_leaf() {
    const EMPTY: u64 = 7;
    const EMPTIES: u64 = 8;
    const BESIDE: u64 = 9;
    const TOO_MANY: u64 = 10;
    let mut types = types();
    types.push(array(EMPTY, SCALAR, 0));
    types.push(array(EMPTIES, EMPTY, 200_000));
    types.push(record(BESIDE, &[EMPTIES, CHOICE]));
    types.push(array(TOO_MANY, HOLDER, 200_000));
    let root = PlaceId::new(7).unwrap();
    let beside = zeroed_case_predicates(&types, root, id(BESIDE));
    assert_eq!(beside.len(), 1);
    assert_eq!(beside, walked(&types, root, id(BESIDE)));
    assert!(walked(&types, root, id(TOO_MANY)).is_empty());
    assert!(zeroed_case_predicates(&types, root, id(TOO_MANY)).is_empty());
}

#[test]
fn selected_case_byte_storage_uses_the_same_empty_value_as_a_record_field() {
    use terminal_psi::{ByteSequenceCarrier, StructuralAccess};

    let case = StructuralCaseId::new(1).unwrap();
    for (access, allowed) in [
        (Some(StructuralAccess::SharedBorrow), true),
        (Some(StructuralAccess::MutableBorrow), false),
    ] {
        let mut payload = field(3, SCALAR);
        payload.field_type =
            StructuralFieldType::ByteSequence(ByteSequenceCarrier::BorrowedView { access });
        let choice = declaration(
            CHOICE,
            StructuralTypeShape::Sum {
                cases: vec![StructuralCaseDeclaration {
                    declaration_identity: Default::default(),
                    id: case,
                    identity: TerminalIdentityIndex(1),
                    fields: vec![payload.clone()],
                }],
            },
        );
        let leaves = crate::zeroed_structural_leaves(std::iter::once(&choice), id(CHOICE));
        assert_eq!(leaves.is_some(), allowed);
        if let Some(leaves) = leaves {
            assert!(leaves.contains(&crate::ZeroedStructuralLeaf::EmptyView {
                path: vec![StructuralPathSegment::Case(case)],
                field: Some(payload.id),
                carrier: payload.field_type,
            }));
        }
    }
}
