//! Descriptor liveness supplies disposition candidates, not permission to
//! dispose storage. The affine storage walk resolves these against its exact
//! available owned regions, then ordinary ReleaseReference operations carry
//! the result to the checker, interpreter and native lowering.

use crate::descriptor_lifetimes::{
    DescriptorLocation, DescriptorPosition, descriptor_location_lifetime, descriptor_path,
};
use crate::descriptor_results::{DescriptorAnalysis, DescriptorSlotKind};
use semantic_vocabulary::{IdTable, MachineId, OperationId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{StructuralAccess, StructuralArgument, TerminalModule};

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

pub fn candidates(module: &TerminalModule) -> Result<IdTable<MachineId, Candidates>, Error> {
    let has_references = |machine: &terminal_psi::TerminalMachine| {
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
                terminal_psi::structural_type_contains_reference_by(
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
            .filter(|machine| has_references(machine) && !machine.blocks.is_empty())
            .map(|machine| machine.id),
        &[],
    )
    .map_err(|error| Error {
        machine: error.machine(),
        operation: error.operation(module),
        detail: error.in_module(module),
    })?;
    graph.sort_unstable_by_key(|machine| machine.id);
    let mut analysis =
        DescriptorAnalysis::with_borrowed_storage_slots(&module.structural_types, &graph);
    let mut result = IdTable::default();
    for machine in &module.machines {
        let candidates = if machine.blocks.is_empty() || !has_references(machine) {
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
            let flow = analysis.flow(machine.id).map_err(|error| Error {
                machine: error.machine(),
                operation: error.operation(module),
                detail: error.detail().map_or_else(
                    || {
                        format!(
                            "reference disposition flow is incomplete for {:?}",
                            machine.id
                        )
                    },
                    str::to_owned,
                ),
            })?;
            machine_candidates(module, machine, prepared, &flow, &mut analysis).map_err(
                |detail| Error {
                    machine: machine.id,
                    operation: None,
                    detail,
                },
            )?
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

fn machine_candidates(
    module: &TerminalModule,
    machine: &terminal_psi::TerminalMachine,
    prepared: &crate::descriptor_results::DescriptorMachine,
    flow: &BTreeMap<DescriptorPosition, crate::descriptor_lifetimes::DescriptorFlowStep>,
    analysis: &mut DescriptorAnalysis<'_>,
) -> Result<Candidates, String> {
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
        let slots = DescriptorSlotKind::References
            .owned_slots(
                &module.structural_types,
                parameter.place,
                parameter.structural_type,
            )
            .ok_or("incoming reference disposition has no finite owned slot inventory")?;
        for slot in slots {
            lifetimes.push(crate::descriptor_lifetimes::descriptor_entry_lifetime(
                flow,
                (machine.entry, 0),
                &slot,
            ));
        }
    }
    for block in &machine.blocks {
        for (position, operation) in block.operations.iter().enumerate() {
            let Some(output) = operation.result.as_structural() else {
                continue;
            };
            let slots = DescriptorSlotKind::References
                .owned_slots(
                    &module.structural_types,
                    output.place,
                    output.structural_type,
                )
                .ok_or_else(|| {
                    format!(
                        "reference disposition has no finite owned slot inventory at {:?}",
                        operation.id
                    )
                })?;
            let birth = (
                block.id,
                u32::try_from(position).map_err(|_| "reference formation position overflows")?,
            );
            let retained = crate::reference_call_lifetimes(module, machine, operation, analysis)?;
            if !retained.is_empty() {
                let call = operation
                    .declared_invocation()
                    .ok_or("retained reference disposition lost its invocation")?;
                for (input, lifetime) in retained {
                    let source = call
                        .structural_arguments
                        .get(input)
                        .ok_or("retained reference disposition lost its exact input")?;
                    add_dependencies(
                        module,
                        prepared,
                        flow,
                        birth,
                        &lifetime,
                        source,
                        &mut dependencies,
                    )?;
                    lifetimes.push(lifetime);
                }
            }
            for slot in slots {
                let lifetime = descriptor_location_lifetime(flow, birth, &slot);
                if let terminal_psi::OperationKind::EstablishReference { source, .. } =
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
                    )?;
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
                .ok_or("reference disposition lost its root type")?;
            let path = descriptor_path(&module.structural_types, root_type, &location.path)
                .ok_or("reference disposition has no exact owned path")?;
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
                return Err("reference disposition lost its leaf type".into());
            };
            if !matches!(
                tip.shape,
                terminal_psi::StructuralTypeShape::Reference { .. }
            ) {
                continue;
            }
            let mut unproved_case = None;
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
                if analysis.case_is_established(machine.id, at, &predicate) {
                    continue;
                }
                if analysis.case_feasibility(machine.id, at, &predicate)
                    == crate::descriptor_results::FlowFeasibility::ProvedImpossible
                {
                    continue 'locations;
                }
                unproved_case = Some(predicate);
            }
            selected.insert(
                location.clone(),
                Candidate {
                    argument: StructuralArgument {
                        place: location.root,
                        path,
                        access: StructuralAccess::Owned,
                    },
                    unproved_case,
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
            return Err("reference disposition has cyclic descriptor-storage ancestry".into());
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
    let mut captured = crate::descriptor_lifetimes::descriptor_location(
        &module.structural_types,
        source.place,
        source_type,
        &source.path,
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
        DescriptorSlotKind::References
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
