use super::*;
use crate::descriptor_lifetimes::{DescriptorFlowEdge, DescriptorPathSegment};
use semantic_vocabulary::{BlockId, ContractId, EdgeId, StructuralFieldId};
use terminal_psi::{
    Block, MachineContract, StructuralParameterDeclaration, StructuralPlaceDeclaration,
    StructuralResultDeclaration, TerminalMachineResult,
};

fn place(id: u64) -> PlaceId {
    PlaceId::new(id).unwrap()
}
fn machine(id: u64) -> MachineId {
    MachineId::new(id).unwrap()
}
fn ty(id: u64) -> StructuralTypeId {
    StructuralTypeId::new(id).unwrap()
}
fn pos(index: u32) -> DescriptorPosition {
    (BlockId::new(1).unwrap(), index)
}
fn whole(id: u64) -> DescriptorLocation {
    DescriptorLocation::whole(place(id))
}
fn slot(root: u64, field: u64) -> DescriptorLocation {
    DescriptorLocation {
        root: place(root),
        path: vec![DescriptorPathSegment::Field(
            StructuralFieldId::new(field).unwrap(),
        )],
    }
}
fn types() -> Vec<StructuralTypeDeclaration> {
    vec![
        StructuralTypeDeclaration {
            id: ty(1),
            identity: "view".into(),
            shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                access: None,
            }),
        },
        StructuralTypeDeclaration {
            id: ty(2),
            identity: "record".into(),
            shape: StructuralTypeShape::Record {
                fields: (1..=2)
                    .map(|id| terminal_psi::StructuralFieldDeclaration {
                        id: StructuralFieldId::new(id).unwrap(),
                        identity: format!("field{id}"),
                        relevance: terminal_psi::BindingRelevance::Relevant,
                        field_type: terminal_psi::StructuralFieldType::ByteSequence(
                            ByteSequenceCarrier::BorrowedView {
                                access: Some(StructuralAccess::SharedBorrow),
                            },
                        ),
                    })
                    .collect(),
            },
        },
    ]
}
fn edge(index: u32) -> DescriptorFlowEdge {
    DescriptorFlowEdge {
        target: pos(index),
        parameters: Vec::new(),
        bindings: Vec::new(),
    }
}
fn new_graph(id: u64) -> DescriptorMachine {
    DescriptorMachine {
        id: machine(id),
        returns: None,
        places: vec![
            DescriptorPlace {
                place: place(1),
                definition: DescriptorDefinition::Parameter {
                    position: 0,
                    access: StructuralAccess::SharedBorrow,
                    structural_type: ty(1),
                },
            },
            DescriptorPlace {
                place: place(2),
                definition: DescriptorDefinition::Parameter {
                    position: 1,
                    access: StructuralAccess::MutableBorrow,
                    structural_type: ty(2),
                },
            },
        ],
        place_types: BTreeMap::from([(place(1), ty(1)), (place(2), ty(2))]),
        entry: pos(0),
        flow: BTreeMap::new(),
        originations: BTreeMap::new(),
        projections: BTreeMap::new(),
        calls: BTreeMap::new(),
        normal_exits: Vec::new(),
    }
}
fn transfer(
    source: DescriptorLocation,
    destination: DescriptorLocation,
    next: u32,
) -> DescriptorFlowStep {
    DescriptorFlowStep {
        definitions: vec![destination.clone()],
        derivations: vec![(source.clone(), destination.clone())],
        authority_derivations: vec![(source.clone(), destination)],
        inputs: vec![source],
        successors: vec![edge(next)],
        ..Default::default()
    }
}
fn store_graph() -> DescriptorMachine {
    let mut graph = new_graph(1);
    graph.flow.insert(pos(0), transfer(whole(1), slot(2, 1), 1));
    graph.flow.insert(pos(1), DescriptorFlowStep::default());
    graph.normal_exits.push(pos(1));
    graph
}

#[test]
fn normal_unit_exit_exposes_exact_stored_backing_without_sibling_effect() {
    let graph = [store_graph()];
    let effects = reconstruct_stored_effects(&types(), &graph, machine(1)).unwrap();
    assert_eq!(effects.len(), 1);
    assert_eq!(effects[0].parameter, 1);
    assert_eq!(effects[0].path, slot(2, 1).path);
    assert!(matches!(
        effects[0].origins.as_slice(),
        [DescriptorStoredOrigin::Parameter { position: 0, .. }]
    ));
}

#[test]
fn conditional_store_keeps_incoming_slot_distinct_from_unknown() {
    let mut graph = store_graph();
    graph.flow.insert(
        pos(0),
        DescriptorFlowStep {
            successors: vec![edge(1), edge(2)],
            ..Default::default()
        },
    );
    graph.flow.insert(pos(1), transfer(whole(1), slot(2, 1), 3));
    graph.flow.insert(
        pos(2),
        DescriptorFlowStep {
            successors: vec![edge(3)],
            ..Default::default()
        },
    );
    graph.flow.insert(pos(3), DescriptorFlowStep::default());
    graph.normal_exits = vec![pos(3)];
    let graphs = [graph];
    let catalog = types();
    let mut analysis = DescriptorAnalysis::new(&catalog, &graphs);
    let effects = analysis.stored_effects(machine(1)).unwrap();
    assert_eq!(effects.len(), 1);
    assert_eq!(effects[0].origins.len(), 2);
    assert!(effects[0].origins.iter().any(|origin| matches!(origin, DescriptorStoredOrigin::IncomingSlot { position: 1, path } if path == &slot(2,1).path)));
    let before = analysis
        .source_origins(machine(1), pos(1), &slot(2, 1))
        .unwrap();
    assert!(
        matches!(before.as_slice(), [DescriptorCallOrigin::Argument { source, projection }] if *source == place(2) && projection.indirect)
    );
}

#[test]
fn replacing_one_slot_preserves_copied_old_descriptor_and_sibling() {
    let mut graph = new_graph(1);
    graph.places.push(DescriptorPlace {
        place: place(3),
        definition: DescriptorDefinition::Local,
    });
    graph.place_types.insert(place(3), ty(1));
    graph.flow.insert(pos(0), transfer(slot(2, 1), whole(3), 1));
    graph.flow.insert(pos(1), transfer(whole(1), slot(2, 1), 2));
    graph.flow.insert(pos(2), DescriptorFlowStep::default());
    graph.normal_exits.push(pos(2));
    let graphs = [graph];
    let catalog = types();
    let mut analysis = DescriptorAnalysis::new(&catalog, &graphs);
    for location in [whole(3), slot(2, 2)] {
        assert!(
            matches!(analysis.source_origins(machine(1), pos(2), &location).unwrap().as_slice(), [DescriptorCallOrigin::Argument { source, projection }] if *source == place(2) && projection.indirect)
        );
    }
    assert!(
        matches!(analysis.source_origins(machine(1), pos(2), &slot(2,1)).unwrap().as_slice(), [DescriptorCallOrigin::Argument { source, projection }] if *source == place(1) && !projection.indirect)
    );
}

#[test]
fn stored_effect_rejects_local_backing_and_source_query_keeps_it_local() {
    let mut graph = new_graph(1);
    graph.places.push(DescriptorPlace {
        place: place(3),
        definition: DescriptorDefinition::Local,
    });
    graph.place_types.insert(place(3), ty(1));
    graph.flow.insert(
        pos(0),
        DescriptorFlowStep {
            definitions: vec![whole(3)],
            successors: vec![edge(1)],
            ..Default::default()
        },
    );
    graph.originations.insert(
        pos(0),
        vec![(
            whole(3),
            DescriptorCallOrigin::Argument {
                source: place(3),
                projection: Projection::new(&[]),
            },
        )],
    );
    graph.flow.insert(pos(1), transfer(whole(3), slot(2, 1), 2));
    graph.flow.insert(pos(2), DescriptorFlowStep::default());
    graph.normal_exits.push(pos(2));
    let graphs = [graph];
    let catalog = types();
    let mut analysis = DescriptorAnalysis::new(&catalog, &graphs);
    assert!(
        matches!(analysis.source_origins(machine(1),pos(1),&whole(3)).unwrap().as_slice(), [DescriptorCallOrigin::Argument { source, .. }] if *source == place(3))
    );
    assert!(analysis.stored_effects(machine(1)).is_none());
}

#[test]
fn unit_call_applies_writeback_and_materializes_the_same_cohort_transfer() {
    let callee = store_graph();
    let mut caller = new_graph(2);
    caller.flow.insert(
        pos(0),
        DescriptorFlowStep {
            successors: vec![edge(1)],
            ..Default::default()
        },
    );
    caller.flow.insert(pos(1), DescriptorFlowStep::default());
    caller.normal_exits.push(pos(1));
    caller.calls.insert(
        pos(0),
        DescriptorInvocation {
            callee: machine(1),
            arguments: vec![Some(whole(1)), Some(whole(2))],
            result: None,
        },
    );
    let graphs = [callee, caller];
    let catalog = types();
    let mut analysis = DescriptorAnalysis::new(&catalog, &graphs);
    let effects = analysis.call_stored_effects(machine(2), pos(0)).unwrap();
    assert_eq!(effects.len(), 1);
    assert!(!effects[0].preserves_incoming);
    assert!(
        matches!(analysis.source_origins(machine(2),pos(1),&slot(2,1)).unwrap().as_slice(),[DescriptorCallOrigin::Argument{source,..}] if *source == place(1))
    );
    assert!(
        analysis.flow(machine(2)).unwrap()[&pos(0)]
            .derivations
            .contains(&(whole(1), slot(2, 1)))
    );
}

#[test]
fn borrowed_block_forwarding_updates_original_output_slot() {
    let mut graph = new_graph(1);
    graph.places.push(DescriptorPlace {
        place: place(3),
        definition: DescriptorDefinition::Local,
    });
    graph.place_types.insert(place(3), ty(2));
    graph.flow.insert(
        pos(0),
        DescriptorFlowStep {
            successors: vec![DescriptorFlowEdge {
                target: pos(1),
                parameters: vec![whole(3)],
                bindings: vec![(whole(2), whole(3))],
            }],
            ..Default::default()
        },
    );
    graph.flow.insert(pos(1), transfer(whole(1), slot(3, 1), 2));
    graph.flow.insert(pos(2), DescriptorFlowStep::default());
    graph.normal_exits.push(pos(2));
    let effects = reconstruct_stored_effects(&types(), &[graph], machine(1)).unwrap();
    assert_eq!(effects.len(), 1);
    assert_eq!(effects[0].parameter, 1);
}

#[test]
fn alias_slot_replacement_kills_old_names_but_retains_a_detached_copy() {
    use crate::descriptor_lifetimes::descriptor_location_live_positions;
    let mut graph = new_graph(1);
    for (id, structural_type) in [(3, ty(2)), (4, ty(1))] {
        graph.places.push(DescriptorPlace {
            place: place(id),
            definition: DescriptorDefinition::Local,
        });
        graph.place_types.insert(place(id), structural_type);
    }
    let mut formation = transfer(whole(1), slot(2, 1), 1);
    formation.successors[0].parameters = vec![whole(3)];
    formation.successors[0].bindings = vec![(whole(2), whole(3))];
    graph.flow.insert(pos(0), formation);
    graph.flow.insert(pos(1), transfer(slot(3, 1), whole(4), 2));
    graph.flow.insert(
        pos(2),
        DescriptorFlowStep {
            definitions: vec![slot(3, 1)],
            successors: vec![edge(3)],
            ..Default::default()
        },
    );
    graph
        .originations
        .insert(pos(2), vec![(slot(2, 1), DescriptorCallOrigin::Empty)]);
    graph.flow.insert(
        pos(3),
        DescriptorFlowStep {
            inputs: vec![slot(3, 1)],
            successors: vec![edge(4)],
            ..Default::default()
        },
    );
    graph.flow.insert(
        pos(4),
        DescriptorFlowStep {
            inputs: vec![whole(4)],
            ..Default::default()
        },
    );
    let catalog = types();
    let graphs = [graph];
    let mut analysis = DescriptorAnalysis::new(&catalog, &graphs);
    let mut flow = analysis.flow(machine(1)).unwrap();
    assert!(flow[&pos(2)].definitions.contains(&slot(2, 1)));
    assert!(flow[&pos(2)].definitions.contains(&slot(3, 1)));
    assert!(descriptor_location_live_positions(&flow, pos(0), &slot(2, 1)).contains(&pos(4)));
    flow.get_mut(&pos(4)).unwrap().inputs.clear();
    let live = descriptor_location_live_positions(&flow, pos(0), &slot(2, 1));
    assert!(live.contains(&pos(1)));
    assert!(!live.contains(&pos(3)));
}

#[test]
fn known_empty_slot_survives_conditional_no_store_and_unknown_does_not() {
    let mut callee = store_graph();
    callee.flow.insert(
        pos(0),
        DescriptorFlowStep {
            successors: vec![edge(1), edge(2)],
            ..Default::default()
        },
    );
    callee
        .flow
        .insert(pos(1), transfer(whole(1), slot(2, 1), 2));
    callee.flow.insert(pos(2), DescriptorFlowStep::default());
    callee.normal_exits = vec![pos(2)];
    let mut caller = new_graph(2);
    caller.places.push(DescriptorPlace {
        place: place(3),
        definition: DescriptorDefinition::Local,
    });
    caller.place_types.insert(place(3), ty(2));
    caller.flow.insert(
        pos(0),
        DescriptorFlowStep {
            definitions: vec![whole(3)],
            successors: vec![edge(1)],
            ..Default::default()
        },
    );
    caller.originations.insert(
        pos(0),
        vec![
            (
                whole(3),
                DescriptorCallOrigin::Argument {
                    source: place(3),
                    projection: Projection::new(&[]),
                },
            ),
            (slot(3, 1), DescriptorCallOrigin::Empty),
            (slot(3, 2), DescriptorCallOrigin::Empty),
        ],
    );
    caller.flow.insert(
        pos(1),
        DescriptorFlowStep {
            successors: vec![edge(2)],
            ..Default::default()
        },
    );
    caller.flow.insert(pos(2), DescriptorFlowStep::default());
    caller.calls.insert(
        pos(1),
        DescriptorInvocation {
            callee: machine(1),
            arguments: vec![Some(whole(1)), Some(whole(3))],
            result: None,
        },
    );
    let catalog = types();
    let mut graphs = [callee, caller];
    let origins =
        reconstruct_source_origins(&catalog, &graphs, machine(2), pos(2), &slot(3, 1)).unwrap();
    assert!(origins.contains(&DescriptorCallOrigin::Empty));
    assert_eq!(origins.len(), 2);
    graphs[1]
        .originations
        .get_mut(&pos(0))
        .unwrap()
        .retain(|(location, _)| location != &slot(3, 1));
    assert!(
        reconstruct_source_origins(&catalog, &graphs, machine(2), pos(2), &slot(3, 1)).is_none()
    );
}

#[test]
fn reference_boundary_is_preserved_when_a_reborrow_writes_the_output() {
    let mut catalog = types();
    catalog.push(StructuralTypeDeclaration {
        id: ty(3),
        identity: "reference".into(),
        shape: StructuralTypeShape::Reference {
            referent: ty(2),
            access: StructuralAccess::MutableBorrow,
        },
    });
    let mut graph = new_graph(1);
    graph.places.push(DescriptorPlace {
        place: place(3),
        definition: DescriptorDefinition::Local,
    });
    graph.place_types.insert(place(3), ty(3));
    graph.flow.insert(pos(0), transfer(whole(2), whole(3), 1));
    let mut destination = slot(3, 1);
    destination.path.insert(0, DescriptorPathSegment::Referent);
    graph
        .flow
        .insert(pos(1), transfer(whole(1), destination, 2));
    graph.flow.insert(pos(2), DescriptorFlowStep::default());
    graph.normal_exits.push(pos(2));
    let effects = reconstruct_stored_effects(&catalog, &[graph], machine(1)).unwrap();
    assert_eq!(effects.len(), 1);
    assert_eq!(effects[0].path, slot(2, 1).path);
}

fn returner(identity: u64) -> DescriptorMachine {
    let mut graph = new_graph(identity);
    graph.flow.insert(pos(0), DescriptorFlowStep::default());
    graph.normal_exits.push(pos(0));
    graph.returns = Some(vec![(pos(0), whole(1))]);
    graph
}
fn forwarding_call(identity: u64, callee: u64) -> DescriptorMachine {
    let mut graph = new_graph(identity);
    graph.places.push(DescriptorPlace {
        place: place(3),
        definition: DescriptorDefinition::Local,
    });
    graph.place_types.insert(place(3), ty(1));
    graph.flow.insert(
        pos(0),
        DescriptorFlowStep {
            definitions: vec![whole(3)],
            successors: vec![edge(1)],
            ..Default::default()
        },
    );
    graph.calls.insert(
        pos(0),
        DescriptorInvocation {
            callee: machine(callee),
            arguments: vec![Some(whole(1)), Some(whole(2))],
            result: Some(whole(3)),
        },
    );
    graph.flow.insert(pos(1), DescriptorFlowStep::default());
    graph.normal_exits.push(pos(1));
    graph.returns = Some(vec![(pos(1), whole(3))]);
    graph
}

#[test]
fn nested_descriptor_returns_substitute_original_actual_backing() {
    let graphs = [returner(1), forwarding_call(2, 1), forwarding_call(3, 2)];
    let catalog = types();
    let mut analysis = DescriptorAnalysis::new(&catalog, &graphs);
    let origins = analysis.call_origins(machine(3), place(3)).unwrap();
    assert!(
        matches!(origins.as_slice(), [DescriptorCallOrigin::Argument { source, projection }] if *source == place(1) && projection.path.is_empty())
    );
    let result = reconstruct_result_origins(&catalog, &graphs, machine(2)).unwrap();
    assert!(matches!(
        result.as_slice(),
        [DescriptorResultOrigin::Parameter { position: 0, .. }]
    ));
}

#[test]
fn returning_local_backing_rejects_even_through_a_valid_forwarder() {
    let mut caller = forwarding_call(2, 1);
    caller.places.push(DescriptorPlace {
        place: place(4),
        definition: DescriptorDefinition::Local,
    });
    caller.place_types.insert(place(4), ty(1));
    caller.calls.get_mut(&pos(0)).unwrap().arguments[0] = Some(whole(4));
    caller.entry = pos(2);
    caller.flow.insert(
        pos(2),
        DescriptorFlowStep {
            definitions: vec![whole(4)],
            successors: vec![edge(0)],
            ..Default::default()
        },
    );
    caller.originations.insert(
        pos(2),
        vec![(
            whole(4),
            DescriptorCallOrigin::Argument {
                source: place(4),
                projection: Projection::new(&[]),
            },
        )],
    );
    let graphs = [returner(1), caller];
    let catalog = types();
    assert!(
        matches!(reconstruct_call_origins(&catalog,&graphs,machine(2),place(3)).unwrap().as_slice(),[DescriptorCallOrigin::Argument{source,..}] if *source == place(4))
    );
    assert!(reconstruct_result_origins(&catalog, &graphs, machine(2)).is_none());
    let mut owned = returner(3);
    owned.places[0].definition = DescriptorDefinition::Parameter {
        position: 0,
        access: StructuralAccess::Owned,
        structural_type: ty(3),
    };
    owned.place_types.insert(place(1), ty(3));
    let mut catalog = types();
    catalog.push(StructuralTypeDeclaration {
        id: ty(3),
        identity: "owned bytes".into(),
        shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned { capacity: 8 }),
    });
    assert!(reconstruct_result_origins(&catalog, &[owned], machine(3)).is_none());
}

#[test]
fn descriptor_custody_and_static_backing_outlive_their_carrier() {
    let mut owned = returner(1);
    owned.places[0].definition = DescriptorDefinition::Parameter {
        position: 0,
        access: StructuralAccess::Owned,
        structural_type: ty(1),
    };
    assert!(reconstruct_result_origins(&types(), &[owned], machine(1)).is_some());
    let mut literal = returner(2);
    literal.places.push(DescriptorPlace {
        place: place(4),
        definition: DescriptorDefinition::Literal,
    });
    literal.place_types.insert(place(4), ty(1));
    literal.flow.insert(
        pos(0),
        DescriptorFlowStep {
            definitions: vec![whole(4)],
            successors: vec![edge(1)],
            ..Default::default()
        },
    );
    literal.originations.insert(
        pos(0),
        vec![(
            whole(4),
            DescriptorCallOrigin::Literal {
                machine: machine(2),
                place: place(4),
            },
        )],
    );
    literal.flow.insert(pos(1), DescriptorFlowStep::default());
    literal.returns = Some(vec![(pos(1), whole(4))]);
    literal.normal_exits = vec![pos(1)];
    let graphs = [literal, forwarding_call(3, 2)];
    let result = reconstruct_result_origins(&types(), &graphs, machine(3)).unwrap();
    assert!(
        matches!(result.as_slice(),[DescriptorResultOrigin::Literal{machine:identity,place:source}] if *identity == machine(2) && *source == place(4))
    );
}

#[test]
fn substituted_sources_and_recursive_claims_cannot_invent_provenance() {
    let mut callee = returner(1);
    callee.places.push(DescriptorPlace {
        place: place(4),
        definition: DescriptorDefinition::Parameter {
            position: 2,
            access: StructuralAccess::SharedBorrow,
            structural_type: ty(1),
        },
    });
    callee.place_types.insert(place(4), ty(1));
    callee.returns = Some(vec![(pos(0), whole(4))]);
    let mut caller = forwarding_call(2, 1);
    caller.places.push(DescriptorPlace {
        place: place(5),
        definition: DescriptorDefinition::Parameter {
            position: 2,
            access: StructuralAccess::SharedBorrow,
            structural_type: ty(1),
        },
    });
    caller.place_types.insert(place(5), ty(1));
    caller
        .calls
        .get_mut(&pos(0))
        .unwrap()
        .arguments
        .push(Some(whole(5)));
    let mut graphs = [callee, caller];
    let catalog = types();
    let origins = reconstruct_call_origins(&catalog, &graphs, machine(2), place(3)).unwrap();
    assert!(
        matches!(origins.as_slice(),[DescriptorCallOrigin::Argument{source,..}] if *source == place(5))
    );
    graphs[1].calls.get_mut(&pos(0)).unwrap().arguments[2] = Some(whole(99));
    assert!(reconstruct_call_origins(&catalog, &graphs, machine(2), place(3)).is_none());
    assert!(reconstruct_result_origins(&catalog, &[forwarding_call(3, 3)], machine(3)).is_none());
}

#[test]
fn symbolic_incoming_backing_survives_slot_replacement_and_copy() {
    let mut graph = new_graph(1);
    graph.places.push(DescriptorPlace {
        place: place(3),
        definition: DescriptorDefinition::Local,
    });
    graph.place_types.insert(place(3), ty(1));
    graph.flow.insert(pos(0), transfer(slot(2, 1), whole(3), 1));
    graph.flow.insert(pos(1), transfer(whole(1), slot(2, 1), 2));
    graph.flow.insert(pos(2), DescriptorFlowStep::default());
    let graphs = [graph];
    let catalog = types();
    let mut analysis = DescriptorAnalysis::new(&catalog, &graphs);
    assert_eq!(
        analysis.symbolic_source_origins(machine(1), pos(2), &whole(3)),
        Some(vec![DescriptorSourceOrigin::IncomingSlot(slot(2, 1))])
    );
    assert_eq!(
        analysis.symbolic_source_origins(machine(1), pos(2), &slot(2, 2)),
        Some(vec![DescriptorSourceOrigin::IncomingSlot(slot(2, 2))])
    );
    assert_eq!(
        analysis.symbolic_source_origins(machine(1), pos(1), &slot(2, 1)),
        Some(vec![DescriptorSourceOrigin::IncomingSlot(slot(2, 1))])
    );
    let installed = DescriptorSourceOrigin::Backing(DescriptorCallOrigin::Argument {
        source: place(1),
        projection: Projection::new(&[]),
    });
    assert_eq!(
        analysis.symbolic_source_origins(machine(1), pos(2), &slot(2, 1)),
        Some(vec![installed])
    );
    let symbol = DescriptorSourceOrigin::IncomingSlot(slot(2, 1));
    assert_eq!(
        analysis.source_origins(machine(1), pos(2), &whole(3)),
        Some(vec![
            analysis.flatten_source_origin(machine(1), &symbol).unwrap()
        ])
    );
}

fn compatibility_callee() -> DescriptorMachine {
    let mut graph = new_graph(1);
    graph.flow.insert(pos(0), DescriptorFlowStep::default());
    graph.normal_exits.push(pos(0));
    graph
}
fn compatibility_caller(origin: Option<DescriptorCallOrigin>) -> DescriptorMachine {
    let mut graph = new_graph(2);
    graph.flow.insert(
        pos(0),
        DescriptorFlowStep {
            definitions: vec![slot(2, 1)],
            successors: vec![edge(1)],
            ..Default::default()
        },
    );
    if let Some(origin) = origin {
        graph
            .originations
            .insert(pos(0), vec![(slot(2, 1), origin)]);
    }
    graph.flow.insert(
        pos(1),
        DescriptorFlowStep {
            successors: vec![edge(2)],
            ..Default::default()
        },
    );
    graph.flow.insert(pos(2), DescriptorFlowStep::default());
    graph.calls.insert(
        pos(1),
        DescriptorInvocation {
            callee: machine(1),
            arguments: vec![Some(whole(1)), Some(whole(2))],
            result: None,
        },
    );
    graph
}
fn compatibility_requirement() -> DescriptorCompatibilityRequirement {
    DescriptorCompatibilityRequirement {
        position: pos(0),
        loan: DescriptorSourceOrigin::IncomingSlot(slot(2, 1)),
        demand: DescriptorCallOrigin::Argument {
            source: place(2),
            projection: Projection::new(&[StructuralPathSegment::Field("field1".into())]),
        },
    }
}

#[test]
fn call_substitution_loads_captured_contents_but_addresses_destination_storage() {
    let graphs = [
        compatibility_callee(),
        compatibility_caller(Some(DescriptorCallOrigin::Argument {
            source: place(1),
            projection: Projection::new(&[]),
        })),
    ];
    let catalog = types();
    let mut analysis = DescriptorAnalysis::new(&catalog, &graphs);
    let requirement = compatibility_requirement();
    assert_eq!(
        analysis.substitute_call_source(machine(2), pos(1), &requirement.loan),
        Some(vec![DescriptorSourceOrigin::Backing(
            DescriptorCallOrigin::Argument {
                source: place(1),
                projection: Projection::new(&[]),
            }
        )])
    );
    assert_eq!(
        analysis.substitute_call_storage(machine(2), pos(1), &requirement.demand),
        Some(requirement.demand)
    );
}

#[test]
fn compatibility_requires_current_caller_backing_and_rejects_overlap_or_unknown() {
    let catalog = types();
    let requirements = BTreeMap::from([(machine(1), vec![compatibility_requirement()])]);
    for (origin, accepted) in [
        (
            Some(DescriptorCallOrigin::Argument {
                source: place(1),
                projection: Projection::new(&[]),
            }),
            true,
        ),
        (
            Some(DescriptorCallOrigin::Argument {
                source: place(2),
                projection: Projection::new(&[]),
            }),
            false,
        ),
        (Some(DescriptorCallOrigin::Empty), true),
        (None, false),
    ] {
        let graphs = [compatibility_callee(), compatibility_caller(origin)];
        let result =
            DescriptorAnalysis::new(&catalog, &graphs).compatibility_requirements(&requirements);
        assert_eq!(result.is_ok(), accepted);
        if let Ok(residuals) = result {
            // Internal discharge never silently admits the generic entry itself.
            assert_eq!(residuals[&machine(1)], requirements[&machine(1)]);
            assert!(!residuals.contains_key(&machine(2)));
        } else {
            assert_eq!(
                result.unwrap_err(),
                DescriptorCompatibilityFailure {
                    machine: machine(2),
                    position: pos(1)
                }
            );
        }
    }
}

#[test]
fn compatibility_propagates_exact_incoming_symbols_and_rejects_forged_demands() {
    let catalog = types();
    let mut caller = compatibility_caller(None);
    caller.flow.get_mut(&pos(0)).unwrap().definitions.clear();
    let graphs = [compatibility_callee(), caller];
    let requirement = compatibility_requirement();
    let mut analysis = DescriptorAnalysis::new(&catalog, &graphs);
    let requirements = BTreeMap::from([(machine(1), vec![requirement.clone()])]);
    let residuals = analysis.compatibility_requirements(&requirements).unwrap();
    assert_eq!(
        residuals[&machine(2)],
        vec![DescriptorCompatibilityRequirement {
            position: pos(1),
            ..requirement.clone()
        }]
    );
    let mut forged = requirement.clone();
    forged.demand = DescriptorCallOrigin::Argument {
        source: place(1),
        projection: Projection::new(&[]),
    };
    assert!(
        analysis
            .compatibility_requirements(&BTreeMap::from([(machine(1), vec![forged])]))
            .is_err()
    );
    let mut forged = requirement;
    forged.loan = DescriptorSourceOrigin::IncomingSlot(slot(2, 999));
    assert!(
        analysis
            .compatibility_requirements(&BTreeMap::from([(machine(1), vec![forged])]))
            .is_err()
    );
}

#[test]
fn symbolic_call_writeback_retains_sibling_incoming_identity() {
    let mut callee = compatibility_callee();
    callee
        .flow
        .insert(pos(0), transfer(slot(2, 2), slot(2, 1), 1));
    callee.flow.insert(pos(1), DescriptorFlowStep::default());
    callee.normal_exits = vec![pos(1)];
    let mut caller = compatibility_caller(None);
    caller.flow.get_mut(&pos(0)).unwrap().definitions.clear();
    let graphs = [callee, caller];
    let catalog = types();
    let mut analysis = DescriptorAnalysis::new(&catalog, &graphs);
    let effects = analysis
        .symbolic_call_stored_effects(machine(2), pos(1))
        .unwrap();
    assert_eq!(effects.len(), 1);
    assert_eq!(effects[0].destination, slot(2, 1));
    assert_eq!(
        effects[0].origins,
        vec![DescriptorSourceOrigin::IncomingSlot(slot(2, 2))]
    );
    assert!(!effects[0].preserves_incoming);
    assert_eq!(
        analysis.symbolic_source_origins(machine(2), pos(2), &slot(2, 1)),
        Some(vec![DescriptorSourceOrigin::IncomingSlot(slot(2, 2))])
    );
    let flattened = analysis.call_stored_effects(machine(2), pos(1)).unwrap();
    assert_eq!(
        flattened[0].origins,
        vec![
            analysis
                .flatten_source_origin(machine(2), &effects[0].origins[0])
                .unwrap()
        ]
    );
}

#[test]
fn recast_carrier_keeps_same_backing_path_in_call_compatibility() {
    let mut catalog = types();
    catalog.push(StructuralTypeDeclaration {
        id: ty(3),
        identity: "retyped_shared_view".into(),
        shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
            access: Some(StructuralAccess::SharedBorrow),
        }),
    });
    let requirement = compatibility_requirement();
    let mut caller = new_graph(2);
    caller.places.push(DescriptorPlace {
        place: place(3),
        definition: DescriptorDefinition::Local,
    });
    caller.place_types.insert(place(3), ty(3));
    caller.flow.insert(
        pos(0),
        DescriptorFlowStep {
            definitions: vec![whole(3)],
            successors: vec![edge(1)],
            ..Default::default()
        },
    );
    // The adapter retains the original address when retyping a descriptor.
    // A distinct carrier type supplies no disjointness from the exact same slot.
    caller
        .originations
        .insert(pos(0), vec![(whole(3), requirement.demand.clone())]);
    caller
        .flow
        .insert(pos(1), transfer(whole(3), slot(2, 1), 2));
    caller.flow.insert(
        pos(2),
        DescriptorFlowStep {
            successors: vec![edge(3)],
            ..Default::default()
        },
    );
    caller.flow.insert(pos(3), DescriptorFlowStep::default());
    caller.calls.insert(
        pos(2),
        DescriptorInvocation {
            callee: machine(1),
            arguments: vec![Some(whole(1)), Some(whole(2))],
            result: None,
        },
    );
    let graphs = [compatibility_callee(), caller];
    let mut analysis = DescriptorAnalysis::new(&catalog, &graphs);
    assert_eq!(
        analysis.source_origins(machine(2), pos(2), &slot(2, 1)),
        Some(vec![requirement.demand.clone()])
    );
    assert_eq!(
        analysis.compatibility_requirements(&BTreeMap::from([(machine(1), vec![requirement])])),
        Err(DescriptorCompatibilityFailure {
            machine: machine(2),
            position: pos(2)
        })
    );
}

#[test]
fn compatibility_keeps_inexact_actuals_in_the_call_roster() {
    let catalog = types();
    let requirements = BTreeMap::from([(machine(1), vec![compatibility_requirement()])]);
    for (unknown_actual, accepted) in [(0, true), (1, false)] {
        let mut caller = compatibility_caller(Some(DescriptorCallOrigin::Argument {
            source: place(1),
            projection: Projection::new(&[]),
        }));
        // Runtime-selected actual coordinates remain unknown at their formal
        // position. They neither remove the call nor shift later actuals.
        caller.calls.get_mut(&pos(1)).unwrap().arguments[unknown_actual] = None;
        let graphs = [compatibility_callee(), caller];
        let mut analysis = DescriptorAnalysis::new(&catalog, &graphs);
        let result = analysis.compatibility_requirements(&requirements);
        assert_eq!(result.is_ok(), accepted);
        if !accepted {
            assert_eq!(
                result.unwrap_err(),
                DescriptorCompatibilityFailure {
                    machine: machine(2),
                    position: pos(1),
                }
            );
        }
        // Whole output reconstruction remains conservative, even when this
        // particular compatibility pair needs only exact actual number one.
        assert!(analysis.call_stored_effects(machine(2), pos(1)).is_none());
        // No compatibility obligation is invented for an unrelated invocation.
        assert!(
            analysis
                .compatibility_requirements(&BTreeMap::new())
                .unwrap()
                .is_empty()
        );
    }
}

#[test]
fn bounded_byte_reference_demand_replays_actual_array_backing() {
    let mut catalog = types();
    catalog.extend([
        StructuralTypeDeclaration {
            id: ty(3),
            identity: "bounded-byte-reference".into(),
            shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedView {
                access: None,
                capacity: 4,
            }),
        },
        StructuralTypeDeclaration {
            id: ty(4),
            identity: "byte-array".into(),
            shape: StructuralTypeShape::FixedArray {
                element: ty(5),
                length: 4,
            },
        },
        StructuralTypeDeclaration {
            id: ty(5),
            identity: "byte".into(),
            shape: StructuralTypeShape::PrimitiveScalar(semantic_vocabulary::ScalarType::Integer(
                semantic_vocabulary::IntegerType::new(
                    semantic_vocabulary::IntegerSign::Unsigned,
                    8,
                )
                .unwrap(),
            )),
        },
    ]);
    let mut callee = compatibility_callee();
    callee.places.push(DescriptorPlace {
        place: place(3),
        definition: DescriptorDefinition::Parameter {
            position: 2,
            access: StructuralAccess::MutableBorrow,
            structural_type: ty(3),
        },
    });
    callee.place_types.insert(place(3), ty(3));
    let mut caller = compatibility_caller(Some(DescriptorCallOrigin::Argument {
        source: place(4),
        projection: Projection::new(&[]),
    }));
    for (root, position, access) in [
        (3, 2, StructuralAccess::MutableBorrow),
        (4, 3, StructuralAccess::SharedBorrow),
    ] {
        caller.places.push(DescriptorPlace {
            place: place(root),
            definition: DescriptorDefinition::Parameter {
                position,
                access,
                structural_type: ty(4),
            },
        });
        caller.place_types.insert(place(root), ty(4));
    }
    caller
        .calls
        .get_mut(&pos(1))
        .unwrap()
        .arguments
        .push(Some(whole(3)));
    let requirement = DescriptorCompatibilityRequirement {
        position: pos(0),
        loan: DescriptorSourceOrigin::IncomingSlot(slot(2, 1)),
        demand: DescriptorCallOrigin::Argument {
            source: place(3),
            projection: Projection::new(&[StructuralPathSegment::FixedIndex(0)]),
        },
    };
    let mut graphs = [callee, caller];
    let requirements = BTreeMap::from([(machine(1), vec![requirement.clone()])]);
    assert!(
        DescriptorAnalysis::new(&catalog, &graphs)
            .compatibility_requirements(&requirements)
            .is_ok()
    );
    graphs[1].calls.get_mut(&pos(1)).unwrap().arguments[2] = Some(whole(4));
    assert_eq!(
        DescriptorAnalysis::new(&catalog, &graphs).compatibility_requirements(&requirements),
        Err(DescriptorCompatibilityFailure {
            machine: machine(2),
            position: pos(1)
        })
    );
    // A retyped payload alias is not fresh storage at its carrier home.
    graphs[1].places.push(DescriptorPlace {
        place: place(5),
        definition: DescriptorDefinition::Local,
    });
    graphs[1].place_types.insert(place(5), ty(3));
    graphs[1]
        .flow
        .get_mut(&pos(0))
        .unwrap()
        .definitions
        .push(whole(5));
    graphs[1].originations.get_mut(&pos(0)).unwrap().push((
        whole(5),
        DescriptorCallOrigin::Argument {
            source: place(4),
            projection: Projection::new(&[]),
        },
    ));
    graphs[1].calls.get_mut(&pos(1)).unwrap().arguments[2] = Some(whole(5));
    assert_eq!(
        DescriptorAnalysis::new(&catalog, &graphs).compatibility_requirements(&requirements),
        Err(DescriptorCompatibilityFailure {
            machine: machine(2),
            position: pos(1)
        })
    );
    let mut outside = requirement;
    outside.demand = DescriptorCallOrigin::Argument {
        source: place(3),
        projection: Projection::new(&[StructuralPathSegment::FixedIndex(4)]),
    };
    assert_eq!(
        DescriptorAnalysis::new(&catalog, &graphs)
            .compatibility_requirements(&BTreeMap::from([(machine(1), vec![outside])])),
        Err(DescriptorCompatibilityFailure {
            machine: machine(1),
            position: pos(0)
        })
    );
}

fn aggregate_returner() -> DescriptorMachine {
    let mut graph = new_graph(1);
    graph.places.push(DescriptorPlace {
        place: place(3),
        definition: DescriptorDefinition::Local,
    });
    graph.place_types.insert(place(3), ty(2));
    graph.flow.insert(
        pos(0),
        DescriptorFlowStep {
            definitions: vec![whole(3)],
            derivations: vec![(whole(1), slot(3, 1)), (slot(2, 2), slot(3, 2))],
            successors: vec![edge(1)],
            ..Default::default()
        },
    );
    graph.originations.insert(
        pos(0),
        vec![(
            whole(3),
            DescriptorCallOrigin::Argument {
                source: place(3),
                projection: Projection::new(&[]),
            },
        )],
    );
    graph.flow.insert(pos(1), DescriptorFlowStep::default());
    graph.normal_exits.push(pos(1));
    graph.returns = Some(vec![(pos(1), whole(3))]);
    graph
}

fn aggregate_caller() -> DescriptorMachine {
    let mut graph = new_graph(2);
    for (identity, structural_type) in [(3, ty(2)), (4, ty(1)), (5, ty(1))] {
        graph.places.push(DescriptorPlace {
            place: place(identity),
            definition: DescriptorDefinition::Local,
        });
        graph.place_types.insert(place(identity), structural_type);
    }
    graph.flow.insert(
        pos(0),
        DescriptorFlowStep {
            successors: vec![edge(1)],
            ..Default::default()
        },
    );
    graph.flow.insert(
        pos(1),
        DescriptorFlowStep {
            definitions: vec![whole(3)],
            successors: vec![edge(2)],
            ..Default::default()
        },
    );
    graph.originations.insert(
        pos(1),
        vec![(
            whole(3),
            DescriptorCallOrigin::Argument {
                source: place(3),
                projection: Projection::new(&[]),
            },
        )],
    );
    graph.calls.insert(
        pos(1),
        DescriptorInvocation {
            callee: machine(1),
            arguments: vec![Some(whole(1)), Some(whole(2))],
            result: Some(whole(3)),
        },
    );
    graph.flow.insert(pos(2), transfer(slot(3, 1), whole(4), 3));
    graph.flow.insert(
        pos(3),
        DescriptorFlowStep {
            successors: vec![DescriptorFlowEdge {
                target: pos(4),
                parameters: vec![whole(5)],
                bindings: vec![(whole(4), whole(5))],
            }],
            ..Default::default()
        },
    );
    graph.flow.insert(
        pos(4),
        DescriptorFlowStep {
            inputs: vec![whole(5)],
            ..Default::default()
        },
    );
    graph.normal_exits.push(pos(4));
    graph
}

#[test]
fn returned_aggregate_slots_preserve_backing_and_distinct_carrier_storage() {
    use crate::descriptor_lifetimes::descriptor_location_live_positions;
    let catalog = types();
    let graphs = [aggregate_returner(), aggregate_caller()];
    let mut analysis = DescriptorAnalysis::new(&catalog, &graphs);
    assert_eq!(
        analysis.source_origins(machine(2), pos(4), &whole(5)),
        Some(vec![DescriptorCallOrigin::Argument {
            source: place(1),
            projection: Projection::new(&[])
        }])
    );
    assert_eq!(
        analysis.source_origins(machine(2), pos(2), &whole(3)),
        Some(vec![DescriptorCallOrigin::Argument {
            source: place(3),
            projection: Projection::new(&[])
        }])
    );
    assert_eq!(
        analysis.symbolic_source_origins(machine(2), pos(2), &slot(3, 2)),
        Some(vec![DescriptorSourceOrigin::IncomingSlot(slot(2, 2))])
    );
    // An aggregate result is not itself the borrowed backing. Its field's
    // lineage nevertheless extends the original loan through the successor.
    assert!(analysis.call_origins(machine(2), place(3)).is_none());
    let flow = analysis.flow(machine(2)).unwrap();
    assert!(flow[&pos(1)].derivations.contains(&(whole(1), slot(3, 1))));
    assert!(!flow[&pos(1)].derivations.contains(&(whole(1), whole(3))));
    assert!(descriptor_location_live_positions(&flow, pos(0), &whole(1)).contains(&pos(4)));
}

#[test]
fn returned_aggregate_slots_reject_unknown_contents_and_local_backing() {
    let catalog = types();
    for replace_with_local in [false, true] {
        let mut callee = aggregate_returner();
        if replace_with_local {
            callee.places.push(DescriptorPlace {
                place: place(4),
                definition: DescriptorDefinition::Local,
            });
            callee.place_types.insert(place(4), ty(1));
            callee.entry = pos(2);
            callee.flow.insert(
                pos(2),
                DescriptorFlowStep {
                    definitions: vec![whole(4)],
                    successors: vec![edge(0)],
                    ..Default::default()
                },
            );
            callee.originations.insert(
                pos(2),
                vec![(
                    whole(4),
                    DescriptorCallOrigin::Argument {
                        source: place(4),
                        projection: Projection::new(&[]),
                    },
                )],
            );
            callee.flow.get_mut(&pos(0)).unwrap().derivations[0].0 = whole(4);
        } else {
            callee.flow.get_mut(&pos(0)).unwrap().derivations.remove(0);
        }
        let graphs = [callee, aggregate_caller()];
        let mut analysis = DescriptorAnalysis::new(&catalog, &graphs);
        assert!(
            analysis
                .source_origins(machine(2), pos(4), &whole(5))
                .is_none()
        );
        assert!(analysis.flow(machine(2)).is_none());
    }
}

#[test]
fn returned_aggregate_slots_do_not_exchange_sibling_backing() {
    let catalog = types();
    let mut callee = aggregate_returner();
    let fields = &mut callee.flow.get_mut(&pos(0)).unwrap().derivations;
    fields[0].1 = slot(3, 2);
    fields[1].1 = slot(3, 1);
    let graphs = [callee, aggregate_caller()];
    let mut analysis = DescriptorAnalysis::new(&catalog, &graphs);
    assert_eq!(
        analysis.symbolic_source_origins(machine(2), pos(4), &whole(5)),
        Some(vec![DescriptorSourceOrigin::IncomingSlot(slot(2, 2))])
    );
}

fn terminal_parameter(
    access: StructuralAccess,
    structural_type: StructuralTypeId,
) -> StructuralParameterDeclaration {
    StructuralParameterDeclaration {
        place: place(1),
        position: 0,
        is_self: false,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }
}
fn terminal_body(
    identity: u64,
    access: StructuralAccess,
    structural_type: StructuralTypeId,
) -> TerminalMachine {
    let entry = BlockId::new(1).unwrap();
    TerminalMachine {
        id: machine(identity),
        attachment: None,
        parameters: Vec::new(),
        structural_parameters: vec![terminal_parameter(access, structural_type)],
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: vec![StructuralPlaceDeclaration {
            id: place(1),
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        }],
        entry_claims: Vec::new(),
        declared_service_reach: Vec::new(),
        closed_reach_application: None,
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry,
        blocks: vec![Block {
            id: entry,
            parameters: Vec::new(),
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            structural_parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::ReturnUnit {
                edge: EdgeId::new(1).unwrap(),
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: MachineContract {
            id: ContractId::new(identity).unwrap(),
            crash_routes: Vec::new(),
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    }
}

#[test]
fn descriptor_free_crash_result_does_not_poison_unrelated_slot_replay() {
    let mut catalog = types();
    catalog.push(StructuralTypeDeclaration {
        id: ty(3),
        identity: "empty result".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    });
    let mut callee = terminal_body(1, StructuralAccess::SharedBorrow, ty(1));
    callee.result = TerminalMachineResult::Structural(StructuralResultDeclaration {
        place: place(9),
        structural_type: ty(3),
        multiplicity: StructuralMultiplicity::Affine,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        reference_sources: Vec::new(),
    });
    callee.structural_places.push(StructuralPlaceDeclaration {
        id: place(9),
        kind: StructuralPlaceKind::Result,
    });
    callee.blocks[0].terminator = Terminator::Crash {
        edge: EdgeId::new(1).unwrap(),
        cause: terminal_psi::CrashCause::Abort,
        site_guard: Vec::new(),
        frontier_lower_bound: Vec::new(),
    };
    let mut module = TerminalModule::for_entry(machine(1));
    module.structural_types = catalog;
    module.machines.push(callee);
    let mut graphs = terminal_graph(&module);
    assert_eq!(graphs.len(), 1);
    assert!(graphs[0].returns.is_none());
    assert!(graphs[0].normal_exits.is_empty());
    let mut caller = new_graph(2);
    caller.flow.insert(
        pos(0),
        DescriptorFlowStep {
            successors: vec![edge(1)],
            ..Default::default()
        },
    );
    caller.flow.insert(pos(1), DescriptorFlowStep::default());
    caller.calls.insert(
        pos(0),
        DescriptorInvocation {
            callee: machine(1),
            arguments: vec![Some(whole(1))],
            result: None,
        },
    );
    graphs.push(caller);
    let mut analysis = DescriptorAnalysis::new(&module.structural_types, &graphs);
    // No result backing is invented, and the absence of any normal return
    // does not erase the caller's independently known descriptor contents.
    assert!(analysis.flow(machine(2)).is_some());
    assert_eq!(
        analysis.symbolic_source_origins(machine(2), pos(1), &slot(2, 1)),
        Some(vec![DescriptorSourceOrigin::IncomingSlot(slot(2, 1))])
    );

    let TerminalMachineResult::Structural(result) = &mut module.machines[0].result else {
        unreachable!()
    };
    result.structural_type = ty(2);
    let graphs = terminal_graph(&module);
    assert!(graphs[0].returns.as_ref().unwrap().is_empty());
    // A real descriptor-bearing result with no return remains unavailable;
    // it is not treated as an empty descriptor or as a descriptor-free result.
    assert!(reconstruct_stored_effects(&module.structural_types, &graphs, machine(1)).is_none());
}

#[test]
fn returned_case_slots_are_empty_only_for_a_declared_inactive_payload() {
    use semantic_vocabulary::{OperationId, StructuralCaseId};
    use terminal_psi::{
        OperationResult, RecordFieldInitializer, RecordFieldValue, StructuralCaseDeclaration,
        StructuralOperationResult,
    };
    let sound = StructuralCaseId::new(1).unwrap();
    let empty = StructuralCaseId::new(2).unwrap();
    let mut module = TerminalModule::for_entry(machine(1));
    module.structural_types = types();
    let StructuralTypeShape::Record { fields } = &module.structural_types[1].shape else {
        unreachable!()
    };
    let payload = fields[0].clone();
    module.structural_types.push(StructuralTypeDeclaration {
        id: ty(3),
        identity: "result cases".into(),
        shape: StructuralTypeShape::Sum {
            cases: vec![
                StructuralCaseDeclaration {
                    id: sound,
                    identity: "sound".into(),
                    fields: vec![payload.clone()],
                },
                StructuralCaseDeclaration {
                    id: empty,
                    identity: "empty".into(),
                    fields: Vec::new(),
                },
            ],
        },
    });
    let mut callee = terminal_body(1, StructuralAccess::SharedBorrow, ty(1));
    callee.result = TerminalMachineResult::Structural(StructuralResultDeclaration {
        place: place(9),
        structural_type: ty(3),
        multiplicity: StructuralMultiplicity::Affine,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        reference_sources: Vec::new(),
    });
    callee.structural_places.extend([
        StructuralPlaceDeclaration {
            id: place(9),
            kind: StructuralPlaceKind::Result,
        },
        StructuralPlaceDeclaration {
            id: place(3),
            kind: StructuralPlaceKind::OperationResult {
                producer: OperationId::new(1).unwrap(),
                structural_type: ty(3),
            },
        },
    ]);
    callee.blocks[0].operations.push(Operation {
        id: OperationId::new(1).unwrap(),
        static_reach_binding: None,
        suspension_crossing: None,
        result: OperationResult::Structural(StructuralOperationResult {
            place: place(3),
            structural_type: ty(3),
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            qualification_establishments: Vec::new(),
            claims: Vec::new(),
        }),
        kind: OperationKind::EstablishStructuralCase {
            result_case: sound,
            fields: vec![RecordFieldInitializer {
                field: payload.id,
                value: RecordFieldValue::Structural(StructuralArgument {
                    place: place(1),
                    path: Vec::new(),
                    access: StructuralAccess::SharedBorrow,
                }),
            }],
        },
    });
    callee.blocks[0].terminator = Terminator::ReturnStructural {
        edge: EdgeId::new(1).unwrap(),
        source: place(3),
        returned_claims: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    module.machines.push(callee);
    let location = DescriptorLocation {
        root: place(3),
        path: vec![
            DescriptorPathSegment::Case(sound),
            DescriptorPathSegment::Field(payload.id),
        ],
    };
    let graphs = terminal_graph(&module);
    let mut analysis = DescriptorAnalysis::new(&module.structural_types, &graphs);
    assert_eq!(
        analysis.source_origins(machine(1), pos(1), &location),
        Some(vec![DescriptorCallOrigin::Argument {
            source: place(1),
            projection: Projection::new(&[])
        }])
    );
    assert!(analysis.stored_effects(machine(1)).is_some());

    let OperationKind::EstablishStructuralCase { fields, .. } =
        &mut module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    fields.clear();
    let graphs = terminal_graph(&module);
    assert!(reconstruct_stored_effects(&module.structural_types, &graphs, machine(1)).is_none());

    let OperationKind::EstablishStructuralCase { result_case, .. } =
        &mut module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    *result_case = empty;
    let graphs = terminal_graph(&module);
    let mut analysis = DescriptorAnalysis::new(&module.structural_types, &graphs);
    assert_eq!(
        analysis.source_origins(machine(1), pos(1), &location),
        Some(vec![DescriptorCallOrigin::Empty])
    );
    assert!(analysis.stored_effects(machine(1)).is_some());

    let OperationKind::EstablishStructuralCase { result_case, .. } =
        &mut module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    *result_case = StructuralCaseId::new(99).unwrap();
    assert!(terminal_graph(&module).is_empty());
}
