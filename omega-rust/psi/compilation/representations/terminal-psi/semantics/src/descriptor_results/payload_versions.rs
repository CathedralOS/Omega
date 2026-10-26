//! A completed result's original contents are not interchangeable with equal
//! reconstructed data. This query proves only that one produced storage home
//! has not been written since its latest establishment on every incoming path.
//! The consumer separately authenticates the producer, selected call, result
//! type and custody. In particular this does not authorize a profile or memory.

use super::{
    DescriptorAnalysis, DescriptorContentsQuery, DescriptorMachine, DescriptorPayloadTarget,
    descriptor_machine,
};
use crate::descriptor_lifetimes::{DescriptorFlowStep, DescriptorLocation, DescriptorPosition};
use semantic_vocabulary::MachineId;
use std::collections::{BTreeMap, VecDeque};
use terminal_psi::StructuralTypeDeclaration;

/// A current borrow resolves to this exact whole produced storage version.
/// The producer's operation kind, selected provider and original input custody
/// are deliberately absent: the consumer must authenticate those separately.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProducedPayloadVersion {
    pub producer: DescriptorPosition,
    pub payload: DescriptorLocation,
}

/// Resolve an observed payload through the ordinary reference graph and retain
/// its unchanged producer. Equal copies retain their own producer, not the
/// copied value's provenance. Ambiguous aliases and relocated values need a
/// separate occurrence/custody transport proof; this query never guesses one.
pub fn unchanged_payload_producer(
    types: &[StructuralTypeDeclaration],
    graph: &[DescriptorMachine],
    identity: MachineId,
    observation: DescriptorPosition,
    query: &DescriptorContentsQuery,
) -> Result<ProducedPayloadVersion, &'static str> {
    let machine =
        descriptor_machine(graph, identity).ok_or("produced payload machine is absent")?;
    let mut analysis = DescriptorAnalysis::with_reference_slots(types, graph);
    let targets = analysis
        .payload_targets(identity, observation, query)
        .ok_or("produced payload borrow has unresolved backing")?;
    let [DescriptorPayloadTarget::Storage(payload)] = targets.as_slice() else {
        return Err("produced payload borrow does not resolve to one original storage home");
    };
    let mut producers = machine
        .payload_producers
        .iter()
        .filter(|(_, root)| **root == payload.root);
    let (&producer, _) = producers
        .next()
        .ok_or("observed payload has no local whole-value producer")?;
    if producers.next().is_some() {
        return Err("observed payload has multiple whole-value producers");
    }
    require_version(machine, &mut analysis, producer, observation, payload)?;
    Ok(ProducedPayloadVersion {
        producer,
        payload: payload.clone(),
    })
}

/// Require the exact current contents of a whole produced value. Descriptor
/// aliases are resolved at each write, rather than compared by operand place.
/// An independent value copy has another home and cannot satisfy this query for
/// the original producer. Genuine value relocation is a separate transport
/// relationship, not inferred from byte equality by this storage-version test.
/// The supplied graph must retain ordinary reference slots.
pub fn require_unchanged_produced_payload(
    types: &[StructuralTypeDeclaration],
    graph: &[DescriptorMachine],
    identity: MachineId,
    producer: DescriptorPosition,
    observation: DescriptorPosition,
    payload: &DescriptorLocation,
) -> Result<(), &'static str> {
    let machine =
        descriptor_machine(graph, identity).ok_or("produced payload machine is absent")?;
    let mut analysis = DescriptorAnalysis::with_reference_slots(types, graph);
    require_version(machine, &mut analysis, producer, observation, payload)
}

fn require_version(
    machine: &DescriptorMachine,
    analysis: &mut DescriptorAnalysis<'_>,
    producer: DescriptorPosition,
    observation: DescriptorPosition,
    payload: &DescriptorLocation,
) -> Result<(), &'static str> {
    if !payload.path.is_empty()
        || machine.payload_producers.get(&producer) != Some(&payload.root)
        || producer == observation
    {
        return Err("payload query does not name a completed whole-value producer");
    }
    require_unchanged_between(
        machine,
        analysis,
        producer,
        observation,
        &DescriptorPayloadTarget::Storage(payload.clone()),
    )
}

/// Preserve an existing storage occurrence after a completed observation.
/// Unlike a produced-result query, the storage may be an owned formal. The
/// caller must separately establish that the starting operation only borrows
/// this storage and completes normally; this query does not grant that fact.
/// Writes, replacement, unresolved aliases and incoming formal assignments
/// between that completion and the use all retire the observation.
pub fn require_unchanged_observed_storage(
    types: &[StructuralTypeDeclaration],
    graph: &[DescriptorMachine],
    identity: MachineId,
    completed_observation: DescriptorPosition,
    use_position: DescriptorPosition,
    storage: &DescriptorPayloadTarget,
) -> Result<(), &'static str> {
    let machine =
        descriptor_machine(graph, identity).ok_or("observed storage machine is absent")?;
    if completed_observation == use_position {
        return Err("storage observation has not completed before its use");
    }
    let mut analysis = DescriptorAnalysis::with_reference_slots(types, graph);
    require_unchanged_between(
        machine,
        &mut analysis,
        completed_observation,
        use_position,
        storage,
    )
}

fn require_unchanged_between(
    machine: &DescriptorMachine,
    analysis: &mut DescriptorAnalysis<'_>,
    producer: DescriptorPosition,
    observation: DescriptorPosition,
    payload: &DescriptorPayloadTarget,
) -> Result<(), &'static str> {
    let identity = machine.id;
    let relevant = super::payload_reachability::predecessors(machine, observation)
        .ok_or("produced payload observation has unresolved control flow")?;
    // A symbolic incoming referent is not a local allocation or an address.
    // Its exact query identifies the same borrowed payload at both uses. Keep
    // the descriptor binding unchanged too; rebinding it would change which
    // original storage the observation described. Unknown incoming aliases
    // still invalidate the observation rather than proving disjointness.
    let home = match payload {
        DescriptorPayloadTarget::Storage(location) => location.clone(),
        DescriptorPayloadTarget::Incoming(query) => DescriptorLocation::whole(query.root),
    };
    let overlaps =
        |location: &DescriptorLocation| home.contains(location) || location.contains(&home);
    let unchanged = version_flow(
        &machine.flow,
        machine.entry,
        producer,
        observation,
        &home,
        |position, ordinal| relevant.includes_edge(position, ordinal),
        |position, step| {
            if machine.unresolved_contents_writes.contains(&position) {
                return Ok(true);
            }
            if step.definitions.iter().any(overlaps) {
                return Ok(true);
            }
            for query in machine
                .payload_invalidations
                .get(&position)
                .into_iter()
                .flatten()
            {
                let Some(targets) = query
                    .as_ref()
                    .and_then(|query| analysis.payload_targets(identity, position, query))
                else {
                    return Ok(true);
                };
                if targets.iter().any(|target| match target {
                    DescriptorPayloadTarget::Storage(location) => overlaps(location),
                    DescriptorPayloadTarget::Incoming(_) => true,
                }) {
                    return Ok(true);
                }
            }
            // Ordinary structural stores need no scalar-write row. A new,
            // distinct result home is not a mutation of this existing value.
            for (_, destination) in &step.derivations {
                if machine.payload_producers.get(&position) == Some(&destination.root) {
                    continue;
                }
                let Some(destinations) = analysis.storage_origins(identity, position, destination)
                else {
                    return Ok(true);
                };
                if destinations.iter().any(overlaps) {
                    return Ok(true);
                }
            }
            Ok(false)
        },
    )?;
    if unchanged {
        Ok(())
    } else {
        Err("produced payload is absent, replaced or possibly modified before this observation")
    }
}

/// One bit per reachable position: every arrival must still have the current
/// producer's version. Re-execution of that producer starts a new version;
/// loops do not preserve a stale observation from an earlier dynamic instance.
fn version_flow(
    flow: &BTreeMap<DescriptorPosition, DescriptorFlowStep>,
    entry: DescriptorPosition,
    producer: DescriptorPosition,
    observation: DescriptorPosition,
    payload: &DescriptorLocation,
    includes_edge: impl Fn(DescriptorPosition, usize) -> bool,
    mut modifies: impl FnMut(DescriptorPosition, &DescriptorFlowStep) -> Result<bool, &'static str>,
) -> Result<bool, &'static str> {
    let positions = flow.keys().copied().collect::<Vec<_>>();
    let position_index = |position| {
        positions
            .binary_search(&position)
            .map_err(|_| "payload version flow names an absent position")
    };
    let entry_index = position_index(entry)?;
    let observation_index = position_index(observation)?;
    position_index(producer)?;
    let mut versions = vec![None; positions.len()];
    let mut writes = vec![None; positions.len()];
    versions[entry_index] = Some(false);
    let mut pending = VecDeque::from([entry_index]);
    while let Some(current_index) = pending.pop_front() {
        let current = positions[current_index];
        let step = flow
            .get(&current)
            .ok_or("payload version flow lost its step")?;
        let before = versions[current_index].ok_or("payload version arrival is absent")?;
        let after = if current == producer {
            true
        } else if before {
            let modifies = match writes[current_index] {
                Some(modifies) => modifies,
                None => {
                    let result = modifies(current, step)?;
                    writes[current_index] = Some(result);
                    result
                }
            };
            !modifies
        } else {
            false
        };
        for (ordinal, edge) in step.successors.iter().enumerate() {
            if !includes_edge(current, ordinal) {
                continue;
            }
            let target_index = position_index(edge.target)?;
            // A transition assigns its formals simultaneously, after the
            // source operation. A binding of equal contents is not the old
            // produced instance; only a separate custody-transport proof may
            // connect those instances. Killing one incoming edge must weaken
            // the join, not remove that edge from reachability.
            let arrival = after
                && !edge
                    .parameters
                    .iter()
                    .any(|parameter| parameter.contains(payload) || payload.contains(parameter));
            let joined = versions[target_index].map_or(arrival, |old| old && arrival);
            if versions[target_index] != Some(joined) {
                versions[target_index] = Some(joined);
                if !pending.contains(&target_index) {
                    pending.push_back(target_index);
                }
            }
        }
    }
    Ok(versions[observation_index] == Some(true))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptor_lifetimes::DescriptorFlowEdge;
    use semantic_vocabulary::BlockId;

    fn position(value: u64) -> DescriptorPosition {
        (BlockId::new(value).unwrap(), 0)
    }

    fn payload() -> DescriptorLocation {
        DescriptorLocation::whole(semantic_vocabulary::PlaceId::new(1).unwrap())
    }

    fn copy_graph() -> (Vec<StructuralTypeDeclaration>, DescriptorMachine) {
        use super::super::{DescriptorDefinition, DescriptorPlace};
        use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType, StructuralTypeId};
        use terminal_psi::{StructuralTypeShape, TerminalIdentityIndex};
        let original = payload();
        let copy = DescriptorLocation::whole(semantic_vocabulary::PlaceId::new(2).unwrap());
        let carrier = StructuralTypeId::new(1).unwrap();
        let types = vec![StructuralTypeDeclaration {
            quotient_carrier: None,
            id: carrier,
            identity: TerminalIdentityIndex(1),
            owns_linear_claim: false,
            no_code_affine_cleanup: true,
            copyable: false,
            descriptor_cleanup: true,
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
                    place,
                    definition: DescriptorDefinition::Local,
                })
                .collect(),
            place_types: BTreeMap::from([(original.root, carrier), (copy.root, carrier)]),
            entry: position(1),
            flow,
            payload_producers: BTreeMap::from([
                (position(2), original.root),
                (position(3), copy.root),
            ]),
            originations: BTreeMap::new(),
            projections: BTreeMap::new(),
            contents_queries: BTreeMap::new(),
            contents_transfers: BTreeMap::new(),
            payload_entry_predicates: None,
            payload_scalar_definitions: BTreeMap::new(),
            payload_scalar_postconditions: BTreeMap::new(),
            payload_scalar_parameters: BTreeMap::new(),
            scalar_payload_reads: BTreeMap::new(),
            payload_edge_predicates: BTreeMap::new(),
            payload_scalar_bindings: BTreeMap::new(),
            scalar_payload_writes: BTreeMap::new(),
            payload_invalidations: BTreeMap::new(),
            payload_arrivals: BTreeMap::new(),
            unresolved_contents_writes: Default::default(),
            storage_derivations: Default::default(),
            calls: BTreeMap::new(),
            normal_exits: vec![position(5)],
        };
        (types, machine)
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
    fn ambiguous_producers_and_unfinished_results_do_not_supply_a_version() {
        let (types, machine) = copy_graph();
        let identity = machine.id;
        let mut graph = vec![machine];
        let original = DescriptorContentsQuery::from_location(&payload());
        assert!(
            unchanged_payload_producer(&types, &graph, identity, position(2), &original).is_err()
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
        // The extent was supplied by the caller, not produced by the profile
        // call. Its separate shared-only completion contract is a precondition
        // of this query; no result producer is invented for the extent.
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
                &DescriptorPayloadTarget::Storage(payload()),
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
                &DescriptorPayloadTarget::Storage(payload())
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
            version_flow(
                &diamond,
                position(1),
                position(2),
                position(5),
                &payload(),
                |_, _| true,
                |current, _| Ok(writes.iter().any(|write| position(*write) == current)),
            )
            .unwrap()
        };
        assert!(query(&[]));
        assert!(!query(&[3]));
        assert!(!query(&[4]));
        let bypass = flow(&[(1, &[2, 4]), (2, &[3]), (3, &[5]), (4, &[5]), (5, &[])]);
        assert!(
            !version_flow(
                &bypass,
                position(1),
                position(2),
                position(5),
                &payload(),
                |_, _| true,
                |_, _| Ok(false)
            )
            .unwrap()
        );
    }

    #[test]
    fn transition_replacement_retires_only_the_overlapping_version() {
        let original = payload();
        let other = DescriptorLocation::whole(semantic_vocabulary::PlaceId::new(2).unwrap());
        let field = DescriptorLocation {
            root: original.root,
            path: vec![crate::descriptor_lifetimes::DescriptorPathSegment::Field(
                semantic_vocabulary::StructuralFieldId::new(1).unwrap(),
            )],
        };
        for (parameter, retained) in [
            (other.clone(), true),
            (original.clone(), false),
            (field, false),
        ] {
            let mut diamond = flow(&[(1, &[2]), (2, &[3, 4]), (3, &[5]), (4, &[5]), (5, &[])]);
            let edge = &mut diamond.get_mut(&position(4)).unwrap().successors[0];
            edge.parameters.push(parameter.clone());
            edge.bindings.push((other.clone(), parameter));
            assert_eq!(
                version_flow(
                    &diamond,
                    position(1),
                    position(2),
                    position(5),
                    &original,
                    |_, _| true,
                    |_, _| Ok(false),
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
            version_flow(
                &loop_flow,
                position(1),
                position(2),
                position(3),
                &payload(),
                |_, _| true,
                |current, _| Ok(current == position(4))
            )
            .unwrap()
        );
        let stale_loop = flow(&[(1, &[2]), (2, &[3]), (3, &[4]), (4, &[3, 5]), (5, &[])]);
        assert!(
            !version_flow(
                &stale_loop,
                position(1),
                position(2),
                position(3),
                &payload(),
                |_, _| true,
                |current, _| Ok(current == position(4))
            )
            .unwrap()
        );
    }
}

/// Recover actual scalar operands stored into the exact version observed by a
/// read. Arithmetic facts and call guarantees cannot create this transport:
/// the adapter separately authenticates every writer against its operation.
pub fn unchanged_stored_scalar_sources(
    types: &[StructuralTypeDeclaration],
    graph: &[DescriptorMachine],
    identity: MachineId,
    value: semantic_vocabulary::ValueId,
    mut actual_writes: impl FnMut(
        DescriptorPosition,
    ) -> Vec<(DescriptorLocation, semantic_vocabulary::ValueId)>,
) -> Option<Vec<semantic_vocabulary::ValueId>> {
    let machine = descriptor_machine(graph, identity)?;
    let mut reads = machine
        .scalar_payload_reads
        .iter()
        .flat_map(|(position, rows)| {
            rows.iter().filter_map(move |(location, term)| {
                matches!(term, semantic_vocabulary::ScalarTerm::Value { id, .. } if *id == value)
                    .then_some((*position, location))
            })
        });
    let (observation, observed) = reads.next()?;
    if reads.next().is_some() {
        return None;
    }
    let mut analysis = DescriptorAnalysis::with_reference_slots(types, graph);
    let home = direct_owned_storage(types, machine, observed)
        .or_else(|| analysis.location(identity, observation, observed))?;
    let mut sources = Vec::new();
    for &producer in machine.flow.keys() {
        if producer == observation {
            continue;
        }
        for (written, id) in actual_writes(producer) {
            let destinations = if machine.payload_producers.get(&producer) == Some(&written.root) {
                vec![written.clone()]
            } else {
                let Some(destination) = direct_owned_storage(types, machine, &written)
                    .or_else(|| analysis.location(identity, producer, &written))
                else {
                    continue;
                };
                vec![destination]
            };
            let [destination] = destinations.as_slice() else {
                continue;
            };
            if destination != &home {
                continue;
            }
            if require_unchanged_between(
                machine,
                &mut analysis,
                producer,
                observation,
                &DescriptorPayloadTarget::Storage(home.clone()),
            )
            .is_ok()
            {
                sources.push(id);
            }
        }
    }
    sources.sort();
    sources.dedup();
    (!sources.is_empty()).then_some(sources)
}

/// A plain local carrier has its own address even if a later unresolved call
/// prevents whole-machine contents replay. This proves only the address of
/// its typed scalar/aggregate storage, never the contents or borrow access.
fn direct_owned_storage(
    types: &[StructuralTypeDeclaration],
    machine: &DescriptorMachine,
    location: &DescriptorLocation,
) -> Option<DescriptorLocation> {
    let mut places = machine
        .places
        .iter()
        .filter(|row| row.place == location.root);
    let place = places.next()?;
    if places.next().is_some() || !matches!(place.definition, super::DescriptorDefinition::Local) {
        return None;
    }
    let ty = *machine.place_types.get(&location.root)?;
    if !super::plain_owned_shape(types.iter(), ty) {
        return None;
    }
    crate::descriptor_lifetimes::descriptor_path(types, ty, &location.path)?;
    Some(location.clone())
}
