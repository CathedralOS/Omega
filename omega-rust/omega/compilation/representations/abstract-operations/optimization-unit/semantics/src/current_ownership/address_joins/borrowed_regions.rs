//! Possible backing regions shared by address joins and descriptor loans.
//!
//! Reconstruct the borrowed alias graph once. Every possible origin can cause
//! interference; only exact prefixes compose with later projections. Unknown
//! references, dynamic selectors and unresolved cycles remain conservative.

use super::range_relations::RangeRelations;
use abstract_operations::AbstractOperation as O;
use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::{MachineId, PlaceId, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{StructuralAccess, StructuralPathSegment, StructuralTypeDeclaration};

use terminal_semantics::borrowed_storage::{AliasSources, resolve};
pub(in crate::current_ownership) use terminal_semantics::borrowed_storage::{
    Position, Projection, Region,
};
use terminal_semantics::descriptor_lifetimes::{DescriptorFlowStep, DescriptorLocation};
use terminal_semantics::descriptor_results::{
    DescriptorAnalysis, DescriptorCallOrigin, DescriptorSourceOrigin, DescriptorStoredCallEffect,
};

pub(in crate::current_ownership) type BorrowedReferenceSources =
    BTreeMap<PlaceId, Option<Vec<terminal_psi::StructuralArgument>>>;

type ObservationKey = (Position, PlaceId, Vec<StructuralPathSegment>);

pub(in crate::current_ownership) struct Access {
    pub place: PlaceId,
    pub regions: Vec<Region>,
    pub location: Option<DescriptorLocation>,
}

#[derive(Default)]
pub(in crate::current_ownership) struct BorrowedRegions {
    // Exact incoming storage is distinct from conservative overlap geometry.
    // An unresolved borrowed binding retains None, never its own phantom root.
    reference_sources: BorrowedReferenceSources,
    aliases: AliasSources,
    // Exact immediate parent edges are distinct from transitive backing. The
    // address-join suspension judgment must recognize a fresh child of a join.
    parents: AliasSources,
    returned: BTreeMap<PlaceId, Vec<DescriptorCallOrigin>>,
    sources: BTreeMap<ObservationKey, Vec<DescriptorCallOrigin>>,
    descriptor_sources: BTreeSet<ObservationKey>,
    symbolic_sources: BTreeMap<ObservationKey, Vec<(DescriptorCallOrigin, DescriptorSourceOrigin)>>,
    locations: BTreeMap<ObservationKey, DescriptorLocation>,
    effects: BTreeMap<Position, Vec<DescriptorStoredCallEffect>>,
    flow: Option<BTreeMap<Position, DescriptorFlowStep>>,
    relations: Option<RangeRelations>,
    writes: BTreeMap<Position, Vec<Access>>,
}

impl BorrowedRegions {
    pub(in crate::current_ownership) fn new(
        function: &PsiOptimizationFunction,
        functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        call_accesses: &crate::current_ownership::CallAccesses,
    ) -> Self {
        let mut result = Self::origins(function, functions, call_accesses.boundaries(), types);
        let mut writes = BTreeMap::new();
        for block in &function.blocks {
            for (index, node) in block.nodes.iter().enumerate() {
                let regions = call_accesses
                    .written_regions(function, types, &node.operation)
                    .into_iter()
                    .map(|(place, region)| {
                        let mut projection =
                            Projection::new(&region.path).at((block.id, index as u32));
                        projection.indirect |= region.indirect;
                        Access {
                            place,
                            location: result
                                .access_location((block.id, index as u32), place, &projection.path)
                                .cloned(),
                            regions: result.resolve_at((block.id, index as u32), place, projection),
                        }
                    })
                    .collect::<Vec<_>>();
                if !regions.is_empty() {
                    writes.insert(
                        (
                            block.id,
                            u32::try_from(index).expect("unit node index fits u32"),
                        ),
                        regions,
                    );
                }
            }
        }
        result.relations = result
            .aliases
            .values()
            .flatten()
            .any(|(_, projection)| projection.window.is_some())
            .then(|| RangeRelations::new(function, functions));
        result.writes = writes;
        result
    }

    pub(in crate::current_ownership) fn origins(
        function: &PsiOptimizationFunction,
        functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
        boundaries: &[&terminal_psi::BoundaryMachineDeclaration],
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    ) -> Self {
        // Origin and cohort queries share one disposable current-body replay.
        // Cache only locations actually selected or carried by this function.
        let declarations = types.values().copied().cloned().collect::<Vec<_>>();
        let mut catalog = functions.values().copied().collect::<Vec<_>>();
        if !functions.contains_key(&function.machine) {
            catalog.push(function);
        }
        let graph = crate::descriptor_results::optimized_catalog_graph(
            catalog,
            boundaries.iter().copied(),
            &declarations,
            |_, node| Some(std::borrow::Cow::Borrowed(&node.operation)),
        );
        let mut reference_sources = BorrowedReferenceSources::new();
        for parameter in function.block_structural_parameters() {
            if parameter.access != StructuralAccess::Owned
                && !terminal_semantics::descriptor_results::is_shared_descriptor(
                    declarations.iter(),
                    parameter.structural_type,
                )
            {
                reference_sources.insert(parameter.place, None);
            }
        }
        let mut returned = BTreeMap::new();
        let mut sources = BTreeMap::new();
        let mut descriptor_sources = BTreeSet::new();
        let mut symbolic_sources = BTreeMap::new();
        let mut locations = BTreeMap::new();
        let mut effects = BTreeMap::new();
        let mut flow = None;
        if let Some(graph) = &graph {
            let mut analysis = DescriptorAnalysis::new(&declarations, graph);
            flow = analysis.flow(function.machine);
            if let Some(current) =
                terminal_semantics::descriptor_results::descriptor_machine(graph, function.machine)
            {
                for block in &function.blocks {
                    for parameter in &block.structural_parameters {
                        if !reference_sources.contains_key(&parameter.place) {
                            continue;
                        }
                        let origins = analysis
                            .storage_origins(
                                function.machine,
                                (block.id, 0),
                                &DescriptorLocation::whole(parameter.place),
                            )
                            .and_then(|origins| {
                                origins
                                    .into_iter()
                                    .map(|origin| {
                                        let path = terminal_semantics::descriptor_lifetimes::descriptor_path(
                                            &declarations,
                                            *current.place_types.get(&origin.root)?,
                                            &origin.path,
                                        )?;
                                        Some(terminal_psi::StructuralArgument {
                                            place: origin.root,
                                            path,
                                            access: parameter.access,
                                        })
                                    })
                                    .collect()
                            });
                        reference_sources.insert(parameter.place, origins);
                    }
                }
                let mut queries: BTreeMap<Position, BTreeSet<DescriptorLocation>> = BTreeMap::new();
                for (position, step) in &current.flow {
                    let observed = queries.entry(*position).or_default();
                    observed.extend(step.inputs.iter().cloned());
                    observed.extend(step.definitions.iter().cloned());
                    for (source, destination) in &step.derivations {
                        observed.insert(source.clone());
                        observed.insert(destination.clone());
                    }
                    for edge in &step.successors {
                        observed.extend(edge.bindings.iter().map(|(source, _)| source.clone()));
                    }
                    // A cohort is born after its defining operation. Unit
                    // field stores have no structural result place, but their
                    // destination slot still needs its post-store symbolic
                    // contents for incoming-slot compatibility replay.
                    let completed = queries.entry((position.0, position.1 + 1)).or_default();
                    completed.extend(step.definitions.iter().cloned());
                    completed.extend(
                        step.derivations
                            .iter()
                            .map(|(_, destination)| destination.clone()),
                    );
                }
                for block in &function.blocks {
                    for (index, node) in block.nodes.iter().enumerate() {
                        let position = (block.id, index as u32);
                        let mut roots =
                            crate::unit_validation::function_structure::operation_place_inputs(
                                &node.operation,
                            );
                        if let Some((_, result)) = node.operation.structural_result() {
                            roots.push(result.place);
                            queries
                                .entry((position.0, position.1 + 1))
                                .or_default()
                                .insert(DescriptorLocation::whole(result.place));
                        }
                        let observed = queries.entry(position).or_default();
                        for root in roots {
                            observed.insert(DescriptorLocation::whole(root));
                            if let Some(ty) = current.place_types.get(&root)
                                && let Some(slots) =
                                    terminal_semantics::descriptor_results::descriptor_slots(
                                        &declarations,
                                        root,
                                        *ty,
                                    )
                            {
                                observed.extend(slots);
                            }
                        }
                        if let Some(actual_effects) =
                            analysis.call_stored_effects(function.machine, position)
                        {
                            queries
                                .entry((position.0, position.1 + 1))
                                .or_default()
                                .extend(
                                    actual_effects
                                        .iter()
                                        .map(|effect| effect.destination.clone()),
                                );
                            effects.insert(position, actual_effects);
                        }
                    }
                }
                for (position, observed) in queries {
                    for location in observed {
                        let Some(ty) = current.place_types.get(&location.root) else {
                            continue;
                        };
                        let Some(path) = terminal_semantics::descriptor_lifetimes::descriptor_path(
                            &declarations,
                            *ty,
                            &location.path,
                        ) else {
                            continue;
                        };
                        let key = (position, location.root, path);
                        if terminal_semantics::descriptor_results::descriptor_slots(
                            &declarations,
                            location.root,
                            *ty,
                        )
                        .is_some_and(|slots| slots.contains(&location))
                        {
                            descriptor_sources.insert(key.clone());
                        }
                        if let Some(canonical) =
                            analysis.location(function.machine, position, &location)
                        {
                            locations.insert(key.clone(), canonical);
                        }
                        if let Some(origins) =
                            analysis.symbolic_source_origins(function.machine, position, &location)
                        {
                            let paired = origins
                                .into_iter()
                                .filter_map(|origin| {
                                    analysis
                                        .flatten_source_origin(function.machine, &origin)
                                        .map(|flat| (flat, origin))
                                })
                                .collect::<Vec<_>>();
                            sources.insert(
                                key.clone(),
                                paired
                                    .iter()
                                    .map(|(flat, _)| flat.clone())
                                    .collect::<Vec<_>>(),
                            );
                            symbolic_sources.insert(key, paired);
                        }
                    }
                }
                for node in function.block_nodes() {
                    if let O::CallStructural {
                        result:
                            abstract_operations::AbstractStructuralCallResult::Structural {
                                result, ..
                            },
                        ..
                    } = &node.operation
                        && result.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
                        && terminal_semantics::descriptor_results::is_shared_descriptor(
                            declarations.iter(),
                            result.structural_type,
                        )
                        && let Some(origins) = analysis.call_origins(function.machine, result.place)
                    {
                        returned.insert(result.place, origins);
                    }
                }
            }
        }
        let mut aliases = AliasSources::new();
        let mut parents = AliasSources::new();
        for block in &function.blocks {
            for (index, node) in block.nodes.iter().enumerate() {
                let position = (block.id, index as u32);
                let descriptor = match &node.operation {
                    O::EstablishElementView {
                        result,
                        source,
                        region,
                        ..
                    } => {
                        let mut projection = Projection::new(&source.path).at(position);
                        // Descendant indices use target-element units, not source bytes.
                        if matches!(
                            region,
                            terminal_psi::ElementViewRegion::RepresentationBytes { .. }
                        ) {
                            projection.forget_address();
                        }
                        Some((result.place, source.place, projection))
                    }
                    abstract_operations::element_view_or_byte_subslice_pattern!(
                        result,
                        source,
                        start,
                        end,
                        ..
                    ) => Some((
                        result.place,
                        source.place,
                        Projection::new(&source.path).with_window(*start, *end, position),
                    )),
                    O::StructuralLeafCopy {
                        result,
                        source,
                        path: abstract_operations::StructuralCopyPath::Ordinary(path),
                        ..
                    } if result.multiplicity
                        == terminal_psi::StructuralMultiplicity::Unrestricted
                        && terminal_semantics::descriptor_results::is_shared_descriptor(
                            types.values().copied(),
                            result.structural_type,
                        ) =>
                    {
                        let mut projection = Projection::new(path).at(position);
                        projection.indirect = !path.is_empty();
                        Some((result.place, *source, projection))
                    }
                    _ => None,
                };
                if let Some((result, source, projection)) = descriptor {
                    parents
                        .entry(result)
                        .or_default()
                        .push((source, projection.clone()));
                    if let Some(origins) =
                        sources.get(&((position.0, position.1 + 1), result, vec![]))
                    {
                        aliases
                            .entry(result)
                            .or_default()
                            .extend(origins.iter().filter_map(|origin| match origin {
                                DescriptorCallOrigin::Argument { source, projection } => {
                                    Some((*source, projection.clone()))
                                }
                                DescriptorCallOrigin::Literal { .. }
                                | DescriptorCallOrigin::PlacedFormation { .. }
                                | DescriptorCallOrigin::Empty => None,
                            }));
                    } else {
                        aliases
                            .entry(result)
                            .or_default()
                            .push((source, projection));
                    }
                }
                if let Some((_, result)) = node.operation.structural_result()
                    && let Some(origins) = returned.get(&result.place)
                {
                    for origin in origins {
                        if let DescriptorCallOrigin::Argument { source, projection } = origin {
                            parents
                                .entry(result.place)
                                .or_default()
                                .push((*source, projection.clone().at(position)));
                            aliases
                                .entry(result.place)
                                .or_default()
                                .push((*source, projection.clone().at(position)));
                        }
                    }
                }
                for edge in &node.successors {
                    for binding in &edge.structural_bindings {
                        if binding.argument.access != StructuralAccess::Owned {
                            aliases.entry(binding.parameter).or_default().push((
                                binding.argument.place,
                                Projection::new(&binding.argument.path).at(position),
                            ));
                        }
                    }
                }
            }
        }
        Self {
            reference_sources,
            aliases,
            parents,
            returned,
            sources,
            descriptor_sources,
            symbolic_sources,
            locations,
            effects,
            flow,
            relations: None,
            writes: BTreeMap::new(),
        }
    }

    pub(super) fn reference_sources(&self) -> &BorrowedReferenceSources {
        &self.reference_sources
    }

    pub(super) fn descriptor_flow(&self) -> Option<&BTreeMap<Position, DescriptorFlowStep>> {
        self.flow.as_ref()
    }

    pub(super) fn stored_effects(
        &self,
        position: Position,
    ) -> Option<&[DescriptorStoredCallEffect]> {
        self.effects.get(&position).map(Vec::as_slice)
    }

    pub(super) fn source_origins_at(
        &self,
        position: Position,
        root: PlaceId,
        path: &[StructuralPathSegment],
    ) -> Option<&[DescriptorCallOrigin]> {
        self.sources
            .get(&(position, root, path.to_vec()))
            .map(Vec::as_slice)
    }

    pub(super) fn incoming_origin_at(
        &self,
        position: Position,
        location: &DescriptorLocation,
        source: PlaceId,
        projection: &Projection,
    ) -> Option<DescriptorSourceOrigin> {
        self.locations
            .iter()
            .filter(|((at, _, _), canonical)| *at == position && *canonical == location)
            .filter_map(|(key, _)| self.symbolic_sources.get(key))
            .flatten()
            .find_map(|(flat, origin)| {
                (flat
                    == &DescriptorCallOrigin::Argument {
                        source,
                        projection: projection.clone(),
                    }
                    && matches!(origin, DescriptorSourceOrigin::IncomingSlot(_)))
                .then(|| origin.clone())
            })
    }

    pub(super) fn location_at(
        &self,
        position: Position,
        root: PlaceId,
        path: &[StructuralPathSegment],
    ) -> Option<&DescriptorLocation> {
        self.locations.get(&(position, root, path.to_vec()))
    }

    pub(super) fn access_location(
        &self,
        position: Position,
        root: PlaceId,
        path: &[StructuralPathSegment],
    ) -> Option<&DescriptorLocation> {
        (0..=path.len())
            .rev()
            .find_map(|length| self.location_at(position, root, &path[..length]))
    }

    /// Resolve a dereferenced slot's current value, not a global alias for its
    /// mutable field. Replacing a descriptor itself retains direct slot geometry.
    pub(in crate::current_ownership) fn resolve_at(
        &self,
        position: Position,
        root: PlaceId,
        projection: Projection,
    ) -> Vec<Region> {
        if projection.indirect {
            for length in (0..=projection.path.len()).rev() {
                // An aggregate's storage origin is not the backing of its
                // embedded descriptors. Unknown aggregate effects stay indirect.
                if !self.descriptor_sources.contains(&(
                    position,
                    root,
                    projection.path[..length].to_vec(),
                )) {
                    continue;
                }
                if let Some(origins) =
                    self.source_origins_at(position, root, &projection.path[..length])
                {
                    // Conservative paths omit captured address steps. Rebuilding
                    // overlap geometry cannot restore that missing exact history.
                    let mut suffix = Projection::new(&projection.path[length..])
                        .at(position)
                        .without_captured_address();
                    suffix.window = projection.window;
                    if suffix.window.is_some() {
                        suffix.captured_at = projection.captured_at;
                        suffix.exact = false;
                    }
                    return origins
                        .iter()
                        .filter_map(|origin| match origin {
                            DescriptorCallOrigin::Argument { source, projection } => Some(Region {
                                root: Some(*source),
                                projection: projection.then(&suffix),
                            }),
                            DescriptorCallOrigin::Literal { .. }
                            | DescriptorCallOrigin::Empty
                            | DescriptorCallOrigin::PlacedFormation { .. } => None,
                        })
                        .collect();
                }
            }
        }
        self.resolve(root, projection)
    }

    pub(super) fn call_origins(&self, result: PlaceId) -> Option<&[DescriptorCallOrigin]> {
        self.returned.get(&result).map(Vec::as_slice)
    }

    pub(super) fn direct_aliases(&self, result: PlaceId) -> Option<&[(PlaceId, Projection)]> {
        self.parents.get(&result).map(Vec::as_slice)
    }

    pub(in crate::current_ownership) fn resolve(
        &self,
        place: PlaceId,
        projection: Projection,
    ) -> Vec<Region> {
        resolve(&self.aliases, place, projection)
    }

    pub(in crate::current_ownership) fn writes_at(&self, position: Position) -> &[Access] {
        self.writes.get(&position).map(Vec::as_slice).unwrap_or(&[])
    }

    pub(super) fn regions_overlap_at(
        &self,
        left: &Region,
        right: &Region,
        position: Position,
    ) -> bool {
        self.relations.as_ref().map_or_else(
            || left.overlaps(right),
            |relations| {
                terminal_semantics::borrowed_storage::regions_overlap_at(
                    left,
                    right,
                    position,
                    relations.ordering(),
                )
            },
        )
    }

    pub(in crate::current_ownership) fn overlaps(
        &self,
        pinned: &[Region],
        place: PlaceId,
        path: &[StructuralPathSegment],
    ) -> bool {
        self.resolve(place, Projection::new(path))
            .iter()
            .any(|region| pinned.iter().any(|loan| loan.overlaps(region)))
    }

    pub(in crate::current_ownership) fn conflicted_operation(
        &self,
        pinned: &[Region],
        position: Position,
        moved: &[PlaceId],
    ) -> Option<PlaceId> {
        moved
            .iter()
            .copied()
            .find(|place| self.overlaps(pinned, *place, &[]))
            .or_else(|| {
                self.writes_at(position).iter().find_map(|access| {
                    access
                        .regions
                        .iter()
                        .any(|region| pinned.iter().any(|loan| loan.overlaps(region)))
                        .then_some(access.place)
                })
            })
    }
}
