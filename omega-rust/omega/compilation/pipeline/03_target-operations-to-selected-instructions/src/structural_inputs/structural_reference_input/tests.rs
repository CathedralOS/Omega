//! Structural reference input tests.

use super::{
    ScalarType, StructuralFieldId, StructuralFieldType, StructuralPathSegment,
    StructuralTypeDeclaration, StructuralTypeId, StructuralTypeShape, ValueShape,
};
use crate::structural_inputs::structural_reference_input::byte_field_length;
use crate::structural_inputs::structural_reference_input::field_read;
use crate::structural_inputs::structural_reference_input::field_shape;
use crate::structural_inputs::structural_reference_input::owned_aggregate_shape;
use crate::structural_inputs::structural_reference_input::plain_record_shape;
use crate::structural_inputs::structural_reference_input::projected_byte_field;
use crate::structural_inputs::structural_reference_input::shape;
use crate::structural_inputs::structural_reference_input::store;

#[test]
fn reference_projection_restarts_offsets_and_checks_each_permission() {
    use semantic_vocabulary::{IntegerSign, IntegerType};
    use terminal_psi::{BindingRelevance, StructuralAccess, StructuralFieldDeclaration};
    let identity = |value| StructuralTypeId::new(value).unwrap();
    let scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap());
    let field = |position, name: &str, field_type| StructuralFieldDeclaration {
        id: StructuralFieldId::new(position).unwrap(),
        identity: name.into(),
        relevance: BindingRelevance::Relevant,
        field_type,
    };
    let record = |number, name: &str, fields| StructuralTypeDeclaration {
        id: identity(number),
        identity: name.into(),
        shape: StructuralTypeShape::Record { fields },
    };
    let mut declarations = vec![
        record(
            1,
            "Cell",
            vec![
                field(1, "stamp", StructuralFieldType::Scalar(scalar)),
                field(2, "value", StructuralFieldType::Scalar(scalar)),
            ],
        ),
        StructuralTypeDeclaration {
            id: identity(2),
            identity: "CellReference".into(),
            shape: StructuralTypeShape::Reference {
                referent: identity(1),
                access: StructuralAccess::SharedBorrow,
            },
        },
        record(
            3,
            "Message",
            vec![
                field(1, "stamp", StructuralFieldType::Scalar(scalar)),
                field(2, "body", StructuralFieldType::Structural(identity(2))),
            ],
        ),
        StructuralTypeDeclaration {
            id: identity(4),
            identity: "MessageReference".into(),
            shape: StructuralTypeShape::Reference {
                referent: identity(3),
                access: StructuralAccess::SharedBorrow,
            },
        },
        record(
            5,
            "Envelope",
            vec![
                field(1, "stamp", StructuralFieldType::Scalar(scalar)),
                field(2, "message", StructuralFieldType::Structural(identity(4))),
            ],
        ),
        StructuralTypeDeclaration {
            id: identity(6),
            identity: "i32".into(),
            shape: StructuralTypeShape::PrimitiveScalar(scalar),
        },
    ];
    let path = vec![
        StructuralPathSegment::Field("message".into()),
        StructuralPathSegment::Referent,
        StructuralPathSegment::Field("body".into()),
        StructuralPathSegment::Referent,
        StructuralPathSegment::Field("value".into()),
    ];
    let resolve =
        |declarations: &[StructuralTypeDeclaration], path: &[StructuralPathSegment], access| {
            super::reference_projection(identity(5), path, access, declarations)
        };
    let projected = resolve(&declarations, &path, StructuralAccess::SharedBorrow).unwrap();
    assert_eq!(projected.selected, identity(6));
    assert_eq!(projected.descriptor_offsets, [8, 8]);
    assert_eq!(projected.referent_offset, 4);
    for access in [
        StructuralAccess::Owned,
        StructuralAccess::MutableBorrow,
        StructuralAccess::WriteOnlyBorrow,
    ] {
        assert!(resolve(&declarations, &path, access).is_none());
    }
    let mut missing_crossing = path.clone();
    missing_crossing.remove(1);
    assert!(
        resolve(
            &declarations,
            &missing_crossing,
            StructuralAccess::SharedBorrow
        )
        .is_none()
    );
    let mut extra_crossing = path.clone();
    extra_crossing.push(StructuralPathSegment::Referent);
    assert!(
        resolve(
            &declarations,
            &extra_crossing,
            StructuralAccess::SharedBorrow
        )
        .is_none()
    );
    // Write-only final storage is usable; write-only intermediate storage
    // cannot supply the next descriptor, even under a write-only request.
    declarations[1].shape = StructuralTypeShape::Reference {
        referent: identity(1),
        access: StructuralAccess::WriteOnlyBorrow,
    };
    declarations[3].shape = StructuralTypeShape::Reference {
        referent: identity(3),
        access: StructuralAccess::MutableBorrow,
    };
    assert!(resolve(&declarations, &path, StructuralAccess::WriteOnlyBorrow).is_some());
    declarations[3].shape = StructuralTypeShape::Reference {
        referent: identity(3),
        access: StructuralAccess::WriteOnlyBorrow,
    };
    assert!(resolve(&declarations, &path, StructuralAccess::WriteOnlyBorrow).is_none());
}

#[test]
fn refined_scalar_geometry_uses_each_exact_carrier() {
    use semantic_vocabulary::{
        IeeeFloatFormat, IntegerSign, IntegerType, Proposition, ScalarRefinement,
    };
    for scalar in [
        ScalarType::Boolean,
        ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap()),
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary64),
    ] {
        let refined = StructuralFieldType::RefinedScalar(
            ScalarRefinement::new(scalar, Proposition::Truth).unwrap(),
        );
        assert_eq!(
            field_shape(&refined, &[], &mut Vec::new()),
            super::scalar_shape(scalar)
        );
        assert_eq!(
            refined.canonical_leaf_shape(),
            None,
            "carrier layout grants no plain scalar reference"
        );
    }
}

#[test]
fn empty_record_array_geometry_validates_the_element_shell() {
    let root = StructuralTypeId::new(1).unwrap();
    let element = StructuralTypeId::new(2).unwrap();
    let mut declarations = [
        StructuralTypeDeclaration {
            id: root,
            identity: "EmptyRecords".into(),
            shape: StructuralTypeShape::FixedArray { element, length: 0 },
        },
        StructuralTypeDeclaration {
            id: element,
            identity: "Element".into(),
            shape: StructuralTypeShape::Record { fields: Vec::new() },
        },
    ];
    assert_eq!(shape(root, &declarations), Some(ValueShape::integer(0, 1)));
    for invalid in [root, StructuralTypeId::new(99).unwrap()] {
        declarations[0].shape = StructuralTypeShape::FixedArray {
            element: invalid,
            length: 0,
        };
        assert_eq!(shape(root, &declarations), None);
    }
}

#[test]
fn owned_aggregate_geometry_preserves_bounded_byte_siblings() {
    use terminal_psi::{BindingRelevance, ByteSequenceCarrier, StructuralFieldDeclaration};

    let root = StructuralTypeId::new(1).unwrap();
    let scalar = ScalarType::Integer(
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Signed, 32)
            .unwrap(),
    );
    let byte_field = StructuralFieldId::new(1).unwrap();
    let scalar_field = StructuralFieldId::new(2).unwrap();
    for (capacity, expected) in [
        (0, Some((16, 8))),
        (3, Some((16, 12))),
        (9, Some((24, 20))),
        (65_520, Some((65_536, 65_528))),
        (65_528, Some((65_544, 65_536))),
        (u64::from(u32::MAX) - 8, None),
        (u64::MAX, None),
    ] {
        let declarations = [StructuralTypeDeclaration {
            id: root,
            identity: "BufferedCounter".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![
                    StructuralFieldDeclaration {
                        id: byte_field,
                        identity: "bytes".into(),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::ByteSequence(
                            ByteSequenceCarrier::BoundedOwned { capacity },
                        ),
                    },
                    StructuralFieldDeclaration {
                        id: scalar_field,
                        identity: "counter".into(),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Scalar(scalar),
                    },
                ],
            },
        }];
        assert_eq!(
            shape(root, &declarations),
            expected.map(|(size, _)| ValueShape::integer(size, 8)),
        );
        let expected_access = expected.map(|(_, offset)| (offset, 4));
        assert_eq!(
            store(root, &[], scalar_field, scalar, &declarations),
            expected_access.map(|(offset, bytes)| (offset, bytes, Vec::new()))
        );
        assert_eq!(
            field_read(root, &[], scalar_field, scalar, &declarations),
            expected_access
        );
        assert_eq!(store(root, &[], byte_field, scalar, &declarations), None);
        let length_type = ScalarType::Integer(
            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
                .unwrap(),
        );
        assert_eq!(
            byte_field_length(root, &[], byte_field, length_type, &declarations),
            expected.map(|_| (0, 8)),
        );
        assert_eq!(
            byte_field_length(root, &[], byte_field, scalar, &declarations),
            None
        );
        assert_eq!(
            byte_field_length(root, &[], scalar_field, length_type, &declarations),
            None
        );
        // A shared view field's extent is the word after its backing
        // pointer, independent of any owner capacity; an exclusive view or a
        // scalar carries no observable length.
        for (carrier, length) in [
            (
                StructuralFieldType::ByteSequence(ByteSequenceCarrier::BorrowedView {
                    access: Some(terminal_psi::StructuralAccess::SharedBorrow),
                }),
                Some((8, 8)),
            ),
            (
                StructuralFieldType::ByteSequence(ByteSequenceCarrier::BorrowedView {
                    access: Some(terminal_psi::StructuralAccess::MutableBorrow),
                }),
                None,
            ),
            (StructuralFieldType::Scalar(length_type), None),
        ] {
            let mut changed = declarations.clone();
            let StructuralTypeShape::Record { fields } = &mut changed[0].shape else {
                panic!("record");
            };
            fields[0].field_type = carrier;
            assert_eq!(
                byte_field_length(root, &[], byte_field, length_type, &changed),
                length
            );
        }
        assert_eq!(
            field_read(root, &[], byte_field, scalar, &declarations),
            None
        );
        assert_eq!(
            field_read(root, &[], scalar_field, ScalarType::Boolean, &declarations),
            None,
        );
        assert_eq!(
            field_read(
                root,
                &[StructuralPathSegment::Field("bytes".into())],
                scalar_field,
                scalar,
                &declarations,
            ),
            None,
        );
        assert_eq!(
            owned_aggregate_shape(root, &declarations),
            expected.map(|(size, _)| ValueShape::integer(size, 8)),
        );
    }
}

#[test]
fn byte_observation_geometry_accepts_only_exact_deep_static_paths() {
    use semantic_vocabulary::{ObligationId, ValueId};
    use terminal_psi::{BindingRelevance, ByteSequenceCarrier, StructuralFieldDeclaration};

    let main = StructuralTypeId::new(1).unwrap();
    let bag = StructuralTypeId::new(2).unwrap();
    let rooms = StructuralTypeId::new(3).unwrap();
    let room = StructuralTypeId::new(4).unwrap();
    let exits = StructuralTypeId::new(5).unwrap();
    let exit = StructuralTypeId::new(6).unwrap();
    let bag_field = StructuralFieldId::new(1).unwrap();
    let rooms_field = StructuralFieldId::new(2).unwrap();
    let exits_field = StructuralFieldId::new(3).unwrap();
    let command_field = StructuralFieldId::new(4).unwrap();
    let structural_field = |id, identity: &str, structural_type| StructuralFieldDeclaration {
        id,
        identity: identity.into(),
        relevance: BindingRelevance::Relevant,
        field_type: StructuralFieldType::Structural(structural_type),
    };
    let declarations = [
        StructuralTypeDeclaration {
            id: main,
            identity: "Main".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![structural_field(bag_field, "bag", bag)],
            },
        },
        StructuralTypeDeclaration {
            id: bag,
            identity: "Bag".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![structural_field(rooms_field, "rooms", rooms)],
            },
        },
        StructuralTypeDeclaration {
            id: rooms,
            identity: "Rooms".into(),
            shape: StructuralTypeShape::FixedArray {
                element: room,
                length: 2,
            },
        },
        StructuralTypeDeclaration {
            id: room,
            identity: "Room".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![structural_field(exits_field, "exits", exits)],
            },
        },
        StructuralTypeDeclaration {
            id: exits,
            identity: "Exits".into(),
            shape: StructuralTypeShape::FixedArray {
                element: exit,
                length: 2,
            },
        },
        StructuralTypeDeclaration {
            id: exit,
            identity: "Exit".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: command_field,
                    identity: "command".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::ByteSequence(
                        ByteSequenceCarrier::BoundedOwned { capacity: 8 },
                    ),
                }],
            },
        },
    ];
    let path = vec![
        StructuralPathSegment::Field("bag".into()),
        StructuralPathSegment::Field("rooms".into()),
        StructuralPathSegment::FixedIndex(1),
        StructuralPathSegment::Field("exits".into()),
        StructuralPathSegment::FixedIndex(1),
    ];
    let length_type = ScalarType::Integer(
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
            .unwrap(),
    );

    let geometry = projected_byte_field(main, &path, command_field, &declarations).unwrap();
    assert_eq!(geometry.length_offset, 48);
    assert_eq!(geometry.capacity, Some(8));
    assert_eq!(geometry.descriptor_offset, None);
    assert!(geometry.elements.is_empty());
    let mut dynamic_path = path.clone();
    dynamic_path[2] = StructuralPathSegment::RuntimeIndex {
        index: ValueId::new(1).unwrap(),
        obligation: ObligationId::new(1).unwrap(),
    };
    let dynamic = projected_byte_field(main, &dynamic_path, command_field, &declarations).unwrap();
    assert_eq!(dynamic.length_offset, 16);
    assert_eq!(dynamic.capacity, Some(8));
    assert_eq!(dynamic.elements.len(), 1);
    assert_eq!(dynamic.elements[0].stride, 32);
    assert_eq!(dynamic.elements[0].extent, Some(2));
    // Mutation geometry deliberately retains the narrower, established
    // carrier grammar even though the same leaf is readable.
    assert_eq!(
        byte_field_length(main, &path, command_field, length_type, &declarations),
        None,
    );

    for invalid in [
        {
            let mut changed = path.clone();
            changed[2] = StructuralPathSegment::FixedIndex(2);
            changed
        },
        {
            let mut changed = path.clone();
            changed[1] = StructuralPathSegment::Field("missing".into());
            changed
        },
        {
            let mut changed = path.clone();
            changed.insert(0, StructuralPathSegment::Referent);
            changed
        },
    ] {
        assert!(
            projected_byte_field(main, &invalid, command_field, &declarations).is_none(),
            "{invalid:?}",
        );
    }

    let mut element_view = declarations.clone();
    element_view[2].shape = StructuralTypeShape::ElementView { element: room };
    assert!(projected_byte_field(main, &path, command_field, &element_view).is_none());
}

#[test]
fn owned_transport_keeps_byte_predicates_and_shared_view_field_descriptors() {
    use terminal_psi::{
        BindingRelevance, ByteSequenceCarrier, ByteSequencePredicate, ByteSequencePredicateSet,
        StructuralAccess, StructuralFieldDeclaration,
    };

    let root = StructuralTypeId::new(1).unwrap();
    let nested = StructuralTypeId::new(2).unwrap();
    let mut predicates = ByteSequencePredicateSet::EMPTY;
    predicates.insert(ByteSequencePredicate::ValidUtf8);
    // Per carrier: the two-record array whose records hold the carrier as a
    // field, then the two-element array of the carrier itself, then the
    // carrier as a root.
    for (carrier, field_array, leaf_array, leaf_root) in [
        // A bounded owner travels with its length word and inline capacity
        // wherever it sits.
        (
            ByteSequenceCarrier::BoundedOwned { capacity: 9 },
            Some(48),
            Some(48),
            Some(17),
        ),
        // A bounded view's extent lives in its declaration, not its stored
        // pointer; owned transport does not carry it.
        (
            ByteSequenceCarrier::BoundedView {
                access: Some(StructuralAccess::SharedBorrow),
                capacity: 9,
            },
            None,
            None,
            None,
        ),
        // A shared view field travels as its pointer and extent, as a
        // reference field travels as its address; the copy keeps the
        // source's loan. An array element or root view stays a descriptor
        // rather than becoming an owned aggregate.
        (
            ByteSequenceCarrier::BorrowedView {
                access: Some(StructuralAccess::SharedBorrow),
            },
            Some(32),
            None,
            None,
        ),
        // An exclusive or write-only view cannot be duplicated by a copy.
        (
            ByteSequenceCarrier::BorrowedView {
                access: Some(StructuralAccess::MutableBorrow),
            },
            None,
            None,
            None,
        ),
        (
            ByteSequenceCarrier::BorrowedView {
                access: Some(StructuralAccess::WriteOnlyBorrow),
            },
            None,
            None,
            None,
        ),
        (
            ByteSequenceCarrier::BorrowedView { access: None },
            None,
            None,
            None,
        ),
    ] {
        for qualified in [false, true] {
            let field_type = if qualified {
                StructuralFieldType::PredicateByteSequence {
                    carrier,
                    predicates,
                }
            } else {
                StructuralFieldType::ByteSequence(carrier)
            };
            let mut declarations = vec![
                StructuralTypeDeclaration {
                    id: root,
                    identity: "Container".into(),
                    shape: StructuralTypeShape::FixedArray {
                        element: nested,
                        length: 2,
                    },
                },
                StructuralTypeDeclaration {
                    id: nested,
                    identity: "Label".into(),
                    shape: StructuralTypeShape::Record {
                        fields: vec![StructuralFieldDeclaration {
                            id: StructuralFieldId::new(1).unwrap(),
                            identity: "text".into(),
                            relevance: BindingRelevance::Relevant,
                            field_type,
                        }],
                    },
                },
            ];
            assert_eq!(
                owned_aggregate_shape(root, &declarations),
                field_array.map(|size| ValueShape::integer(size, 8)),
                "carrier {carrier:?}, qualified {qualified}",
            );
            declarations[1].shape = StructuralTypeShape::ByteSequence(carrier);
            assert_eq!(
                owned_aggregate_shape(root, &declarations),
                leaf_array.map(|size| ValueShape::integer(size, 8)),
                "carrier {carrier:?}",
            );
            assert_eq!(
                owned_aggregate_shape(nested, &declarations),
                leaf_root.map(|size| ValueShape::integer(size, 8)),
                "carrier {carrier:?}",
            );
        }
    }
}

#[test]
fn owned_byte_roots_transport_their_bounded_geometry_but_are_not_plain_records() {
    use terminal_psi::ByteSequenceCarrier;

    assert_eq!(
        field_shape(
            &StructuralFieldType::ByteSequence(ByteSequenceCarrier::BorrowedView {
                access: Some(terminal_psi::StructuralAccess::SharedBorrow),
            }),
            &[],
            &mut Vec::new(),
        ),
        Some(ValueShape::integer(16, 8)),
    );
    let root = StructuralTypeId::new(1).unwrap();
    // The length header and inline capacity are the owned payload a bounded
    // byte result or argument transports. That geometry does not make byte
    // storage a plain record.
    for (capacity, expected_size) in [
        (0, Some(8)),
        (3, Some(11)),
        (65_527, Some(65_535)),
        (65_528, Some(65_536)),
        (u64::from(u32::MAX) - 8, Some(u32::MAX)),
        (u64::from(u32::MAX) - 7, None),
        (u64::MAX, None),
    ] {
        let declarations = [StructuralTypeDeclaration {
            id: root,
            identity: "Buffer".into(),
            shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned {
                capacity,
            }),
        }];
        assert_eq!(
            shape(root, &declarations),
            expected_size.map(|size| ValueShape::integer(size, 8)),
            "capacity {capacity}",
        );
        assert_eq!(plain_record_shape(root, &declarations), None);
        assert_eq!(
            owned_aggregate_shape(root, &declarations),
            expected_size.map(|size| ValueShape::integer(size, 8)),
            "capacity {capacity}",
        );
    }
}

#[test]
fn mixed_declarations_have_tag_prefixed_geometry_and_common_field_reads() {
    use terminal_psi::{BindingRelevance, StructuralCaseDeclaration, StructuralFieldDeclaration};

    let root = StructuralTypeId::new(1).unwrap();
    let mixed = StructuralTypeId::new(2).unwrap();
    let nested = StructuralTypeId::new(3).unwrap();
    let scalar = ScalarType::Integer(
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Signed, 32)
            .unwrap(),
    );
    let field = |id, identity: &str, field_type| StructuralFieldDeclaration {
        id: StructuralFieldId::new(id).unwrap(),
        identity: identity.into(),
        relevance: BindingRelevance::Relevant,
        field_type,
    };
    let case = |id, identity: &str, fields| StructuralCaseDeclaration {
        id: semantic_vocabulary::StructuralCaseId::new(id).unwrap(),
        identity: identity.into(),
        fields,
    };
    // Tag at 0, common `flag` at 4, payloads overlaid at 8: `Raised` covers
    // twelve bytes at alignment four, identical to the target-side
    // `conventional_sum_layout_from_parts` evaluation of the same shape.
    let declarations = vec![
        StructuralTypeDeclaration {
            id: root,
            identity: "Main".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![
                    field(1, "mixed", StructuralFieldType::Structural(mixed)),
                    field(2, "tail", StructuralFieldType::Scalar(scalar)),
                ],
            },
        },
        StructuralTypeDeclaration {
            id: mixed,
            identity: "Mixed".into(),
            shape: StructuralTypeShape::Mixed {
                fields: vec![field(1, "flag", StructuralFieldType::Scalar(scalar))],
                cases: vec![
                    case(1, "Flat", vec![]),
                    case(
                        2,
                        "Raised",
                        vec![field(1, "level", StructuralFieldType::Scalar(scalar))],
                    ),
                ],
            },
        },
        StructuralTypeDeclaration {
            id: nested,
            identity: "Pair".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![field(1, "first", StructuralFieldType::Scalar(scalar))],
            },
        },
    ];
    assert_eq!(
        shape(mixed, &declarations),
        Some(ValueShape::integer(12, 4))
    );
    assert_eq!(shape(root, &declarations), Some(ValueShape::integer(16, 4)));
    // Structural payloads and erased-relevance common fields shape the same
    // way the target evaluator sees them. A common field reads and stores at
    // its offset beside the tag whatever case is active; nothing else in the
    // mixed node is a store target.
    let mut with_structural = declarations.clone();
    let StructuralTypeShape::Mixed { cases, fields } = &mut with_structural[1].shape else {
        panic!("mixed");
    };
    cases[1].fields[0].field_type = StructuralFieldType::Structural(nested);
    fields.push(StructuralFieldDeclaration {
        relevance: BindingRelevance::Erased,
        ..field(2, "erased", StructuralFieldType::Scalar(scalar))
    });
    assert_eq!(
        shape(mixed, &with_structural),
        Some(ValueShape::integer(12, 4))
    );
    let flag = StructuralFieldId::new(1).unwrap();
    let into_mixed = vec![StructuralPathSegment::Field("mixed".into())];
    assert_eq!(
        field_read(root, &into_mixed, flag, scalar, &declarations),
        Some((4, 4)),
        "the common `flag` sits after the four-byte tag"
    );
    assert_eq!(field_read(root, &[], flag, scalar, &declarations), None);
    assert_eq!(
        store(root, &into_mixed, flag, scalar, &declarations),
        Some((4, 4, Vec::new())),
        "a common-field store writes exactly the bytes its read observes"
    );
    assert_eq!(store(root, &[], flag, scalar, &declarations), None);
    // Payload ids are a separate namespace: no common field 2 exists, and a
    // mismatched scalar type selects nothing.
    let absent = StructuralFieldId::new(2).unwrap();
    assert_eq!(
        store(root, &into_mixed, absent, scalar, &declarations),
        None
    );
    assert_eq!(
        store(root, &into_mixed, flag, ScalarType::Boolean, &declarations),
        None
    );
    // An empty case set has no inhabitant to provision.
    let mut empty = declarations.clone();
    let StructuralTypeShape::Mixed { cases, .. } = &mut empty[1].shape else {
        panic!("mixed");
    };
    cases.clear();
    assert_eq!(shape(mixed, &empty), None);
    assert_eq!(shape(root, &empty), None);
}

/// A whole `[u8; N]` lent to a borrowed byte view has two readings: the fixed
/// byte window, and (since `u8` element views were admitted) an element view
/// with stride 1. The exclusive and local-array call routes let the byte
/// reading win when both apply, which is sound only because the two readings
/// agree on offset, length and stride, for the whole array and for a window
/// that does not start at zero alike.
#[test]
fn whole_byte_array_loans_read_as_byte_and_element_views_with_one_geometry() {
    use crate::structural_inputs::structural_reference_input::{
        fixed_byte_array_view, fixed_element_array_view,
    };
    use terminal_psi::{
        ByteSequenceCarrier, StructuralAccess, StructuralArgument, StructuralMultiplicity,
        StructuralParameterDeclaration,
    };

    let byte = StructuralTypeId::new(1).unwrap();
    let array = StructuralTypeId::new(2).unwrap();
    let view = StructuralTypeId::new(3).unwrap();
    let declarations = [
        StructuralTypeDeclaration {
            id: byte,
            identity: "u8".into(),
            shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
                semantic_vocabulary::IntegerType::new(
                    semantic_vocabulary::IntegerSign::Unsigned,
                    8,
                )
                .unwrap(),
            )),
        },
        StructuralTypeDeclaration {
            id: array,
            identity: "pause".into(),
            shape: StructuralTypeShape::FixedArray {
                element: byte,
                length: 4,
            },
        },
        StructuralTypeDeclaration {
            id: view,
            identity: "bytes".into(),
            shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                access: None,
            }),
        },
    ];
    let source = StructuralParameterDeclaration {
        place: semantic_vocabulary::PlaceId::new(1).unwrap(),
        position: 0,
        is_self: false,
        structural_type: array,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::MutableBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    for access in [
        StructuralAccess::MutableBorrow,
        StructuralAccess::SharedBorrow,
    ] {
        for (path, expected) in [
            (Vec::new(), (0, 4)),
            (
                vec![StructuralPathSegment::FixedByteRange { start: 1, end: 3 }],
                (1, 2),
            ),
        ] {
            let argument = StructuralArgument {
                place: source.place,
                path,
                access,
            };
            assert_eq!(
                fixed_byte_array_view(&source, &argument, view, &declarations),
                Some(expected)
            );
            assert_eq!(
                fixed_element_array_view(&source, &argument, view, &declarations),
                Some((expected.0, expected.1, 1))
            );
        }
    }
}

#[test]
fn fixed_array_root_reads_its_literal_elements_record_fields() {
    use terminal_psi::{BindingRelevance, StructuralFieldDeclaration};

    let row = StructuralTypeId::new(1).unwrap();
    let rows = StructuralTypeId::new(2).unwrap();
    let scalar = ScalarType::Integer(
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Signed, 32)
            .unwrap(),
    );
    let tag = StructuralFieldId::new(1).unwrap();
    let value = StructuralFieldId::new(2).unwrap();
    let declarations = [
        StructuralTypeDeclaration {
            id: row,
            identity: "Row".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![
                    StructuralFieldDeclaration {
                        id: tag,
                        identity: "tag".into(),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Scalar(scalar),
                    },
                    StructuralFieldDeclaration {
                        id: value,
                        identity: "value".into(),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Scalar(scalar),
                    },
                ],
            },
        },
        StructuralTypeDeclaration {
            id: rows,
            identity: "[Row; 3]".into(),
            shape: StructuralTypeShape::FixedArray {
                element: row,
                length: 3,
            },
        },
    ];
    // `rows[2].tag` and `rows[2].value` sit past two eight-byte records.
    assert_eq!(
        field_read(
            rows,
            &[StructuralPathSegment::FixedIndex(2)],
            tag,
            scalar,
            &declarations
        ),
        Some((16, 4)),
    );
    assert_eq!(
        field_read(
            rows,
            &[StructuralPathSegment::FixedIndex(2)],
            value,
            scalar,
            &declarations
        ),
        Some((20, 4)),
    );
    // No element past the declared extent, and no field without a selected
    // element record.
    assert_eq!(
        field_read(
            rows,
            &[StructuralPathSegment::FixedIndex(3)],
            tag,
            scalar,
            &declarations
        ),
        None,
    );
    assert_eq!(field_read(rows, &[], tag, scalar, &declarations), None);
    assert_eq!(
        field_read(
            rows,
            &[StructuralPathSegment::FixedIndex(0)],
            tag,
            ScalarType::Boolean,
            &declarations
        ),
        None,
    );
}

#[test]
fn case_payload_projection_retains_prefix_stride_and_exact_case_field() {
    use semantic_vocabulary::{StructuralCaseId, StructuralFieldId};
    use terminal_psi::{BindingRelevance, StructuralCaseDeclaration, StructuralFieldDeclaration};
    let leaf = StructuralTypeId::new(3).unwrap();
    let sum = StructuralTypeId::new(2).unwrap();
    let root = StructuralTypeId::new(1).unwrap();
    let selected_case = StructuralCaseId::new(21).unwrap();
    let mut types = vec![
        StructuralTypeDeclaration {
            id: root,
            identity: "Slots".into(),
            shape: StructuralTypeShape::FixedArray {
                element: sum,
                length: 2,
            },
        },
        StructuralTypeDeclaration {
            id: sum,
            identity: "Choice".into(),
            shape: StructuralTypeShape::Sum {
                cases: vec![
                    StructuralCaseDeclaration {
                        id: selected_case,
                        identity: "Present".into(),
                        fields: vec![StructuralFieldDeclaration {
                            id: StructuralFieldId::new(31).unwrap(),
                            identity: "item".into(),
                            relevance: BindingRelevance::Relevant,
                            field_type: StructuralFieldType::Structural(leaf),
                        }],
                    },
                    StructuralCaseDeclaration {
                        id: StructuralCaseId::new(22).unwrap(),
                        identity: "Absent".into(),
                        fields: Vec::new(),
                    },
                ],
            },
        },
        StructuralTypeDeclaration {
            id: leaf,
            identity: "Item".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: StructuralFieldId::new(32).unwrap(),
                    identity: "value".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Scalar(ScalarType::IeeeFloat(
                        semantic_vocabulary::IeeeFloatFormat::Binary64,
                    )),
                }],
            },
        },
    ];
    // Each sum is 16 bytes: tag at zero, eight-byte payload at eight.
    // Borrowing the second element's payload retains backing offset 24.
    let path = vec![
        StructuralPathSegment::FixedIndex(1),
        StructuralPathSegment::Case(selected_case),
        StructuralPathSegment::Field("item".into()),
    ];
    assert_eq!(super::project(root, &path, &types), Some((leaf, 24)));
    assert_eq!(
        super::runtime_projection(root, &path, &types).map(|(leaf, offset, indices)| (
            leaf,
            offset,
            indices.len()
        )),
        Some((leaf, 24, 0))
    );
    for invalid in [
        vec![
            StructuralPathSegment::FixedIndex(2),
            StructuralPathSegment::Case(selected_case),
            StructuralPathSegment::Field("item".into()),
        ],
        vec![
            StructuralPathSegment::FixedIndex(1),
            StructuralPathSegment::Case(StructuralCaseId::new(22).unwrap()),
            StructuralPathSegment::Field("item".into()),
        ],
        vec![
            StructuralPathSegment::FixedIndex(1),
            StructuralPathSegment::Case(selected_case),
        ],
        vec![
            StructuralPathSegment::FixedIndex(1),
            StructuralPathSegment::Case(selected_case),
            StructuralPathSegment::Field("missing".into()),
        ],
        vec![
            StructuralPathSegment::Case(selected_case),
            StructuralPathSegment::Field("item".into()),
        ],
    ] {
        assert_eq!(super::project(root, &invalid, &types), None);
    }
    let StructuralTypeShape::Sum { cases } = &mut types[1].shape else {
        unreachable!()
    };
    cases[0].fields[0].relevance = BindingRelevance::Erased;
    assert_eq!(super::project(root, &path, &types), None);
}

#[test]
fn indexed_byte_field_geometry_keeps_carrier_stride_and_descriptor_offsets() {
    use semantic_vocabulary::{IntegerSign, IntegerType, ObligationId, ValueId};
    use terminal_psi::{
        BindingRelevance, ByteSequenceCarrier, StructuralAccess, StructuralFieldDeclaration,
    };
    let record = StructuralTypeId::new(1).unwrap();
    let view = StructuralTypeId::new(2).unwrap();
    let array = StructuralTypeId::new(3).unwrap();
    let field = StructuralFieldId::new(2).unwrap();
    let count = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let types = vec![
        StructuralTypeDeclaration {
            id: record,
            identity: "Record".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![
                    StructuralFieldDeclaration {
                        id: StructuralFieldId::new(1).unwrap(),
                        identity: "prefix".into(),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Scalar(count),
                    },
                    StructuralFieldDeclaration {
                        id: field,
                        identity: "bytes".into(),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::ByteSequence(
                            ByteSequenceCarrier::BorrowedView {
                                access: Some(StructuralAccess::SharedBorrow),
                            },
                        ),
                    },
                ],
            },
        },
        StructuralTypeDeclaration {
            id: view,
            identity: "View".into(),
            shape: StructuralTypeShape::ElementView { element: record },
        },
        StructuralTypeDeclaration {
            id: array,
            identity: "Array".into(),
            shape: StructuralTypeShape::FixedArray {
                element: record,
                length: 3,
            },
        },
    ];
    let index = ValueId::new(1).unwrap();
    let obligation = ObligationId::new(1).unwrap();
    let path = [StructuralPathSegment::RuntimeIndex { index, obligation }];
    for (root, extent) in [(view, None), (array, Some(3))] {
        let geometry = super::projected_byte_field(root, &path, field, &types).unwrap();
        assert_eq!(geometry.length_offset, 16);
        assert_eq!(geometry.descriptor_offset, Some(8));
        assert_eq!(geometry.capacity, None);
        assert_eq!(geometry.elements.len(), 1);
        let element = &geometry.elements[0];
        assert_eq!(
            (
                element.index,
                element.obligation,
                element.stride,
                element.extent
            ),
            (index, obligation, 24, extent)
        );
        assert!(!super::runtime_indices_match(&[], &geometry.elements));
        let indices = vec![legalized_operations::LegalizedRuntimeIndexOperand {
            operand: abstract_operations::AbstractResult {
                value: index,
                scalar_type: count,
            },
            stride: 24,
            extent,
            obligation,
            accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes([0; 32]),
        }];
        assert!(super::runtime_indices_match(&indices, &geometry.elements));
        let mut wrong = indices.clone();
        wrong[0].stride = 16;
        assert!(!super::runtime_indices_match(&wrong, &geometry.elements));
        wrong = indices.clone();
        wrong[0].operand.value = ValueId::new(2).unwrap();
        assert!(!super::runtime_indices_match(&wrong, &geometry.elements));
        wrong = indices;
        wrong[0].obligation = ObligationId::new(2).unwrap();
        assert!(!super::runtime_indices_match(&wrong, &geometry.elements));
    }
    assert!(super::projected_byte_field(view, &[], field, &types).is_none());
    assert!(
        super::projected_byte_field(view, &path, StructuralFieldId::new(3).unwrap(), &types)
            .is_none()
    );
}
