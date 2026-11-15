//! Derive explicit local reference dispositions from the unsealed resource flow.
//!
//! Backward demand chooses a possible cut. Forward carrier custody, immediate
//! parents and storage dependencies decide whether a release can be proposed.
//! The affine checker independently checks the carrier and captured storage at
//! each emitted release; this plan does not establish absent referent contents.

use std::collections::{BTreeMap, BTreeSet};

use semantic_vocabulary::{MachineId, OperationId};
use terminal_psi::{
    Operation, OperationKind, StructuralAccess, StructuralArgument, StructuralPathSegment,
    StructuralTypeShape, TerminalMachine, TerminalModule, Terminator,
};

use crate::descriptor_lifetimes::{
    DescriptorFlowStep, DescriptorLifetime, DescriptorLocation, DescriptorPosition,
    descriptor_entry_lifetime, descriptor_location, descriptor_location_lifetime, descriptor_path,
};
use crate::descriptor_results::{
    DescriptorAnalysis, DescriptorSlotKind, terminal_reference_graph_from,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceRelease {
    pub at: DescriptorPosition,
    pub descriptor: DescriptorLocation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub at: Option<DescriptorPosition>,
    pub detail: &'static str,
}

impl Error {
    fn at(at: DescriptorPosition, detail: &'static str) -> Self {
        Self {
            at: Some(at),
            detail,
        }
    }
}

type Flow = BTreeMap<DescriptorPosition, DescriptorFlowStep>;
type Names = BTreeSet<DescriptorLocation>;

// These are existing occurrence coordinates, not newly minted loan identities.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Occurrence {
    Incoming(DescriptorLocation),
    Formation(OperationId),
    CallInput(OperationId, usize),
}

struct Resource {
    identity: Occurrence,
    birth: DescriptorPosition,
    entry: bool,
    seeds: Vec<DescriptorLocation>,
    source: Option<StructuralArgument>,
    captured: Option<DescriptorLocation>,
    access: StructuralAccess,
    // Authority lineage and borrowing a carrier's own storage are separate
    // reasons that the referenced local resource must remain undisposed.
    dependencies: Vec<usize>,
}

impl Resource {
    fn lifetime(&self, flow: &Flow) -> DescriptorLifetime {
        let mut result = DescriptorLifetime {
            live: BTreeSet::new(),
            demanded: BTreeMap::new(),
            authorized: BTreeMap::new(),
            surviving: BTreeMap::new(),
        };
        for seed in &self.seeds {
            let lifetime = if self.entry {
                descriptor_entry_lifetime(flow, self.birth, seed)
            } else {
                descriptor_location_lifetime(flow, self.birth, seed)
            };
            result.live.extend(lifetime.live);
            for (at, names) in lifetime.demanded {
                result.demanded.entry(at).or_default().extend(names);
            }
            for (at, names) in lifetime.authorized {
                result.authorized.entry(at).or_default().extend(names);
            }
            for (at, names) in lifetime.surviving {
                result.surviving.entry(at).or_default().extend(names);
            }
        }
        result
    }
}

fn spelling(
    module: &TerminalModule,
    machine: &TerminalMachine,
    argument: &StructuralArgument,
) -> Option<DescriptorLocation> {
    descriptor_location(
        &module.structural_types,
        argument.place,
        machine.structural_place_type(argument.place)?,
        &argument.path,
        None,
    )
}

fn overlaps(left: &DescriptorLocation, right: &DescriptorLocation) -> bool {
    left.root == right.root && crate::storage::place_paths::may_overlap(&left.path, &right.path)
}

fn owned_reference(
    module: &TerminalModule,
    machine: &TerminalMachine,
    location: &DescriptorLocation,
) -> bool {
    let Some(source) = crate::structural_argument_source(machine, location.root, 0) else {
        return false;
    };
    if source.access != StructuralAccess::Owned {
        return false;
    }
    let Some(path) = descriptor_path(
        &module.structural_types,
        source.structural_type,
        &location.path,
    ) else {
        return false;
    };
    crate::projection_stays_in_inline_storage(
        module.structural_types.iter(),
        source.structural_type,
        &path,
    ) && crate::runtime_structural_path_tip(
        module.structural_types.iter(),
        source.structural_type,
        &path,
    )
    .is_some_and(|row| matches!(row.shape, StructuralTypeShape::Reference { .. }))
}

fn block_at(
    machine: &TerminalMachine,
    identity: semantic_vocabulary::BlockId,
) -> Option<&terminal_psi::Block> {
    let position = usize::try_from(identity.get()).ok()?.checked_sub(1)?;
    machine
        .blocks
        .get(position)
        .filter(|block| block.id == identity)
}

fn operation_at(machine: &TerminalMachine, at: DescriptorPosition) -> Option<&Operation> {
    block_at(machine, at.0)?.operations.get(at.1 as usize)
}

fn normal_return(machine: &TerminalMachine, at: DescriptorPosition) -> bool {
    block_at(machine, at.0).is_some_and(|block| {
        at.1 as usize == block.operations.len()
            && matches!(
                block.terminator,
                Terminator::Return { .. }
                    | Terminator::ReturnUnit { .. }
                    | Terminator::ReturnStructural { .. }
            )
    })
}

fn prepare(
    module: &TerminalModule,
    machine: &TerminalMachine,
    analysis: &mut DescriptorAnalysis<'_>,
    flow: &Flow,
) -> Result<Vec<Resource>, Error> {
    let mut resources = Vec::new();
    for parameter in &machine.structural_parameters {
        for descriptor in DescriptorSlotKind::References
            .slots(
                &module.structural_types,
                parameter.place,
                parameter.structural_type,
            )
            .ok_or(Error::at(
                (machine.entry, 0),
                "incoming reference leaves have no exact finite inventory",
            ))?
        {
            resources.push(Resource {
                identity: Occurrence::Incoming(descriptor.clone()),
                birth: (machine.entry, 0),
                entry: true,
                seeds: vec![descriptor],
                source: None,
                captured: None,
                access: StructuralAccess::SharedBorrow,
                dependencies: Vec::new(),
            });
        }
    }
    for block in &machine.blocks {
        for (ordinal, operation) in block.operations.iter().enumerate() {
            let at = (
                block.id,
                u32::try_from(ordinal).map_err(|_| {
                    Error::at((block.id, 0), "reference formation ordinal overflows")
                })?,
            );
            match &operation.kind {
                OperationKind::EstablishReference { source, .. }
                | OperationKind::EstablishAccessReference { source, .. }
                | OperationKind::EstablishElementView { source, .. }
                | OperationKind::EstablishMutableElementView { source, .. } => {
                    let result = operation.result.as_structural().ok_or(Error::at(
                        at,
                        "reference formation has no actual result carrier",
                    ))?;
                    resources.push(Resource {
                        identity: Occurrence::Formation(operation.id),
                        birth: at,
                        entry: false,
                        seeds: vec![DescriptorLocation::whole(result.place)],
                        source: Some(source.clone()),
                        captured: None,
                        access: source.access,
                        dependencies: Vec::new(),
                    });
                }
                _ => {}
            }
            for (input, lifetime) in
                crate::reference_call_lifetimes(module, machine, operation, analysis)
                    .map_err(|detail| Error::at(at, detail))?
            {
                let source = operation
                    .kind
                    .structural_arguments()
                    .get(input)
                    .ok_or(Error::at(
                        at,
                        "retained call resource lost its actual input",
                    ))?;
                let after = (
                    block.id,
                    at.1.checked_add(1)
                        .ok_or(Error::at(at, "reference call cut overflows"))?,
                );
                let seeds = lifetime.surviving.get(&after).cloned().unwrap_or_default();
                if seeds.is_empty() || lifetime.authorized.get(&after) != Some(&seeds) {
                    return Err(Error::at(
                        at,
                        "retained call resource has no exact complete output cohort",
                    ));
                }
                resources.push(Resource {
                    identity: Occurrence::CallInput(operation.id, input),
                    birth: at,
                    entry: false,
                    seeds: seeds.into_iter().collect(),
                    source: Some(source.clone()),
                    captured: None,
                    access: source.access,
                    dependencies: Vec::new(),
                });
            }
        }
    }
    let lifetimes = resources
        .iter()
        .map(|resource| resource.lifetime(flow))
        .collect::<Vec<_>>();
    for position in 0..resources.len() {
        if !resources[position].entry
            && lifetimes[position]
                .live
                .contains(&resources[position].birth)
        {
            return Err(Error::at(
                resources[position].birth,
                "reference occurrence crosses its own formation",
            ));
        }
        let Some(source) = resources[position].source.clone() else {
            continue;
        };
        let at = resources[position].birth;
        let source_location = spelling(module, machine, &source).ok_or(Error::at(
            at,
            "reference formation has no exact source place",
        ))?;
        let captured = analysis
            .location(machine.id, at, &source_location)
            .ok_or(Error::at(
                at,
                "reference formation has no exact captured storage",
            ))?;
        let mut dependencies = Vec::new();
        if let Some(crossing) = source
            .path
            .iter()
            .rposition(|part| *part == StructuralPathSegment::Referent)
        {
            let descriptor = spelling(
                module,
                machine,
                &StructuralArgument {
                    place: source.place,
                    path: source.path[..crossing].to_vec(),
                    access: source.access,
                },
            )
            .ok_or(Error::at(at, "reborrow has no exact immediate carrier"))?;
            let possible = lifetimes
                .iter()
                .enumerate()
                .filter(|(candidate, life)| {
                    *candidate != position
                        && life
                            .surviving
                            .get(&at)
                            .is_some_and(|names| names.contains(&descriptor))
                })
                .map(|(candidate, _)| candidate)
                .collect::<Vec<_>>();
            if possible.len() > 1
                || possible.first().is_some_and(|candidate| {
                    !lifetimes[*candidate]
                        .authorized
                        .get(&at)
                        .is_some_and(|names| names.contains(&descriptor))
                })
            {
                return Err(Error::at(
                    at,
                    "immediate reference parent has only possible carrier custody",
                ));
            }
            let parent = crate::reference_loan_parent(
                module,
                machine,
                analysis,
                at,
                &descriptor,
                resources
                    .iter()
                    .enumerate()
                    .filter(|(candidate, _)| *candidate != position)
                    .map(|(candidate, resource)| (&resource.identity, &lifetimes[candidate])),
            )
            .map_err(|detail| Error::at(at, detail))?;
            if let Some(parent) = parent {
                dependencies.push(
                    resources
                        .iter()
                        .position(|resource| &resource.identity == parent)
                        .ok_or(Error::at(at, "reference parent occurrence is absent"))?,
                );
            }
        }
        for (candidate, lifetime) in lifetimes.iter().enumerate() {
            if candidate != position
                && lifetime
                    .surviving
                    .get(&at)
                    .is_some_and(|names| names.iter().any(|name| overlaps(name, &captured)))
            {
                dependencies.push(candidate);
            }
        }
        dependencies.sort_unstable();
        dependencies.dedup();
        resources[position].captured = Some(captured);
        resources[position].dependencies = dependencies;
    }
    Ok(resources)
}

fn ancestry(resources: &[Resource]) -> Result<Vec<Vec<usize>>, Error> {
    let mut result = Vec::with_capacity(resources.len());
    for (position, resource) in resources.iter().enumerate() {
        let mut parents = Vec::new();
        let mut pending = resource.dependencies.clone();
        while let Some(parent) = pending.pop() {
            if parent == position {
                return Err(Error::at(
                    resource.birth,
                    "reference resource dependencies are cyclic",
                ));
            }
            if parents.contains(&parent) {
                continue;
            }
            parents.push(parent);
            pending.extend_from_slice(&resources[parent].dependencies);
        }
        parents.sort_unstable();
        result.push(parents);
    }
    Ok(result)
}

// A receiving home is required, but the descriptor it displaces is not read.
// Keep the RHS and each crossed reference carrier as genuine value demands.
fn store_demands(
    module: &TerminalModule,
    machine: &TerminalMachine,
    analysis: &mut DescriptorAnalysis<'_>,
    flow: &mut Flow,
) -> Result<(), Error> {
    for block in &machine.blocks {
        for (ordinal, operation) in block.operations.iter().enumerate() {
            let OperationKind::StoreStructuralField {
                destination,
                path,
                value,
                ..
            } = &operation.kind
            else {
                continue;
            };
            let at = (
                block.id,
                u32::try_from(ordinal)
                    .map_err(|_| Error::at((block.id, 0), "store ordinal overflows"))?,
            );
            let source = spelling(module, machine, value)
                .ok_or(Error::at(at, "reference store RHS has no exact carrier"))?;
            let mut inputs = vec![
                source.clone(),
                analysis
                    .location(machine.id, at, &source)
                    .ok_or(Error::at(at, "reference store RHS backing is unresolved"))?,
            ];
            for crossing in path.iter().enumerate().filter_map(|(ordinal, segment)| {
                (*segment == StructuralPathSegment::Referent).then_some(ordinal)
            }) {
                let descriptor = spelling(
                    module,
                    machine,
                    &StructuralArgument {
                        place: *destination,
                        path: path[..crossing].to_vec(),
                        access: StructuralAccess::MutableBorrow,
                    },
                )
                .ok_or(Error::at(
                    at,
                    "reference store crosses an unresolved permission",
                ))?;
                inputs.push(descriptor);
            }
            inputs.sort();
            inputs.dedup();
            flow.get_mut(&at)
                .ok_or(Error::at(at, "reference store has no flow cut"))?
                .inputs = inputs;
        }
    }
    Ok(())
}

fn transfer(
    names: &Names,
    definitions: impl Iterator<Item = DescriptorLocation>,
    derivations: impl Iterator<Item = (DescriptorLocation, DescriptorLocation)>,
) -> Names {
    let definitions = definitions.collect::<Vec<_>>();
    let mut after = names
        .iter()
        .filter(|name| {
            !definitions
                .iter()
                .any(|definition| definition.contains(name))
        })
        .cloned()
        .collect::<Names>();
    for (source, destination) in derivations {
        after.extend(
            names
                .iter()
                .filter_map(|name| name.transported(&source, &destination)),
        );
    }
    after
}

fn consuming_call(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
    name: &DescriptorLocation,
) -> bool {
    let consumes = |argument: &StructuralArgument, access, multiplicity| {
        argument.access == StructuralAccess::Owned
            && access == StructuralAccess::Owned
            && multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
            && spelling(module, machine, argument).is_some_and(|source| source.contains(name))
    };
    if let Some(call) = operation.declared_invocation() {
        let Some(parameters) =
            module.invocation_structural_parameters(call.callee, call.required_contract)
        else {
            return false;
        };
        call.structural_arguments
            .iter()
            .zip(parameters)
            .any(|(argument, parameter)| {
                consumes(argument, parameter.access, parameter.multiplicity)
            })
    } else if let OperationKind::CallDynamic {
        interface,
        slot,
        structural_arguments,
        ..
    } = &operation.kind
    {
        let Some(slot) = module
            .dynamic_dispatch
            .interface(*interface)
            .and_then(|interface| interface.slots.get(*slot as usize))
        else {
            return false;
        };
        structural_arguments
            .iter()
            .zip(&slot.structural_parameters)
            .any(|(argument, parameter)| {
                consumes(argument, parameter.access, parameter.multiplicity)
            })
    } else {
        false
    }
}

type ReverseFlow = BTreeMap<DescriptorPosition, Vec<DescriptorPosition>>;

fn reverse_flow(flow: &Flow) -> ReverseFlow {
    let mut reverse = ReverseFlow::new();
    for (position, step) in flow {
        for edge in &step.successors {
            reverse.entry(edge.target).or_default().push(*position);
        }
    }
    reverse
}

fn predecessors(
    reverse: &ReverseFlow,
    seeds: impl Iterator<Item = DescriptorPosition>,
) -> BTreeSet<DescriptorPosition> {
    let mut pending = seeds.collect::<Vec<_>>();
    let mut reached = BTreeSet::new();
    while let Some(at) = pending.pop() {
        if reached.insert(at) {
            pending.extend(reverse.get(&at).into_iter().flatten().copied());
        }
    }
    reached
}

fn output_protection(
    machine: &TerminalMachine,
    reverse: &ReverseFlow,
    lifetimes: &[DescriptorLifetime],
    parents: &[Vec<usize>],
) -> Vec<BTreeSet<DescriptorPosition>> {
    let mut protected = lifetimes
        .iter()
        .map(|lifetime| {
            predecessors(
                reverse,
                machine.blocks.iter().filter_map(|block| {
                    let Terminator::ReturnStructural { source, .. } = block.terminator else {
                        return None;
                    };
                    let at = (block.id, u32::try_from(block.operations.len()).ok()?);
                    lifetime
                        .surviving
                        .get(&at)
                        .is_some_and(|names| names.iter().any(|name| name.root == source))
                        .then_some(at)
                }),
            )
        })
        .collect::<Vec<_>>();
    for (child, ancestors) in parents.iter().enumerate() {
        let demand = protected[child].clone();
        for parent in ancestors {
            protected[*parent].extend(&demand);
        }
    }
    protected
}

fn observed_storage(
    module: &TerminalModule,
    machine: &TerminalMachine,
    analysis: &mut DescriptorAnalysis<'_>,
    at: DescriptorPosition,
    operation: &Operation,
) -> Result<Vec<DescriptorLocation>, Error> {
    use terminal_psi::StructuralStorageUse as Use;
    let mut locations = Vec::new();
    let field_use = match &operation.kind {
        OperationKind::StructuralScalarFieldStore {
            destination,
            path,
            field,
            ..
        }
        | OperationKind::StructuralByteSequenceFieldByteStore {
            destination,
            path,
            field,
            ..
        } => Some((*destination, path.as_slice(), Some(*field))),
        OperationKind::StructuralByteSequenceFieldLength {
            source,
            path,
            field,
        }
        | OperationKind::StructuralByteSequenceFieldRead {
            source,
            path,
            field,
            ..
        } => Some((*source, path.as_slice(), Some(*field))),
        OperationKind::StructuralByteSequenceFieldStore {
            destination,
            path,
            field,
            source,
            ..
        } => {
            locations.push(DescriptorLocation::whole(*source));
            Some((*destination, path.as_slice(), Some(*field)))
        }
        OperationKind::StructuralByteViewFieldStore {
            destination,
            path,
            field,
            source,
            ..
        } => {
            locations.push(DescriptorLocation::whole(*source));
            Some((*destination, path.as_slice(), *field))
        }
        OperationKind::MoveStructuralField {
            source,
            path,
            field,
        } => Some((*source, path.as_slice(), *field)),
        OperationKind::StoreStructuralField {
            destination,
            path,
            field,
            value,
            ..
        } => {
            locations.push(
                spelling(module, machine, value)
                    .ok_or(Error::at(at, "store RHS has no exact observed place"))?,
            );
            Some((*destination, path.as_slice(), *field))
        }
        _ => None,
    };
    if let Some((root, path, field)) = field_use {
        locations.push(
            descriptor_location(
                &module.structural_types,
                root,
                machine
                    .structural_place_type(root)
                    .ok_or(Error::at(at, "field use has no root type"))?,
                path,
                field,
            )
            .ok_or(Error::at(at, "field use has no exact selected place"))?,
        );
    } else if let OperationKind::ScalarStructuralField {
        source,
        path,
        field,
    }
    | OperationKind::BooleanStructuralField {
        source,
        path,
        field,
    } = &operation.kind
    {
        let mut path = path.clone();
        path.push(semantic_vocabulary::CanonicalStructuralPathSegment::Field(
            *field,
        ));
        locations.push(
            crate::descriptor_lifetimes::canonical_descriptor_location(
                &module.structural_types,
                *source,
                machine
                    .structural_place_type(*source)
                    .ok_or(Error::at(at, "field observation has no root type"))?,
                &path,
            )
            .ok_or(Error::at(
                at,
                "field observation has no exact selected place",
            ))?,
        );
    } else {
        for used in operation.kind.storage_uses() {
            let location = match used {
                Use::Place(root) => Some(DescriptorLocation::whole(root)),
                Use::Projection(projection)
                | Use::Element {
                    descriptor: projection,
                    ..
                } => spelling(
                    module,
                    machine,
                    &StructuralArgument {
                        place: projection.root,
                        path: projection.path.to_vec(),
                        access: StructuralAccess::SharedBorrow,
                    },
                ),
                Use::Canonical { root, path } | Use::ObservedField { root, path } => {
                    crate::descriptor_lifetimes::canonical_descriptor_location(
                        &module.structural_types,
                        root,
                        machine
                            .structural_place_type(root)
                            .ok_or(Error::at(at, "observation has no root type"))?,
                        path,
                    )
                }
                Use::Type(_) => continue,
            };
            locations.push(location.ok_or(Error::at(
                at,
                "reference closure observation has unresolved storage",
            ))?);
        }
    }
    locations
        .into_iter()
        .map(|location| {
            analysis
                .location(machine.id, at, &location)
                .ok_or(Error::at(
                    at,
                    "reference closure observation has unresolved backing",
                ))
        })
        .collect()
}

fn closure_demand(
    module: &TerminalModule,
    machine: &TerminalMachine,
    analysis: &mut DescriptorAnalysis<'_>,
    flow: &Flow,
    reverse: &ReverseFlow,
    resources: &[Resource],
    lifetimes: &[DescriptorLifetime],
    parents: &[Vec<usize>],
) -> Result<Vec<BTreeSet<DescriptorPosition>>, Error> {
    let mut demands = vec![BTreeSet::new(); resources.len()];
    for (&at, step) in flow {
        let observations = operation_at(machine, at)
            .map(|operation| observed_storage(module, machine, analysis, at, operation))
            .transpose()?
            .unwrap_or_default();
        for (position, resource) in resources.iter().enumerate() {
            let lifetime = &lifetimes[position];
            let before = lifetime.surviving.get(&at).cloned().unwrap_or_default();
            let loses_name = before.iter().any(|name| {
                step.definitions
                    .iter()
                    .any(|definition| definition.contains(name))
                    && !step
                        .authority_derivations
                        .iter()
                        .any(|(source, destination)| {
                            name.transported(source, destination).is_some()
                        })
            });
            let reused = resource.access != StructuralAccess::SharedBorrow
                && !lifetime.live.contains(&at)
                && !lifetimes.iter().enumerate().any(|(child, life)| {
                    parents[child].contains(&position) && life.live.contains(&at)
                })
                && resource.captured.as_ref().is_some_and(|captured| {
                    observations
                        .iter()
                        .any(|observation| overlaps(captured, observation))
                });
            let reenters_formation = !resource.entry
                && !before.is_empty()
                && step
                    .successors
                    .iter()
                    .any(|edge| edge.target == resource.birth);
            if normal_return(machine, at) || loses_name || reused || reenters_formation {
                demands[position].insert(at);
            }
        }
    }
    // A child's disposition must precede renewed use of an ancestor carrier.
    for (child, ancestors) in parents.iter().enumerate() {
        for parent in ancestors {
            demands[child].extend(&lifetimes[*parent].live);
        }
    }
    Ok(demands
        .into_iter()
        .map(|demand| predecessors(reverse, demand.into_iter()))
        .collect())
}

/// Propose ordinary `ReleaseReference` operations. Selected normal returns keep
/// their output ancestry for the existing capture/cleanup completion transaction.
/// The caller must materialize the operations and run independent affine and
/// resource checking; neither a dead name nor this plan restores authority.
pub fn plan(module: &TerminalModule, identity: MachineId) -> Result<Vec<ReferenceRelease>, Error> {
    let machine = module.machine(identity).ok_or(Error {
        at: None,
        detail: "reference release machine is absent",
    })?;
    if machine.blocks.is_empty() {
        return Ok(Vec::new());
    }
    if !machine
        .structural_places
        .iter()
        .filter_map(|place| machine.structural_place_type(place.id))
        .any(|ty| {
            terminal_psi::structural_type_contains_reference_by(|id| module.structural_type(id), ty)
        })
    {
        return Ok(Vec::new());
    }
    let graph = terminal_reference_graph_from(module, [identity]).map_err(|_| Error {
        at: None,
        detail: "reference release graph is incomplete",
    })?;
    let mut analysis = DescriptorAnalysis::with_reference_slots(&module.structural_types, &graph);
    let mut flow = analysis.flow(identity).ok_or(Error {
        at: None,
        detail: "reference release flow is unresolved",
    })?;
    let resources = prepare(module, machine, &mut analysis, &flow)?;
    if let Some(result) = machine.result.structural().filter(|result| {
        terminal_psi::structural_type_contains_reference_by(
            |id| module.structural_type(id),
            result.structural_type,
        )
    }) {
        let slots = analysis.result_slots(identity).ok_or(Error {
            at: None,
            detail: "reference output has no actual source correspondence",
        })?;
        let actual = crate::reference_result_sources(
            &module.structural_types,
            &machine.structural_parameters,
            result,
            &slots,
        )
        .map_err(|detail| Error { at: None, detail })?;
        if actual != result.reference_sources {
            return Err(Error {
                at: None,
                detail: "reference output differs from its actual sources",
            });
        }
    }
    if resources.is_empty() {
        return Ok(Vec::new());
    }
    store_demands(module, machine, &mut analysis, &mut flow)?;
    let parents = ancestry(&resources)?;
    let mut order = (0..resources.len()).collect::<Vec<_>>();
    order.sort_by_key(|position| std::cmp::Reverse(parents[*position].len()));
    let original = flow.clone();
    let original_lifetimes = resources
        .iter()
        .map(|resource| resource.lifetime(&original))
        .collect::<Vec<_>>();
    let owned_names = original_lifetimes
        .iter()
        .flat_map(|lifetime| lifetime.surviving.values().flatten())
        .cloned()
        .collect::<Names>()
        .into_iter()
        .filter(|name| owned_reference(module, machine, name))
        .collect::<Names>();
    let reverse = reverse_flow(&original);
    let protected = output_protection(machine, &reverse, &original_lifetimes, &parents);
    let demand = closure_demand(
        module,
        machine,
        &mut analysis,
        &original,
        &reverse,
        &resources,
        &original_lifetimes,
        &parents,
    )?;
    let mut releases = Vec::<ReferenceRelease>::new();
    // Every iteration adds a finite carrier definition. Recompute forward
    // custody after those real proposed dispositions, including branch joins.
    loop {
        let lifetimes = resources
            .iter()
            .map(|resource| resource.lifetime(&flow))
            .collect::<Vec<_>>();
        let mut additions = Vec::new();
        for (&at, step) in &flow {
            if step.successors.is_empty() {
                continue;
            }
            let mut closed_here = vec![false; resources.len()];
            for &position in &order {
                let lifetime = &lifetimes[position];
                let Some(may) = lifetime
                    .surviving
                    .get(&at)
                    .filter(|names| !names.is_empty())
                else {
                    continue;
                };
                if !demand[position].contains(&at)
                    || protected[position].contains(&at)
                    || lifetime.authorized.get(&at) != Some(may)
                {
                    continue;
                }
                if lifetimes.iter().enumerate().any(|(child, life)| {
                    parents[child].contains(&position)
                        && !closed_here[child]
                        && life
                            .surviving
                            .get(&at)
                            .is_some_and(|names| !names.is_empty())
                }) {
                    continue;
                }
                let descriptors = may
                    .iter()
                    .filter(|name| {
                        owned_names.contains(*name)
                            && !lifetime
                                .demanded
                                .get(&at)
                                .is_some_and(|demanded| demanded.contains(*name))
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                for descriptor in &descriptors {
                    if !releases
                        .iter()
                        .chain(&additions)
                        .any(|release: &ReferenceRelease| {
                            release.at == at && release.descriptor == *descriptor
                        })
                    {
                        additions.push(ReferenceRelease {
                            at,
                            descriptor: descriptor.clone(),
                        });
                    }
                }
                closed_here[position] = !descriptors.is_empty() && descriptors.len() == may.len();
            }
        }
        if additions.is_empty() {
            break;
        }
        for release in additions {
            flow.get_mut(&release.at)
                .ok_or(Error::at(
                    release.at,
                    "proposed reference release has no cut",
                ))?
                .definitions
                .push(release.descriptor.clone());
            releases.push(release);
        }
    }
    let proposed_lifetimes = resources
        .iter()
        .map(|resource| resource.lifetime(&flow))
        .collect::<Vec<_>>();
    releases.retain(|release| {
        proposed_lifetimes.iter().any(|lifetime| {
            lifetime.surviving.get(&release.at).is_some_and(|names| {
                names.contains(&release.descriptor)
                    && lifetime.authorized.get(&release.at) == Some(names)
            })
        })
    });
    // A virtual definition at a later redundant cut is not a disposition.
    // Remove it before auditing the actual returned proposal, including a
    // branch where only one arrival had already closed this resource.
    flow = original.clone();
    for release in &releases {
        flow.get_mut(&release.at)
            .ok_or(Error::at(
                release.at,
                "retained reference release has no cut",
            ))?
            .definitions
            .push(release.descriptor.clone());
    }
    let final_lifetimes = resources
        .iter()
        .map(|resource| resource.lifetime(&flow))
        .collect::<Vec<_>>();
    // A retained exclusive resource must have an actual local disposition
    // before an operation resumes access to its captured lender storage.
    for &at in flow.keys() {
        let Some(operation) = operation_at(machine, at) else {
            continue;
        };
        let observations = observed_storage(module, machine, &mut analysis, at, operation)?;
        for (position, resource) in resources.iter().enumerate() {
            let lifetime = &final_lifetimes[position];
            if resource.access == StructuralAccess::SharedBorrow
                || lifetime.live.contains(&at)
                || final_lifetimes.iter().enumerate().any(|(child, life)| {
                    parents[child].contains(&position) && life.live.contains(&at)
                })
            {
                continue;
            }
            if !resource.captured.as_ref().is_some_and(|captured| {
                observations
                    .iter()
                    .any(|observation| overlaps(captured, observation))
            }) {
                continue;
            }
            let Some(names) = lifetime.surviving.get(&at) else {
                continue;
            };
            if names.iter().any(|name| {
                owned_names.contains(name)
                    && !releases
                        .iter()
                        .any(|release| release.at == at && release.descriptor == *name)
            }) {
                return Err(Error::at(
                    at,
                    "exclusive lender reuse has no authenticated complete reference release",
                ));
            }
        }
    }
    for (position, lifetime) in final_lifetimes.iter().enumerate() {
        for (&at, names) in &lifetime.surviving {
            let Some(Operation {
                kind: OperationKind::ReleaseReference { source, path },
                ..
            }) = operation_at(machine, at)
            else {
                continue;
            };
            let released = spelling(
                module,
                machine,
                &StructuralArgument {
                    place: *source,
                    path: path.clone(),
                    access: StructuralAccess::Owned,
                },
            )
            .ok_or(Error::at(at, "actual release has no exact owned leaf"))?;
            if !names.contains(&released) {
                continue;
            }
            for (child, child_lifetime) in final_lifetimes.iter().enumerate() {
                if !parents[child].contains(&position) {
                    continue;
                }
                if child_lifetime.surviving.get(&at).is_some_and(|names| {
                    names.iter().any(|name| {
                        !releases
                            .iter()
                            .any(|release| release.at == at && release.descriptor == *name)
                    })
                }) {
                    return Err(Error::at(
                        at,
                        "actual parent release precedes complete child disposition",
                    ));
                }
            }
        }
    }
    // Neither an erased member nor an empty common name set proves that a
    // cohort was disposed. Audit each lost carrier on the selected operation.
    for (position, lifetime) in final_lifetimes.iter().enumerate() {
        let resource = &resources[position];
        for (&at, step) in &flow {
            let Some(before) = lifetime
                .surviving
                .get(&at)
                .filter(|names| !names.is_empty())
            else {
                continue;
            };
            for name in before {
                let after = transfer(
                    &Names::from([name.clone()]),
                    step.definitions.iter().cloned(),
                    step.authority_derivations.iter().cloned(),
                );
                if !after.is_empty() {
                    continue;
                }
                let actual_release = operation_at(machine, at).is_some_and(|operation| {
                    matches!(operation.kind, OperationKind::ReleaseReference { .. })
                        || consuming_call(module, machine, operation, name)
                });
                let planned = releases
                    .iter()
                    .any(|release| release.at == at && release.descriptor == *name);
                if !actual_release && !planned {
                    return Err(Error::at(
                        at,
                        "reference custody disappears without an actual disposition",
                    ));
                }
            }
            for (ordinal, edge) in step.successors.iter().enumerate() {
                let after = transfer(
                    before,
                    step.definitions.iter().cloned(),
                    step.authority_derivations.iter().cloned(),
                );
                for name in after {
                    let arrived = transfer(
                        &Names::from([name.clone()]),
                        edge.parameters.iter().chain(&edge.definitions).cloned(),
                        edge.derivations()
                            .map(|(source, destination)| (source.clone(), destination.clone())),
                    );
                    if !resource.entry && edge.target == resource.birth && !arrived.is_empty() {
                        return Err(Error::at(
                            at,
                            "reference resource reenters its formation before disposition",
                        ));
                    }
                    if !arrived.is_empty() {
                        continue;
                    }
                    let cleanup = block_at(machine, at.0)
                        .filter(|block| at.1 as usize == block.operations.len())
                        .and_then(|block| block.terminator.cleanup_rosters().nth(ordinal));
                    let disposed = cleanup.is_some_and(|(_, actions)| {
                        machine
                            .structural_place_type(name.root)
                            .and_then(|ty| {
                                descriptor_path(&module.structural_types, ty, &name.path)
                            })
                            .is_some_and(|path| {
                                crate::cleanup_disposes_reference(actions, name.root, &path)
                            })
                    });
                    if !disposed {
                        return Err(Error::at(
                            at,
                            "selected edge erases reference custody without transport or cleanup",
                        ));
                    }
                }
            }
        }
    }
    releases.sort_by(|left, right| {
        left.at.cmp(&right.at).then_with(|| {
            let depth = |release: &ReferenceRelease| {
                original_lifetimes
                    .iter()
                    .enumerate()
                    .filter(|(_, lifetime)| {
                        lifetime
                            .surviving
                            .get(&release.at)
                            .is_some_and(|names| names.contains(&release.descriptor))
                    })
                    .map(|(position, _)| parents[position].len())
                    .max()
                    .unwrap_or(0)
            };
            depth(right)
                .cmp(&depth(left))
                .then_with(|| left.descriptor.cmp(&right.descriptor))
        })
    });
    releases.dedup();
    Ok(releases)
}
