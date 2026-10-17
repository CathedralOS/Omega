//! Memberships describe current payloads, not the storage's arrival value.
//! Stores snapshot their actual source theory; writes retire overlapping rows;
//! reachable control-flow joins intersect evidence. Descriptor replay resolves
//! backing separately. Neither replay grants access, custody or an origin route.

use super::contents::select_payload_memberships;
use super::*;
use crate::descriptor_lifetimes::descriptor_path;
use std::collections::VecDeque;

type State = BTreeMap<PlaceId, PayloadMembershipContract>;

/// Reconstruct memberships at an exact pre-operation position. Entry supplies
/// only actual input contracts; producer rosters enter at establishment. A
/// receiving store or requested copy annotation never seeds the fixed point.
/// The bounded worklist fails closed if the current graph cannot be resolved.
pub fn current_payload_contract(
    types: &[StructuralTypeDeclaration],
    domains: &[terminal_psi::StructuralDomainDeclaration],
    graph: &[DescriptorMachine],
    identity: MachineId,
    position: DescriptorPosition,
    root: PlaceId,
    source: &impl Fn(PlaceId) -> Option<PayloadMembershipContract>,
) -> Result<PayloadMembershipContract, &'static str> {
    let mut failure = "descriptor machine is absent";
    let result = (|| {
        let machine = descriptor_machine(graph, identity)?;
        let mut initial = State::new();
        for place in &machine.places {
            if matches!(place.definition, DescriptorDefinition::Parameter { .. }) {
                failure = "entry payload contract is absent";
                let mut contract = source(place.place)?;
                super::payload_entry::close(types, domains, machine, place.place, &mut contract);
                initial.insert(place.place, contract);
            }
        }
        let mut snapshots = BTreeMap::from([(machine.entry, initial)]);
        let mut pending = VecDeque::from([machine.entry]);
        let mut analysis = DescriptorAnalysis::new(types, graph);
        let mut remaining = 65_536usize;
        while let Some(current) = pending.pop_front() {
            failure = "payload history worklist exceeded its finite budget";
            remaining = remaining.checked_sub(1)?;
            failure = "payload history flow position is absent";
            let mut before = snapshots.get(&current)?.clone();
            if let Some(definitions) = machine.payload_scalar_definitions.get(&current) {
                retire_scalar_instances(&mut before, definitions);
            }
            let step = machine.flow.get(&current)?;
            let mut after = before.clone();
            if let Some(writes) = machine.payload_invalidations.get(&current) {
                for query in writes {
                    let targets = query
                        .as_ref()
                        .and_then(|query| analysis.payload_targets(identity, current, query));
                    match targets {
                        Some(targets) => {
                            for target in targets {
                                match target {
                                    DescriptorPayloadTarget::Storage(location) => {
                                        if let Some(contract) = after.get_mut(&location.root) {
                                            failure = "payload write path has no typed coordinate";
                                            let path = descriptor_path(
                                                types,
                                                contract.carrier,
                                                &location.path,
                                            )?;
                                            retire(contract, &path, &location.path);
                                        }
                                    }
                                    DescriptorPayloadTarget::Incoming(_) => clear(&mut after),
                                }
                            }
                        }
                        None => clear(&mut after),
                    }
                }
            }
            // Snapshot all RHSs before applying any destination update. A repeated
            // or overlapping destination cannot acquire the last writer's theory.
            let mut stores = Vec::new();
            for (actual, destination) in &step.derivations {
                if machine.payload_producers.get(&current) == Some(&destination.root)
                    || machine.storage_derivations.contains(&(
                        current,
                        actual.clone(),
                        destination.clone(),
                    ))
                {
                    continue;
                }
                let incoming = before.get(&actual.root).and_then(|contract| {
                    select_payload_memberships(types, domains, contract, &actual.path)
                });
                failure = "structural store backing is unresolved";
                let destinations = analysis.storage_origins(identity, current, destination)?;
                let strong = destinations.len() == 1;
                for destination in destinations {
                    stores.push((destination, incoming.clone(), strong));
                }
            }
            // Decide simultaneous overlap before consuming the snapshots. The
            // exact-carrier path can then move its theory without cloning it.
            for index in 0..stores.len() {
                let destination = &stores[index].0;
                if stores.iter().enumerate().any(|(other, (candidate, _, _))| {
                    index != other
                        && candidate.root == destination.root
                        && (candidate.path.starts_with(&destination.path)
                            || destination.path.starts_with(&candidate.path))
                }) {
                    stores[index].2 = false;
                }
            }
            for (destination, incoming, strong) in stores {
                let Some(contract) = after.get_mut(&destination.root) else {
                    continue;
                };
                failure = "structural store path has no typed coordinate";
                let path = descriptor_path(types, contract.carrier, &destination.path)?;
                retire(contract, &path, &destination.path);
                if !strong {
                    continue;
                }
                let Some(incoming) = incoming else {
                    continue;
                };
                failure = "structural store carrier does not match its actual source";
                let destination_carrier =
                    crate::qualification_path_carrier(types.iter(), contract.carrier, &path)?;
                let incoming = incoming.presented(types, domains, destination_carrier)?;
                if path.is_empty() {
                    *contract = incoming;
                } else {
                    append_scalar_observations(contract, &incoming, &destination.path);
                    terminal_psi::StructuralPathQualification::append_under(
                        &incoming.qualifications,
                        &incoming.projected_qualifications,
                        &path,
                        &mut contract.projected_qualifications,
                    );
                }
            }
            if let Some(producer) = machine.payload_producers.get(&current) {
                failure = "established payload contract is absent";
                let mut produced = source(*producer)?;
                // Constructors and copies snapshot their actual children before
                // consuming them. A result annotation supplies no scalar content.
                for (actual, destination) in &step.derivations {
                    if destination.root != *producer
                        || machine.storage_derivations.contains(&(
                            current,
                            actual.clone(),
                            destination.clone(),
                        ))
                    {
                        continue;
                    }
                    if let Some(incoming) = before.get(&actual.root).and_then(|contract| {
                        select_payload_memberships(types, domains, contract, &actual.path)
                    }) {
                        let path = descriptor_path(types, produced.carrier, &destination.path)?;
                        let destination_carrier = crate::qualification_path_carrier(
                            types.iter(),
                            produced.carrier,
                            &path,
                        )?;
                        let Some(incoming) =
                            incoming.presented(types, domains, destination_carrier)
                        else {
                            continue;
                        };
                        append_scalar_observations(&mut produced, &incoming, &destination.path);
                        if path.is_empty() {
                            produced.qualifications.extend(&incoming.qualifications);
                            produced
                                .projected_qualifications
                                .extend(incoming.projected_qualifications.clone());
                        } else {
                            terminal_psi::StructuralPathQualification::append_under(
                                &incoming.qualifications,
                                &incoming.projected_qualifications,
                                &path,
                                &mut produced.projected_qualifications,
                            );
                        }
                        produced.qualifications.sort();
                        produced.qualifications.dedup();
                        produced.projected_qualifications.sort();
                        produced.projected_qualifications.dedup();
                    }
                }
                after.insert(*producer, produced);
            }
            if let Some(writes) = machine.scalar_payload_writes.get(&current) {
                for (destination, value) in writes {
                    let Some(contract) = after.get_mut(&destination.root) else {
                        continue;
                    };
                    if super::payload_predicates::observation_type(
                        types,
                        contract.carrier,
                        &destination.path,
                    ) != Some(value.scalar_type())
                    {
                        continue;
                    }
                    contract.scalar_observations.retain(|observation| {
                        !observation.path.starts_with(&destination.path)
                            && !destination.path.starts_with(&observation.path)
                    });
                    contract
                        .scalar_observations
                        .push(super::PayloadScalarObservation {
                            path: destination.path.clone(),
                            value: value.clone(),
                        });
                }
            }
            if let Some(reads) = machine.scalar_payload_reads.get(&current) {
                for (location, value) in reads {
                    if let Some(contract) = after.get_mut(&location.root) {
                        contract
                            .scalar_observations
                            .retain(|observation| observation.path != location.path);
                        contract
                            .scalar_observations
                            .push(super::PayloadScalarObservation {
                                path: location.path.clone(),
                                value: value.clone(),
                            });
                    }
                }
            }
            for contract in after.values_mut() {
                super::payload_predicates::close(types, domains, contract);
            }
            for (ordinal, edge) in step.successors.iter().enumerate() {
                let mut arrival = after.clone();
                if let Some(Some(predicate)) = machine
                    .payload_edge_predicates
                    .get(&current)
                    .and_then(|rows| rows.get(ordinal))
                {
                    for contract in arrival.values_mut() {
                        if !contract.scalar_requirements.contains(predicate) {
                            contract.scalar_requirements.push(predicate.clone());
                        }
                        super::payload_predicates::close(types, domains, contract);
                    }
                }
                let edge_source = arrival.clone();
                for parameter in &edge.parameters {
                    arrival.remove(&parameter.root);
                }
                let mut bindings = Vec::new();
                for (actual, parameter) in &edge.bindings {
                    failure = "edge formal is not a whole payload root";
                    if !parameter.path.is_empty() {
                        return None;
                    }
                    if let Some(contract) = edge_source.get(&actual.root).and_then(|contract| {
                        select_payload_memberships(types, domains, contract, &actual.path)
                    }) {
                        // Exact descriptor bindings also carry reference access
                        // conversions. Memberships are carrier-specific: an
                        // incompatible row supplies no evidence for that formal,
                        // but cannot invalidate other payloads on the edge.
                        let Some(contract) = contract.presented(
                            types,
                            domains,
                            *machine.place_types.get(&parameter.root)?,
                        ) else {
                            continue;
                        };
                        bindings.push((parameter.root, contract));
                    }
                }
                // A view selection is a readable set of payload candidates, not
                // an exact address. Intersect its actual source memberships while
                // keeping descriptor custody's exact-only bindings untouched.
                if let Some(additional) = machine
                    .payload_arrivals
                    .get(&current)
                    .and_then(|edges| edges.get(ordinal))
                {
                    for binding in additional {
                        let contract = binding.source.as_ref().and_then(|query| {
                            let targets =
                                analysis.arrival_payload_targets(identity, current, query)?;
                            payload_memberships(types, domains, targets, |root| {
                                edge_source.get(&root).cloned()
                            })
                        });
                        if let Some(contract) = contract {
                            failure = "selected edge actual and formal payload carriers differ";
                            let contract = contract.presented(
                                types,
                                domains,
                                *machine.place_types.get(&binding.parameter)?,
                            )?;
                            bindings.push((binding.parameter, contract));
                        }
                    }
                }
                for (parameter, contract) in bindings {
                    arrival.insert(parameter, contract);
                }
                if let Some(parameters) = machine.payload_scalar_parameters.get(&edge.target.0) {
                    retire_scalar_instances(&mut arrival, parameters);
                }
                let changed = if let Some(existing) = snapshots.get_mut(&edge.target) {
                    let old = existing.clone();
                    existing.retain(|place, contract| {
                        let Some(incoming) = arrival.get(place) else {
                            return false;
                        };
                        if contract.carrier != incoming.carrier {
                            return false;
                        }
                        contract
                            .scalar_requirements
                            .retain(|premise| incoming.scalar_requirements.contains(premise));
                        contract.scalar_observations.retain(|observation| {
                            incoming.scalar_observations.contains(observation)
                        });
                        contract
                            .qualifications
                            .retain(|domain| incoming.qualifications.contains(domain));
                        contract
                            .projected_qualifications
                            .retain(|row| incoming.projected_qualifications.contains(row));
                        true
                    });
                    *existing != old
                } else {
                    snapshots.insert(edge.target, arrival);
                    true
                };
                if changed && !pending.contains(&edge.target) {
                    pending.push_back(edge.target);
                }
            }
        }
        failure = "requested payload root has no reached incoming membership history";
        let mut current = snapshots.get(&position)?.get(&root)?.clone();
        failure = "requested source has no declared payload contract";
        let declared = source(root)?;
        failure = "current and declared source payload carriers differ";
        if current.carrier != declared.carrier {
            return None;
        }
        // Pure byte predicates in an actual source's retained contract are
        // standing validity obligations. Independent arrival, primitive-write,
        // replacement and writable-call checks preserve that floor. History owns
        // additional transient/routed memberships; the receiving copy contributes
        // none. This query must never discharge those independent checks itself.
        let byte_domain = |carrier, domain| {
            crate::byte_predicate_carrier(types.iter(), carrier)
                && crate::byte_qualification_predicates_in(domains.iter(), carrier, &[domain])
                    .is_some()
        };
        current.qualifications.extend(
            declared
                .qualifications
                .into_iter()
                .filter(|domain| byte_domain(declared.carrier, *domain)),
        );
        current.projected_qualifications.extend(
            declared.projected_qualifications.into_iter().filter(|row| {
                crate::qualification_path_carrier(types.iter(), declared.carrier, &row.path)
                    .is_some_and(|carrier| byte_domain(carrier, row.domain))
            }),
        );
        current.qualifications.sort();
        current.qualifications.dedup();
        current.projected_qualifications.sort();
        current.projected_qualifications.dedup();
        Some(current)
    })();
    result.ok_or(failure)
}

fn retire(
    contract: &mut PayloadMembershipContract,
    path: &[StructuralPathSegment],
    scalar_path: &[crate::descriptor_lifetimes::DescriptorPathSegment],
) {
    contract.scalar_observations.retain(|observation| {
        !observation.path.starts_with(scalar_path) && !scalar_path.starts_with(&observation.path)
    });
    contract.qualifications.clear();
    contract
        .projected_qualifications
        .retain(|row| !crate::structural_paths_may_overlap(&row.path, path));
}

fn clear(state: &mut State) {
    for contract in state.values_mut() {
        contract.qualifications.clear();
        contract.projected_qualifications.clear();
        contract.scalar_observations.clear();
    }
}

fn append_scalar_observations(
    destination: &mut PayloadMembershipContract,
    source: &PayloadMembershipContract,
    prefix: &[crate::descriptor_lifetimes::DescriptorPathSegment],
) {
    for premise in &source.scalar_requirements {
        if !destination.scalar_requirements.contains(premise) {
            destination.scalar_requirements.push(premise.clone());
        }
    }
    for observation in &source.scalar_observations {
        let mut path = prefix.to_vec();
        path.extend_from_slice(&observation.path);
        destination
            .scalar_observations
            .retain(|previous| previous.path != path);
        destination
            .scalar_observations
            .push(super::PayloadScalarObservation {
                path,
                value: observation.value.clone(),
            });
    }
}

/// SSA names acquire new dynamic instances on repeated execution. Completed
/// payload memberships survive; observations and premises naming old instances
/// must not classify newly read values or rebound block formals.
fn retire_scalar_instances(state: &mut State, definitions: &[semantic_vocabulary::ValueId]) {
    if definitions.is_empty() {
        return;
    }
    for contract in state.values_mut() {
        contract.scalar_observations.retain(|observation| {
            !observation
                .value
                .any_value_id(|value| definitions.contains(&value))
        });
        contract.scalar_requirements.retain(|premise| {
            let mut changed = false;
            let closed = premise.visit_value_ids(|value| {
                changed |= definitions.contains(&value);
            });
            closed && !changed
        });
    }
}
