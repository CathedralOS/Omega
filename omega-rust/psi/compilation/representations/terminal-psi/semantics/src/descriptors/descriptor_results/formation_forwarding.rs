//! Borrowing a saved pointer requires its complete current duties.
//! Keep the conditional provider requirements that this actual use needs,
//! including providers reached through ordinary helper summaries. This query
//! records obligations; it does not choose a provider or establish its promise.

use super::*;
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
    let mut required = module.boundary_reference_forwarding.clone();
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
                        collect_origin(module, &origin, &mut required)?;
                    }
                }
            }
        }
    }
    required.sort_by_key(|row| (row.machine, row.operation, row.output));
    for row in &mut required {
        row.sources.sort();
        row.sources.dedup();
    }
    Ok(required)
}

fn collect_origin(
    module: &TerminalModule,
    origin: &DescriptorSourceOrigin,
    required: &mut Vec<BoundaryReferenceForwardingDemand>,
) -> Result<(), &'static str> {
    let DescriptorSourceOrigin::Provider(provider) = origin else {
        return Ok(());
    };
    let declaration = module
        .boundary_machine(provider.boundary)
        .ok_or("the required provider output has no public declaration")?;
    let output = declaration
        .reference_outputs
        .get(usize::try_from(provider.output).map_err(|_| "the provider output exceeds usize")?)
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
        let body = module
            .machine(site.machine)
            .ok_or("the provider invocation has no machine")?;
        let mut operations = body
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter(|operation| operation.id == site.operation);
        let operation = operations
            .next()
            .ok_or("the provider invocation operation is absent")?;
        if operations.next().is_some() {
            return Err("the provider invocation operation is ambiguous");
        }
        if !matches!(&operation.kind, OperationKind::BoundaryCall { boundary, .. } if *boundary == provider.boundary)
        {
            continue;
        }
        found = true;
        if let Some(row) = required.iter_mut().find(|row| {
            row.machine == site.machine
                && row.operation == site.operation
                && row.output == provider.output
        }) {
            // Separate uses require both bounds. Union would weaken an older
            // condition and could silently admit a previously excluded source.
            row.sources.retain(|source| sources.contains(source));
            if row.sources.is_empty() {
                return Err("the reference uses require incompatible forwarding choices");
            }
        } else {
            required.push(BoundaryReferenceForwardingDemand {
                machine: site.machine,
                operation: site.operation,
                output: provider.output,
                sources: sources.clone(),
            });
        }
    }
    if !found {
        return Err("the provider origin has no matching boundary invocation");
    }
    if let Some(sources) = &provider.forwarded_sources {
        for source in sources {
            match source {
                DescriptorForwardingSource::Value(origin)
                | DescriptorForwardingSource::Address(origin) => {
                    collect_origin(module, origin, required)?
                }
                DescriptorForwardingSource::ReferenceArgument { .. } => {
                    return Err("a completed provider retains an unresolved forwarding argument");
                }
            }
        }
    }
    Ok(())
}
