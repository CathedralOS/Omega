//! Descriptor liveness proposes disposal candidates, not permission to
//! dispose storage. The affine storage walk checks them against its exact
//! available owned regions, then ordinary ReleaseReference operations
//! carry the result to the checker, interpreter and native lowering.

use crate::descriptor_lifetimes::{
    DeadTailCutoff, DescriptorLocation, DescriptorPosition, FlowPredecessors,
    canonical_descriptor_location, descriptor_continuation_locations_indexed,
    descriptor_entry_lifetime_with_contents_indexed,
    descriptor_location_lifetime_with_contents_indexed, descriptor_path,
};
use crate::descriptor_results::{DescriptorAnalysis, DescriptorSlotKind};
use arena::IdTable;
use semantic_vocabulary::{BlockId, MachineId, OperationId, PlaceId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{OperationKind, StructuralAccess, StructuralArgument, TerminalModule};

#[derive(Debug, PartialEq)]
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
    candidates_with(module, None)
}

/// `candidates`, taking and publishing callee summaries through the table
/// that the affine walks of this same module share. Its graph is built
/// without nominal calls, as each walk's is, so a summary from the table is
/// the one its own replay would produce.
pub fn candidates_sharing(
    module: &TerminalModule,
    summaries: &crate::plain_affine::AffineSummaries<'_>,
) -> Result<IdTable<MachineId, Candidates>, Error> {
    candidates_with(module, Some(summaries))
}

fn candidates_with(
    module: &TerminalModule,
    summaries: Option<&crate::plain_affine::AffineSummaries<'_>>,
) -> Result<IdTable<MachineId, Candidates>, Error> {
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
        DescriptorAnalysis::with_borrowed_storage_slots(&module.types.structural_types, &graph);
    if let Some(summaries) = summaries {
        summaries.share_with(&mut analysis);
    }
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
                .flow_arc(machine.id)
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
                        &module.types.structural_types,
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
                &module.types.structural_types,
                root_type,
                path,
            ) else {
                continue;
            };
            if !crate::projection_stays_in_inline_storage(
                module.types.structural_types.iter(),
                root_type,
                path,
            ) {
                continue;
            }
            let location = canonical_descriptor_location(
                &module.types.structural_types,
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

fn is_acyclic_block_graph(
    machine: &terminal_psi::TerminalMachine,
    flow: &BTreeMap<DescriptorPosition, crate::descriptor_lifetimes::DescriptorFlowStep>,
) -> bool {
    let mut ids: Vec<BlockId> = machine.blocks.iter().map(|b| b.id).collect();
    ids.sort_unstable();
    let count = ids.len();
    let mut in_degree = vec![0usize; count];
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); count];
    for (&(src_blk, src_ord), step) in flow {
        let Ok(u) = ids.binary_search(&src_blk) else {
            return false;
        };
        for edge in &step.successors {
            if edge.target.0 == src_blk {
                if edge.target.1 <= src_ord {
                    return false;
                }
                continue;
            }
            let Ok(v) = ids.binary_search(&edge.target.0) else {
                return false;
            };
            adj[u].push(v);
            in_degree[v] += 1;
        }
    }
    let mut queue: Vec<usize> = (0..count).filter(|&i| in_degree[i] == 0).collect();
    let mut visited = 0usize;
    while let Some(u) = queue.pop() {
        visited += 1;
        for &v in &adj[u] {
            in_degree[v] -= 1;
            if in_degree[v] == 0 {
                queue.push(v);
            }
        }
    }
    visited == count
}

fn build_dead_tail_cutoff(
    module: &TerminalModule,
    machine: &terminal_psi::TerminalMachine,
    flow: &BTreeMap<DescriptorPosition, crate::descriptor_lifetimes::DescriptorFlowStep>,
    predecessors: &FlowPredecessors<'_>,
    contents: &[(DescriptorPosition, DescriptorLocation)],
) -> DeadTailCutoff {
    if !is_acyclic_block_graph(machine, flow) {
        return DeadTailCutoff::new(false, Vec::new());
    }
    #[derive(Default, Clone)]
    struct Touch {
        ineligible: bool,
        block: Option<BlockId>,
        last_ordinal: u32,
    }
    let mut touches: Vec<Touch> = Vec::new();
    fn touch_slot(touches: &mut Vec<Touch>, place: PlaceId) -> &mut Touch {
        let idx = usize::try_from(place.get() - 1).unwrap_or(usize::MAX);
        if idx >= touches.len() {
            touches.resize_with(idx + 1, Touch::default);
        }
        &mut touches[idx]
    }
    let mark_ineligible = |touches: &mut Vec<Touch>, place: PlaceId| {
        touch_slot(touches, place).ineligible = true;
    };
    let record = |touches: &mut Vec<Touch>, place: PlaceId, block: BlockId, ordinal: u32| {
        let entry = touch_slot(touches, place);
        match entry.block {
            None => {
                entry.block = Some(block);
                entry.last_ordinal = ordinal;
            }
            Some(existing) if existing == block => {
                if ordinal > entry.last_ordinal {
                    entry.last_ordinal = ordinal;
                }
            }
            Some(_) => entry.ineligible = true,
        }
    };
    for param in &machine.structural_parameters {
        mark_ineligible(&mut touches, param.place);
    }
    for (_, loc) in contents {
        mark_ineligible(&mut touches, loc.root);
    }
    for block in &machine.blocks {
        for (ordinal, operation) in block.operations.iter().enumerate() {
            let Ok(ordinal) = u32::try_from(ordinal) else {
                continue;
            };
            if let Some(output) = operation.result.as_structural() {
                record(&mut touches, output.place, block.id, ordinal);
            }
            match &operation.kind {
                OperationKind::EstablishReference { source, .. }
                | OperationKind::EstablishAccessReference { source, .. }
                | OperationKind::EstablishElementView { source, .. }
                | OperationKind::EstablishMutableElementView { source, .. } => {
                    mark_ineligible(&mut touches, source.place);
                }
                _ => {}
            }
            for arg in operation.kind.structural_arguments() {
                record(&mut touches, arg.place, block.id, ordinal);
            }
            if (operation.declared_invocation().is_some()
                || operation.kind.callable_invocation().is_some()
                || matches!(operation.kind, OperationKind::CallDynamic { .. }))
                && operation.result.as_structural().is_some_and(|result| {
                    terminal_psi::structural_type_has_borrowed_storage_by(
                        |id| module.structural_type(id),
                        result.structural_type,
                    )
                })
            {
                for arg in operation.kind.structural_arguments() {
                    mark_ineligible(&mut touches, arg.place);
                }
            }
        }
    }
    for (&(block, ordinal), step) in flow {
        for input in step.observed_inputs() {
            record(&mut touches, input.root, block, ordinal);
            if step.may_escape {
                mark_ineligible(&mut touches, input.root);
            }
        }
        for def in &step.definitions {
            record(&mut touches, def.root, block, ordinal);
        }
        for (src, dst) in step.derivations.iter().chain(&step.authority_derivations) {
            record(&mut touches, src.root, block, ordinal);
            record(&mut touches, dst.root, block, ordinal);
        }
        for edge in &step.successors {
            for loc in edge.parameters.iter().chain(&edge.definitions) {
                mark_ineligible(&mut touches, loc.root);
            }
            for (_, effect) in &edge.effects {
                for loc in &effect.definitions {
                    mark_ineligible(&mut touches, loc.root);
                }
                for (src, dst) in effect
                    .derivations
                    .iter()
                    .chain(&effect.authority_derivations)
                {
                    mark_ineligible(&mut touches, src.root);
                    mark_ineligible(&mut touches, dst.root);
                }
            }
            for binding in &edge.bindings {
                mark_ineligible(&mut touches, binding.parameter.root);
                if let Some(src) = binding.source.exact_location() {
                    mark_ineligible(&mut touches, src.root);
                }
            }
        }
    }
    let stops: Vec<Option<DescriptorPosition>> = touches
        .iter()
        .map(|entry| {
            if entry.ineligible {
                return None;
            }
            let block = entry.block?;
            let last_pos = (block, entry.last_ordinal);
            let next_pos = (block, entry.last_ordinal.checked_add(1)?);
            let step = flow.get(&last_pos)?;
            if step.successors.len() == 1
                && step.successors[0].is_plain_fallthrough()
                && step.successors[0].target == next_pos
                && predecessors.count(next_pos) == 1
            {
                Some(next_pos)
            } else {
                None
            }
        })
        .collect();
    DeadTailCutoff::new(true, stops)
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
    let predecessors = FlowPredecessors::new(flow);
    let cutoff = build_dead_tail_cutoff(module, machine, flow, &predecessors, &contents);
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
        let inventory = DescriptorSlotKind::BorrowedStorage
            .owned_inventory(
                &module.types.structural_types,
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
        // Only known descriptor leaves can receive an individual release.
        // Unknown contents stay with the whole carrier; its ownership walk
        // still owes every possible claim and contained loan.
        for slot in inventory.slots {
            lifetimes.push(descriptor_entry_lifetime_with_contents_indexed(
                flow,
                &predecessors,
                Some(&cutoff),
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
            let inventory = DescriptorSlotKind::BorrowedStorage
                .owned_inventory(
                    &module.types.structural_types,
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
            let retained =
                crate::references::reference_completion::reference_call_lifetimes_indexed(
                    module,
                    machine,
                    operation,
                    analysis,
                    Some(&predecessors),
                    Some(&cutoff),
                    Some(birth),
                )
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
                        &predecessors,
                        cutoff.is_acyclic(),
                        birth,
                        &retained.lifetime,
                        &source,
                        &mut dependencies,
                    )
                    .map_err(|detail| Error::local(machine.id, Some(operation.id), detail))?;
                    lifetimes.push(retained.lifetime);
                }
            }
            for slot in inventory.slots {
                let lifetime = descriptor_location_lifetime_with_contents_indexed(
                    flow,
                    &predecessors,
                    Some(&cutoff),
                    birth,
                    &slot,
                    needs_contents,
                );
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
                        &predecessors,
                        cutoff.is_acyclic(),
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
            if locations.is_empty() {
                continue;
            }
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
    let mut resolved_paths = BTreeMap::<DescriptorLocation, Option<_>>::new();
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
            if !resolved_paths.contains_key(&location) {
                let root_type = prepared
                    .place_types
                    .get(&location.root)
                    .copied()
                    .ok_or_else(|| {
                        Error::local(machine.id, None, "reference disposition lost its root type")
                    })?;
                let path =
                    descriptor_path(&module.types.structural_types, root_type, &location.path)
                        .ok_or_else(|| {
                            Error::local(
                                machine.id,
                                None,
                                "reference disposition has no exact owned path",
                            )
                        })?;
                let resolved = if !crate::projection_stays_in_inline_storage(
                    module.types.structural_types.iter(),
                    root_type,
                    &path,
                ) {
                    None
                } else {
                    let Some(tip) = crate::runtime_structural_path_tip(
                        module.types.structural_types.iter(),
                        root_type,
                        &path,
                    ) else {
                        return Err(Error::local(
                            machine.id,
                            None,
                            "reference disposition lost its leaf type",
                        ));
                    };
                    tip.shape.is_borrowed_descriptor().then_some(path)
                };
                resolved_paths.insert(location.clone(), resolved);
            }
            let Some(path) = resolved_paths[&location].clone() else {
                continue;
            };
            for (position, segment) in location.path.iter().enumerate() {
                let crate::descriptor_lifetimes::DescriptorPathSegment::Case(case) = segment else {
                    continue;
                };
                let predicate = crate::descriptor_results::DescriptorCasePredicate {
                    subject: DescriptorLocation {
                        root: location.root,
                        path: location.path[..position].to_vec(),
                    }
                    .into(),
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

#[allow(clippy::too_many_arguments)]
fn add_dependencies(
    module: &TerminalModule,
    prepared: &crate::descriptor_results::DescriptorMachine,
    flow: &BTreeMap<DescriptorPosition, crate::descriptor_lifetimes::DescriptorFlowStep>,
    predecessors: &FlowPredecessors<'_>,
    acyclic: bool,
    birth: DescriptorPosition,
    lifetime: &crate::descriptor_lifetimes::DescriptorLifetime,
    source: &StructuralArgument,
    dependencies: &mut BTreeMap<
        DescriptorPosition,
        BTreeSet<(DescriptorLocation, DescriptorLocation)>,
    >,
) -> Result<(), String> {
    if lifetime.authorized.is_empty() {
        return Ok(());
    }
    let same_block_max = if acyclic {
        let mut max_ord = birth.1;
        let mut all_same_block = true;
        for &(block, ord) in lifetime.authorized.keys() {
            if block != birth.0 {
                all_same_block = false;
                break;
            }
            if ord > max_ord {
                max_ord = ord;
            }
        }
        all_same_block.then_some((birth.0, max_ord))
    } else {
        None
    };
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
        &module.types.structural_types,
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
            .owned_slots(&module.types.structural_types, source.place, source_type)
            .ok_or("reference parent has no finite descriptor inventory")?
            .into_iter()
            .filter(|parent| captured.contains(parent))
            .collect()
    };
    for parent in parents {
        let homes = descriptor_continuation_locations_indexed(
            flow,
            predecessors,
            same_block_max,
            birth,
            &parent,
        );
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
