//! Descriptor liveness proposes disposal candidates, not permission to
//! dispose storage. The affine storage walk checks them against its exact
//! available owned regions, then ordinary ReleaseReference operations
//! carry the result to the checker, interpreter and native lowering.

use crate::descriptor_lifetimes::{
    DescriptorLocation, DescriptorPosition, canonical_descriptor_location,
    descriptor_entry_lifetime_with_contents, descriptor_location_lifetime_with_contents,
    descriptor_path,
};
use crate::descriptor_results::{DescriptorAnalysis, DescriptorSlotKind};
use semantic_vocabulary::{IdTable, MachineId, OperationId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{OperationKind, StructuralAccess, StructuralArgument, TerminalModule};

pub struct Candidate {
    pub argument: StructuralArgument,
    pub unproved_case: Option<crate::descriptor_results::DescriptorCasePredicate>,
}

pub type Candidates = BTreeMap<DescriptorPosition, Vec<Candidate>>;

pub struct Error {
    pub machine: MachineId,
    pub operation: Option<OperationId>,
    pub detail: String,
}

impl Error {
    fn local(
        machine: MachineId,
        operation: Option<OperationId>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            machine,
            operation,
            detail: detail.into(),
        }
    }

    fn from_graph(
        module: &TerminalModule,
        error: crate::descriptor_results::DescriptorGraphError,
    ) -> Self {
        Self {
            machine: error.machine(),
            operation: error.operation(module),
            detail: error.in_module(module),
        }
    }
}

pub fn candidates(module: &TerminalModule) -> Result<IdTable<MachineId, Candidates>, Error> {
    let has_borrowed_storage = |machine: &terminal_psi::TerminalMachine| {
        machine
            .structural_parameters
            .iter()
            .map(|parameter| parameter.structural_type)
            .chain(machine.operations().filter_map(|operation| {
                operation
                    .result
                    .as_structural()
                    .map(|result| result.structural_type)
            }))
            .any(|ty| {
                terminal_psi::structural_type_has_borrowed_storage_by(
                    |id| module.structural_type(id),
                    ty,
                )
            })
    };
    let mut graph = crate::descriptor_results::terminal_borrowed_storage_graph_from(
        module,
        module
            .machines
            .iter()
            .filter(|machine| has_borrowed_storage(machine) && !machine.blocks.is_empty())
            .map(|machine| machine.id),
        &[],
    )
    .map_err(|error| Error::from_graph(module, error))?;
    graph.sort_unstable_by_key(|machine| machine.id);
    let mut analysis =
        DescriptorAnalysis::with_borrowed_storage_slots(&module.structural_types, &graph);
    let mut result = IdTable::default();
    for machine in &module.machines {
        let candidates = if machine.blocks.is_empty() || !has_borrowed_storage(machine) {
            Candidates::new()
        } else {
            let prepared = graph
                .binary_search_by_key(&machine.id, |prepared| prepared.id)
                .map(|position| &graph[position])
                .map_err(|_| Error {
                    machine: machine.id,
                    operation: None,
                    detail: "reference disposition lost its prepared descriptor graph".into(),
                })?;
            let flow = analysis
                .flow(machine.id)
                .map_err(|error| Error::from_graph(module, error))?;
            machine_candidates(module, machine, prepared, &flow, &mut analysis)?
        };
        let owner = result.push(candidates);
        if owner != machine.id {
            return Err(Error {
                machine: machine.id,
                operation: None,
                detail: "reference disposition machines are not dense".into(),
            });
        }
    }
    Ok(result)
}

// Liveness uses genuine source cuts. Edge cleanup keeps its own action owner
// at the terminator cut, so no candidate can be assigned a fabricated source
// operation or lose the selected edge on which its last observation occurs.
fn validate_call_owners(
    module: &TerminalModule,
    machine: &terminal_psi::TerminalMachine,
    prepared: &crate::descriptor_results::DescriptorMachine,
) -> Result<(), String> {
    use crate::descriptor_results::DescriptorInvocationOwner;
    for (&(block, ordinal), calls) in &prepared.calls {
        let body = machine
            .block(&block)
            .ok_or("reference call mapping lost its source block")?;
        let mut owners = BTreeSet::new();
        for call in calls.iter().flatten() {
            if !owners.insert(call.owner) {
                return Err("reference call mapping repeats one executable owner".into());
            }
            match call.owner {
                DescriptorInvocationOwner::Operation(operation) => {
                    if body.operations.get(ordinal as usize).map(|row| row.id) != Some(operation) {
                        return Err("reference invocation has a different source operation".into());
                    }
                }
                DescriptorInvocationOwner::InvokeNoNormal(edge) => {
                    if usize::try_from(ordinal).ok() != Some(body.operations.len())
                        || !matches!(body.terminator, terminal_psi::Terminator::InvokeNoNormal { edge: actual, .. } if actual == edge)
                    {
                        return Err("reference invocation has a different source terminator".into());
                    }
                }
                DescriptorInvocationOwner::Cleanup {
                    edge,
                    action_ordinal,
                } => {
                    if usize::try_from(ordinal).ok() != Some(body.operations.len()) {
                        return Err(
                            "reference cleanup is not mapped to its source terminator".into()
                        );
                    }
                    let actions = body
                        .terminator
                        .cleanup_rosters()
                        .find(|(identity, _)| *identity == edge)
                        .map(|(_, actions)| actions)
                        .ok_or("reference cleanup lost its selected source edge")?;
                    let Some(terminal_psi::TerminalAffineCleanupAction::InvokeOwned(cleanup)) =
                        actions.get(action_ordinal as usize)
                    else {
                        return Err("reference cleanup lost its exact source action".into());
                    };
                    if call.callee() != Some(cleanup.cleanup_machine) || call.result.is_some() {
                        return Err("reference cleanup mapping changes its ordinary target".into());
                    }
                    let root = prepared
                        .place_types
                        .get(&cleanup.subject.place)
                        .ok_or("reference cleanup source has no declared type")?;
                    let expected = crate::descriptor_lifetimes::descriptor_location(
                        &module.structural_types,
                        cleanup.subject.place,
                        *root,
                        &cleanup.subject.path,
                        None,
                    )
                    .ok_or("reference cleanup source has no exact captured path")?;
                    if call.arguments.as_slice() != [Some(expected)] {
                        return Err("reference cleanup mapping changes its exact subject".into());
                    }
                }
            }
        }
    }
    Ok(())
}

fn indexed_store_contents(
    module: &TerminalModule,
    machine: &terminal_psi::TerminalMachine,
    prepared: &crate::descriptor_results::DescriptorMachine,
) -> Result<Vec<(DescriptorPosition, DescriptorLocation)>, Error> {
    let mut contents = Vec::new();
    for block in &machine.blocks {
        for (ordinal, operation) in block.operations.iter().enumerate() {
            let (OperationKind::StoreStructuralField {
                destination, path, ..
            }
            | OperationKind::StructuralByteViewFieldStore {
                destination, path, ..
            }) = &operation.kind
            else {
                continue;
            };
            if !prepared
                .place_definition(*destination)
                .is_some_and(|definition| definition.has_independent_storage())
            {
                continue;
            }
            let root_type = *prepared.place_types.get(destination).ok_or_else(|| {
                Error::local(
                    machine.id,
                    Some(operation.id),
                    "indexed store lost its destination type",
                )
            })?;
            let Some(array) = crate::plain_affine::runtime_array_region(
                &module.structural_types,
                root_type,
                path,
            ) else {
                continue;
            };
            if !crate::projection_stays_in_inline_storage(
                module.structural_types.iter(),
                root_type,
                path,
            ) {
                continue;
            }
            let location = canonical_descriptor_location(
                &module.structural_types,
                *destination,
                root_type,
                &array,
            )
            .ok_or_else(|| {
                Error::local(
                    machine.id,
                    Some(operation.id),
                    "indexed store lost its inline array",
                )
            })?;
            let ordinal = u32::try_from(ordinal).map_err(|_| {
                Error::local(
                    machine.id,
                    Some(operation.id),
                    "indexed store position overflows",
                )
            })?;
            contents.push(((block.id, ordinal), location));
        }
    }
    contents.sort_unstable_by_key(|(at, _)| *at);
    Ok(contents)
}

fn machine_candidates(
    module: &TerminalModule,
    machine: &terminal_psi::TerminalMachine,
    prepared: &crate::descriptor_results::DescriptorMachine,
    flow: &BTreeMap<DescriptorPosition, crate::descriptor_lifetimes::DescriptorFlowStep>,
    analysis: &mut DescriptorAnalysis<'_>,
) -> Result<Candidates, Error> {
    validate_call_owners(module, machine, prepared)
        .map_err(|detail| Error::local(machine.id, None, detail))?;
    // An indexed replacement requires an intact array at its own cut.
    // Ending an old descriptor before that cut would make the compiler's
    // own cleanup create a hole. Retaining it there does not read or escape
    // its backing, and does not change how affine checking judges authored moves.
    let contents = indexed_store_contents(module, machine, prepared)?;
    let needs_contents = |at, name: &DescriptorLocation| {
        contents
            .binary_search_by_key(&at, |(position, _)| *position)
            .is_ok_and(|index| contents[index].1.contains(name))
    };
    let mut dead = BTreeMap::<DescriptorPosition, BTreeSet<DescriptorLocation>>::new();
    let mut live = BTreeMap::<DescriptorPosition, BTreeSet<DescriptorLocation>>::new();
    let mut lifetimes = Vec::new();
    let mut dependencies =
        BTreeMap::<DescriptorPosition, BTreeSet<(DescriptorLocation, DescriptorLocation)>>::new();
    for parameter in machine
        .structural_parameters
        .iter()
        .filter(|parameter| parameter.access == StructuralAccess::Owned)
    {
        let slots = DescriptorSlotKind::BorrowedStorage
            .owned_slots(
                &module.structural_types,
                parameter.place,
                parameter.structural_type,
            )
            .ok_or_else(|| {
                Error::local(
                    machine.id,
                    None,
                    "incoming reference disposition has no finite owned slot inventory",
                )
            })?;
        for slot in slots {
            lifetimes.push(descriptor_entry_lifetime_with_contents(
                flow,
                (machine.entry, 0),
                &slot,
                needs_contents,
            ));
        }
    }
    for block in &machine.blocks {
        for (position, operation) in block.operations.iter().enumerate() {
            let Some(output) = operation.result.as_structural() else {
                continue;
            };
            let slots = DescriptorSlotKind::BorrowedStorage
                .owned_slots(
                    &module.structural_types,
                    output.place,
                    output.structural_type,
                )
                .ok_or_else(|| {
                    Error::local(
                        machine.id,
                        Some(operation.id),
                        "reference disposition has no finite owned slot inventory",
                    )
                })?;
            let birth = (
                block.id,
                u32::try_from(position).map_err(|_| {
                    Error::local(
                        machine.id,
                        Some(operation.id),
                        "reference formation position overflows",
                    )
                })?,
            );
            let retained = crate::reference_call_lifetimes(module, machine, operation, analysis)
                .map_err(|error| Error::from_graph(module, error))?;
            if !retained.is_empty() {
                let call = operation.declared_invocation().ok_or_else(|| {
                    Error::local(
                        machine.id,
                        Some(operation.id),
                        "retained reference disposition lost its invocation",
                    )
                })?;
                for retained in retained {
                    let input = call
                        .structural_arguments
                        .get(retained.position)
                        .ok_or_else(|| {
                            Error::local(
                                machine.id,
                                Some(operation.id),
                                "retained reference disposition lost its exact input",
                            )
                        })?;
                    // The retained region, which can be narrower than the input.
                    let source = StructuralArgument {
                        place: input.place,
                        path: retained.path,
                        access: input.access,
                    };
                    add_dependencies(
                        module,
                        prepared,
                        flow,
                        birth,
                        &retained.lifetime,
                        &source,
                        &mut dependencies,
                    )
                    .map_err(|detail| Error::local(machine.id, Some(operation.id), detail))?;
                    lifetimes.push(retained.lifetime);
                }
            }
            for slot in slots {
                let lifetime =
                    descriptor_location_lifetime_with_contents(flow, birth, &slot, needs_contents);
                if let terminal_psi::OperationKind::EstablishReference { source, .. }
                | terminal_psi::OperationKind::EstablishAccessReference { source, .. }
                | terminal_psi::OperationKind::EstablishElementView { source, .. }
                | terminal_psi::OperationKind::EstablishMutableElementView { source, .. } =
                    &operation.kind
                {
                    add_dependencies(
                        module,
                        prepared,
                        flow,
                        birth,
                        &lifetime,
                        source,
                        &mut dependencies,
                    )
                    .map_err(|detail| Error::local(machine.id, Some(operation.id), detail))?;
                }
                lifetimes.push(lifetime);
            }
        }
    }
    for lifetime in lifetimes {
        for (at, locations) in lifetime.authorized {
            let target = if lifetime.live.contains(&at) {
                &mut live
            } else {
                &mut dead
            };
            target.entry(at).or_default().extend(locations);
        }
    }
    // A parent descriptor backs every child, even when the parent has no
    // direct observation left. Closing only its own use interval would erase
    // the resource the child's permission still depends on.
    for (at, dependencies) in &dependencies {
        let dependencies: Vec<_> = dependencies.iter().collect();
        let live = live.entry(*at).or_default();
        let mut pending: Vec<_> = live.iter().cloned().collect();
        while let Some(child) = pending.pop() {
            let start = dependencies.partition_point(|(source, _)| *source < child);
            let end = dependencies.partition_point(|(source, _)| *source <= child);
            for (_, parent) in &dependencies[start..end] {
                if live.insert(parent.clone()) {
                    pending.push(parent.clone());
                }
            }
        }
    }
    let mut candidates = Candidates::new();
    for (at, locations) in dead {
        let mut selected = BTreeMap::new();
        'locations: for location in locations {
            if live
                .get(&at)
                .is_some_and(|locations| locations.contains(&location))
            {
                continue;
            }
            let root_type = prepared
                .place_types
                .get(&location.root)
                .copied()
                .ok_or_else(|| {
                    Error::local(machine.id, None, "reference disposition lost its root type")
                })?;
            let path = descriptor_path(&module.structural_types, root_type, &location.path)
                .ok_or_else(|| {
                    Error::local(
                        machine.id,
                        None,
                        "reference disposition has no exact owned path",
                    )
                })?;
            if !crate::projection_stays_in_inline_storage(
                module.structural_types.iter(),
                root_type,
                &path,
            ) {
                continue;
            }
            let Some(tip) = crate::runtime_structural_path_tip(
                module.structural_types.iter(),
                root_type,
                &path,
            ) else {
                return Err(Error::local(
                    machine.id,
                    None,
                    "reference disposition lost its leaf type",
                ));
            };
            if !tip.shape.is_borrowed_descriptor() {
                continue;
            }
            for (position, segment) in location.path.iter().enumerate() {
                let crate::descriptor_lifetimes::DescriptorPathSegment::Case(case) = segment else {
                    continue;
                };
                let predicate = crate::descriptor_results::DescriptorCasePredicate {
                    subject: DescriptorLocation {
                        root: location.root,
                        path: location.path[..position].to_vec(),
                    },
                    case: *case,
                };
                // A possible payload is not an actual descriptor. Propose an
                // inline release only for an established case; ordinary
                // guarded whole-carrier cleanup still owns the other paths.
                if !analysis.case_is_established(machine.id, at, &predicate) {
                    continue 'locations;
                }
            }
            selected.insert(
                location.clone(),
                Candidate {
                    argument: StructuralArgument {
                        place: location.root,
                        path,
                        access: StructuralAccess::Owned,
                    },
                    unproved_case: None,
                },
            );
        }
        // End descriptors that borrow another carrier's storage before
        // ending that carrier. Arena or block order is not ancestry.
        let dependencies: Vec<_> = dependencies
            .get(&at)
            .into_iter()
            .flatten()
            .filter(|(child, parent)| selected.contains_key(child) && selected.contains_key(parent))
            .collect();
        let mut children: BTreeMap<_, usize> = selected
            .keys()
            .cloned()
            .map(|location| (location, 0))
            .collect();
        for (_, parent) in &dependencies {
            *children.get_mut(parent).expect("selected parent") += 1;
        }
        let mut ready: BTreeSet<_> = children
            .iter()
            .filter(|(_, count)| **count == 0)
            .map(|(location, _)| location.clone())
            .collect();
        while let Some(next) = ready.pop_first() {
            candidates
                .entry(at)
                .or_default()
                .push(selected.remove(&next).expect("selected reference end"));
            let start = dependencies.partition_point(|(child, _)| *child < next);
            let end = dependencies.partition_point(|(child, _)| *child <= next);
            for (_, parent) in &dependencies[start..end] {
                let count = children.get_mut(parent).expect("selected parent");
                *count -= 1;
                if *count == 0 {
                    ready.insert(parent.clone());
                }
            }
        }
        if !selected.is_empty() {
            return Err(Error::local(
                machine.id,
                None,
                "reference disposition has cyclic descriptor-storage ancestry",
            ));
        }
    }
    Ok(candidates)
}

fn add_dependencies(
    module: &TerminalModule,
    prepared: &crate::descriptor_results::DescriptorMachine,
    flow: &BTreeMap<DescriptorPosition, crate::descriptor_lifetimes::DescriptorFlowStep>,
    birth: DescriptorPosition,
    lifetime: &crate::descriptor_lifetimes::DescriptorLifetime,
    source: &StructuralArgument,
    dependencies: &mut BTreeMap<
        DescriptorPosition,
        BTreeSet<(DescriptorLocation, DescriptorLocation)>,
    >,
) -> Result<(), String> {
    let source_type = prepared
        .place_types
        .get(&source.place)
        .copied()
        .ok_or("reference parent lost its source type")?;
    // A runtime element selector has no static slot, but every parent
    // descriptor the reference can borrow from lies inside the storage before
    // that selector. Ordering against all of them is a superset of the real
    // ancestry, so it can only add end-ordering constraints, never drop one.
    let static_prefix = source
        .path
        .iter()
        .position(|segment| segment.runtime_index().is_some())
        .unwrap_or(source.path.len());
    let mut captured = crate::descriptor_lifetimes::descriptor_location(
        &module.structural_types,
        source.place,
        source_type,
        &source.path[..static_prefix],
        None,
    )
    .ok_or("reference parent has no exact captured path")?;
    let parents = if let Some(crossing) = captured.path.iter().position(|segment| {
        matches!(
            segment,
            crate::descriptor_lifetimes::DescriptorPathSegment::Referent
        )
    }) {
        captured.path.truncate(crossing);
        vec![captured]
    } else {
        DescriptorSlotKind::BorrowedStorage
            .owned_slots(&module.structural_types, source.place, source_type)
            .ok_or("reference parent has no finite descriptor inventory")?
            .into_iter()
            .filter(|parent| captured.contains(parent))
            .collect()
    };
    for parent in parents {
        let homes =
            crate::descriptor_lifetimes::descriptor_continuation_locations(flow, birth, &parent);
        for (at, children) in &lifetime.authorized {
            for child in children {
                for parent in homes
                    .get(at)
                    .into_iter()
                    .flatten()
                    .filter(|parent| *parent != child)
                {
                    dependencies
                        .entry(*at)
                        .or_default()
                        .insert((child.clone(), parent.clone()));
                }
            }
        }
    }
    Ok(())
}
