use super::*;
use crate::descriptors::descriptor_lifetimes::{
    DescriptorFlowBinding, DescriptorFlowEdge,
};
use semantic_vocabulary::BlockId;

fn position(value: u64) -> DescriptorPosition {
    (BlockId::new(value).unwrap(), 0)
}

fn payload() -> DescriptorLocation {
    DescriptorLocation::whole(semantic_vocabulary::PlaceId::new(1).unwrap())
}

fn joins(edge: &DescriptorFlowEdge, location: &DescriptorLocation) -> bool {
    edge.parameters
        .iter()
        .any(|parameter| parameter.contains(location) || location.contains(parameter))
}

fn copy_graph() -> (Vec<StructuralTypeDeclaration>, DescriptorMachine) {
    use super::super::{DescriptorDefinition, DescriptorPlace};
    use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType, StructuralTypeId};
    use terminal_psi::{StructuralTypeShape, TerminalIdentityIndex};
    let original = payload();
    let copy = DescriptorLocation::whole(semantic_vocabulary::PlaceId::new(2).unwrap());
    let carrier = StructuralTypeId::new(1).unwrap();
    let types = vec![StructuralTypeDeclaration {
        declaration_identity: Default::default(),
        requires: Vec::new(),
        quotient_carrier: None,
        id: carrier,
        identity: TerminalIdentityIndex(1),
        owns_linear_claim: false,
        no_code_affine_cleanup: true,
        multiplicity: terminal_psi::StructuralMultiplicity::Affine,
        descriptor_cleanup: true,
        nominal_cleanup: None,
        shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
            IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        )),
        scalar_restriction: None,
    }];
    let mut flow = flow(&[(1, &[2]), (2, &[3]), (3, &[4]), (4, &[5]), (5, &[])]);
    flow.get_mut(&position(2))
        .unwrap()
        .definitions
        .push(original.clone());
    let copying = flow.get_mut(&position(3)).unwrap();
    copying.definitions.push(copy.clone());
    copying.derivations.push((original.clone(), copy.clone()));
    let machine = DescriptorMachine {
        id: MachineId::new(1).unwrap(),
        returns: None,
        places: [original.root, copy.root]
            .into_iter()
            .map(|place| DescriptorPlace {
                producer: None,
                place,
                definition: DescriptorDefinition::Local,
            })
            .collect(),
        place_types: BTreeMap::from([(original.root, carrier), (copy.root, carrier)]),
        entry: position(1),
        flow,
        case_constructors: BTreeMap::new(),
        case_successors: BTreeMap::new(),
        payload_producers: BTreeMap::from([
            (position(2), original.root),
            (position(3), copy.root),
        ]),
        originations: BTreeMap::new(),
        projections: BTreeMap::new(),
        contents_queries: BTreeMap::new(),
        contents_transfers: BTreeMap::new(),
        payload_entry_predicates: None,
        unsigned_index_literals: Vec::new(),
        payload_scalar_definitions: BTreeMap::new(),
        payload_scalar_postconditions: BTreeMap::new(),
        payload_scalar_parameters: BTreeMap::new(),
        scalar_payload_reads: BTreeMap::new(),
        payload_edge_predicates: BTreeMap::new(),
        payload_scalar_bindings: BTreeMap::new(),
        scalar_payload_writes: BTreeMap::new(),
        payload_invalidations: BTreeMap::new(),
        private_observation_inputs: Default::default(),
        unresolved_contents_writes: Default::default(),
        storage_derivations: Default::default(),
        calls: BTreeMap::new(),
        normal_exits: vec![position(5)],
    };
    (types, machine)
}

#[test]
fn entry_storage_refuses_overlapping_clobbers_without_tainting_other_homes_or_earlier_cuts()
 {
    let (types, mut machine) = copy_graph();
    for step in machine.flow.values_mut() {
        step.definitions.clear();
        step.derivations.clear();
    }
    let original = payload();
    let other =
        DescriptorLocation::whole(semantic_vocabulary::PlaceId::new(2).unwrap());
    let preserved = |machine: &DescriptorMachine| {
        let graph = std::slice::from_ref(machine);
        let mut analysis =
            DescriptorAnalysis::with_borrowed_storage_slots(&types, graph);
        require_unchanged_entry_storage(
            &types,
            graph,
            &mut analysis,
            machine.id,
            position(4),
            &DescriptorPayloadTarget::current(original.clone()),
        )
    };
    let writes = |machine: &DescriptorMachine, at| {
        let mut analysis = DescriptorAnalysis::with_borrowed_storage_slots(
            &types,
            std::slice::from_ref(machine),
        );
        analysis.storage_writes(machine.id, at)
    };
    machine.payload_invalidations.insert(
        position(2),
        vec![Some(super::super::DescriptorContentsQuery::from_location(
            &other,
        ))],
    );
    assert!(preserved(&machine).is_ok());
    assert_eq!(writes(&machine, position(2)).unwrap(), Some(vec![other]));
    machine.payload_invalidations.insert(
        position(2),
        vec![Some(super::super::DescriptorContentsQuery::from_location(
            &original,
        ))],
    );
    assert!(preserved(&machine).is_err());
    assert_eq!(
        writes(&machine, position(2)).unwrap(),
        Some(vec![original.clone()])
    );
    machine
        .payload_invalidations
        .insert(position(2), vec![None]);
    assert!(preserved(&machine).is_err());
    assert_eq!(writes(&machine, position(2)).unwrap(), None);
    machine.payload_invalidations.clear();
    machine
        .payload_invalidations
        .insert(position(5), vec![None]);
    assert!(preserved(&machine).is_ok());
    assert_eq!(writes(&machine, position(2)).unwrap(), Some(Vec::new()));
    assert_eq!(writes(&machine, position(5)).unwrap(), None);
    assert_eq!(
        writes(&machine, position(99)).unwrap_err(),
        "storage write has no operation or edge cut"
    );
}

#[test]
fn scalar_formal_renews_on_arrival_not_between_operations_in_its_block() {
    let (_, mut machine) = copy_graph();
    let entry = position(1);
    let capture = (semantic_vocabulary::BlockId::new(2).unwrap(), 0);
    let middle = (capture.0, 1);
    let observation = (capture.0, 2);
    let parameter = ValueId::new(8).unwrap();
    let step = |target: Option<DescriptorPosition>| DescriptorFlowStep {
        successors: target
            .into_iter()
            .map(|target| DescriptorFlowEdge {
                borrowed_header_ends: Box::default(),
                edge: None,
                effects: Vec::new(),
                definitions: Vec::new(),
                target,
                parameters: Vec::new(),
                bindings: Vec::new(),
            })
            .collect(),
        ..Default::default()
    };
    machine.entry = entry;
    machine.flow = BTreeMap::from([
        (entry, step(Some(capture))),
        (capture, step(Some(middle))),
        (middle, step(Some(observation))),
        (observation, step(None)),
    ]);
    machine.payload_scalar_parameters = BTreeMap::from([(capture.0, vec![parameter])]);
    machine.payload_scalar_definitions.clear();
    let capture_at = |source| {
        require_completed_scalar_capture(
            std::slice::from_ref(&machine),
            machine.id,
            source,
            observation,
            &[parameter],
        )
    };
    capture_at(capture).unwrap();
    assert!(capture_at(entry).is_err());
}

#[test]
fn scalar_capture_refuses_renewed_operands() {
    let (_, mut machine) = copy_graph();
    let status = ValueId::new(8).unwrap();
    machine
        .payload_scalar_definitions
        .insert(position(1), vec![status]);
    let capture = |machine: &DescriptorMachine, source, observation, values: &[ValueId]| {
        require_completed_scalar_capture(
            std::slice::from_ref(machine),
            machine.id,
            source,
            observation,
            values,
        )
    };
    capture(&machine, position(2), position(5), &[status]).unwrap();

    machine
        .payload_scalar_definitions
        .insert(position(4), vec![status]);
    assert!(capture(&machine, position(2), position(5), &[status]).is_err());
    assert!(capture(&machine, position(2), position(2), &[status]).is_err());
    assert!(capture(&machine, position(2), position(5), &[]).is_ok());
}

#[test]
fn entry_version_includes_the_first_operation_write() {
    let (_, machine) = copy_graph();
    let observe = |start, first_writes| {
        version_flow_from(
            &machine.flow,
            machine.entry,
            start,
            position(5),
            |_, _| true,
            |position, _| Ok(first_writes && position == machine.entry),
            |_| false,
        )
        .unwrap()
    };
    assert!(observe(VersionStart::Entry, false));
    assert!(!observe(VersionStart::Entry, true));
    assert!(observe(VersionStart::Completed(machine.entry), true));
}

#[test]
fn opaque_output_identity_needs_unchanged_actual_storage() {
    use semantic_vocabulary::{IntegerSign, IntegerType, ScalarTerm, ScalarType};

    #[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
    struct NativeOutput(DescriptorPosition, u32);

    let (types, mut machine) = copy_graph();
    let identity = machine.id;
    let read = ValueId::new(7).unwrap();
    machine.scalar_payload_reads.insert(
        position(5),
        vec![(
            payload(),
            ScalarTerm::value(
                read,
                ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                ),
            ),
        )],
    );
    let mut graph = vec![machine];
    let observe = |graph: &[DescriptorMachine]| {
        unchanged_stored_sources(&types, graph, identity, read, |writer_position| {
            if writer_position == position(2) {
                vec![(payload(), NativeOutput(writer_position, 3))]
            } else {
                Vec::new()
            }
        })
    };
    assert_eq!(observe(&graph), Some(vec![NativeOutput(position(2), 3)]));
    graph[0].unresolved_contents_writes.insert(position(4));
    assert!(observe(&graph).is_none());
}

#[test]
fn copied_projection_names_its_pre_copy_source_and_keeps_its_own_version() {
    use crate::descriptors::descriptor_lifetimes::DescriptorPathSegment;
    use semantic_vocabulary::StructuralTypeId;
    use terminal_psi::{ArrayExtent, StructuralTypeShape};
    let (mut types, mut machine) = copy_graph();
    let mut element = types[0].clone();
    element.id = StructuralTypeId::new(2).unwrap();
    types[0].shape = StructuralTypeShape::FixedArray {
        element: element.id,
        length: ArrayExtent::Literal(2),
    };
    types.push(element);
    let original = payload();
    let copy = DescriptorLocation::whole(semantic_vocabulary::PlaceId::new(2).unwrap());
    let mut observed = copy.clone();
    observed.path.push(DescriptorPathSegment::FixedIndex(1));
    let mut source_leaf = original.clone();
    source_leaf.path = observed.path.clone();
    machine.payload_invalidations.insert(
        position(4),
        vec![Some(DescriptorContentsQuery::from_location(&source_leaf))],
    );
    let observe = |machine: &DescriptorMachine| {
        let graph = std::slice::from_ref(machine);
        let mut analysis =
            DescriptorAnalysis::with_borrowed_storage_slots(&types, graph);
        projected_stored_sources_in(
            &types,
            graph,
            &mut analysis,
            machine.id,
            position(5),
            &observed,
            |cut| {
                if cut == position(3) {
                    vec![(copy.clone(), (cut, original.clone()))]
                } else {
                    Vec::new()
                }
            },
            |(capture, mut source), suffix| {
                source.path.extend_from_slice(suffix);
                Some((capture, source))
            },
        )
    };
    assert_eq!(observe(&machine), Some(vec![(position(3), source_leaf)]));
    machine.payload_invalidations.insert(
        position(4),
        vec![Some(DescriptorContentsQuery::from_location(&observed))],
    );
    assert!(observe(&machine).is_none());
}

#[test]
fn equal_copies_keep_distinct_producers_and_writes_retire_only_their_contents() {
    let (types, machine) = copy_graph();
    let identity = machine.id;
    let mut graph = vec![machine];
    let original = DescriptorContentsQuery::from_location(&payload());
    let copy = DescriptorContentsQuery::from_location(&DescriptorLocation::whole(
        semantic_vocabulary::PlaceId::new(2).unwrap(),
    ));
    let observe = |graph: &[DescriptorMachine], query| {
        unchanged_payload_producer(&types, graph, identity, position(5), query)
    };
    assert_eq!(observe(&graph, &original).unwrap().producer, position(2));
    assert_eq!(observe(&graph, &copy).unwrap().producer, position(3));
    assert!(
        require_unchanged_produced_payload(
            &types,
            &graph,
            identity,
            position(2),
            position(5),
            &copy.exact_location().unwrap(),
        )
        .is_err()
    );
    graph[0]
        .payload_invalidations
        .insert(position(4), vec![Some(copy.clone())]);
    assert!(observe(&graph, &original).is_ok());
    assert!(observe(&graph, &copy).is_err());
    graph[0]
        .payload_invalidations
        .insert(position(4), vec![None]);
    assert!(observe(&graph, &original).is_err());
    assert!(observe(&graph, &copy).is_err());
}

#[test]
fn whole_borrowed_arrival_keeps_the_original_producer_and_invalidations() {
    use super::super::DescriptorDefinition;
    use crate::descriptors::descriptor_lifetimes::DescriptorFlowSource;
    use terminal_psi::StructuralAccess;

    let (types, mut machine) = copy_graph();
    let identity = machine.id;
    let borrowed =
        DescriptorLocation::whole(semantic_vocabulary::PlaceId::new(2).unwrap());
    machine.originations.insert(
        position(2),
        vec![(
            payload(),
            super::super::DescriptorCallOrigin::Argument {
                source: payload().root,
                projection: super::super::Projection::new(&[]),
            },
        )],
    );
    machine.places[1].definition = DescriptorDefinition::Arrival {
        access: StructuralAccess::SharedBorrow,
    };
    machine.payload_producers.remove(&position(3));
    let capture = machine.flow.get_mut(&position(3)).unwrap();
    capture.definitions.clear();
    capture.derivations.clear();
    capture.successors[0].parameters.push(borrowed.clone());
    capture.successors[0].bindings.push(
        DescriptorFlowBinding::new(
            DescriptorFlowSource::Location(payload()),
            borrowed.clone(),
            StructuralAccess::SharedBorrow,
            StructuralAccess::SharedBorrow,
        )
        .unwrap(),
    );
    let query = DescriptorContentsQuery::from_location(&borrowed);
    let mut graph = vec![machine];
    let observe = |graph: &[DescriptorMachine]| {
        unchanged_payload_producer(&types, graph, identity, position(5), &query)
    };
    assert_eq!(
        observe(&graph).unwrap(),
        ProducedPayloadVersion {
            producer: position(2),
            payload: payload()
        },
    );
    graph[0]
        .payload_invalidations
        .insert(position(4), vec![Some(query.clone())]);
    assert!(observe(&graph).is_err());
    graph[0].payload_invalidations.clear();
    graph[0].places[1].definition = DescriptorDefinition::Arrival {
        access: StructuralAccess::Owned,
    };
    graph[0].flow.get_mut(&position(3)).unwrap().successors[0].bindings[0] =
        DescriptorFlowBinding::new(
            DescriptorFlowSource::Location(payload()),
            borrowed,
            StructuralAccess::Owned,
            StructuralAccess::Owned,
        )
        .unwrap();
    assert!(observe(&graph).is_err());
}

#[test]
fn ambiguous_producers_and_unfinished_results_do_not_supply_a_version() {
    let (types, machine) = copy_graph();
    let identity = machine.id;
    let mut graph = vec![machine];
    let original = DescriptorContentsQuery::from_location(&payload());
    assert!(
        unchanged_payload_producer(&types, &graph, identity, position(2), &original)
            .is_err()
    );
    graph[0]
        .payload_producers
        .insert(position(4), payload().root);
    assert_eq!(
        unchanged_payload_producer(&types, &graph, identity, position(5), &original),
        Err("observed payload has multiple whole-value producers"),
    );
}

#[test]
fn completed_shared_observation_preserves_existing_storage_not_replacements() {
    let (types, mut machine) = copy_graph();
    let identity = machine.id;
    machine.payload_producers.remove(&position(2));
    machine
        .flow
        .get_mut(&position(2))
        .unwrap()
        .definitions
        .clear();
    let mut graph = vec![machine];
    let observe = |graph: &[DescriptorMachine]| {
        require_unchanged_observed_storage(
            &types,
            graph,
            identity,
            position(2),
            position(5),
            &DescriptorPayloadTarget::current(payload()),
        )
    };
    assert!(observe(&graph).is_ok());
    assert!(
        require_unchanged_produced_payload(
            &types,
            &graph,
            identity,
            position(2),
            position(5),
            &payload()
        )
        .is_err()
    );
    assert!(
        require_unchanged_observed_storage(
            &types,
            &graph,
            identity,
            position(2),
            position(2),
            &DescriptorPayloadTarget::current(payload())
        )
        .is_err()
    );
    graph[0].payload_invalidations.insert(
        position(4),
        vec![Some(DescriptorContentsQuery::from_location(&payload()))],
    );
    assert!(observe(&graph).is_err());
    graph[0].payload_invalidations.clear();
    graph[0].flow.get_mut(&position(4)).unwrap().successors[0]
        .parameters
        .push(payload());
    assert!(
        observe(&graph).is_err(),
        "an equal replacement is not this observation"
    );
    graph[0].flow.get_mut(&position(4)).unwrap().successors[0]
        .parameters
        .clear();
    graph[0]
        .flow
        .get_mut(&position(1))
        .unwrap()
        .successors
        .push(DescriptorFlowEdge {
            borrowed_header_ends: Box::default(),
            edge: None,
            effects: Vec::new(),
            definitions: Vec::new(),
            target: position(5),
            parameters: Vec::new(),
            bindings: Vec::new(),
        });
    assert!(
        observe(&graph).is_err(),
        "every arrival must have completed the observation"
    );
}

fn flow(edges: &[(u64, &[u64])]) -> BTreeMap<DescriptorPosition, DescriptorFlowStep> {
    edges
        .iter()
        .map(|(source, targets)| {
            (
                position(*source),
                DescriptorFlowStep {
                    successors: targets
                        .iter()
                        .map(|target| DescriptorFlowEdge {
                            borrowed_header_ends: Box::default(),
                            edge: None,
                            effects: Vec::new(),
                            definitions: Vec::new(),
                            target: position(*target),
                            parameters: Vec::new(),
                            bindings: Vec::new(),
                        })
                        .collect(),
                    ..Default::default()
                },
            )
        })
        .collect()
}

#[test]
fn all_arrivals_need_the_completed_unchanged_version() {
    let diamond = flow(&[(1, &[2]), (2, &[3, 4]), (3, &[5]), (4, &[5]), (5, &[])]);
    let query = |writes: &[u64]| {
        version_flow_from(
            &diamond,
            position(1),
            VersionStart::Completed(position(2)),
            position(5),
            |_, _| true,
            |current, _| Ok(writes.iter().any(|write| position(*write) == current)),
            |edge| joins(edge, &payload()),
        )
        .unwrap()
    };
    assert!(query(&[]));
    assert!(!query(&[3]));
    assert!(!query(&[4]));
    let bypass = flow(&[(1, &[2, 4]), (2, &[3]), (3, &[5]), (4, &[5]), (5, &[])]);
    assert!(
        !version_flow_from(
            &bypass,
            position(1),
            VersionStart::Completed(position(2)),
            position(5),
            |_, _| true,
            |_, _| Ok(false),
            |edge| joins(edge, &payload()),
        )
        .unwrap()
    );
}

#[test]
fn transition_replacement_retires_only_the_overlapping_version() {
    let original = payload();
    let other =
        DescriptorLocation::whole(semantic_vocabulary::PlaceId::new(2).unwrap());
    let field = DescriptorLocation {
        root: original.root,
        path: vec![
            crate::descriptors::descriptor_lifetimes::DescriptorPathSegment::Field(
                semantic_vocabulary::StructuralFieldId::new(1).unwrap(),
            ),
        ],
    };
    for (parameter, retained) in [
        (other.clone(), true),
        (original.clone(), false),
        (field, false),
    ] {
        let mut diamond =
            flow(&[(1, &[2]), (2, &[3, 4]), (3, &[5]), (4, &[5]), (5, &[])]);
        let edge = &mut diamond.get_mut(&position(4)).unwrap().successors[0];
        edge.parameters.push(parameter.clone());
        edge.bindings.push(
            DescriptorFlowBinding::new(
                other.clone(),
                parameter,
                terminal_psi::StructuralAccess::Owned,
                terminal_psi::StructuralAccess::Owned,
            )
            .unwrap(),
        );
        assert_eq!(
            version_flow_from(
                &diamond,
                position(1),
                VersionStart::Completed(position(2)),
                position(5),
                |_, _| true,
                |_, _| Ok(false),
                |edge| joins(edge, &original),
            )
            .unwrap(),
            retained,
        );
    }
}

#[test]
fn new_loop_completion_replaces_the_previous_dynamic_version() {
    let loop_flow = flow(&[(1, &[2]), (2, &[3]), (3, &[4]), (4, &[2, 5]), (5, &[])]);
    assert!(
        version_flow_from(
            &loop_flow,
            position(1),
            VersionStart::Completed(position(2)),
            position(3),
            |_, _| true,
            |current, _| Ok(current == position(4)),
            |edge| joins(edge, &payload()),
        )
        .unwrap()
    );
    let stale_loop = flow(&[(1, &[2]), (2, &[3]), (3, &[4]), (4, &[3, 5]), (5, &[])]);
    assert!(
        !version_flow_from(
            &stale_loop,
            position(1),
            VersionStart::Completed(position(2)),
            position(3),
            |_, _| true,
            |current, _| Ok(current == position(4)),
            |edge| joins(edge, &payload()),
        )
        .unwrap()
    );
}
