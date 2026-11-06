use super::*;
use semantic_vocabulary::{BlockId, ScalarType, StructuralFieldId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    BindingRelevance, StructuralFieldDeclaration, StructuralFieldType, StructuralRecordStorage,
    TerminalIdentityIndex,
};

fn ty(id: u64, shape: StructuralTypeShape) -> StructuralTypeDeclaration {
    StructuralTypeDeclaration {
        id: StructuralTypeId::new(id).unwrap(),
        identity: TerminalIdentityIndex(id as u32),
        quotient_carrier: None,
        owns_linear_claim: false,
        no_code_affine_cleanup: true,
        copyable: matches!(&shape, StructuralTypeShape::PrimitiveScalar(_)),
        descriptor_cleanup: matches!(
            &shape,
            StructuralTypeShape::Reference { .. }
                | StructuralTypeShape::Record { .. }
                | StructuralTypeShape::ElementView { .. }
        ),
        scalar_restriction: None,
        shape,
    }
}

fn field(id: u64, field_type: StructuralFieldType) -> StructuralFieldDeclaration {
    StructuralFieldDeclaration {
        id: StructuralFieldId::new(id).unwrap(),
        identity: TerminalIdentityIndex(id as u32 + 10),
        inline_carrier: None,
        relevance: BindingRelevance::Relevant,
        storage: None,
        field_type,
    }
}

fn types() -> Vec<StructuralTypeDeclaration> {
    vec![
        ty(1, StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean)),
        ty(
            2,
            StructuralTypeShape::Reference {
                referent: StructuralTypeId::new(1).unwrap(),
                access: StructuralAccess::MutableBorrow,
            },
        ),
        ty(
            3,
            StructuralTypeShape::Record {
                storage: StructuralRecordStorage::Packed,
                fields: vec![
                    field(
                        1,
                        StructuralFieldType::Structural(StructuralTypeId::new(2).unwrap()),
                    ),
                    field(
                        2,
                        StructuralFieldType::Structural(StructuralTypeId::new(6).unwrap()),
                    ),
                    field(3, StructuralFieldType::Atomic(ScalarType::Boolean)),
                ],
            },
        ),
        ty(
            4,
            StructuralTypeShape::Reference {
                referent: StructuralTypeId::new(99).unwrap(),
                access: StructuralAccess::MutableBorrow,
            },
        ),
        ty(
            5,
            StructuralTypeShape::Record {
                storage: StructuralRecordStorage::Packed,
                fields: vec![field(
                    1,
                    StructuralFieldType::Structural(StructuralTypeId::new(4).unwrap()),
                )],
            },
        ),
        ty(
            6,
            StructuralTypeShape::ElementView {
                element: StructuralTypeId::new(1).unwrap(),
                access: Some(StructuralAccess::MutableBorrow),
            },
        ),
        ty(
            7,
            StructuralTypeShape::FixedArray {
                element: StructuralTypeId::new(1).unwrap(),
                length: terminal_psi::ArrayExtent::Literal(2),
            },
        ),
    ]
}

fn position(ordinal: u32) -> DescriptorPosition {
    (BlockId::new(1).unwrap(), ordinal)
}

fn fixture() -> (DescriptorMachine, StructuralArgument) {
    let changed = PlaceId::new(1).unwrap();
    let unrelated = PlaceId::new(2).unwrap();
    let input = PlaceId::new(3).unwrap();
    let elements = PlaceId::new(4).unwrap();
    let view = PlaceId::new(5).unwrap();
    let slot = DescriptorLocation {
        root: input,
        path: vec![DescriptorPathSegment::Field(
            StructuralFieldId::new(1).unwrap(),
        )],
    };
    let view_slot = DescriptorLocation {
        root: input,
        path: vec![DescriptorPathSegment::Field(
            StructuralFieldId::new(2).unwrap(),
        )],
    };
    let flow = (0..4)
        .map(|ordinal| {
            (
                position(ordinal),
                DescriptorFlowStep {
                    definitions: if ordinal == 0 {
                        vec![DescriptorLocation::whole(view)]
                    } else {
                        Vec::new()
                    },
                    derivations: if ordinal == 0 {
                        vec![(
                            DescriptorLocation::whole(elements),
                            DescriptorLocation::whole(view),
                        )]
                    } else if ordinal == 1 {
                        vec![
                            (DescriptorLocation::whole(changed), slot.clone()),
                            (DescriptorLocation::whole(view), view_slot.clone()),
                        ]
                    } else {
                        Vec::new()
                    },
                    successors: if ordinal < 3 {
                        vec![DescriptorFlowEdge {
                            target: position(ordinal + 1),
                            parameters: vec![],
                            bindings: vec![],
                        }]
                    } else {
                        Vec::new()
                    },
                    ..Default::default()
                },
            )
        })
        .collect();
    let place_types = BTreeMap::from([
        (changed, StructuralTypeId::new(1).unwrap()),
        (unrelated, StructuralTypeId::new(1).unwrap()),
        (input, StructuralTypeId::new(3).unwrap()),
        (elements, StructuralTypeId::new(7).unwrap()),
        (view, StructuralTypeId::new(6).unwrap()),
    ]);
    let machine = DescriptorMachine {
        id: MachineId::new(1).unwrap(),
        entry: position(0),
        returns: None,
        places: place_types
            .iter()
            .enumerate()
            .map(|(index, (place, ty))| DescriptorPlace {
                place: *place,
                definition: if *place == view {
                    DescriptorDefinition::Local
                } else {
                    DescriptorDefinition::Parameter {
                        position: index,
                        access: StructuralAccess::Owned,
                        structural_type: *ty,
                    }
                },
            })
            .collect(),
        place_types,
        flow,
        storage_derivations: BTreeSet::from([(
            position(1),
            DescriptorLocation::whole(changed),
            slot,
        )]),
        normal_exits: vec![position(3)],
        originations: BTreeMap::new(),
        projections: BTreeMap::new(),
        contents_queries: BTreeMap::new(),
        contents_transfers: BTreeMap::new(),
        payload_producers: BTreeMap::new(),
        payload_stores: BTreeMap::new(),
        primitive_payload_writes: Default::default(),
        opaque_payload_results: Default::default(),
        payload_entry_predicates: None,
        payload_scalar_definitions: BTreeMap::new(),
        payload_scalar_postconditions: BTreeMap::new(),
        payload_scalar_parameters: BTreeMap::new(),
        scalar_payload_reads: BTreeMap::new(),
        payload_edge_predicates: BTreeMap::new(),
        payload_scalar_bindings: BTreeMap::new(),
        scalar_payload_writes: BTreeMap::new(),
        payload_invalidations: BTreeMap::new(),
        case_constructors: BTreeMap::new(),
        case_successors: BTreeMap::new(),
        unresolved_contents_writes: BTreeSet::new(),
        calls: BTreeMap::new(),
    };
    (
        machine,
        StructuralArgument {
            place: input,
            path: vec![],
            access: StructuralAccess::MutableBorrow,
        },
    )
}

#[test]
fn call_payload_writes_follow_current_pointers_without_invalidating_unrelated_storage() {
    let types = types();
    let (machine, mut argument) = fixture();
    let graph = std::slice::from_ref(&machine);
    let mut analysis = DescriptorAnalysis::with_borrowed_storage_slots(&types, graph);
    for access in [
        StructuralAccess::MutableBorrow,
        StructuralAccess::SharedBorrow,
        StructuralAccess::WriteOnlyBorrow,
    ] {
        argument.access = access;
        let queries = for_argument(&types, &argument, StructuralTypeId::new(3).unwrap()).unwrap();
        let targets = queries
            .iter()
            .flat_map(|query| {
                analysis
                    .payload_targets(machine.id, position(2), query)
                    .unwrap()
            })
            .collect::<BTreeSet<_>>();
        let mut expected = BTreeSet::from([DescriptorPayloadTarget::Storage(
            DescriptorLocation::whole(PlaceId::new(3).unwrap()),
        )]);
        if access == StructuralAccess::MutableBorrow {
            expected.insert(DescriptorPayloadTarget::Storage(DescriptorLocation::whole(
                PlaceId::new(1).unwrap(),
            )));
            for index in 0..2 {
                expected.insert(DescriptorPayloadTarget::Storage(DescriptorLocation {
                    root: PlaceId::new(4).unwrap(),
                    path: vec![DescriptorPathSegment::FixedIndex(index)],
                }));
            }
        }
        assert_eq!(targets, expected);
    }
}

#[test]
fn call_payload_writes_keep_unknown_reachable_storage_unavailable() {
    let mut types = types();
    let (_, mut argument) = fixture();
    for access in [
        StructuralAccess::MutableBorrow,
        StructuralAccess::SharedBorrow,
    ] {
        argument.access = access;
        assert!(for_argument(&types, &argument, StructuralTypeId::new(5).unwrap()).is_err());
    }
    argument.access = StructuralAccess::WriteOnlyBorrow;
    assert_eq!(
        for_argument(&types, &argument, StructuralTypeId::new(5).unwrap())
            .unwrap()
            .len(),
        1
    );
    assert!(for_argument(&types, &argument, StructuralTypeId::new(99).unwrap()).is_err());
    argument.access = StructuralAccess::MutableBorrow;
    types
        .iter_mut()
        .find(|ty| ty.id == StructuralTypeId::new(6).unwrap())
        .unwrap()
        .shape = StructuralTypeShape::ElementView {
        element: StructuralTypeId::new(3).unwrap(),
        access: Some(StructuralAccess::MutableBorrow),
    };
    assert!(
        for_argument(&types, &argument, StructuralTypeId::new(3).unwrap()).is_err(),
        "view elements cannot hide further pointer writes"
    );
    types
        .iter_mut()
        .find(|ty| ty.id == StructuralTypeId::new(6).unwrap())
        .unwrap()
        .shape =
        StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView {
            access: Some(StructuralAccess::MutableBorrow),
        });
    assert!(
        for_argument(&types, &argument, StructuralTypeId::new(3).unwrap()).is_err(),
        "unavailable byte backing cannot become an empty effect"
    );
}

#[test]
fn call_payload_writes_detect_mutable_views_below_element_records() {
    let mut types = types();
    types.push(ty(
        8,
        StructuralTypeShape::Record {
            storage: StructuralRecordStorage::Packed,
            fields: vec![field(
                1,
                StructuralFieldType::Structural(StructuralTypeId::new(6).unwrap()),
            )],
        },
    ));
    types.push(ty(
        9,
        StructuralTypeShape::ElementView {
            element: StructuralTypeId::new(8).unwrap(),
            access: Some(StructuralAccess::MutableBorrow),
        },
    ));
    let (_, argument) = fixture();
    assert_eq!(
        crate::shared_mutation(types.iter(), StructuralTypeId::new(8).unwrap(), &[]),
        crate::SharedMutation::Unbounded
    );
    assert!(
        for_argument(&types, &argument, StructuralTypeId::new(9).unwrap()).is_err(),
        "a nested mutable view must not disappear from the element frontier"
    );
    for access in [Some(StructuralAccess::SharedBorrow), None] {
        types
            .iter_mut()
            .find(|ty| ty.id == StructuralTypeId::new(6).unwrap())
            .unwrap()
            .shape = StructuralTypeShape::ElementView {
            element: StructuralTypeId::new(1).unwrap(),
            access,
        };
        assert_eq!(
            crate::shared_mutation(types.iter(), StructuralTypeId::new(8).unwrap(), &[]),
            crate::SharedMutation::None
        );
        if access.is_none() {
            assert!(
                for_argument(&types, &argument, StructuralTypeId::new(9).unwrap()).is_err(),
                "an outer writable element does not supply a canonical inner descriptor's occurrence access"
            );
        } else {
            assert!(for_argument(&types, &argument, StructuralTypeId::new(9).unwrap()).is_ok());
        }
    }
}
