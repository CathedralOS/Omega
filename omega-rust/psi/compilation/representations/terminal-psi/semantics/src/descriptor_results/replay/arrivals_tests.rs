use super::*;
use crate::descriptor_lifetimes::{DescriptorFlowBinding, DescriptorFlowEdge, DescriptorFlowStep};
use semantic_vocabulary::{BlockId, IntegerSign, IntegerType, ScalarType};
use terminal_psi::TerminalIdentityIndex;

#[test]
fn borrowed_arrival_keeps_stored_reference_cohort_and_unknown_pointer_fences() {
    let scalar = StructuralTypeId::new(1).unwrap();
    let pointer = StructuralTypeId::new(2).unwrap();
    let array = StructuralTypeId::new(3).unwrap();
    let view = StructuralTypeId::new(4).unwrap();
    let declaration = StructuralTypeDeclaration {
        quotient_carrier: None,
        id: scalar,
        identity: TerminalIdentityIndex(1),
        owns_linear_claim: false,
        no_code_affine_cleanup: true,
        copyable: true,
        descriptor_cleanup: false,
        scalar_restriction: None,
        shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
            IntegerType::new(IntegerSign::Signed, 64).unwrap(),
        )),
    };
    let types = vec![
        declaration.clone(),
        StructuralTypeDeclaration {
            id: pointer,
            identity: TerminalIdentityIndex(2),
            shape: StructuralTypeShape::Reference {
                referent: scalar,
                access: StructuralAccess::SharedBorrow,
            },
            ..declaration.clone()
        },
        StructuralTypeDeclaration {
            id: array,
            identity: TerminalIdentityIndex(3),
            shape: StructuralTypeShape::FixedArray {
                element: pointer,
                length: terminal_psi::ArrayExtent::Literal(1),
            },
            ..declaration.clone()
        },
        StructuralTypeDeclaration {
            id: view,
            identity: TerminalIdentityIndex(4),
            shape: StructuralTypeShape::Reference {
                referent: array,
                access: StructuralAccess::WriteOnlyBorrow,
            },
            ..declaration
        },
    ];
    let location = |index| DescriptorLocation::whole(PlaceId::new(index).unwrap());
    let parameter = location(1);
    let temporary = location(2);
    let arrival = location(3);
    let source = DescriptorLocation {
        root: temporary.root,
        path: vec![DescriptorPathSegment::Referent],
    };
    let block = BlockId::new(1).unwrap();
    let entry = (block, 0);
    let received = (block, 1);
    let graph = vec![DescriptorMachine {
        id: MachineId::new(1).unwrap(),
        returns: None,
        places: vec![
            DescriptorPlace {
                place: parameter.root,
                definition: DescriptorDefinition::Parameter {
                    position: 0,
                    access: StructuralAccess::MutableBorrow,
                    structural_type: array,
                },
            },
            DescriptorPlace {
                place: temporary.root,
                definition: DescriptorDefinition::Local,
            },
            DescriptorPlace {
                place: arrival.root,
                definition: DescriptorDefinition::Arrival {
                    access: StructuralAccess::WriteOnlyBorrow,
                },
            },
        ],
        place_types: BTreeMap::from([
            (parameter.root, array),
            (temporary.root, view),
            (arrival.root, array),
        ]),
        entry,
        flow: BTreeMap::from([
            (
                entry,
                DescriptorFlowStep {
                    definitions: vec![temporary.clone()],
                    derivations: vec![(parameter.clone(), temporary.clone())],
                    successors: vec![DescriptorFlowEdge {
                        target: received,
                        parameters: vec![arrival.clone()],
                        bindings: vec![
                            DescriptorFlowBinding::new(
                                source,
                                arrival.clone(),
                                StructuralAccess::WriteOnlyBorrow,
                                StructuralAccess::WriteOnlyBorrow,
                            )
                            .unwrap(),
                        ],
                    }],
                    ..Default::default()
                },
            ),
            (received, DescriptorFlowStep::default()),
        ]),
        case_constructors: BTreeMap::new(),
        case_successors: BTreeMap::new(),
        originations: BTreeMap::new(),
        projections: BTreeMap::new(),
        contents_queries: BTreeMap::new(),
        contents_transfers: BTreeMap::new(),
        payload_producers: BTreeMap::new(),
        payload_entry_predicates: None,
        payload_scalar_definitions: BTreeMap::new(),
        payload_scalar_postconditions: BTreeMap::new(),
        payload_scalar_parameters: BTreeMap::new(),
        scalar_payload_reads: BTreeMap::new(),
        payload_edge_predicates: BTreeMap::new(),
        payload_scalar_bindings: BTreeMap::new(),
        scalar_payload_writes: BTreeMap::new(),
        payload_invalidations: BTreeMap::new(),
        unresolved_contents_writes: BTreeSet::new(),
        storage_derivations: BTreeSet::from([(entry, parameter.clone(), temporary)]),
        calls: BTreeMap::new(),
        normal_exits: vec![received],
    }];
    let mut replay = Replay::with_slot_kind(&types, &graph, DescriptorSlotKind::References);
    let states = replay.states(graph[0].id, 0).unwrap();
    let mut destination = arrival;
    destination.path.push(DescriptorPathSegment::FixedIndex(0));
    let mut original = parameter;
    original.path.push(DescriptorPathSegment::FixedIndex(0));
    let query = DescriptorContentsQuery::from_location(&destination);
    assert_eq!(
        replay.contents_targets(&graph[0], &states[&received], &query),
        Some(BTreeSet::from([DescriptorPayloadTarget::Storage(
            original.clone()
        )])),
        "a borrowed arrival must preserve the field storage reached through its formed view",
    );
    // The alias identifies this field's storage, not an unknown pointer stored
    // inside it. Retiring that pointer still blocks a deeper pointee access.
    let mut invalidated = states[&received].clone();
    invalidated.slots_mut().insert(original, None);
    destination.path.push(DescriptorPathSegment::Referent);
    assert!(
        replay
            .contents_targets(
                &graph[0],
                &invalidated,
                &DescriptorContentsQuery::from_location(&destination),
            )
            .is_none()
    );
}
