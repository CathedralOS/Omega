//! Snapshot backing separately from the reference's executable source spelling.
//! The shared current-body descriptor replay owns stores, calls and arrivals.
//! This audit neither replays a source initializer nor invents a view origin.
//! Error closures retain the rejecting call site; passing the track-caller
//! constructor as a function pointer reports Rust's adapter instead.
use super::*;
use std::borrow::Cow;
use terminal_semantics::borrowed_storage::{Projection, Region};
use terminal_semantics::descriptor_lifetimes::{DescriptorPosition, descriptor_path};
use terminal_semantics::descriptor_results::{
    DescriptorAnalysis, DescriptorCallOrigin, DescriptorSourceOrigin, descriptor_machine,
    descriptor_slot_inventory,
};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Backing {
    Source(Root),
    Descriptor {
        // Incoming slot contents remain distinct from the slot's own storage.
        // Geometry is conservative; it grants no authority or disjointness.
        origin: DescriptorSourceOrigin,
        geometry: DescriptorCallOrigin,
        address: Option<Vec<StructuralPathSegment>>,
    },
}

type Observation = (OperationId, PlaceId, Vec<StructuralPathSegment>);
#[derive(Default)]
pub(super) struct Observations {
    sources: BTreeMap<Observation, Option<Vec<Backing>>>,
    positions: BTreeMap<OperationId, DescriptorPosition>,
    ordering: Option<terminal_semantics::scalar_ordering::ScalarOrdering>,
}

impl std::fmt::Debug for Observations {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Observations")
            .field("sources", &self.sources)
            .field("positions", &self.positions)
            .finish_non_exhaustive()
    }
}

pub(super) fn prepare(
    function: &PsiOptimizationFunction,
    native: &target_operations::TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<Observations, LegalizationError> {
    let types = &plan.structural_types[..];
    if !function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .any(|node| {
            node.operation
                .structural_result()
                .is_some_and(|(_, result)| contains_reference(types, result.structural_type))
        })
    {
        return Ok(Observations::default());
    }
    let graph = optimization_unit_semantics::descriptor_results::optimized_catalog_graph(
        &unit.functions,
        &unit.boundary_machines,
        types,
        &unit.dynamic_dispatch,
        |caller, node| {
            super::super::call_origin::selected_boundary_operation(node, caller, native, plan)
                .ok()
                .map(|selected| match selected {
                    Some((operation, _)) => Cow::Owned(operation),
                    None => Cow::Borrowed(&node.operation),
                })
        },
    );
    let mut observations = Observations::default();
    let functions = unit
        .functions
        .iter()
        .map(|function| (function.machine, function))
        .collect();
    observations.ordering = Some(
        optimization_unit_semantics::descriptor_results::captured_scalar_ordering(
            function, &functions,
        ),
    );
    // An unavailable replay is not an empty origin set. A descriptor-dependent
    // formation below requires its exact observation and therefore fails closed.
    let Some(graph) = graph else {
        return Ok(observations);
    };
    let current = descriptor_machine(&graph, function.machine).ok_or_else(|| invalid())?;
    let mut analysis = DescriptorAnalysis::new(types, &graph);
    for block in &function.blocks {
        for (node_index, node) in block.nodes.iter().enumerate() {
            let Some(operation) = node.operation.psi_operation() else {
                continue;
            };
            let position = (block.id, u32::try_from(node_index).map_err(|_| invalid())?);
            if observations.positions.insert(operation, position).is_some() {
                return Err(invalid());
            }
            let step = current.flow.get(&position).ok_or_else(|| invalid())?;
            let mut slots = BTreeSet::new();
            for input in &step.inputs {
                let Some(ty) = current.place_types.get(&input.root) else {
                    continue;
                };
                // Snapshot materialized slots; opaque siblings remain unknown
                // in the shared analysis, not a reason to erase this input.
                slots.extend(
                    descriptor_slot_inventory(types, input.root, *ty)
                        .map(|inventory| inventory.slots)
                        .unwrap_or_default(),
                );
            }
            for location in slots {
                let ty = *current
                    .place_types
                    .get(&location.root)
                    .ok_or_else(|| invalid())?;
                let path = descriptor_path(types, ty, &location.path).ok_or_else(|| invalid())?;
                let origins = analysis
                    .symbolic_source_origins(function.machine, position, &location)
                    .and_then(|origins| {
                        origins
                            .into_iter()
                            .map(|origin| {
                                let geometry =
                                    analysis.flatten_source_origin(function.machine, &origin)?;
                                Some(Backing::Descriptor {
                                    origin,
                                    geometry,
                                    address: None,
                                })
                            })
                            .collect()
                    });
                observations
                    .sources
                    .insert((operation, location.root, path), origins);
            }
        }
    }
    Ok(observations)
}

pub(super) fn capture(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &[StructuralTypeDeclaration],
    source: &StructuralArgument,
    operation: OperationId,
    root: &Root,
) -> Result<(Vec<Backing>, Vec<super::OccurrenceIdentity>), LegalizationError> {
    let ty = source_contract(function, custody, source.place).ok_or_else(|| invalid())?;
    // Formation already authenticated the complete source projection. Inspect
    // only its materialized prefixes: an intermediate Case selector has no
    // endpoint, and unrelated reference fields are not part of this borrow.
    // Enumerating the whole type here would demand an unbounded catalog for a
    // growing reference edge even when the operation never follows that edge.
    // Naming a descriptor itself borrows its storage; only a proper prefix
    // crosses into the backing captured by that descriptor instance. Capture
    // follows descriptor storage, not shared-read eligibility: exclusive views
    // also retain backing origins. Formation authenticates access separately.
    let descriptor = (0..source.path.len()).find_map(|length| {
        let path = &source.path[..length];
        let carrier = leaf_type(types, ty, path)?;
        terminal_semantics::descriptor_results::is_descriptor(types.iter(), carrier)
            .then(|| path.to_vec())
    });
    if let Some(path) = descriptor {
        let mut backing = custody
            .descriptor_backing
            .sources
            .get(&(operation, source.place, path.clone()))
            .and_then(Option::as_ref)
            .filter(|origins| !origins.is_empty())
            .cloned()
            .ok_or_else(|| invalid())?;
        let position = *custody
            .descriptor_backing
            .positions
            .get(&operation)
            .ok_or_else(|| invalid())?;
        let mut parents = Vec::new();
        for origin in &mut backing {
            if let Backing::Descriptor {
                geometry:
                    DescriptorCallOrigin::Argument {
                        source: backing_root,
                        projection,
                    },
                address,
                ..
            } = origin
            {
                // Keep captured windows intact: a relative selector cannot
                // refine an inexact parent into another storage coordinate.
                *projection =
                    projection.then(&Projection::new(&source.path[path.len()..]).at(position));
                *address = projection.fixed_address(
                    custody
                        .descriptor_backing
                        .ordering
                        .as_ref()
                        .ok_or_else(|| invalid())?,
                );
                // A view formed through a live reference retains that parent
                // even though its runtime descriptor is independently owned.
                parents.extend(custody.leaves.iter().filter_map(|(location, leaf)| {
                    (location.0 == *backing_root && projection.path.starts_with(&location.1))
                        .then_some(leaf.identity.clone())
                }));
            }
        }
        return Ok((backing, parents));
    }
    if let Some(crossing) = source
        .path
        .iter()
        .rposition(|part| *part == StructuralPathSegment::Referent)
        && let Some(leaf) = custody.leaf(source.place, &source.path[..crossing])
    {
        return Ok((leaf.backing.clone(), Vec::new()));
    }
    Ok((vec![Backing::Source(root.clone())], Vec::new()))
}

pub(super) fn overlaps(
    backing: &Backing,
    machine: semantic_vocabulary::MachineId,
    place: PlaceId,
    path: &[StructuralPathSegment],
    activation_local: bool,
    origin_overlaps: &impl Fn(&[terminal_psi::ReferenceOriginProjection]) -> bool,
) -> bool {
    match backing {
        Backing::Source(Root::Place(root)) => *root == place,
        Backing::Source(Root::Ingress(root, origin)) => {
            *root == place
                && origin_overlaps(
                    &origin
                        .iter()
                        .cloned()
                        .map(terminal_psi::ReferenceOriginProjection::Exact)
                        .collect::<Vec<_>>(),
                )
        }
        Backing::Source(Root::Alternatives(origins)) => origins
            .iter()
            .any(|origin| origin.root == place && origin_overlaps(&origin.path)),
        // The captured value is outside this activation's owned inline homes.
        // Keep its conservative geometry for accesses to external storage;
        // ordinary in-body backing still protects its actual owned region.
        Backing::Descriptor {
            origin:
                DescriptorSourceOrigin::IncomingSlot(_) | DescriptorSourceOrigin::IncomingContents(_),
            ..
        } if activation_local => false,
        Backing::Descriptor { geometry, .. } => match geometry {
            // Formation tokens carry no disjoint physical storage evidence.
            DescriptorCallOrigin::PlacedFormation { .. } => true,
            DescriptorCallOrigin::Empty => false,
            DescriptorCallOrigin::Literal {
                machine: owner,
                place: root,
            } => *owner == machine && *root == place,
            DescriptorCallOrigin::Argument { source, projection } => Region {
                root: Some(*source),
                projection: projection.clone(),
            }
            .overlaps(&Region {
                root: Some(place),
                projection: Projection::new(path),
            }),
        },
    }
}

/// Exact return identity, separate from conservative storage interference.
/// Every possible captured origin must name an admitted formal coordinate.
pub(super) fn matches_return(backing: &[Backing], declared: &impl Fn(&Root) -> bool) -> bool {
    !backing.is_empty()
        && backing.iter().all(|origin| match origin {
            Backing::Source(root) => declared(root),
            Backing::Descriptor {
                origin: DescriptorSourceOrigin::Backing(DescriptorCallOrigin::Argument { .. }),
                geometry: DescriptorCallOrigin::Argument { source, .. },
                address: Some(path),
            } => declared(&if path.is_empty() {
                Root::Place(*source)
            } else {
                Root::Ingress(*source, path.clone())
            }),
            _ => false,
        })
}
