//! Borrowing a saved pointer requires its complete current duties.
//! Keep the conditional provider requirements that this actual use needs,
//! including providers reached through ordinary helper summaries. This query
//! records obligations; it does not choose a provider or establish its promise.

use super::*;
use semantic_vocabulary::{BoundaryMachineId, OperationId};
use terminal_psi::{
    BoundaryLifetimeDependency, BoundaryReferenceForwardingDemand, BoundaryReferenceSource,
};

/// Producer-side requirements for actual exclusive reference formations.
/// Receiving validation must still reject bytes that omitted a needed row;
/// it must never call this query to repair an incoming artifact.
pub fn formation_reference_forwarding_requirements(
    module: &TerminalModule,
) -> Result<Vec<BoundaryReferenceForwardingDemand>, &'static str> {
    let graph = terminal_borrowed_storage_graph(module);
    let mut analysis =
        DescriptorAnalysis::with_borrowed_storage_slots(&module.structural_types, &graph);
    let mut required: Option<ForwardingRequirements<'_>> = None;
    for body in &module.machines {
        let Some(descriptor) = graph.iter().find(|candidate| candidate.id == body.id) else {
            continue;
        };
        let mut history = PayloadMembershipHistory::for_module_preservation(module, &graph, body);
        for block in &body.blocks {
            for (ordinal, operation) in block.operations.iter().enumerate() {
                let OperationKind::EstablishReference { source, storage } = &operation.kind else {
                    continue;
                };
                if !matches!(
                    source.access,
                    StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
                ) {
                    continue;
                }
                let Some(result) = operation.result.structural() else {
                    continue;
                };
                let position = (
                    block.id,
                    u32::try_from(ordinal)
                        .map_err(|_| "the reference formation ordinal exceeds u32")?,
                );
                let Some(root_type) = descriptor.place_types.get(&source.place).copied() else {
                    continue;
                };
                let path = crate::reference_source_storage_path(
                    module.structural_types.iter(),
                    root_type,
                    &source.path,
                );
                let Some(selected) = descriptor_lifetimes::descriptor_location(
                    &module.structural_types,
                    source.place,
                    root_type,
                    path,
                    None,
                ) else {
                    continue;
                };
                let Some(carrier) = descriptor_lifetimes::descriptor_location_type(
                    &module.structural_types,
                    root_type,
                    &selected.path,
                ) else {
                    continue;
                };
                let Some(inventory) = DescriptorSlotKind::BorrowedStorage.inventory(
                    &module.structural_types,
                    selected.root,
                    carrier,
                ) else {
                    continue;
                };
                let references = inventory
                    .slots
                    .into_iter()
                    .filter(|slot| {
                        descriptor_lifetimes::descriptor_location_type(
                            &module.structural_types,
                            carrier,
                            &slot.path,
                        )
                        .and_then(|tip| {
                            terminal_psi::find_structural_type(&module.structural_types, tip)
                        })
                        .is_some_and(|tip| {
                            matches!(tip.shape, StructuralTypeShape::Reference { .. })
                        })
                    })
                    .collect::<Vec<_>>();
                if references.is_empty() {
                    continue;
                }
                if history
                    .reference_formation_memberships(
                        &module.identities,
                        &mut analysis,
                        position,
                        source,
                        storage.as_ref(),
                        result,
                    )
                    .is_ok()
                {
                    continue;
                }
                let mut reads = BTreeSet::new();
                for (index, segment) in selected.path.iter().enumerate() {
                    if *segment == DescriptorPathSegment::Referent {
                        reads.insert(DescriptorLocation {
                            root: selected.root,
                            path: selected.path[..index].to_vec(),
                        });
                    }
                }
                {
                    for slot in references {
                        let mut read = selected.clone();
                        read.path.extend(slot.path);
                        reads.insert(read);
                    }
                }
                for read in reads {
                    let Some(origins) = analysis.symbolic_source_origins(body.id, position, &read)
                    else {
                        continue;
                    };
                    for origin in origins {
                        if !matches!(origin, DescriptorSourceOrigin::Provider(_)) {
                            continue;
                        }
                        required
                            .get_or_insert_with(|| ForwardingRequirements::new(module))
                            .collect_origin(&origin)?;
                    }
                }
            }
        }
    }
    let mut required = required.map_or_else(
        || module.boundary_reference_forwarding.clone(),
        |collection| collection.required,
    );
    required.sort_by_key(|row| (row.machine, row.operation, row.output));
    for row in &mut required {
        row.sources.sort();
        row.sources.dedup();
    }
    Ok(required)
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct ForwardingRequestCoordinate {
    machine: MachineId,
    operation: OperationId,
    output: u32,
}

/// Index the immutable module and the growing request roster at this collector's
/// scope. Operation rows retain every duplicate within a machine; request keys
/// name an invocation output, rather than an arena position or a raw-ID domain.
struct ForwardingRequirements<'module> {
    module: &'module TerminalModule,
    machines: Vec<(MachineId, usize)>,
    boundaries: Vec<(BoundaryMachineId, usize)>,
    operations: Vec<(usize, OperationId, usize, usize)>,
    required: Vec<BoundaryReferenceForwardingDemand>,
    positions: BTreeMap<ForwardingRequestCoordinate, usize>,
}

impl<'module> ForwardingRequirements<'module> {
    fn new(module: &'module TerminalModule) -> Self {
        let mut machines = Vec::with_capacity(module.machines.len());
        let mut operations = Vec::new();
        for (machine_index, machine) in module.machines.iter().enumerate() {
            machines.push((machine.id, machine_index));
            for (block_index, block) in machine.blocks.iter().enumerate() {
                for (operation_index, operation) in block.operations.iter().enumerate() {
                    operations.push((machine_index, operation.id, block_index, operation_index));
                }
            }
        }
        machines.sort_unstable();
        operations.sort_unstable();
        let mut boundaries = module
            .boundary_machines
            .iter()
            .enumerate()
            .map(|(index, boundary)| (boundary.id, index))
            .collect::<Vec<_>>();
        boundaries.sort_unstable();
        let required = module.boundary_reference_forwarding.clone();
        let mut positions = BTreeMap::new();
        for (index, row) in required.iter().enumerate() {
            // Preserve the original first-row intersection rule. Independent
            // module validation still owns duplicate declaration/request rows.
            positions
                .entry(ForwardingRequestCoordinate {
                    machine: row.machine,
                    operation: row.operation,
                    output: row.output,
                })
                .or_insert(index);
        }
        Self {
            module,
            machines,
            boundaries,
            operations,
            required,
            positions,
        }
    }

    fn boundary(
        &self,
        id: BoundaryMachineId,
    ) -> Result<&'module terminal_psi::BoundaryMachineDeclaration, &'static str> {
        let index = self.boundaries.partition_point(|row| row.0 < id);
        self.boundaries
            .get(index)
            .filter(|row| row.0 == id)
            .map(|row| &self.module.boundary_machines[row.1])
            .ok_or("the required provider output has no public declaration")
    }

    fn operation(
        &self,
        site: &DescriptorInvocationSite,
    ) -> Result<&'module Operation, &'static str> {
        let index = self.machines.partition_point(|row| row.0 < site.machine);
        let machine_index = self
            .machines
            .get(index)
            .filter(|row| row.0 == site.machine)
            .map(|row| row.1)
            .ok_or("the provider invocation has no machine")?;
        let key = (machine_index, site.operation);
        let first = self.operations.partition_point(|row| (row.0, row.1) < key);
        let last = self.operations.partition_point(|row| (row.0, row.1) <= key);
        match &self.operations[first..last] {
            [(_, _, block, operation)] => {
                Ok(&self.module.machines[machine_index].blocks[*block].operations[*operation])
            }
            [] => Err("the provider invocation operation is absent"),
            _ => Err("the provider invocation operation is ambiguous"),
        }
    }

    fn collect_origin(&mut self, origin: &DescriptorSourceOrigin) -> Result<(), &'static str> {
        let DescriptorSourceOrigin::Provider(provider) = origin else {
            return Ok(());
        };
        let declaration = self.boundary(provider.boundary)?;
        let output = declaration
            .reference_outputs
            .get(
                usize::try_from(provider.output)
                    .map_err(|_| "the provider output exceeds usize")?,
            )
            .ok_or("the required provider output is absent")?;
        let mut sources = output
            .dependencies
            .iter()
            .map(|source| match source {
                BoundaryLifetimeDependency::Referent { argument_index, .. } => {
                    BoundaryReferenceSource::Reference {
                        argument_index: *argument_index,
                    }
                }
                BoundaryLifetimeDependency::Stored {
                    argument_index,
                    path,
                    ..
                } => BoundaryReferenceSource::Stored {
                    argument_index: *argument_index,
                    path: path.clone(),
                },
            })
            .collect::<Vec<_>>();
        sources.sort();
        sources.dedup();
        if sources.is_empty() {
            return Err("the opaque output has no captured choices for a forwarding requirement");
        }
        let mut found = false;
        for site in &provider.invocations {
            let operation = self.operation(site)?;
            if !matches!(&operation.kind, OperationKind::BoundaryCall { boundary, .. } if *boundary == provider.boundary)
            {
                continue;
            }
            found = true;
            let coordinate = ForwardingRequestCoordinate {
                machine: site.machine,
                operation: site.operation,
                output: provider.output,
            };
            if let Some(index) = self.positions.get(&coordinate) {
                let row = &mut self.required[*index];
                // Separate uses require both bounds. Union would weaken an older
                // condition and could silently admit a previously excluded source.
                row.sources.retain(|source| sources.contains(source));
                if row.sources.is_empty() {
                    return Err("the reference uses require incompatible forwarding choices");
                }
            } else {
                let index = self.required.len();
                self.required.push(BoundaryReferenceForwardingDemand {
                    machine: site.machine,
                    operation: site.operation,
                    output: provider.output,
                    sources: sources.clone(),
                });
                self.positions.insert(coordinate, index);
            }
        }
        if !found {
            return Err("the provider origin has no matching boundary invocation");
        }
        if let Some(sources) = &provider.forwarded_sources {
            for source in sources {
                match source {
                    DescriptorForwardingSource::Value(origin)
                    | DescriptorForwardingSource::Address(origin) => self.collect_origin(origin)?,
                    DescriptorForwardingSource::ReferenceArgument { .. } => {
                        return Err(
                            "a completed provider retains an unresolved forwarding argument",
                        );
                    }
                }
            }
        }
        Ok(())
    }
}
