/*
We reconstruct and replay live ownership frontiers across each function's
control-flow graph from its current executable operations and signatures. We
track live claims, non-unrestricted owned places, partial-affine field move
paths, and outstanding reference loans across block entries, operation nodes,
call regions, address joins, and edge transitions, ensuring that every disposal
obligation, loan ceiling, and window restoration contract is satisfied without
trusting cached ownership events.
*/

use std::collections::{BTreeMap, BTreeSet};

use optimization_unit::{
    OptimizationBlock, OwnershipFrontierFact, OwnershipFrontierLiveClaim,
    OwnershipFrontierOwnedPlace, OwnershipFrontierPartialCustody, OwnershipFrontierSnapshot,
    PsiOptimizationFunction,
};
use semantic_vocabulary::{BlockId, ClaimId, MachineId, PlaceId, StructuralTypeId};
use terminal_psi::{
    BoundaryMachineDeclaration, StructuralAccess, StructuralMultiplicity, StructuralPathSegment,
    StructuralTypeDeclaration,
};

use crate::OptimizationUnitValidationError;
use crate::unit_validation::references::LiveReference;

pub(crate) mod address_joins {
    pub(super) mod borrowed_regions {
        use super::range_relations::RangeRelations;
        use abstract_operations::AbstractOperation as O;
        use optimization_unit::PsiOptimizationFunction;
        use semantic_vocabulary::{MachineId, PlaceId, StructuralTypeId};
        use std::collections::{BTreeMap, BTreeSet};
        use terminal_psi::{StructuralAccess, StructuralPathSegment, StructuralTypeDeclaration};

        use terminal_psi::StructuralArgument;
        use terminal_semantics::borrowed_storage::{AliasSources, resolve};
        pub(in crate::current_ownership) use terminal_semantics::borrowed_storage::{
            Position, Projection, Region,
        };
        use terminal_semantics::descriptor_lifetimes::descriptor_path;
        use terminal_semantics::descriptor_lifetimes::{DescriptorFlowStep, DescriptorLocation};
        use terminal_semantics::descriptor_results::descriptor_slots;
        use terminal_semantics::descriptor_results::is_shared_descriptor;
        use terminal_semantics::descriptor_results::{
            DescriptorAnalysis, DescriptorCallOrigin, DescriptorSourceOrigin,
            DescriptorStoredCallEffect,
        };

        pub(in crate::current_ownership) type BorrowedReferenceSources =
            BTreeMap<PlaceId, Option<Vec<StructuralArgument>>>;

        type ObservationKey = (Position, PlaceId, Vec<StructuralPathSegment>);

        pub(in crate::current_ownership) struct Access {
            pub place: PlaceId,
            pub regions: Vec<Region>,
            pub location: Option<DescriptorLocation>,
        }

        #[derive(Default)]
        pub(in crate::current_ownership) struct BorrowedRegions {
            reference_sources: BorrowedReferenceSources,
            aliases: AliasSources,
            parents: AliasSources,
            returned: BTreeMap<PlaceId, Vec<DescriptorCallOrigin>>,
            sources: BTreeMap<ObservationKey, Vec<DescriptorCallOrigin>>,
            descriptor_sources: BTreeSet<ObservationKey>,
            symbolic_sources:
                BTreeMap<ObservationKey, Vec<(DescriptorCallOrigin, DescriptorSourceOrigin)>>,
            locations: BTreeMap<ObservationKey, DescriptorLocation>,
            effects: BTreeMap<Position, Vec<DescriptorStoredCallEffect>>,
            flow: Option<BTreeMap<Position, DescriptorFlowStep>>,
            relations: Option<RangeRelations>,
            writes: BTreeMap<Position, Vec<Access>>,
        }

        impl BorrowedRegions {
            pub(in crate::current_ownership) fn has_returned_call_origins(
                &self,
                place: PlaceId,
            ) -> bool {
                self.returned.contains_key(&place)
            }

            pub(in crate::current_ownership) fn new(
                function: &PsiOptimizationFunction,
                functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
                types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
                call_accesses: &mut crate::current_ownership::CallAccesses,
            ) -> Self {
                let mut result = call_accesses
                    .take_borrowed_origins(function.machine)
                    .unwrap_or_else(|| {
                        Self::origins(
                            function,
                            functions,
                            call_accesses.boundaries(),
                            call_accesses.dynamic_dispatch,
                            types,
                        )
                    });
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
                                        .access_location(
                                            (block.id, index as u32),
                                            place,
                                            &projection.path,
                                        )
                                        .cloned(),
                                    regions: result.resolve_at(
                                        (block.id, index as u32),
                                        place,
                                        projection,
                                    ),
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
                dynamic_dispatch: &terminal_psi::TerminalDynamicDispatchCatalog,
                types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            ) -> Self {
                let declarations = types.values().copied().cloned().collect::<Vec<_>>();
                let mut catalog = functions.values().copied().collect::<Vec<_>>();
                if !functions.contains_key(&function.machine) {
                    catalog.push(function);
                }
                let graph = crate::descriptor_results::optimized_catalog_graph(
                    catalog,
                    boundaries.iter().copied(),
                    &declarations,
                    dynamic_dispatch,
                    |_, node| Some(std::borrow::Cow::Borrowed(&node.operation)),
                );
                let mut analysis = graph
                    .as_ref()
                    .map(|graph| DescriptorAnalysis::new(&declarations, graph));
                Self::origins_with_analysis(
                    function,
                    &declarations,
                    graph.as_deref(),
                    analysis.as_mut(),
                    types,
                )
            }

            pub(in crate::current_ownership) fn origins_with_analysis(
                function: &PsiOptimizationFunction,
                declarations: &[StructuralTypeDeclaration],
                graph: Option<&[terminal_semantics::descriptor_results::DescriptorMachine]>,
                analysis: Option<&mut DescriptorAnalysis<'_>>,
                types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            ) -> Self {
                let mut reference_sources = BorrowedReferenceSources::new();
                for parameter in function.block_structural_parameters() {
                    if parameter.access != StructuralAccess::Owned
                        && !is_shared_descriptor(declarations.iter(), parameter.structural_type)
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
                if let (Some(graph), Some(analysis)) = (graph, analysis) {
                    flow = analysis.flow(function.machine);
                    if let Some(current) =
                        terminal_semantics::descriptor_results::descriptor_machine(
                            graph,
                            function.machine,
                        )
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
                                                let path = descriptor_path(
                                                    declarations,
                                                    *current.place_types.get(&origin.root)?,
                                                    &origin.path,
                                                )?;
                                                Some(StructuralArgument {
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
                        let mut queries: BTreeMap<Position, BTreeSet<DescriptorLocation>> =
                            BTreeMap::new();
                        for (position, step) in &current.flow {
                            let observed = queries.entry(*position).or_default();
                            observed.extend(step.inputs.iter().cloned());
                            observed.extend(step.definitions.iter().cloned());
                            for (source, destination) in &step.derivations {
                                observed.insert(source.clone());
                                observed.insert(destination.clone());
                            }
                            for edge in &step.successors {
                                observed.extend(
                                    edge.bindings
                                        .iter()
                                        .map(|binding| binding.source.observation()),
                                );
                            }
                            let completed =
                                queries.entry((position.0, position.1 + 1)).or_default();
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
                                            descriptor_slots(declarations, root, *ty)
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
                                        .extend(actual_effects.iter().filter_map(|effect| {
                                            effect.destination.exact_location()
                                        }));
                                    effects.insert(position, actual_effects);
                                }
                            }
                        }
                        for (position, observed) in queries {
                            for location in observed {
                                let Some(ty) = current.place_types.get(&location.root) else {
                                    continue;
                                };
                                let Some(path) = descriptor_path(declarations, *ty, &location.path)
                                else {
                                    continue;
                                };
                                let key = (position, location.root, path);
                                if descriptor_slots(declarations, location.root, *ty)
                                    .is_some_and(|slots| slots.contains(&location))
                                {
                                    descriptor_sources.insert(key.clone());
                                }
                                if let Some(canonical) =
                                    analysis.location(function.machine, position, &location)
                                {
                                    locations.insert(key.clone(), canonical);
                                }
                                if let Some(origins) = analysis.symbolic_source_origins(
                                    function.machine,
                                    position,
                                    &location,
                                ) {
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
                                        result,
                                        ..
                                    },
                                ..
                            } = &node.operation
                                && result.multiplicity
                                    == terminal_psi::StructuralMultiplicity::Unrestricted
                                && is_shared_descriptor(declarations.iter(), result.structural_type)
                                && let Some(origins) =
                                    analysis.call_origins(function.machine, result.place)
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
                                if matches!(
                                    region,
                                    terminal_psi::ElementViewRegion::RepresentationBytes { .. }
                                        | terminal_psi::ElementViewRegion::WholeRepresentation { .. }
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
                                && is_shared_descriptor(
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
                                aliases.entry(result).or_default().extend(
                                    origins.iter().filter_map(|origin| match origin {
                                        DescriptorCallOrigin::Argument { source, projection } => {
                                            Some((*source, projection.clone()))
                                        }
                                        DescriptorCallOrigin::Invocation { .. }
                                        | DescriptorCallOrigin::Literal { .. }
                                        | DescriptorCallOrigin::AccessFormation { .. }
                                        | DescriptorCallOrigin::Empty => None,
                                    }),
                                );
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
                                if let DescriptorCallOrigin::Argument { source, projection } =
                                    origin
                                {
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

            pub(super) fn descriptor_flow(
                &self,
            ) -> Option<&BTreeMap<Position, DescriptorFlowStep>> {
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

            pub(in crate::current_ownership) fn resolve_at(
                &self,
                position: Position,
                root: PlaceId,
                projection: Projection,
            ) -> Vec<Region> {
                if projection.indirect {
                    for length in (0..=projection.path.len()).rev() {
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
                                    DescriptorCallOrigin::Argument { source, projection } => {
                                        Some(Region {
                                            root: Some(*source),
                                            projection: projection.then(&suffix),
                                        })
                                    }
                                    DescriptorCallOrigin::Invocation { projection, .. } => {
                                        Some(Region {
                                            root: None,
                                            projection: projection.then(&suffix),
                                        })
                                    }
                                    DescriptorCallOrigin::Literal { .. }
                                    | DescriptorCallOrigin::Empty
                                    | DescriptorCallOrigin::AccessFormation { .. } => None,
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

            pub(super) fn direct_aliases(
                &self,
                result: PlaceId,
            ) -> Option<&[(PlaceId, Projection)]> {
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
    }
    mod descriptor_loans {
        use super::super::call_accesses::argument_storage_is_closed;
        use super::borrowed_regions::{Access, BorrowedRegions, Position, Projection, Region};
        use crate::unit_validation::structural_catalog::shared_view_declaration;
        use abstract_operations::AbstractOperation as O;
        use optimization_unit::{OptimizationEdge, PsiOptimizationFunction};
        use semantic_vocabulary::{BlockId, PlaceId, StructuralTypeId};
        use std::collections::{BTreeMap, BTreeSet};
        use terminal_semantics::descriptor_results::{
            DescriptorCallOrigin, DescriptorCompatibilityRequirement, DescriptorSourceOrigin,
        };
        mod compatibility {
            use crate::{OperationSite, OptimizationUnitValidationError};
            use optimization_unit::PsiOptimizationUnit;
            use semantic_vocabulary::MachineId;
            use std::collections::{BTreeMap, BTreeSet};
            use terminal_semantics::descriptor_results::{
                DescriptorAnalysis, DescriptorCallOrigin, DescriptorCompatibilityRequirement,
            };

            pub(crate) fn validate_compatibility(
                unit: &PsiOptimizationUnit,
                requirements: &BTreeMap<MachineId, Vec<DescriptorCompatibilityRequirement>>,
                view_graph: Option<&[terminal_semantics::descriptor_results::DescriptorMachine]>,
            ) -> Result<(), OptimizationUnitValidationError> {
                if requirements.values().all(Vec::is_empty) {
                    return Ok(());
                }
                let error = |machine, position: terminal_semantics::borrowed_storage::Position| {
                    let function = unit.function(machine).expect("validated function");
                    let place = requirements
                        .get(&machine)
                        .and_then(|rows| {
                            rows.iter().find_map(|row| match &row.demand {
                                DescriptorCallOrigin::Argument { source, .. } => Some(*source),
                                _ => None,
                            })
                        })
                        .or_else(|| function.structural_places.first().map(|place| place.id))
                        .expect("derived storage requirement");
                    OptimizationUnitValidationError::CurrentSharedJoinOriginDisturbed(
                        OperationSite::new(machine, position.0, position.1),
                        place,
                    )
                };
                let graph = view_graph.ok_or_else(|| {
                    let (&machine, rows) = requirements
                        .iter()
                        .find(|(_, rows)| !rows.is_empty())
                        .expect("nonempty requirements");
                    error(machine, rows[0].position)
                })?;
                let mut analysis = DescriptorAnalysis::new(&unit.structural_types, graph);
                let residuals = analysis
                    .compatibility_requirements(requirements)
                    .map_err(|failure| error(failure.machine, failure.position))?;
                let mut external = BTreeSet::from([unit.entry]);
                external.extend(
                    unit.provider_candidates
                        .iter()
                        .map(|candidate| candidate.candidate),
                );
                for machine in external {
                    if let Some(requirement) = residuals.get(&machine).and_then(|rows| rows.first())
                    {
                        return Err(error(machine, requirement.position));
                    }
                }
                Ok(())
            }
        }
        use crate::descriptor_results::element_coordinates_preserved;
        use crate::unit_validation::function_structure::operation_place_inputs;
        use crate::unit_validation::operation_contracts::structural_source_contract;
        use abstract_operations::element_view_or_byte_subslice_pattern;
        use abstract_operations::return_or_return_unit_pattern;
        pub(crate) use compatibility::validate_compatibility;
        use terminal_psi::{StructuralAccess, StructuralTypeDeclaration, live_field_by_id};
        use terminal_semantics::descriptor_lifetimes::{
            DescriptorFlowEdge, DescriptorFlowStep, DescriptorLocation,
        };
        use terminal_semantics::runtime_structural_path_tip;

        struct DescriptorLoan {
            incoming: Option<DescriptorSourceOrigin>,
            regions: Vec<Region>,
            live: BTreeSet<Position>,
            access: StructuralAccess,
            authorized: BTreeMap<Position, BTreeSet<DescriptorLocation>>,
        }

        #[derive(Default)]
        pub(super) struct DescriptorLoans {
            loans: Vec<DescriptorLoan>,
            observations: BTreeMap<Position, Vec<Access>>,
            exclusive_inputs: BTreeSet<PlaceId>,
            requirements: Vec<DescriptorCompatibilityRequirement>,
        }

        impl DescriptorLoans {
            pub(super) fn new(
                function: &PsiOptimizationFunction,
                borrowed_regions: &BorrowedRegions,
                types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            ) -> Self {
                let mut births = Vec::new();
                let mut escaped_births = BTreeSet::new();
                for block in &function.blocks {
                    for (index, node) in block.nodes.iter().enumerate() {
                        let position = (
                            block.id,
                            u32::try_from(index).expect("unit node index fits u32"),
                        );
                        if let O::StructuralByteViewFieldStore {
                            destination,
                            path,
                            field,
                            source,
                            ..
                        } = &node.operation
                        {
                            let selected = super::super::storage_regions::projected(
                                function,
                                types,
                                *destination,
                                path,
                                *field,
                            )
                            .path;
                            if let Some(slot) =
                                borrowed_regions.location_at(position, *destination, &selected)
                                && let Some(origins) =
                                    borrowed_regions.source_origins_at(position, *source, &[])
                            {
                                births.extend(origins.iter().filter_map(|origin| match origin {
                            terminal_semantics::descriptor_results::DescriptorCallOrigin::Argument { source, projection } => Some((position, slot.clone(), *source, projection.clone(), StructuralAccess::SharedBorrow)),
                            _ => None,
                        }));
                            }
                        }
                        for effect in borrowed_regions
                            .stored_effects(position)
                            .into_iter()
                            .flatten()
                        {
                            let destination = effect.destination.observation();
                            if effect.destination.exact_location().is_none() {
                                escaped_births.insert((position, destination.clone()));
                            }
                            births.extend(effect.origins.iter().filter_map(|origin| match origin {
                        terminal_semantics::descriptor_results::DescriptorCallOrigin::Argument { source, projection } => Some((position, destination.clone(), *source, projection.clone(), StructuralAccess::SharedBorrow)),
                        _ => None,
                    }));
                        }
                        let descriptor = match &node.operation {
                    O::EstablishElementView { result, source, element, region, .. }
                        if source.access == StructuralAccess::SharedBorrow
                            || !element_coordinates_preserved(function, types.values().copied(), source, *element, *region) => {
                        let mut projection = Projection::new(&source.path);
                        if !element_coordinates_preserved(function, types.values().copied(), source, *element, *region) {
                            projection.forget_address();
                        }
                        Some((result.place, source.place, projection, source.access))
                    },
                    element_view_or_byte_subslice_pattern!(result,
                        source,
                        start,
                        end, ..) => Some((
                        result.place,
                        source.place,
                        Projection::new(&source.path).with_window(*start, *end, position),
                        crate::unit_validation::operation_contracts::structural_operation_result_contract(
                            function, &node.operation,
                        ).map_or(StructuralAccess::MutableBorrow, |source| source.access),
                    )),
                    _ => None
                };
                        if let Some((_, result)) = node.operation.structural_result() {
                            if let Some(origins) = borrowed_regions.call_origins(result.place) {
                                births.extend(origins.iter().filter_map(|origin| match origin {
                            terminal_semantics::descriptor_results::DescriptorCallOrigin::Argument { source, projection } => Some((position, DescriptorLocation::whole(result.place), *source, projection.clone(), StructuralAccess::SharedBorrow)),
                            terminal_semantics::descriptor_results::DescriptorCallOrigin::Invocation { .. }
                            | terminal_semantics::descriptor_results::DescriptorCallOrigin::Empty
                            | terminal_semantics::descriptor_results::DescriptorCallOrigin::Literal { .. }
                            | terminal_semantics::descriptor_results::DescriptorCallOrigin::AccessFormation { .. } => None,
                        }));
                            } else if matches!(node.operation, O::StructuralLeafCopy { .. })
                                && terminal_semantics::descriptor_results::is_shared_descriptor(
                                    types.values().copied(),
                                    result.structural_type,
                                )
                            {
                                if let Some(origins) = borrowed_regions.source_origins_at(
                                    (position.0, position.1 + 1),
                                    result.place,
                                    &[],
                                ) {
                                    births.extend(origins.iter().filter_map(|origin| match origin {
                                terminal_semantics::descriptor_results::DescriptorCallOrigin::Argument { source, projection } => Some((position, DescriptorLocation::whole(result.place), *source, projection.clone(), StructuralAccess::SharedBorrow)),
                                _ => None,
                            }));
                                } else if let Some(aliases) =
                                    borrowed_regions.direct_aliases(result.place)
                                {
                                    births.extend(aliases.iter().map(|(source, projection)| {
                                        (
                                            position,
                                            DescriptorLocation::whole(result.place),
                                            *source,
                                            projection.clone(),
                                            StructuralAccess::SharedBorrow,
                                        )
                                    }));
                                }
                            }
                        }
                        if let Some((result, source, projection, access)) = descriptor {
                            births.push((
                                position,
                                DescriptorLocation::whole(result),
                                source,
                                projection,
                                access,
                            ));
                        }
                    }
                }
                if births.is_empty() {
                    return Self::default();
                }
                let flow = descriptor_flow(function, types, borrowed_regions);
                let mut loans = Vec::with_capacity(births.len());
                for (birth, descriptor, source, projection, access) in births {
                    let mut lifetime =
                        terminal_semantics::descriptor_lifetimes::descriptor_location_lifetime(
                            &flow,
                            birth,
                            &descriptor,
                        );
                    if escaped_births.contains(&(birth, descriptor.clone())) {
                        lifetime.authorized.clear();
                        let mut pending = flow
                            .get(&birth)
                            .into_iter()
                            .flat_map(|step| step.successors.iter().map(|edge| edge.target))
                            .collect::<Vec<_>>();
                        let mut visited = BTreeSet::new();
                        while let Some(position) = pending.pop() {
                            if visited.insert(position) {
                                lifetime.live.insert(position);
                                if let Some(step) = flow.get(&position) {
                                    pending.extend(step.successors.iter().map(|edge| edge.target));
                                }
                            }
                        }
                    }
                    loans.push(DescriptorLoan {
                        incoming: borrowed_regions.incoming_origin_at(
                            (birth.0, birth.1 + 1),
                            &descriptor,
                            source,
                            &projection,
                        ),
                        regions: borrowed_regions.resolve(source, projection),
                        live: lifetime.live,
                        access,
                        authorized: lifetime.authorized,
                    });
                }
                let observations = function
                    .blocks
                    .iter()
                    .flat_map(|block| {
                        block.nodes.iter().enumerate().map(move |(index, node)| {
                            let position = (block.id, index as u32);
                            (
                                position,
                                operation_observations(
                                    function,
                                    types,
                                    borrowed_regions,
                                    position,
                                    &node.operation,
                                ),
                            )
                        })
                    })
                    .collect();
                let mut result = Self {
                    loans,
                    observations,
                    exclusive_inputs: function
                        .structural_parameters
                        .iter()
                        .filter(|parameter| {
                            matches!(
                                parameter.access,
                                StructuralAccess::Owned
                                    | StructuralAccess::MutableBorrow
                                    | StructuralAccess::WriteOnlyBorrow
                            )
                        })
                        .map(|parameter| parameter.place)
                        .collect(),
                    requirements: Vec::new(),
                };
                let mut requirements = Vec::new();
                for block in &function.blocks {
                    for (index, node) in block.nodes.iter().enumerate() {
                        result.conflicted_operation(
                            borrowed_regions,
                            block.id,
                            index as u32,
                            &node.operation,
                            &[],
                            &mut requirements,
                        );
                        for edge in &node.successors {
                            if let Some(target) = function.block(edge.target) {
                                result.conflicted_edge(
                                    borrowed_regions,
                                    block.id,
                                    index as u32,
                                    edge,
                                    &target.structural_parameters,
                                    &mut requirements,
                                );
                            }
                        }
                    }
                }
                let mut observed = Vec::new();
                for requirement in requirements {
                    if !observed.contains(&requirement) {
                        observed.push(requirement);
                    }
                }
                result.requirements = observed;
                result
            }

            fn compatibility_requirement(
                &self,
                loan: &DescriptorLoan,
                demand: &Region,
                position: Position,
            ) -> Option<DescriptorCompatibilityRequirement> {
                let incoming = loan.incoming.as_ref()?;
                let source = demand.root?;
                if loan.access != StructuralAccess::SharedBorrow
                    || !self.exclusive_inputs.contains(&source)
                {
                    return None;
                }
                Some(DescriptorCompatibilityRequirement {
                    position,
                    loan: incoming.clone(),
                    demand: DescriptorCallOrigin::Argument {
                        source,
                        projection: demand.projection.enclosing_storage()?,
                    },
                })
            }

            pub(super) fn requirements(&self) -> Vec<DescriptorCompatibilityRequirement> {
                self.requirements.clone()
            }

            pub(super) fn conflicted_operation(
                &self,
                borrowed_regions: &BorrowedRegions,
                block: BlockId,
                node: u32,
                operation: &O,
                moved: &[PlaceId],
                requirements: &mut Vec<DescriptorCompatibilityRequirement>,
            ) -> Option<PlaceId> {
                let position = (block, node);
                let moved = if matches!(operation, return_or_return_unit_pattern!()) {
                    &[][..]
                } else {
                    moved
                };
                let moved = moved
                    .iter()
                    .map(|place| {
                        (
                            *place,
                            borrowed_regions.resolve(*place, Projection::new(&[])),
                        )
                    })
                    .collect::<Vec<_>>();
                for loan in self
                    .loans
                    .iter()
                    .filter(|loan| loan.live.contains(&position))
                {
                    if let Some((_, result)) = operation.structural_result() {
                        let replacement = Region {
                            root: Some(result.place),
                            projection: Projection::new(&[]),
                        };
                        if loan
                            .regions
                            .iter()
                            .any(|backing| backing.overlaps(&replacement))
                        {
                            return Some(result.place);
                        }
                    }
                    let observations = (loan.access != StructuralAccess::SharedBorrow)
                        .then(|| self.observations.get(&position))
                        .flatten()
                        .map(Vec::as_slice)
                        .unwrap_or(&[]);
                    for observation in borrowed_regions
                        .writes_at(position)
                        .iter()
                        .chain(observations)
                    {
                        if loan.authorizes(
                            position,
                            observation.location.as_ref(),
                            &observation.regions,
                        ) {
                            continue;
                        }
                        for region in &observation.regions {
                            for backing in &loan.regions {
                                if !borrowed_regions.regions_overlap_at(backing, region, position) {
                                    continue;
                                }
                                match self.compatibility_requirement(loan, region, position) {
                                    Some(requirement) => requirements.push(requirement),
                                    None => return Some(observation.place),
                                }
                            }
                        }
                    }
                    for (place, regions) in &moved {
                        if regions.iter().any(|region| {
                            loan.regions.iter().any(|backing| {
                                borrowed_regions.regions_overlap_at(backing, region, position)
                            })
                        }) {
                            return Some(*place);
                        }
                    }
                }
                None
            }

            pub(super) fn conflicted_edge(
                &self,
                borrowed_regions: &BorrowedRegions,
                block: BlockId,
                node: u32,
                edge: &OptimizationEdge,
                destination: &[terminal_psi::StructuralParameterDeclaration],
                requirements: &mut Vec<DescriptorCompatibilityRequirement>,
            ) -> Option<PlaceId> {
                for loan in &self.loans {
                    if !loan.live.contains(&(block, node)) || !loan.live.contains(&(edge.target, 0))
                    {
                        continue;
                    }
                    let disturbed = edge
                        .structural_bindings
                        .iter()
                        .zip(destination)
                        .filter(|(binding, parameter)| match binding.argument.access {
                            StructuralAccess::SharedBorrow => {
                                loan.access != StructuralAccess::SharedBorrow
                            }
                            StructuralAccess::Owned => {
                                parameter.multiplicity
                                    != terminal_psi::StructuralMultiplicity::Unrestricted
                            }
                            terminal_psi::exclusive_borrow_pair_pattern!() => true,
                        })
                        .map(|(binding, _)| {
                            (
                                binding.argument.place,
                                Projection::new(&binding.argument.path),
                                binding.argument.access == StructuralAccess::Owned,
                            )
                        })
                        .chain(
                            edge.cleanup_actions
                                .iter()
                                .map(|action| (action.place(), Projection::new(&[]), true)),
                        );
                    for (place, projection, forced) in disturbed {
                        let location = borrowed_regions.access_location(
                            (block, node),
                            place,
                            &projection.path,
                        );
                        let regions = borrowed_regions.resolve_at(
                            (block, node),
                            place,
                            projection.at((block, node)),
                        );
                        if forced {
                            if regions.iter().any(|region| {
                                loan.regions.iter().any(|backing| {
                                    borrowed_regions.regions_overlap_at(
                                        backing,
                                        region,
                                        (block, node),
                                    )
                                })
                            }) {
                                return Some(place);
                            }
                            continue;
                        }
                        if loan.authorizes((block, node), location, &regions) {
                            continue;
                        }
                        for region in &regions {
                            for backing in &loan.regions {
                                if !borrowed_regions.regions_overlap_at(
                                    backing,
                                    region,
                                    (block, node),
                                ) {
                                    continue;
                                }
                                match self.compatibility_requirement(loan, region, (block, node)) {
                                    Some(requirement) => requirements.push(requirement),
                                    None => return Some(place),
                                }
                            }
                        }
                    }
                }
                None
            }
        }

        impl DescriptorLoan {
            fn authorizes(
                &self,
                position: Position,
                location: Option<&DescriptorLocation>,
                regions: &[Region],
            ) -> bool {
                self.authorized.get(&position).is_some_and(|places| {
                    location.is_some_and(|location| {
                        places
                            .iter()
                            .any(|authorized| authorized.contains(location))
                    })
                }) && !regions.is_empty()
                    && regions
                        .iter()
                        .all(|region| region.root.is_some() && !region.projection.indirect)
            }
        }

        fn operation_observations(
            function: &PsiOptimizationFunction,
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            borrowed_regions: &BorrowedRegions,
            position: Position,
            operation: &O,
        ) -> Vec<Access> {
            use super::super::storage_regions;
            use terminal_psi::StructuralPathSegment;
            if matches!(
                operation,
                O::ElementViewLength { .. }
                    | O::ByteSequenceLength { .. }
                    | abstract_operations::jump_or_conditional_pattern!()
            ) {
                return Vec::new();
            }
            let mut remaining = descriptor_inputs(function, types, operation);
            let mut result = Vec::new();
            let mut closed_storage = BTreeMap::new();
            let mut retain = |place, projection: Projection| {
                if let Some(index) = remaining.iter().position(|input| *input == place) {
                    remaining.remove(index);
                }
                result.push(Access {
                    place,
                    location: borrowed_regions
                        .access_location(position, place, &projection.path)
                        .cloned(),
                    regions: borrowed_regions.resolve_at(position, place, projection.at(position)),
                });
            };
            let projected = |place, path: &[StructuralPathSegment], field| {
                let region = storage_regions::projected(function, types, place, path, field);
                let mut projection = Projection::new(&region.path);
                projection.indirect |= region.indirect;
                let root = structural_source_contract(function, place);
                projection.indirect |= root.is_none_or(|root| {
                    (0..path.len()).any(|length| {
                        runtime_structural_path_tip(
                            types.values().copied(),
                            root.structural_type,
                            &path[..length],
                        )
                        .is_none_or(|declaration| {
                            length != 0
                                && (matches!(
                                    declaration.shape,
                                    terminal_psi::StructuralTypeShape::Reference { .. }
                                        | terminal_psi::StructuralTypeShape::ElementView { .. }
                                ) || declaration.shape.is_borrowed_byte_view())
                        })
                    })
                });
                projection
            };
            let byte_field_projection = |place, path: &[StructuralPathSegment], field| {
                let mut projection = projected(place, path, Some(field));
                let owned = structural_source_contract(function, place)
                    .and_then(|root| {
                        runtime_structural_path_tip(
                            types.values().copied(),
                            root.structural_type,
                            path,
                        )
                    })
                    .and_then(|declaration| match &declaration.shape {
                        terminal_psi::record_or_mixed_shape_pattern!(fields) => {
                            live_field_by_id(fields, field)
                        }
                        _ => None,
                    })
                    .and_then(|field| field.field_type.byte_sequence_carrier())
                    .is_some_and(|carrier| {
                        matches!(
                            carrier,
                            terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                        )
                    });
                projection.indirect |= !owned;
                projection
            };
            let mut observe_byte_operand =
                |operand: &terminal_psi::control_flow::ByteSequenceOperand| match operand {
                    terminal_psi::control_flow::ByteSequenceOperand::Whole { source, .. } => {
                        retain(source.place, projected(source.place, &source.path, None))
                    }
                    terminal_psi::control_flow::ByteSequenceOperand::Field {
                        source,
                        field,
                        ..
                    } => retain(
                        source.place,
                        byte_field_projection(source.place, &source.path, *field),
                    ),
                };
            match operation {
                O::EstablishByteSequenceConcatenation { left, right, .. }
                | O::ByteSequenceEqual { left, right, .. } => {
                    observe_byte_operand(left);
                    observe_byte_operand(right);
                }
                O::ByteSequencePredicate { operand, .. } => observe_byte_operand(operand),
                abstract_operations::primitive_storage_observation_pattern!(source, path, ..) => {
                    retain(*source, projected(*source, path, None));
                }
                O::StructuralScalarFieldRead {
                    source,
                    path,
                    field,
                    ..
                } => {
                    let path =
                        storage_regions::canonical_field(function, types, *source, path, *field);
                    retain(*source, projected(*source, &path, None));
                }
                O::ElementViewRead {
                    source,
                    index,
                    obligation,
                    path,
                    ..
                } => {
                    let mut selected = vec![StructuralPathSegment::RuntimeIndex {
                        index: *index,
                        obligation: *obligation,
                    }];
                    selected.extend_from_slice(path);
                    retain(*source, projected(*source, &selected, None));
                }
                O::ByteSequenceRead {
                    source,
                    index,
                    obligation,
                    ..
                } => {
                    retain(
                        *source,
                        projected(
                            *source,
                            &[StructuralPathSegment::RuntimeIndex {
                                index: *index,
                                obligation: *obligation,
                            }],
                            None,
                        ),
                    );
                }
                element_view_or_byte_subslice_pattern!(source, start, end, ..) => {
                    retain(
                        source.place,
                        projected(source.place, &source.path, None)
                            .with_window(*start, *end, position),
                    );
                }
                O::StructuralByteSequenceFieldLength {
                    source,
                    path,
                    field,
                    ..
                } => {
                    retain(*source, projected(*source, path, Some(*field)));
                }
                O::StructuralByteSequenceFieldRead {
                    source,
                    path,
                    field,
                    ..
                } => {
                    retain(*source, byte_field_projection(*source, path, *field));
                }
                abstract_operations::call_structural_or_boundary_call_pattern!(
                    structural_arguments
                ) => {
                    for argument in structural_arguments {
                        let mut projection = projected(argument.place, &argument.path, None);
                        projection.indirect |=
                            !structural_source_contract(function, argument.place).is_some_and(
                                |source| {
                                    argument_storage_is_closed(
                                        types,
                                        source.structural_type,
                                        &argument.path,
                                        argument.access,
                                        &mut closed_storage,
                                    )
                                },
                            );
                        retain(argument.place, projection);
                    }
                }
                O::EstablishReference { source, .. }
                | O::EstablishAccessReference { source, .. }
                | O::EstablishElementView { source, .. } => {
                    retain(source.place, projected(source.place, &source.path, None));
                }
                _ => {
                    for (place, region) in
                        storage_regions::written_or_exclusive_regions(function, types, operation)
                    {
                        let mut projection = Projection::new(&region.path);
                        projection.indirect |= region.indirect;
                        retain(place, projection);
                    }
                }
            }
            for place in remaining {
                result.push(Access {
                    place,
                    location: borrowed_regions.location_at(position, place, &[]).cloned(),
                    regions: borrowed_regions.resolve_at(position, place, Projection::new(&[])),
                });
            }
            result
        }

        fn descriptor_flow(
            function: &PsiOptimizationFunction,
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            borrowed_regions: &BorrowedRegions,
        ) -> BTreeMap<Position, DescriptorFlowStep> {
            let mut steps = borrowed_regions
                .descriptor_flow()
                .cloned()
                .unwrap_or_else(|| unknown_flow(function));
            for block in &function.blocks {
                for (index, node) in block.nodes.iter().enumerate() {
                    let position = (block.id, index as u32);
                    let Some(step) = steps.get_mut(&position) else {
                        continue;
                    };
                    let mut all = operation_place_inputs(&node.operation);
                    for retained in descriptor_inputs(function, types, &node.operation) {
                        if let Some(index) = all.iter().position(|place| *place == retained) {
                            all.remove(index);
                        }
                    }
                    for disposed in all {
                        let raw = DescriptorLocation::whole(disposed);
                        let canonical = borrowed_regions.location_at(position, disposed, &[]);
                        if let Some(index) = step
                            .inputs
                            .iter()
                            .rposition(|input| input == &raw || Some(input) == canonical)
                        {
                            step.inputs.remove(index);
                        }
                    }
                }
            }
            steps
        }

        fn unknown_flow(
            function: &PsiOptimizationFunction,
        ) -> BTreeMap<Position, DescriptorFlowStep> {
            let mut steps = BTreeMap::new();
            for block in &function.blocks {
                for (index, node) in block.nodes.iter().enumerate() {
                    let targets = if node.successors.is_empty() {
                        (index + 1 < block.nodes.len())
                            .then_some((block.id, index as u32 + 1))
                            .into_iter()
                            .collect::<Vec<_>>()
                    } else {
                        node.successors
                            .iter()
                            .map(|edge| (edge.target, 0))
                            .collect()
                    };
                    steps.insert(
                        (block.id, index as u32),
                        DescriptorFlowStep {
                            inputs: function
                                .structural_places
                                .iter()
                                .map(|place| DescriptorLocation::whole(place.id))
                                .collect(),
                            may_escape: true,
                            successors: targets
                                .into_iter()
                                .map(|target| DescriptorFlowEdge {
                                    target,
                                    parameters: vec![],
                                    bindings: vec![],
                                })
                                .collect(),
                            ..DescriptorFlowStep::default()
                        },
                    );
                }
            }
            steps
        }

        fn descriptor_inputs(
            function: &PsiOptimizationFunction,
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            operation: &O,
        ) -> Vec<PlaceId> {
            let mut inputs = operation_place_inputs(operation);
            let mut dispose = |place: PlaceId| {
                let descriptor =
                    structural_source_contract(function, place).is_some_and(|source| {
                        source.operation_result().is_some()
                            && source.multiplicity == terminal_psi::StructuralMultiplicity::Affine
                            && matches!(source.access, terminal_psi::reading_borrow_pair_pattern!())
                            && types
                                .get(&source.structural_type)
                                .is_some_and(|row| shared_view_declaration(row))
                    });
                if descriptor
                    && let Some(position) = inputs.iter().rposition(|input| *input == place)
                {
                    inputs.remove(position);
                }
            };
            match operation {
                O::Jump {
                    cleanup_actions, ..
                }
                | return_or_return_unit_pattern!(cleanup_actions, ..)
                | O::ReturnStructural {
                    cleanup_actions, ..
                } => {
                    for cleanup in cleanup_actions {
                        if let terminal_psi::TerminalAffineCleanupAction::DiscardRoot(place)
                        | terminal_psi::TerminalAffineCleanupAction::RetireHome(place) = cleanup
                        {
                            dispose(*place);
                        }
                    }
                }
                _ => {}
            }
            inputs
        }
    }
    pub(crate) use descriptor_loans::validate_compatibility;
    pub(crate) mod range_relations {
        use abstract_operations::{AbstractOperation as O, ValueBinding};
        use optimization_unit::PsiOptimizationFunction;
        use semantic_vocabulary::{BlockId, MachineId, Proposition, ScalarType, ValueId};
        use std::collections::BTreeMap;
        use terminal_semantics::scalar_ordering::{Binding, Edge, Effect, ScalarOrdering, Step};

        pub(crate) struct RangeRelations(ScalarOrdering);
        impl RangeRelations {
            pub(crate) fn new(
                function: &PsiOptimizationFunction,
                functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
            ) -> Self {
                let positions = function
                    .blocks
                    .iter()
                    .flat_map(|block| {
                        (0..block.nodes.len()).filter_map(|index| {
                            u32::try_from(index).ok().map(|index| (block.id, index))
                        })
                    })
                    .collect::<Vec<_>>();
                let mut steps = Vec::with_capacity(positions.len());
                for position in &positions {
                    let block = function
                        .block(position.0)
                        .expect("position comes from block");
                    let operation = &block.nodes[position.1 as usize].operation;
                    let edge = |target: BlockId, bindings: &[ValueBinding], condition| {
                        let target_index = positions
                            .iter()
                            .position(|position| *position == (target, 0))?;
                        let block = function.block(target)?;
                        Some(Edge {
                            target: target_index,
                            enters_block: true,
                            parameters: block
                                .parameters
                                .iter()
                                .map(|parameter| (parameter.value, parameter.scalar_type))
                                .collect(),
                            bindings: bindings
                                .iter()
                                .map(|binding| Binding {
                                    parameter: binding.parameter,
                                    argument: binding.argument,
                                    scalar_type: binding.scalar_type,
                                })
                                .collect(),
                            condition,
                        })
                    };
                    let successors = match operation {
                        O::Jump {
                            target, bindings, ..
                        } => edge(*target, bindings, None).into_iter().collect(),
                        O::Conditional {
                            condition,
                            when_true,
                            when_false,
                        } => [
                            edge(
                                when_true.target,
                                &when_true.bindings,
                                Some((*condition, true)),
                            ),
                            edge(
                                when_false.target,
                                &when_false.bindings,
                                Some((*condition, false)),
                            ),
                        ]
                        .into_iter()
                        .flatten()
                        .collect(),
                        O::StructuralCase { cases, .. } => cases
                            .iter()
                            .filter_map(|case| edge(case.target, &[], None))
                            .collect(),
                        abstract_operations::return_or_return_unit_pattern!()
                        | abstract_operations::return_structural_or_crash_pattern!() => Vec::new(),
                        _ => positions
                            .iter()
                            .position(|next| *next == (position.0, position.1 + 1))
                            .map(|target| Edge {
                                target,
                                enters_block: false,
                                parameters: Vec::new(),
                                bindings: Vec::new(),
                                condition: None,
                            })
                            .into_iter()
                            .collect(),
                    };
                    steps.push(Step {
                        position: *position,
                        definitions:
                            crate::unit_validation::derived_metadata::expected_definitions(
                                operation, position.0, position.1,
                            )
                            .iter()
                            .map(|definition| (definition.value, definition.scalar_type))
                            .collect(),
                        effect: match operation {
                            O::IntegerLessThan {
                                result,
                                left,
                                right,
                                ..
                            }
                            | O::IntegerLessOrEqual {
                                result,
                                left,
                                right,
                                ..
                            } => Effect::Compare {
                                result: *result,
                                left: *left,
                                right: *right,
                                strict: matches!(operation, O::IntegerLessThan { .. }),
                            },
                            O::BooleanNot {
                                result, operand, ..
                            } => Effect::Not {
                                result: *result,
                                operand: *operand,
                            },
                            O::IntegerConstant {
                                result,
                                value: semantic_vocabulary::IntegerValue::Unsigned(value),
                                ..
                            } => Effect::UnsignedLiteral {
                                result: *result,
                                value: *value,
                            },
                            O::EstablishPrimitiveLocal { result, value, .. } => {
                                Effect::EstablishStorage {
                                    place: result.place,
                                    value: value.value,
                                }
                            }
                            O::PrimitiveLocalStore {
                                destination, value, ..
                            } => Effect::StoreStorage {
                                place: *destination,
                                value: value.value,
                            },
                            O::WriteOnlyPrimitiveStore {
                                destination,
                                path,
                                value,
                                ..
                            } if path.is_empty() => Effect::StoreStorage {
                                place: destination.place(),
                                value: value.value,
                            },
                            O::PrimitiveScalarRead {
                                source,
                                path,
                                result,
                                ..
                            } if path.is_empty() => Effect::ReadStorage {
                                place: *source,
                                result: result.value,
                            },
                            O::EstablishReference { .. }
                            | O::EstablishAccessReference { .. }
                            | O::EstablishElementView { .. } => Effect::None,
                            abstract_operations::call_or_call_structural_pattern!(..)
                            | O::BoundaryCall { .. }
                            | O::ExternalCompletion { .. } => Effect::ForgetStorage,
                            _ if !operation.storage_mutations().is_empty() => Effect::ForgetStorage,
                            _ => Effect::None,
                        },
                        successors,
                    });
                }
                let scalar_types = function
                    .parameters
                    .iter()
                    .chain(function.block_parameters())
                    .map(|parameter| (parameter.value, parameter.scalar_type))
                    .chain(
                        steps
                            .iter()
                            .flat_map(|step| step.definitions.iter().copied()),
                    )
                    .collect::<Vec<_>>();
                let normal_returns = function
                    .blocks
                    .iter()
                    .flat_map(|block| {
                        let scalar_types = &scalar_types;
                        block
                            .nodes
                            .iter()
                            .enumerate()
                            .filter_map(move |(index, node)| {
                                let propositions =
                                    call_guarantees(&node.operation, functions, scalar_types)?;
                                Some(((block.id, u32::try_from(index).ok()?), propositions))
                            })
                    })
                    .collect();
                Self(ScalarOrdering::with_contracts(
                    steps,
                    (function.entry, 0),
                    function
                        .parameters
                        .iter()
                        .map(|parameter| (parameter.value, parameter.scalar_type))
                        .collect(),
                    function
                        .verified_contract
                        .as_ref()
                        .map_or(&[], |contract| contract.requires.as_slice()),
                    normal_returns,
                ))
            }
            pub(crate) fn into_ordering(self) -> ScalarOrdering {
                self.0
            }

            pub(super) fn ordering(&self) -> &ScalarOrdering {
                &self.0
            }
        }
        fn call_guarantees(
            operation: &O,
            functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
            scalar_types: &[(ValueId, ScalarType)],
        ) -> Option<Vec<Proposition>> {
            let guarantees = crate::descriptor_results::scalar_call_guarantees(
                operation,
                &|identity| functions.get(&identity).copied(),
                &|identity| {
                    let (_, selected) =
                        scalar_types.iter().find(|(value, _)| *value == identity)?;
                    scalar_types
                        .iter()
                        .all(|(value, candidate)| *value != identity || candidate == selected)
                        .then_some(*selected)
                },
            )?;
            let bindings = scalar_types
                .iter()
                .map(|(value, scalar_type)| {
                    (
                        *value,
                        semantic_vocabulary::ScalarTerm::value(*value, *scalar_type),
                    )
                })
                .collect::<BTreeMap<_, _>>()
                .into_iter()
                .collect::<Vec<_>>();
            Some(
                guarantees
                    .iter()
                    .flat_map(|clause| {
                        terminal_semantics::scalar_ordering::instantiate_contract_ordering(
                            clause, &bindings,
                        )
                    })
                    .collect(),
            )
        }
    }
    use crate::current_ownership::CurrentOwnership;
    use crate::unit_validation::function_structure::operation_place_inputs;
    use crate::unit_validation::operation_contracts::structural_source_contract;
    use crate::unit_validation::references::ReferenceOrigin;
    use crate::{OperationSite, OptimizationUnitValidationError};
    use abstract_operations::AbstractOperation as O;
    use abstract_operations::control_flow::address_joins::is_address_join;
    use borrowed_regions::{BorrowedRegions, Projection, Region};
    use descriptor_loans::DescriptorLoans;
    use optimization_unit::{OptimizationEdge, PsiOptimizationFunction};
    use semantic_vocabulary::{BlockId, MachineId, PlaceId, StructuralTypeId};
    use std::collections::{BTreeMap, BTreeSet};
    use terminal_psi::exclusive_borrow_pair_pattern;
    use terminal_psi::{StructuralAccess, StructuralTypeDeclaration};

    #[derive(Default)]
    pub(super) struct AddressJoinPins {
        borrowed_regions: BorrowedRegions,
        descriptor_loans: DescriptorLoans,
        shared: BTreeMap<BlockId, Vec<Region>>,
        exclusive: BTreeMap<BlockId, BTreeSet<PlaceId>>,
        origins: BTreeMap<PlaceId, BTreeSet<PlaceId>>,
        dominators: BTreeMap<BlockId, BTreeSet<BlockId>>,
        readers: BTreeMap<PlaceId, BTreeSet<BlockId>>,
    }

    impl AddressJoinPins {
        pub(super) fn reference_sources(&self) -> &borrowed_regions::BorrowedReferenceSources {
            self.borrowed_regions.reference_sources()
        }

        pub(super) fn descriptor_requirements(
            &self,
        ) -> Vec<terminal_semantics::descriptor_results::DescriptorCompatibilityRequirement>
        {
            self.descriptor_loans.requirements()
        }

        pub(super) fn new(
            function: &PsiOptimizationFunction,
            functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
            structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            call_accesses: &mut super::CallAccesses,
        ) -> Self {
            let borrowed_regions =
                BorrowedRegions::new(function, functions, structural_types, call_accesses);
            let descriptor_loans =
                DescriptorLoans::new(function, &borrowed_regions, structural_types);
            let joins = function
                .blocks
                .iter()
                .flat_map(|block| {
                    block
                        .structural_parameters
                        .iter()
                        .filter(|parameter| {
                            parameter.access == StructuralAccess::SharedBorrow
                                && is_address_join(parameter, |identity| {
                                    structural_types.get(&identity).copied()
                                })
                        })
                        .map(move |parameter| (parameter.place, block.id))
                })
                .collect::<BTreeMap<PlaceId, BlockId>>();
            let mut origins = BTreeMap::<PlaceId, BTreeSet<PlaceId>>::new();
            for edge in function.block_edges() {
                for binding in &edge.structural_bindings {
                    if joins.contains_key(&binding.parameter) {
                        origins
                            .entry(binding.parameter)
                            .or_default()
                            .insert(binding.argument.place);
                    }
                }
            }
            let address_joins = function
                .block_structural_parameters()
                .filter(|parameter| {
                    is_address_join(parameter, |identity| {
                        structural_types.get(&identity).copied()
                    })
                })
                .map(|parameter| parameter.place)
                .collect::<BTreeSet<_>>();
            if address_joins.is_empty() {
                return Self {
                    borrowed_regions,
                    descriptor_loans,
                    ..Self::default()
                };
            }
            for edge in function.block_edges() {
                for binding in &edge.structural_bindings {
                    if address_joins.contains(&binding.parameter) {
                        origins
                            .entry(binding.parameter)
                            .or_default()
                            .insert(binding.argument.place);
                    }
                }
            }
            loop {
                let mut expanded = origins.clone();
                for roots in expanded.values_mut() {
                    let inherited = roots
                        .iter()
                        .filter_map(|root| origins.get(root))
                        .flatten()
                        .copied()
                        .collect::<Vec<_>>();
                    roots.extend(inherited);
                }
                if expanded == origins {
                    break;
                }
                origins = expanded;
            }
            let dominators =
                crate::candidates::global_value_numbering::independent_reachable_dominators(
                    function,
                );
            let mut predecessors = BTreeMap::<BlockId, BTreeSet<BlockId>>::new();
            for block in &function.blocks {
                for edge in block.successors() {
                    predecessors
                        .entry(edge.target)
                        .or_default()
                        .insert(block.id);
                }
            }
            let mut pins = BTreeMap::<BlockId, Vec<Region>>::new();
            for (join, owner) in &joins {
                let live_blocks =
                    shared_join_live_blocks(function, *join, *owner, &origins, &predecessors);
                let regions = borrowed_regions.resolve(*join, Projection::new(&[]));
                for (block, dominating) in &dominators {
                    if (block == owner || dominating.contains(owner))
                        && live_blocks.as_ref().is_none_or(|live| live.contains(block))
                    {
                        pins.entry(*block)
                            .or_default()
                            .extend(regions.iter().cloned());
                    }
                }
            }
            let mut readers = BTreeMap::<PlaceId, BTreeSet<BlockId>>::new();
            for block in &function.blocks {
                for node in &block.nodes {
                    for place in operation_place_inputs(&node.operation) {
                        readers.entry(place).or_default().insert(block.id);
                    }
                }
            }
            let exclusive_joins = function
                .blocks
                .iter()
                .flat_map(|block| {
                    block
                        .structural_parameters
                        .iter()
                        .filter(|parameter| {
                            matches!(parameter.access, exclusive_borrow_pair_pattern!())
                                && address_joins.contains(&parameter.place)
                        })
                        .map(|parameter| parameter.place)
                })
                .collect::<Vec<_>>();
            if exclusive_joins.is_empty() {
                return Self {
                    borrowed_regions,
                    descriptor_loans,
                    shared: pins,
                    exclusive: BTreeMap::new(),
                    origins,
                    dominators,
                    readers,
                };
            }
            let mut aliases = BTreeMap::<PlaceId, BTreeSet<PlaceId>>::new();
            let mut descriptors = BTreeSet::new();
            for node in function.block_nodes() {
                if let Some((_, result)) = node.operation.structural_result()
                    && let Some(sources) = borrowed_regions.direct_aliases(result.place)
                {
                    descriptors.insert(result.place);
                    aliases
                        .entry(result.place)
                        .or_default()
                        .extend(sources.iter().map(|(source, _)| *source));
                }
            }
            for block in &function.blocks {
                for parameter in &block.structural_parameters {
                    if !address_joins.contains(&parameter.place)
                        && parameter.access != StructuralAccess::Owned
                    {
                        descriptors.insert(parameter.place);
                    }
                }
                for edge in block.successors() {
                    for binding in &edge.structural_bindings {
                        if binding.argument.access != StructuralAccess::Owned {
                            aliases
                                .entry(binding.parameter)
                                .or_default()
                                .insert(binding.argument.place);
                        }
                    }
                }
            }
            let direct_aliases = aliases.clone();
            loop {
                let previous = aliases.clone();
                for roots in aliases.values_mut() {
                    let inherited = roots
                        .iter()
                        .filter_map(|root| previous.get(root))
                        .flatten()
                        .copied()
                        .collect::<Vec<_>>();
                    roots.extend(inherited);
                }
                if aliases == previous {
                    break;
                }
            }
            let available = crate::unit_validation::function_structure::current_mutable_borrows(
                function,
                structural_types,
            );
            let mut exclusive = BTreeMap::<BlockId, BTreeSet<PlaceId>>::new();
            for join in exclusive_joins {
                let roots = origins
                    .get(&join)
                    .into_iter()
                    .flatten()
                    .filter(|root| {
                        function.structural_places.iter().any(|place| {
                            place.id == **root
                                && matches!(
                                    place.kind,
                                    semantic_vocabulary::StructuralPlaceKind::OperationResult { .. }
                                )
                        }) && structural_source_contract(function, **root)
                            .is_some_and(|source| source.access == StructuralAccess::Owned)
                    })
                    .copied()
                    .collect::<BTreeSet<_>>();
                if roots.is_empty() {
                    continue;
                }
                let mut suspended = roots.clone();
                suspended.extend(
                    descriptors
                        .iter()
                        .filter(|descriptor| {
                            aliases.get(*descriptor).is_some_and(|origins| {
                                origins.iter().any(|origin| roots.contains(origin))
                            }) && !entirely_descends_from(
                                **descriptor,
                                join,
                                &direct_aliases,
                                &mut BTreeSet::new(),
                            )
                        })
                        .copied(),
                );
                for block in dominators.keys() {
                    if available.may_be_live(*block, join) {
                        exclusive
                            .entry(*block)
                            .or_default()
                            .extend(suspended.iter().copied());
                    }
                }
            }
            Self {
                borrowed_regions,
                descriptor_loans,
                shared: pins,
                exclusive,
                origins: aliases,
                dominators,
                readers,
            }
        }

        pub(super) fn check_operation(
            &self,
            function: &PsiOptimizationFunction,
            block: BlockId,
            node: u32,
            operation: &O,
            moved: impl IntoIterator<Item = PlaceId>,
        ) -> Result<(), OptimizationUnitValidationError> {
            let moved = moved.into_iter().collect::<Vec<_>>();
            if let Some(place) = self.descriptor_loans.conflicted_operation(
                &self.borrowed_regions,
                block,
                node,
                operation,
                &moved,
                &mut Vec::new(),
            ) {
                return Err(pinned_root_disturbed(function, block, node, place));
            }
            if let Some(pinned) = self.exclusive.get(&block) {
                let inputs =
                    crate::unit_validation::function_structure::operation_place_accesses(operation);
                if let Some(place) = inputs.into_iter().find(|place| pinned.contains(place)) {
                    return Err(pinned_root_disturbed(function, block, node, place));
                }
            }
            let Some(pinned) = self.shared.get(&block) else {
                return Ok(());
            };
            let disturbed =
                self.borrowed_regions
                    .conflicted_operation(pinned, (block, node), &moved);
            match disturbed {
                Some(place) => Err(pinned_root_disturbed(function, block, node, place)),
                None => Ok(()),
            }
        }

        pub(super) fn check_edge(
            &self,
            function: &PsiOptimizationFunction,
            block: BlockId,
            node: u32,
            edge: &OptimizationEdge,
            destination: &[terminal_psi::StructuralParameterDeclaration],
            frontier: &CurrentOwnership,
        ) -> Result<(), OptimizationUnitValidationError> {
            if let Some(place) = self.descriptor_loans.conflicted_edge(
                &self.borrowed_regions,
                block,
                node,
                edge,
                destination,
                &mut Vec::new(),
            ) {
                return Err(pinned_root_disturbed(function, block, node, place));
            }
            for (binding, parameter) in edge.structural_bindings.iter().zip(destination) {
                if !matches!(parameter.access, exclusive_borrow_pair_pattern!()) {
                    continue;
                }
                let place = binding.argument.place;
                let Some(source) = structural_source_contract(function, place)
                    .filter(|source| source.access == StructuralAccess::Owned)
                else {
                    continue;
                };
                let owner_live = match source.multiplicity {
                    terminal_psi::StructuralMultiplicity::Affine => {
                        frontier.owned_places.get(&place)
                            == Some(&terminal_psi::StructuralMultiplicity::Affine)
                    }
                    terminal_psi::StructuralMultiplicity::Unrestricted => true,
                    terminal_psi::StructuralMultiplicity::Linear => false,
                };
                if !owner_live
                    || frontier.partial_custody_paths.contains_key(&place)
                    || frontier
                        .claims
                        .values()
                        .any(|claim| claim.input == Some(place))
                    || frontier.live_references.iter().any(|reference| {
                        reference.root == ReferenceOrigin::Primitive(place)
                            || reference.carrier == place
                    })
                    || edge
                        .structural_bindings
                        .iter()
                        .filter(|other| {
                            other.argument.place == place
                                || self
                                    .origins
                                    .get(&other.argument.place)
                                    .is_some_and(|origins| origins.contains(&place))
                        })
                        .count()
                        != 1
                    || edge
                        .cleanup_actions
                        .iter()
                        .any(|action| action.place() == place)
                {
                    return Err(pinned_root_disturbed(function, block, node, place));
                }
            }
            if let Some(pinned) = self.exclusive.get(&block)
                && let Some(place) = edge
                    .structural_bindings
                    .iter()
                    .map(|binding| binding.argument.place)
                    .chain(
                        edge.cleanup_actions
                            .iter()
                            .filter_map(|action| match action {
                                terminal_psi::TerminalAffineCleanupAction::DiscardRoot(place)
                                | terminal_psi::TerminalAffineCleanupAction::RetireHome(place) => {
                                    self.read_after(place, edge.target).then_some(*place)
                                }
                                action => Some(action.place()),
                            }),
                    )
                    .find(|place| pinned.contains(place))
            {
                return Err(pinned_root_disturbed(function, block, node, place));
            }
            let Some(pinned) = self.shared.get(&block) else {
                return Ok(());
            };
            let disturbed = edge
                .structural_bindings
                .iter()
                .zip(destination)
                .filter(|(binding, parameter)| match binding.argument.access {
                    StructuralAccess::SharedBorrow => false,
                    StructuralAccess::Owned => {
                        parameter.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
                    }
                    exclusive_borrow_pair_pattern!() => true,
                })
                .map(|(binding, _)| (binding.argument.place, binding.argument.path.as_slice()))
                .chain(edge.cleanup_actions.iter().map(|action| match action {
                    terminal_psi::TerminalAffineCleanupAction::DiscardResidual(discard) => {
                        (discard.place, discard.path.as_slice())
                    }
                    action => (action.place(), &[][..]),
                }))
                .find(|(place, path)| {
                    self.borrowed_regions.overlaps(pinned, *place, path)
                        && self.read_after(place, edge.target)
                })
                .map(|(place, _)| place);
            match disturbed {
                Some(place) => Err(pinned_root_disturbed(function, block, node, place)),
                None => Ok(()),
            }
        }

        fn read_after(&self, place: &PlaceId, target: BlockId) -> bool {
            let mut names = BTreeSet::from([*place]);
            for (name, roots) in &self.origins {
                if roots.contains(place) {
                    names.insert(*name);
                }
            }
            names.iter().any(|name| {
                self.readers.get(name).is_some_and(|blocks| {
                    blocks.iter().any(|reader| {
                        *reader == target
                            || self
                                .dominators
                                .get(reader)
                                .is_some_and(|dominating| dominating.contains(&target))
                    })
                })
            })
        }
    }

    fn pinned_root_disturbed(
        function: &PsiOptimizationFunction,
        block: BlockId,
        node: u32,
        place: PlaceId,
    ) -> OptimizationUnitValidationError {
        OptimizationUnitValidationError::CurrentSharedJoinOriginDisturbed(
            OperationSite::new(function.machine, block, node),
            place,
        )
    }

    fn entirely_descends_from(
        place: PlaceId,
        ancestor: PlaceId,
        parents: &BTreeMap<PlaceId, BTreeSet<PlaceId>>,
        active: &mut BTreeSet<PlaceId>,
    ) -> bool {
        if place == ancestor {
            return true;
        }
        if !active.insert(place) {
            return false;
        }
        let descends = parents.get(&place).is_some_and(|sources| {
            !sources.is_empty()
                && sources
                    .iter()
                    .all(|source| entirely_descends_from(*source, ancestor, parents, active))
        });
        active.remove(&place);
        descends
    }

    fn shared_join_live_blocks(
        function: &PsiOptimizationFunction,
        join: PlaceId,
        owner: BlockId,
        origins: &BTreeMap<PlaceId, BTreeSet<PlaceId>>,
        predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
    ) -> Option<BTreeSet<BlockId>> {
        let mut names = BTreeSet::from([join]);
        names.extend(
            origins
                .iter()
                .filter_map(|(name, roots)| roots.contains(&join).then_some(*name)),
        );
        let mut live = BTreeSet::from([owner]);
        for block in &function.blocks {
            if block
                .structural_parameters
                .iter()
                .any(|parameter| names.contains(&parameter.place))
            {
                live.insert(block.id);
            }
            for node in &block.nodes {
                let inputs = operation_place_inputs(&node.operation);
                if !inputs.iter().any(|place| names.contains(place)) {
                    continue;
                }
                let cannot_retain_alias = match &node.operation {
                    abstract_operations::scalar_storage_observation_pattern!(..)
                    | O::ByteSequenceLength { .. }
                    | O::ElementViewLength { .. }
                    | O::ByteSequenceRead { .. }
                    | abstract_operations::byte_len_or_field_read_pattern!(..) => true,
                    abstract_operations::jump_or_conditional_pattern!() => {
                        inputs
                            .iter()
                            .filter(|place| names.contains(place))
                            .all(|place| {
                                node.successors.iter().any(|edge| {
                                    edge.structural_bindings.iter().any(|binding| {
                                        binding.argument.place == *place
                                            && binding.argument.access
                                                == StructuralAccess::SharedBorrow
                                            && names.contains(&binding.parameter)
                                    })
                                })
                            })
                            && node
                                .successors
                                .iter()
                                .flat_map(|edge| &edge.structural_bindings)
                                .filter(|binding| names.contains(&binding.argument.place))
                                .all(|binding| {
                                    binding.argument.access == StructuralAccess::SharedBorrow
                                        && names.contains(&binding.parameter)
                                })
                    }
                    _ => false,
                };
                if !cannot_retain_alias {
                    return None;
                }
                live.insert(block.id);
            }
        }
        let mut pending = live.iter().copied().collect::<Vec<_>>();
        while let Some(block) = pending.pop() {
            for predecessor in predecessors.get(&block).into_iter().flatten() {
                if live.insert(*predecessor) {
                    pending.push(*predecessor);
                }
            }
        }
        Some(live)
    }
}
mod block_parameters {
    use super::references::owned_resident;
    use crate::OptimizationUnitValidationError;
    use crate::current_ownership::structural::projected_root_is_fully_consumed;
    use crate::current_ownership::{CurrentOwnership, LiveClaim};
    use crate::unit_validation::operation_contracts::structural_source_contract;
    use abstract_operations::AbstractStructuralBinding;
    use optimization_unit::{OptimizationBlock, PsiOptimizationFunction};
    use semantic_vocabulary::{BlockId, ClaimId, PlaceId, StructuralCaseId, StructuralTypeId};
    use std::collections::{BTreeMap, BTreeSet};
    use terminal_psi::{
        StructuralAccess, StructuralMultiplicity, StructuralParameterDeclaration,
        StructuralPathSegment, StructuralTypeDeclaration,
    };

    pub(super) struct EdgeTransfer {
        claims: Vec<(ClaimId, LiveClaim)>,
    }

    pub(super) fn consume_edge_sources(
        function: &PsiOptimizationFunction,
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        block: BlockId,
        frontier: &mut CurrentOwnership,
        bindings: &[AbstractStructuralBinding],
        target: &OptimizationBlock,
        case_is_excluded: impl Fn(PlaceId, &[StructuralPathSegment], StructuralCaseId) -> bool,
    ) -> Result<EdgeTransfer, OptimizationUnitValidationError> {
        let invalid = || OptimizationUnitValidationError::CurrentCleanupMismatch {
            machine: function.machine,
            block,
        };
        let mut transferred = Vec::new();
        let mut copied_references = Vec::new();
        for (binding, parameter) in bindings.iter().zip(&target.structural_parameters) {
            if parameter.access != StructuralAccess::Owned {
                continue;
            }
            let source = binding.argument.place;
            if parameter.multiplicity == StructuralMultiplicity::Unrestricted {
                let contract = structural_source_contract(function, source).ok_or_else(invalid)?;
                if contract.access != StructuralAccess::Owned
                    || contract.multiplicity != StructuralMultiplicity::Unrestricted
                    || frontier.owned_places.contains_key(&source)
                    || frontier.partial_custody_paths.contains_key(&source)
                    || frontier
                        .claims
                        .values()
                        .any(|claim| claim.input == Some(source))
                {
                    return Err(invalid());
                }
                let node = function
                    .block(block)
                    .and_then(|block| u32::try_from(block.nodes.len().checked_sub(1)?).ok())
                    .ok_or_else(invalid)?;
                let copied = super::references::copy_reference_carriers(
                    function,
                    types,
                    block,
                    node,
                    &frontier.live_references,
                    source,
                    &binding.argument.path,
                    parameter.structural_type,
                    parameter.place,
                )?;
                let retains_source = source == parameter.place && binding.argument.path.is_empty();
                copied_references.extend(
                    copied
                        .into_iter()
                        .map(|reference| (reference, retains_source)),
                );
                continue;
            }
            if !binding.argument.path.is_empty() {
                transferred.extend(consume_projected_source(
                    function, types, block, frontier, binding, parameter,
                )?);
                continue;
            }
            if parameter.multiplicity != StructuralMultiplicity::Unrestricted
                && !frontier.live_references.is_empty()
            {
                let mut remaining = frontier.live_references.clone();
                super::references::discard_owned(function, types, block, &mut remaining, source)?;
                if remaining.iter().any(|reference| {
                    super::references::borrows_owned_storage(function, types, reference, source)
                        || (reference.carrier == source
                            && owned_resident(&reference.carrier_path, &[]))
                }) {
                    return Err(invalid());
                }
            }
            if frontier.partial_custody_paths.contains_key(&source)
                || (parameter.multiplicity != StructuralMultiplicity::Unrestricted
                    && frontier.owned_places.remove(&source) != Some(parameter.multiplicity))
            {
                return Err(invalid());
            }
            let claims = frontier
                .claims
                .iter()
                .filter_map(|(identity, claim)| (claim.input == Some(source)).then_some(*identity))
                .collect::<Vec<_>>();
            if (parameter.multiplicity == StructuralMultiplicity::Linear && claims.is_empty())
                || (parameter.multiplicity == StructuralMultiplicity::Unrestricted
                    && !claims.is_empty())
            {
                return Err(invalid());
            }
            for identity in claims {
                let mut claim = frontier
                    .claims
                    .remove(&identity)
                    .expect("captured live claim");
                claim.input = Some(parameter.place);
                transferred.push((identity, claim));
            }
        }
        for reference in &mut frontier.live_references {
            let Some((binding, parameter)) = bindings
                .iter()
                .zip(&target.structural_parameters)
                .find(|(binding, parameter)| {
                    parameter.access == StructuralAccess::Owned
                        && parameter.multiplicity != StructuralMultiplicity::Unrestricted
                        && binding.argument.access == StructuralAccess::Owned
                        && binding.argument.place == reference.carrier
                        && owned_resident(&reference.carrier_path, &binding.argument.path)
                })
            else {
                continue;
            };
            reference.carrier = parameter.place;
            reference.carrier_path.drain(..binding.argument.path.len());
        }
        for (copied, retains_source) in copied_references {
            if retains_source
                && frontier
                    .live_references
                    .iter()
                    .any(|reference| reference == &copied)
            {
                continue;
            }
            if frontier.live_references.iter().any(|reference| {
                reference.identity == copied.identity
                    || reference.carrier == copied.carrier
                        && reference.carrier_path == copied.carrier_path
            }) {
                return Err(invalid());
            }
            frontier.live_references.push(copied);
        }
        let mut bound = Vec::with_capacity(transferred.len());
        for (binding, parameter) in bindings.iter().zip(&target.structural_parameters) {
            let catalogue = function
                .block_entry_claims
                .iter()
                .filter(|claim| claim.input == parameter.place)
                .collect::<Vec<_>>();
            let mut active = catalogue
                .iter()
                .copied()
                .filter(|claim| {
                    let mut source_path = binding.argument.path.clone();
                    for segment in &claim.path {
                        if let StructuralPathSegment::Case(case) = segment
                            && case_is_excluded(binding.argument.place, &source_path, *case)
                        {
                            return false;
                        }
                        source_path.push(segment.clone());
                    }
                    true
                })
                .collect::<Vec<_>>();
            for (identity, claim) in transferred
                .iter()
                .filter(|(_, claim)| claim.input == Some(parameter.place))
            {
                let destination = if catalogue.is_empty() {
                    *identity
                } else {
                    let matching = active
                        .iter()
                        .position(|binding| binding.path == claim.path)
                        .ok_or_else(invalid)?;
                    let binding = active.swap_remove(matching);
                    let multiplicity = if binding.path.is_empty() {
                        parameter.multiplicity
                    } else {
                        StructuralMultiplicity::Linear
                    };
                    if claim.multiplicity != Some(multiplicity) {
                        return Err(invalid());
                    }
                    binding.claim
                };
                if bound.iter().any(|(identity, _)| *identity == destination) {
                    return Err(invalid());
                }
                bound.push((destination, claim.clone()));
            }
            if !active.is_empty() {
                return Err(invalid());
            }
        }
        if bound.len() != transferred.len() {
            return Err(invalid());
        }
        Ok(EdgeTransfer { claims: bound })
    }

    fn consume_projected_source(
        function: &PsiOptimizationFunction,
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        block: BlockId,
        frontier: &mut CurrentOwnership,
        binding: &AbstractStructuralBinding,
        parameter: &StructuralParameterDeclaration,
    ) -> Result<Vec<(ClaimId, LiveClaim)>, OptimizationUnitValidationError> {
        let invalid = || OptimizationUnitValidationError::CurrentCleanupMismatch {
            machine: function.machine,
            block,
        };
        let argument = &binding.argument;
        if argument.access != StructuralAccess::Owned
            || !matches!(
                parameter.multiplicity,
                StructuralMultiplicity::Affine | StructuralMultiplicity::Linear
            )
            || frontier.owned_places.get(&argument.place) != Some(&StructuralMultiplicity::Affine)
            || function.content_entry_claim_declared(argument.place)
            || !structural_source_contract(function, argument.place).is_some_and(|source| {
                source.access == StructuralAccess::Owned
                    && source.multiplicity == StructuralMultiplicity::Affine
                    && crate::unit_validation::structural_catalog::resolve_structural_path(
                        types,
                        source.structural_type,
                        &argument.path,
                    ) == Some(parameter.structural_type)
            })
        {
            return Err(invalid());
        }
        let mut transferred = Vec::new();
        for (identity, claim) in &frontier.claims {
            if claim.input != Some(argument.place) {
                continue;
            }
            if let Some(suffix) = claim.path.strip_prefix(argument.path.as_slice()) {
                let mut moved = claim.clone();
                moved.input = Some(parameter.place);
                moved.path = suffix.to_vec();
                transferred.push((*identity, moved));
            } else if argument.path.starts_with(&claim.path) {
                return Err(invalid());
            }
        }
        if match parameter.multiplicity {
            StructuralMultiplicity::Linear => {
                !matches!(transferred.as_slice(), [(_, claim)] if claim.path.is_empty() && claim.multiplicity == Some(StructuralMultiplicity::Linear))
            }
            StructuralMultiplicity::Affine => {
                transferred.iter().any(|(_, claim)| claim.path.is_empty())
            }
            StructuralMultiplicity::Unrestricted => true,
        } {
            return Err(invalid());
        }
        if !frontier.live_references.is_empty() {
            let mut remaining = frontier.live_references.clone();
            super::references::discard_subtree(
                function,
                types,
                block,
                &mut remaining,
                argument.place,
                &argument.path,
            )?;
            let terminator = function
                .block(block)
                .and_then(|candidate| u32::try_from(candidate.nodes.len().checked_sub(1)?).ok())
                .ok_or_else(invalid)?;
            super::references::check_region_access(
                function,
                block,
                terminator,
                &remaining,
                argument.place,
                &argument.path,
            )?;
        }
        let moved = frontier
            .partial_custody_paths
            .entry(argument.place)
            .or_default();
        if moved.iter().any(|existing| {
            existing.starts_with(&argument.path) || argument.path.starts_with(existing)
        }) || !moved.insert(argument.path.clone())
        {
            return Err(invalid());
        }
        for (identity, _) in &transferred {
            frontier.claims.remove(identity);
        }
        if projected_root_is_fully_consumed(function, types, frontier, argument.place) {
            frontier.owned_places.remove(&argument.place);
        }
        Ok(transferred)
    }

    pub(super) fn establish_edge_parameters(
        function: &PsiOptimizationFunction,
        block: BlockId,
        frontier: &mut CurrentOwnership,
        target: &OptimizationBlock,
        transfer: EdgeTransfer,
    ) -> Result<(), OptimizationUnitValidationError> {
        let invalid = || OptimizationUnitValidationError::CurrentCleanupMismatch {
            machine: function.machine,
            block,
        };
        for parameter in &target.structural_parameters {
            if parameter.access == StructuralAccess::Owned
                && parameter.multiplicity != StructuralMultiplicity::Unrestricted
                && frontier
                    .owned_places
                    .insert(parameter.place, parameter.multiplicity)
                    .is_some()
            {
                return Err(invalid());
            }
        }
        for parameter in &target.structural_parameters {
            if parameter.access == StructuralAccess::Owned
                && frontier
                    .claims
                    .values()
                    .any(|claim| claim.input == Some(parameter.place))
            {
                return Err(invalid());
            }
        }
        for (identity, claim) in transfer.claims {
            if frontier.claims.insert(identity, claim).is_some() {
                return Err(invalid());
            }
        }
        Ok(())
    }

    pub(crate) fn parameter_establishment_order(
        function: &PsiOptimizationFunction,
    ) -> Vec<&StructuralParameterDeclaration> {
        let mut parameters = function.structural_parameters.iter().collect::<Vec<_>>();
        if function
            .blocks
            .iter()
            .all(|block| block.structural_parameters.is_empty())
        {
            return parameters;
        }
        let dominators =
            crate::candidates::global_value_numbering::independent_reachable_dominators(function);
        let mut blocks = function
            .blocks
            .iter()
            .filter(|block| !block.structural_parameters.is_empty())
            .collect::<Vec<_>>();
        blocks.sort_by_key(|block| dominators.get(&block.id).map_or(0, BTreeSet::len));
        parameters.extend(
            blocks
                .into_iter()
                .flat_map(|block| &block.structural_parameters),
        );
        parameters
    }
}
mod call_accesses {
    use super::address_joins::borrowed_regions::{BorrowedRegions, Projection};
    use super::storage_regions::{storage_cannot_retain_loan, written_or_exclusive_regions};
    use crate::unit_validation::operation_contracts::structural_source_contract;
    use crate::unit_validation::structural_catalog::resolve_structural_path;
    use abstract_operations::AbstractOperation as O;
    use optimization_unit::PsiOptimizationFunction;
    use semantic_vocabulary::{MachineId, PlaceId, StructuralTypeId};
    use std::collections::BTreeMap;
    use terminal_psi::{
        BoundaryMachineDeclaration, StructuralAccess, StructuralPathSegment,
        StructuralTypeDeclaration, StructuralTypeShape,
    };
    use terminal_semantics::descriptor_results::DescriptorMachine;

    #[derive(Clone)]
    struct ParameterMutation {
        position: usize,
        projection: Projection,
    }

    #[derive(Clone)]
    struct FunctionMutations {
        parameters: usize,
        writes: Vec<ParameterMutation>,
    }

    pub(crate) struct CallAccesses<'unit> {
        functions: BTreeMap<MachineId, Option<FunctionMutations>>,
        boundaries: Vec<&'unit BoundaryMachineDeclaration>,
        pub(crate) dynamic_dispatch: &'unit terminal_psi::TerminalDynamicDispatchCatalog,
        pub(super) references: super::references::call_effects::ReferenceCallEffects,
        view_graph: Option<Vec<DescriptorMachine>>,
        borrowed_origins: BTreeMap<MachineId, BorrowedRegions>,
    }

    impl<'unit> CallAccesses<'unit> {
        pub(crate) fn new(
            functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
            boundaries: impl IntoIterator<Item = &'unit BoundaryMachineDeclaration>,
            dynamic_dispatch: &'unit terminal_psi::TerminalDynamicDispatchCatalog,
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        ) -> Self {
            let boundaries = boundaries.into_iter().collect::<Vec<_>>();
            let references = super::references::call_effects::ReferenceCallEffects::new(
                functions,
                &boundaries,
                dynamic_dispatch,
                types,
            );
            let declarations = types.values().copied().cloned().collect::<Vec<_>>();
            let view_graph = crate::descriptor_results::optimized_catalog_graph(
                functions.values().copied(),
                boundaries.iter().copied(),
                &declarations,
                dynamic_dispatch,
                |_, node| Some(std::borrow::Cow::Borrowed(&node.operation)),
            );
            let mut analysis = view_graph.as_ref().map(|graph| {
                terminal_semantics::descriptor_results::DescriptorAnalysis::new(
                    &declarations,
                    graph,
                )
            });
            let borrowed_origins = functions
                .iter()
                .map(|(machine, function)| {
                    (
                        *machine,
                        BorrowedRegions::origins_with_analysis(
                            function,
                            &declarations,
                            view_graph.as_deref(),
                            analysis.as_mut(),
                            types,
                        ),
                    )
                })
                .collect::<BTreeMap<_, _>>();
            let mut result = Self {
                functions: BTreeMap::new(),
                boundaries,
                dynamic_dispatch,
                references,
                view_graph,
                borrowed_origins: BTreeMap::new(),
            };
            let mut closed_storage = BTreeMap::new();
            for machine in functions.keys() {
                result.reconstruct(
                    *machine,
                    functions,
                    types,
                    &borrowed_origins,
                    &mut closed_storage,
                );
            }
            result.borrowed_origins = borrowed_origins;
            result
        }

        pub(crate) fn boundaries(&self) -> &[&'unit BoundaryMachineDeclaration] {
            &self.boundaries
        }

        pub(crate) fn view_graph(&self) -> Option<&[DescriptorMachine]> {
            self.view_graph.as_deref()
        }

        pub(crate) fn has_descriptor_call_origins(
            &self,
            caller: MachineId,
            result: PlaceId,
        ) -> Option<bool> {
            self.borrowed_origins
                .get(&caller)
                .map(|origins| origins.has_returned_call_origins(result))
        }

        pub(super) fn take_borrowed_origins(
            &mut self,
            machine: MachineId,
        ) -> Option<BorrowedRegions> {
            self.borrowed_origins.remove(&machine)
        }

        fn reconstruct(
            &mut self,
            machine: MachineId,
            functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            borrowed_origins: &BTreeMap<MachineId, BorrowedRegions>,
            closed_storage: &mut BTreeMap<StructuralTypeId, bool>,
        ) {
            if self.functions.contains_key(&machine) {
                return;
            }
            self.functions.insert(machine, None);
            let (Some(function), Some(origins)) =
                (functions.get(&machine), borrowed_origins.get(&machine))
            else {
                return;
            };
            let mut writes = Vec::<ParameterMutation>::new();
            for node in function.block_nodes() {
                if let O::CallStructural { callee, .. } = &node.operation {
                    let Some(machine) = callee.machine() else {
                        return;
                    };
                    self.reconstruct(machine, functions, types, borrowed_origins, closed_storage);
                    if !self.functions.get(&machine).is_some_and(Option::is_some) {
                        return;
                    }
                }
                if matches!(
                    node.operation,
                    O::Call { .. } | O::AtomicEvent { .. } | O::PortWrite { .. }
                ) {
                    return;
                }
                if let O::BoundaryCall {
                    structural_arguments,
                    completion_claim_sources,
                    completion_receipts,
                    ..
                } = &node.operation
                    && (structural_arguments.iter().any(|argument| {
                        argument.access == StructuralAccess::Owned
                            || !structural_source_contract(function, argument.place).is_some_and(
                                |source| {
                                    argument_storage_is_closed(
                                        types,
                                        source.structural_type,
                                        &argument.path,
                                        argument.access,
                                        closed_storage,
                                    )
                                },
                            )
                    }) || !completion_claim_sources.is_empty()
                        || !completion_receipts.is_empty())
                {
                    return;
                }
                if node
                    .operation
                    .cleanup_rosters()
                    .flat_map(|(_, actions)| actions)
                    .any(|action| {
                        matches!(
                            action,
                            terminal_psi::TerminalAffineCleanupAction::InvokeNominal(_)
                        )
                    })
                {
                    return;
                }
                for (place, region) in self.written_regions(function, types, &node.operation) {
                    let mut projection = Projection::new(&region.path);
                    projection.indirect |= region.indirect;
                    for region in origins.resolve(place, projection) {
                        let Some(position) = function
                            .structural_parameters
                            .iter()
                            .position(|parameter| Some(parameter.place) == region.root)
                        else {
                            return;
                        };
                        if function.structural_parameters[position].access
                            == StructuralAccess::Owned
                        {
                            return;
                        }
                        let projection = region.projection.without_runtime_coordinates();
                        if !writes.iter().any(|existing| {
                            existing.position == position && existing.projection == projection
                        }) {
                            writes.push(ParameterMutation {
                                position,
                                projection,
                            });
                        }
                    }
                }
            }
            self.functions.insert(
                machine,
                Some(FunctionMutations {
                    parameters: function.structural_parameters.len(),
                    writes,
                }),
            );
        }

        pub(super) fn written_regions(
            &self,
            function: &PsiOptimizationFunction,
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            operation: &O,
        ) -> Vec<(PlaceId, terminal_semantics::StorageRegion)> {
            if let O::CallStructural {
                callee,
                structural_arguments,
                claim_transfers,
                ..
            } = operation
                && claim_transfers.is_empty()
                && let Some(machine) = callee.machine()
                && let Some(Some(summary)) = self.functions.get(&machine)
                && summary.parameters == structural_arguments.len()
            {
                return summary
                    .writes
                    .iter()
                    .map(|write| {
                        let actual = &structural_arguments[write.position];
                        let projection = Projection::new(&actual.path).then(&write.projection);
                        (
                            actual.place,
                            terminal_semantics::StorageRegion {
                                path: projection.path,
                                indirect: projection.indirect,
                            },
                        )
                    })
                    .collect();
            }
            written_or_exclusive_regions(function, types, operation)
        }
    }

    pub(crate) fn argument_storage_is_closed(
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        source: StructuralTypeId,
        path: &[StructuralPathSegment],
        access: StructuralAccess,
        established: &mut BTreeMap<StructuralTypeId, bool>,
    ) -> bool {
        let Some(selected) = resolve_structural_path(types, source, path) else {
            return false;
        };
        if path.is_empty() && access != StructuralAccess::Owned {
            match types.get(&selected).map(|declaration| &declaration.shape) {
                Some(StructuralTypeShape::ByteSequence(carrier)) if carrier.is_borrowed() => {
                    return true;
                }
                Some(StructuralTypeShape::ElementView { element, .. }) => {
                    return storage_cannot_retain_loan(types, *element, established);
                }
                _ => {}
            }
        }
        storage_cannot_retain_loan(types, selected, established)
    }
}
mod cleanup {
    use super::references::check_region_access;
    use crate::OptimizationUnitValidationError;
    use crate::current_ownership::residuals::selected_affine_residuals;
    use crate::current_ownership::structural::place_structural_type;
    use crate::current_ownership::{CurrentOwnership, parameter_establishment_order};
    use crate::unit_validation::structural_catalog::resolve_structural_path;
    use abstract_operations::AbstractFunctionResult;
    use optimization_unit::PsiOptimizationFunction;
    use semantic_vocabulary::{BlockId, MachineId, PlaceId, StructuralCaseId, StructuralTypeId};
    use std::collections::{BTreeMap, BTreeSet};
    use terminal_psi::NominalAffineCleanup;
    use terminal_psi::{
        StructuralFieldType, StructuralMultiplicity, StructuralPathSegment,
        StructuralTypeDeclaration, StructuralTypeShape, TerminalAffineCleanupAction,
    };
    use terminal_semantics::owned_structure::{
        contains_owned_boundary as contains_cleanup_boundary,
        selected_contains_owned_boundary as selected_contains_nominal_cleanup,
        selected_owned_children as selected_children,
    };
    use terminal_semantics::structural_paths_may_overlap;

    pub(super) type CaseMembership<'a> =
        dyn Fn(PlaceId, &[StructuralPathSegment], StructuralCaseId, bool) -> bool + 'a;

    #[allow(clippy::too_many_arguments)]
    pub(super) fn validate_return_cleanup_actions(
        function: &PsiOptimizationFunction,
        functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
        structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        reference_effects: &super::references::call_effects::ReferenceCallEffects,
        block: BlockId,
        unrestricted: &[PlaceId],
        frontier: &CurrentOwnership,
        actions: &[TerminalAffineCleanupAction],
        membership: &CaseMembership<'_>,
    ) -> Result<CurrentOwnership, OptimizationUnitValidationError> {
        let mut remaining = frontier.clone();
        super::continuation::apply_edge_cleanup_actions(
            function,
            functions,
            structural_types,
            block,
            unrestricted,
            &mut remaining,
            actions,
            membership,
        )?;
        remaining.owned_places.retain(|place, multiplicity| {
            *multiplicity != StructuralMultiplicity::Linear
                || remaining.partial_custody_paths.contains_key(place)
                || remaining
                    .claims
                    .values()
                    .any(|claim| claim.input == Some(*place))
                || place_structural_type(function, *place).is_none_or(|root| {
                    terminal_semantics::owned_structure::validate_owned_claim_frontier(
                        &|identity| structural_types.get(&identity).copied(),
                        root,
                        &[],
                        std::iter::empty(),
                        &|_, _| true,
                    )
                    .is_err()
                })
        });
        if !remaining.owned_places.is_empty()
            || !remaining.partial_custody_paths.is_empty()
            || !remaining.claims.is_empty()
        {
            return Err(OptimizationUnitValidationError::CurrentCleanupMismatch {
                machine: function.machine,
                block,
            });
        }
        super::references::validate_normal_return(
            function,
            structural_types,
            reference_effects,
            block,
            &remaining.live_references,
        )?;
        Ok(remaining)
    }

    /// Copyable values owe no disposal, but an explicit cleanup still needs
    /// their actual owned source and no-code declaration. Foundation validation
    /// checks that each cleanup use follows its dominating producer.
    pub(crate) fn unrestricted_owned_cleanup_roots(
        function: &PsiOptimizationFunction,
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    ) -> Vec<PlaceId> {
        // Prepare this actual source roster once per function, then reuse it
        // across edge cleanup and replay instead of scanning operations per home.
        let mut claimed = function
            .entry_claim_declarations
            .iter()
            .map(|claim| claim.input)
            .chain(
                function
                    .content_entry_claims
                    .iter()
                    .map(|claim| claim.input.root),
            )
            .collect::<Vec<_>>();
        claimed.sort_unstable();
        claimed.dedup();
        let eligible = |place, structural_type, multiplicity, access| {
            access == terminal_psi::StructuralAccess::Owned
                && multiplicity == StructuralMultiplicity::Unrestricted
                && claimed.binary_search(&place).is_err()
                && types.get(&structural_type).is_some_and(|declaration| {
                    declaration.copyable
                        && !declaration.owns_linear_claim
                        && declaration.no_code_affine_cleanup
                        && declaration.descriptor_cleanup
                })
        };
        let mut roots = function
            .structural_parameters
            .iter()
            .chain(function.block_structural_parameters())
            .filter(|parameter| {
                eligible(
                    parameter.place,
                    parameter.structural_type,
                    parameter.multiplicity,
                    parameter.access,
                )
            })
            .map(|parameter| parameter.place)
            .collect::<Vec<_>>();
        for node in function.block_nodes() {
            // Borrowed recipes have their own source rules and cannot own a
            // disposable home, regardless of the carrier's copy permission.
            if !node
                .operation
                .structural_result_metadata()
                .is_some_and(|metadata| {
                    matches!(
                        metadata.role,
                        abstract_operations::StructuralResultRole::OwnedValue
                    )
                })
            {
                continue;
            }
            if let Some(source) =
                crate::unit_validation::operation_contracts::structural_operation_result_contract(
                    function,
                    &node.operation,
                )
                && let Some(result) = source.operation_result()
                && eligible(
                    result.place,
                    source.structural_type,
                    source.multiplicity,
                    source.access,
                )
            {
                roots.push(result.place);
            }
        }
        roots.sort_unstable();
        roots.dedup();
        roots
    }

    pub(super) fn cleanup_root_order(
        function: &PsiOptimizationFunction,
        unrestricted: &[PlaceId],
        frontier: &CurrentOwnership,
    ) -> Vec<PlaceId> {
        let mut results = Vec::new();
        let mut locals = Vec::new();
        for place in &function.structural_places {
            if !frontier.owned_places.contains_key(&place.id)
                && !frontier.partial_custody_paths.contains_key(&place.id)
                && unrestricted.binary_search(&place.id).is_err()
            {
                continue;
            }
            match place.kind {
                semantic_vocabulary::StructuralPlaceKind::OperationResult { producer, .. } => {
                    results.push((producer, place.id))
                }
                semantic_vocabulary::StructuralPlaceKind::TrivialAffineLocal {
                    declaration_ordinal,
                    ..
                } => locals.push((declaration_ordinal, place.id)),
                _ => {}
            }
        }
        results.sort_by_key(|(producer, _)| std::cmp::Reverse(*producer));
        locals.sort_by_key(|(ordinal, _)| std::cmp::Reverse(*ordinal));
        results
            .into_iter()
            .map(|(_, place)| place)
            .chain(locals.into_iter().map(|(_, place)| place))
            .chain(
                parameter_establishment_order(function)
                    .into_iter()
                    .rev()
                    .filter(|parameter| {
                        frontier.owned_places.contains_key(&parameter.place)
                            || frontier
                                .partial_custody_paths
                                .contains_key(&parameter.place)
                            || unrestricted.binary_search(&parameter.place).is_ok()
                    })
                    .map(|parameter| parameter.place),
            )
            .collect()
    }

    struct CleanupSubject {
        path: Vec<StructuralPathSegment>,
        structural_type: StructuralTypeId,
        nominal: bool,
    }

    fn cleanup_subjects(
        functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        root: StructuralTypeId,
        place: PlaceId,
        membership: &CaseMembership<'_>,
        moved: Option<&BTreeSet<Vec<StructuralPathSegment>>>,
        limit: usize,
    ) -> Option<Vec<CleanupSubject>> {
        let nominal: BTreeSet<_> = functions
            .values()
            .filter_map(|function| function.nominal_cleanup_subject)
            .collect();
        if let Some(moved) = moved {
            for path in moved {
                for depth in 0..path.len() {
                    if let Some(StructuralPathSegment::Case(case)) = path[..depth].last() {
                        if !membership(place, &path[..depth - 1], *case, true) {
                            return None;
                        }
                        continue;
                    }
                    let ancestor = resolve_structural_path(types, root, &path[..depth])?;
                    if nominal.contains(&ancestor) {
                        return None;
                    }
                }
            }
        }
        let selected = |path: &[StructuralPathSegment], case| membership(place, path, case, true);
        if moved.is_none()
            && !selected_contains_nominal_cleanup(
                types,
                &nominal,
                &mut BTreeMap::new(),
                &selected,
                root,
                &[],
                limit,
            )?
        {
            return (limit != 0).then_some(vec![CleanupSubject {
                path: Vec::new(),
                structural_type: root,
                nominal: false,
            }]);
        }
        let mut boundaries = nominal.clone();
        boundaries.extend(types.iter().filter_map(|(identity, declaration)| {
            let cases = match &declaration.shape {
                StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } => {
                    cases
                }
                _ => return None,
            };
            declaration
                .shape
                .direct_fields()
                .into_iter()
                .flatten()
                .chain(cases.iter().flat_map(|case| &case.fields))
                .any(|field| {
                    !field.relevance.is_erased() && field.field_type.as_structural().is_some()
                })
                .then_some(*identity)
        }));
        let roots = match moved {
            Some(moved) => selected_affine_residuals(types, root, moved, limit, &selected)?,
            None => vec![(Vec::new(), root)],
        };
        let mut subjects = Vec::new();
        let mut contains = BTreeMap::new();
        for (path, structural_type) in roots {
            append_cleanup_subjects(
                types,
                &nominal,
                &boundaries,
                &mut contains,
                &selected,
                structural_type,
                path,
                limit,
                &mut subjects,
            )?;
        }
        Some(subjects)
    }

    fn append_cleanup_subjects(
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        nominal: &BTreeSet<StructuralTypeId>,
        boundaries: &BTreeSet<StructuralTypeId>,
        cache: &mut BTreeMap<StructuralTypeId, bool>,
        selected: &dyn Fn(&[StructuralPathSegment], StructuralCaseId) -> bool,
        structural_type: StructuralTypeId,
        path: Vec<StructuralPathSegment>,
        limit: usize,
        subjects: &mut Vec<CleanupSubject>,
    ) -> Option<()> {
        let whole = nominal.contains(&structural_type)
            || !contains_cleanup_boundary(
                types,
                boundaries,
                cache,
                &mut BTreeSet::new(),
                structural_type,
            )?;
        let children = if whole {
            Vec::new()
        } else {
            selected_children(types, structural_type, &path, limit, selected)?
        };
        if whole || children.is_empty() {
            if subjects.len() >= limit {
                return None;
            }
            subjects.push(CleanupSubject {
                path,
                structural_type,
                nominal: nominal.contains(&structural_type),
            });
            return Some(());
        }
        for (child_path, child) in children {
            append_cleanup_subjects(
                types, nominal, boundaries, cache, selected, child, child_path, limit, subjects,
            )?;
        }
        Some(())
    }

    pub(super) fn consume_owner_cleanup(
        function: &PsiOptimizationFunction,
        functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        block: BlockId,
        unrestricted: &[PlaceId],
        frontier: &mut CurrentOwnership,
        place: PlaceId,
        actions: &[TerminalAffineCleanupAction],
        membership: &CaseMembership<'_>,
    ) -> Result<(), OptimizationUnitValidationError> {
        let mismatch = || OptimizationUnitValidationError::CurrentCleanupMismatch {
            machine: function.machine,
            block,
        };
        if unrestricted.binary_search(&place).is_ok() {
            if !matches!(actions,
                [TerminalAffineCleanupAction::RetireHome(root) | TerminalAffineCleanupAction::DiscardRoot(root)] if *root == place)
                || frontier.owned_places.contains_key(&place)
                || frontier.partial_custody_paths.contains_key(&place)
                || frontier
                    .claims
                    .values()
                    .any(|claim| claim.input == Some(place))
                || frontier.live_references.iter().any(|reference| {
                    super::references::borrows_owned_storage(function, types, reference, place)
                })
            {
                return Err(mismatch());
            }
            return super::references::discard_owned(
                function,
                types,
                block,
                &mut frontier.live_references,
                place,
            );
        }
        if super::value_custody::uninstalled_home(frontier, place) {
            if !matches!(actions, [TerminalAffineCleanupAction::RetireHome(retired)] if *retired == place)
                || frontier
                    .claims
                    .values()
                    .any(|claim| claim.input == Some(place))
                || frontier.live_references.iter().any(|reference| {
                    reference.carrier == place
                        && super::references::owned_resident(&reference.carrier_path, &[])
                })
            {
                return Err(mismatch());
            }
            frontier.owned_places.remove(&place);
            frontier.partial_custody_paths.remove(&place);
            return Ok(());
        }
        let Some(&multiplicity) = frontier.owned_places.get(&place) else {
            if !matches!(actions, [TerminalAffineCleanupAction::RetireHome(retired)] if *retired == place)
                || !super::structural::projected_root_is_fully_consumed(
                    function, types, frontier, place,
                )
                || frontier.live_references.iter().any(|reference| {
                    reference.carrier == place
                        && super::references::owned_resident(&reference.carrier_path, &[])
                })
            {
                return Err(mismatch());
            }
            frontier.partial_custody_paths.remove(&place);
            return Ok(());
        };
        let root = place_structural_type(function, place).ok_or_else(mismatch)?;
        if function.content_entry_claim_declared(place) {
            return Err(mismatch());
        }
        // Residual discards with no retirement equalize a join: the arrival
        // that still holds a part another arrival moved out discards that
        // part and keeps the home, so both enter with one frontier. Each is
        // the consumption a projected move performs, with the same overlap,
        // claim and access checks; the home stays owned until its own
        // disposal, which a missing retirement leaves for the return or the
        // join to reject.
        if !actions.is_empty()
            && actions.iter().all(|action| {
                matches!(action, TerminalAffineCleanupAction::DiscardResidual(discard) if discard.place == place)
            })
        {
            for action in actions {
                let TerminalAffineCleanupAction::DiscardResidual(discard) = action else {
                    return Err(mismatch());
                };
                if discard.path.is_empty()
                    || multiplicity != StructuralMultiplicity::Affine
                    || resolve_structural_path(types, root, &discard.path)
                        != Some(discard.structural_type)
                    || types
                        .get(&discard.structural_type)
                        .is_none_or(|declaration| !declaration.no_code_affine_cleanup)
                    || frontier.claims.values().any(|claim| {
                        claim.input == Some(place)
                            && structural_paths_may_overlap(&claim.path, &discard.path)
                    })
                {
                    return Err(mismatch());
                }
                let ordinal = function
                    .block(block)
                    .and_then(|block| u32::try_from(block.nodes.len().saturating_sub(1)).ok())
                    .ok_or_else(mismatch)?;
                check_region_access(
                    function,
                    block,
                    ordinal,
                    &frontier.live_references,
                    place,
                    &discard.path,
                )?;
                super::references::discard_subtree(
                    function,
                    types,
                    block,
                    &mut frontier.live_references,
                    place,
                    &discard.path,
                )?;
                let moved = frontier.partial_custody_paths.entry(place).or_default();
                if moved.iter().any(|existing| {
                    existing.starts_with(&discard.path) || discard.path.starts_with(existing)
                }) || !moved.insert(discard.path.clone())
                {
                    return Err(mismatch());
                }
            }
            if super::structural::projected_root_is_fully_consumed(function, types, frontier, place)
            {
                frontier.owned_places.remove(&place);
            }
            return Ok(());
        }
        let (actions, explicit_retirement) = match actions.split_last() {
            Some((TerminalAffineCleanupAction::RetireHome(retired), partition))
                if *retired == place =>
            {
                (partition, true)
            }
            _ => (actions, false),
        };
        let intrinsic_retirement = matches!(actions, [TerminalAffineCleanupAction::DiscardRoot(root)] if *root == place)
            || matches!(actions, [TerminalAffineCleanupAction::InvokeNominal(cleanup)] if cleanup.place == place && cleanup.path.is_empty() && !cleanup.retains_descendants);
        if !explicit_retirement && !intrinsic_retirement {
            return Err(mismatch());
        }
        let subjects = cleanup_subjects(
            functions,
            types,
            root,
            place,
            membership,
            frontier.partial_custody_paths.get(&place),
            actions.len(),
        )
        .ok_or_else(mismatch)?;
        if subjects.len() != actions.len() {
            return Err(mismatch());
        }
        for (subject, action) in subjects.iter().zip(actions) {
            if action.place() != place {
                return Err(mismatch());
            }
            match action {
                TerminalAffineCleanupAction::InvokeNominal(cleanup)
                    if subject.nominal
                        && cleanup.path == subject.path
                        && cleanup.structural_type == subject.structural_type =>
                {
                    consume_nominal_cleanup(function, functions, types, block, frontier, cleanup)?;
                }
                TerminalAffineCleanupAction::DiscardRoot(_)
                    if !subject.nominal
                        && subject.path.is_empty()
                        && multiplicity == StructuralMultiplicity::Affine
                        && !frontier
                            .claims
                            .values()
                            .any(|claim| claim.input == Some(place)) =>
                {
                    super::references::discard_owned(
                        function,
                        types,
                        block,
                        &mut frontier.live_references,
                        place,
                    )?;
                }
                TerminalAffineCleanupAction::DiscardResidual(discard)
                    if !subject.nominal
                        && discard.path == subject.path
                        && discard.structural_type == subject.structural_type
                        && !frontier.claims.values().any(|claim| {
                            claim.input == Some(place)
                                && structural_paths_may_overlap(&claim.path, &subject.path)
                        }) =>
                {
                    let ordinal = function
                        .block(block)
                        .and_then(|block| u32::try_from(block.nodes.len().saturating_sub(1)).ok())
                        .ok_or_else(mismatch)?;
                    check_region_access(
                        function,
                        block,
                        ordinal,
                        &frontier.live_references,
                        place,
                        &subject.path,
                    )?;
                    super::references::discard_subtree(
                        function,
                        types,
                        block,
                        &mut frontier.live_references,
                        place,
                        &subject.path,
                    )?;
                }
                _ => return Err(mismatch()),
            }
        }
        if frontier
            .claims
            .values()
            .any(|claim| claim.input == Some(place))
        {
            return Err(mismatch());
        }
        frontier.owned_places.remove(&place);
        frontier.partial_custody_paths.remove(&place);
        Ok(())
    }

    pub(super) fn consume_nominal_cleanup(
        function: &PsiOptimizationFunction,
        functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
        structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        block: BlockId,
        frontier: &mut CurrentOwnership,
        cleanup: &NominalAffineCleanup,
    ) -> Result<(), OptimizationUnitValidationError> {
        let mismatch = || OptimizationUnitValidationError::CurrentCleanupMismatch {
            machine: function.machine,
            block,
        };
        let multiplicity = *frontier
            .owned_places
            .get(&cleanup.place)
            .ok_or_else(mismatch)?;
        let root = place_structural_type(function, cleanup.place).ok_or_else(mismatch)?;
        let claims = frontier
            .claims
            .iter()
            .filter(|(_, claim)| {
                claim.input == Some(cleanup.place)
                    && structural_paths_may_overlap(&claim.path, &cleanup.path)
            })
            .collect::<Vec<_>>();
        let selected = resolve_structural_path(structural_types, root, &cleanup.path);
        if !cleanup.requirement_obligations.is_empty()
            || selected != Some(cleanup.structural_type)
            || !valid_nominal_cleanup(function, functions, structural_types, cleanup)
            || frontier
                .partial_custody_paths
                .get(&cleanup.place)
                .is_some_and(|paths| {
                    paths
                        .iter()
                        .any(|path| structural_paths_may_overlap(path, &cleanup.path))
                })
            || claims.iter().any(|(_, claim)| {
                claim.path != cleanup.path
                    || claim.multiplicity != Some(StructuralMultiplicity::Linear)
            })
            || (cleanup.path.is_empty()
                && match multiplicity {
                    StructuralMultiplicity::Affine => !claims.is_empty(),
                    StructuralMultiplicity::Linear => claims.len() != 1,
                    StructuralMultiplicity::Unrestricted => true,
                })
            || (!cleanup.path.is_empty() && claims.len() > 1)
            || cleanup.consumed_claims
                != claims
                    .iter()
                    .map(|(identity, _)| **identity)
                    .collect::<Vec<_>>()
        {
            return Err(mismatch());
        }
        let ordinal = function
            .block(block)
            .and_then(|block| u32::try_from(block.nodes.len().saturating_sub(1)).ok())
            .ok_or_else(mismatch)?;
        check_region_access(
            function,
            block,
            ordinal,
            &frontier.live_references,
            cleanup.place,
            &cleanup.path,
        )?;
        for claim in &cleanup.consumed_claims {
            frontier.claims.remove(claim);
        }
        Ok(())
    }

    pub(super) fn valid_nominal_cleanup(
        caller: &PsiOptimizationFunction,
        functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
        structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        cleanup: &NominalAffineCleanup,
    ) -> bool {
        let Some(source) = structural_types.get(&cleanup.structural_type) else {
            return false;
        };
        let Some(target) = functions.get(&cleanup.cleanup_machine).copied() else {
            return false;
        };
        cleanup.cleanup_machine != caller.machine
            && bounded_nominal_cleanup_receiver_shape(&source.shape)
            && !cleanup.retains_descendants
            && target.attachment == Some(cleanup.structural_type)
            && target.nominal_cleanup_subject == Some(cleanup.structural_type)
            && (cleanup.consumed_claims.is_empty() || target.published_service_ceiling.is_empty())
            && functions
                .values()
                .filter(|candidate| {
                    candidate.nominal_cleanup_subject == Some(cleanup.structural_type)
                })
                .count()
                == 1
            && cleanup
                .consumed_claims
                .windows(2)
                .all(|pair| pair[0] < pair[1])
            && matches!(target.result, AbstractFunctionResult::Unit)
            && target.parameters.is_empty()
            && match target.structural_parameters.as_slice() {
                [] => cleanup.cleanup_receiver.is_none() && target.structural_places.is_empty(),
                [parameter] => {
                    parameter.position == 0
                        && parameter.structural_type == cleanup.structural_type
                        && cleanup.cleanup_receiver == Some(parameter.place)
                        && matches!(
                            parameter.access,
                            terminal_psi::StructuralAccess::MutableBorrow
                                | terminal_psi::StructuralAccess::WriteOnlyBorrow
                        )
                        && target.structural_places.iter().any(|place| {
                            place.id == parameter.place
                                && place.kind
                                    == semantic_vocabulary::StructuralPlaceKind::Parameter {
                                        position: parameter.position,
                                    }
                        })
                }
                _ => false,
            }
            && target.entry_claim_declarations.is_empty()
            && target.content_entry_claims.is_empty()
            && target
                .published_service_ceiling
                .iter()
                .all(|service| caller.published_service_ceiling.contains(service))
            && target
                .verified_contract
                .as_ref()
                .is_none_or(|contract| contract.crash_routes.is_empty())
    }

    pub(super) fn bounded_nominal_cleanup_receiver_shape(shape: &StructuralTypeShape) -> bool {
        let StructuralTypeShape::Record { fields, .. } = shape else {
            return false;
        };
        fields.iter().all(|field| {
            !field.relevance.is_erased()
                && match field.field_type {
                    StructuralFieldType::Scalar(semantic_vocabulary::ScalarType::Boolean) => true,
                    StructuralFieldType::Scalar(semantic_vocabulary::ScalarType::Integer(
                        integer,
                    )) => {
                        matches!(integer.bits(), 8 | 16 | 32 | 64)
                            && (!integer.is_address() || integer.bits() == 64)
                    }
                    StructuralFieldType::Scalar(semantic_vocabulary::ScalarType::IeeeFloat(_))
                    | StructuralFieldType::Atomic(_)
                    | StructuralFieldType::AtomicBoundedInteger(_)
                    | StructuralFieldType::IeeeFloat(_)
                    | StructuralFieldType::ByteSequence(_)
                    | StructuralFieldType::PredicateByteSequence { .. }
                    | StructuralFieldType::Structural(_)
                    | StructuralFieldType::Erased { .. }
                    | StructuralFieldType::RefinedScalar(_)
                    | StructuralFieldType::BoundedInteger(_) => false,
                }
        })
    }
}
pub(crate) use call_accesses::{CallAccesses, argument_storage_is_closed};
mod continuation {
    use crate::OptimizationUnitValidationError;
    use crate::current_ownership::CurrentOwnership;
    use optimization_unit::PsiOptimizationFunction;
    use semantic_vocabulary::{BlockId, StructuralTypeId};
    use std::collections::BTreeMap;
    use terminal_psi::StructuralTypeDeclaration;

    pub(super) fn apply_edge_cleanup_actions(
        function: &PsiOptimizationFunction,
        functions: &BTreeMap<semantic_vocabulary::MachineId, &PsiOptimizationFunction>,
        structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        block: BlockId,
        unrestricted: &[semantic_vocabulary::PlaceId],
        frontier: &mut CurrentOwnership,
        actions: &[terminal_psi::TerminalAffineCleanupAction],
        membership: &super::cleanup::CaseMembership<'_>,
    ) -> Result<(), OptimizationUnitValidationError> {
        let mismatch = || OptimizationUnitValidationError::CurrentCleanupMismatch {
            machine: function.machine,
            block,
        };
        let roots = super::cleanup::cleanup_root_order(function, unrestricted, frontier);
        let mut eligible = roots.into_iter();
        for group in actions.chunk_by(|left, right| left.place() == right.place()) {
            let place = group[0].place();
            if !eligible.any(|candidate| candidate == place) {
                return Err(mismatch());
            }
            super::cleanup::consume_owner_cleanup(
                function,
                functions,
                structural_types,
                block,
                unrestricted,
                frontier,
                place,
                group,
                membership,
            )?;
        }
        Ok(())
    }
}
mod mutations {
    use crate::current_ownership::CurrentOwnership;
    use crate::{OperationSite, OptimizationUnitValidationError};
    use optimization_unit::PsiOptimizationFunction;
    use semantic_vocabulary::{BlockId, PlaceId};
    use terminal_psi::StructuralMultiplicity;

    pub(super) fn insert_owned_result(
        function: &PsiOptimizationFunction,
        block: BlockId,
        node: u32,
        frontier: &mut CurrentOwnership,
        place: PlaceId,
        multiplicity: StructuralMultiplicity,
    ) -> Result<(), OptimizationUnitValidationError> {
        if multiplicity == StructuralMultiplicity::Unrestricted {
            return Ok(());
        }
        if frontier.owned_places.insert(place, multiplicity).is_some() {
            return Err(OptimizationUnitValidationError::CurrentOwnedPlaceNotLive(
                OperationSite::new(function.machine, block, node),
                place,
            ));
        }
        frontier.partial_custody_paths.remove(&place);
        Ok(())
    }

    pub(super) fn reject_live_linear_claim(
        function: &PsiOptimizationFunction,
        block: BlockId,
        frontier: &CurrentOwnership,
    ) -> Result<(), OptimizationUnitValidationError> {
        if let Some(claim) = frontier.claims.iter().find_map(|(claim, live)| {
            (live.multiplicity == Some(StructuralMultiplicity::Linear)).then_some(*claim)
        }) {
            return Err(
                OptimizationUnitValidationError::CurrentLinearClaimAtReturn {
                    machine: function.machine,
                    block,
                    claim,
                },
            );
        }
        Ok(())
    }
}
pub(crate) mod references {
    use super::address_joins::borrowed_regions::BorrowedReferenceSources;
    use crate::OptimizationUnitValidationError;
    use crate::StructuralTypes;
    use crate::unit_validation::operation_contracts::structural_source_contract;
    use crate::unit_validation::references::{
        LiveReference, ReferenceIdentity, ReferenceOrigin, ReferenceOriginIdentity,
        ReferenceParent, access_path_type, access_reference_source_compatible,
        access_referent_tail, contains_reference, formal_origin, is_reference_projection,
        leaf_access_within, leaf_paths, leaf_referent, leaf_type, reference_access,
        reference_source_type, storage_projection,
    };
    use abstract_operations::AbstractOperation as O;
    use abstract_operations::AbstractStructuralCallResult;
    use abstract_operations::call_source;
    use optimization_unit::PsiOptimizationFunction;
    use semantic_vocabulary::{BlockId, MachineId, PlaceId, StructuralTypeId};
    use std::collections::{BTreeMap, BTreeSet};
    use terminal_psi::{StructuralAccess, StructuralArgument, StructuralTypeDeclaration};

    const MAX_INCOMING_REFERENCE_LEAVES: usize = 4096;

    #[derive(Clone, Copy, PartialEq, Eq)]
    pub enum CallRegion {
        OrdinaryStorage,
        CarrierStorage,
        Referent,
    }

    mod access_paths {
        use super::ReferenceOriginIdentity;
        use super::{
            BlockId, LiveReference, OptimizationUnitValidationError, PlaceId,
            PsiOptimizationFunction, ReferenceIdentity, ReferenceOrigin, ReferenceParent,
            StructuralAccess, StructuralArgument, StructuralTypeDeclaration, StructuralTypeId,
            access_path_type, access_referent_tail, invalid, leaf_type, origin_place,
            origins_overlap, shared_source_compatible, structural_source_contract,
        };
        use std::collections::BTreeMap;
        use terminal_psi::StructuralPathSegment;

        pub(super) struct AccessReferenceCustody {
            pub(super) origins: Vec<ReferenceOrigin>,
            pub(super) dependencies: Vec<ReferenceOrigin>,
        }

        pub(super) fn access_reference_custody(
            function: &PsiOptimizationFunction,
            structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            block: BlockId,
            node: u32,
            live: &[LiveReference],
            argument: &StructuralArgument,
        ) -> Result<AccessReferenceCustody, OptimizationUnitValidationError> {
            let invalid_path = || {
                invalid(
                    function,
                    block,
                    Some(node),
                    "reference access path exceeds its exact type or access",
                )
            };
            let invalid_live = || {
                invalid(
                    function,
                    block,
                    Some(node),
                    "reference access descriptor is absent, ambiguous or suspended",
                )
            };
            let invalid_opaque = || {
                invalid(
                    function,
                    block,
                    Some(node),
                    "reference access crossing has no exact stored lineage",
                )
            };
            access_referent_tail(function, structural_types, argument).ok_or_else(invalid_path)?;
            let mut current = structural_source_contract(function, argument.place)
                .ok_or_else(|| {
                    invalid(
                        function,
                        block,
                        Some(node),
                        "reference access root is absent",
                    )
                })?
                .structural_type;
            let mut cursors = vec![(argument.place, Vec::new(), Vec::new())];
            let cursor_budget = live
                .len()
                .checked_mul(argument.path.len())
                .ok_or_else(invalid_opaque)?;
            let mut dependencies = Vec::new();
            let mut prefix_start = 0;
            for (position, segment) in argument.path.iter().enumerate() {
                if !matches!(segment, StructuralPathSegment::Referent) {
                    continue;
                }
                let prefix = &argument.path[prefix_start..position];
                let reference_type =
                    access_path_type(structural_types, current, prefix).ok_or_else(invalid_path)?;
                let referent = structural_types
                    .get(&reference_type)
                    .and_then(|declaration| match declaration.shape {
                        terminal_psi::StructuralTypeShape::Reference { referent, .. } => {
                            Some(referent)
                        }
                        _ => None,
                    })
                    .ok_or_else(invalid_path)?;
                let further_crossing =
                    argument.path[position + 1..].contains(&StructuralPathSegment::Referent);
                let mut origins = Vec::new();
                let mut next_cursors = Vec::new();
                for (place, mut selected_path, ancestors) in cursors {
                    selected_path.extend_from_slice(prefix);
                    let root_type = structural_source_contract(function, place)
                        .ok_or_else(invalid_path)?
                        .structural_type;
                    let candidates = descriptor_candidates(
                        function,
                        structural_types,
                        root_type,
                        &selected_path,
                        live.len(),
                    )
                    .ok_or_else(invalid_live)?;
                    for path in candidates {
                        let actual = structural_source_contract(function, place)
                            .and_then(|signature| {
                                leaf_type(structural_types, signature.structural_type, &path)
                            })
                            .ok_or_else(invalid_path)?;
                        if actual != reference_type {
                            return Err(invalid_path());
                        }
                        if !descriptor_storage_available(
                            function,
                            structural_types,
                            live,
                            place,
                            &path,
                            &ancestors,
                            argument.access,
                        ) {
                            return Err(invalid_live());
                        }
                        let mut matching = live.iter().filter(|reference| {
                            reference.carrier == place && reference.carrier_path == path
                        });
                        let carrier = matching.next().ok_or_else(invalid_live)?;
                        if matching.next().is_some()
                            || live.iter().any(|child| {
                                matches!(&child.parent,
                        ReferenceParent::Reference(parent) if parent == &carrier.identity)
                                    && !shared_source_compatible(
                                        function,
                                        structural_types,
                                        child,
                                        argument.access,
                                    )
                            })
                        {
                            return Err(invalid_live());
                        }
                        dependencies.push(ReferenceOrigin::IngressLeaf(
                            ReferenceIdentity { place, path }.into(),
                        ));
                        origins.push(carrier.root.clone());
                        if further_crossing {
                            dependencies.push(carrier.root.clone());
                            let candidates = match &carrier.root {
                                ReferenceOrigin::Invocation { .. } => return Err(invalid_opaque()),
                                ReferenceOrigin::Primitive(place) => {
                                    vec![ReferenceOriginIdentity {
                                        place: *place,
                                        path: Vec::new(),
                                        extent:
                                            semantic_vocabulary::ReferenceOriginExtent::Projected,
                                    }]
                                }
                                ReferenceOrigin::IngressLeaf(identity) => {
                                    vec![identity.clone()]
                                }
                                ReferenceOrigin::Alternatives { origins, .. } => origins.clone(),
                            };
                            for origin in candidates {
                                let cursor = super::storage_cursor(
                                    function,
                                    structural_types,
                                    origin.exact_identity().ok_or_else(invalid_opaque)?,
                                    referent,
                                )
                                .ok_or_else(invalid_opaque)?;
                                let (place, path) = (cursor.place, cursor.path);
                                let mut ancestry = ancestors.clone();
                                ancestry.push(carrier.identity.clone());
                                if !next_cursors.contains(&(place, path.clone(), ancestry.clone()))
                                {
                                    if next_cursors.len() >= cursor_budget {
                                        return Err(invalid_opaque());
                                    }
                                    next_cursors.push((place, path, ancestry));
                                }
                            }
                        }
                    }
                }
                if !further_crossing {
                    if origins.is_empty() {
                        return Err(invalid_live());
                    }
                    return Ok(AccessReferenceCustody {
                        origins,
                        dependencies,
                    });
                }
                cursors = next_cursors;
                current = referent;
                prefix_start = position + 1;
            }
            Err(invalid_path())
        }

        fn descriptor_candidates(
            function: &PsiOptimizationFunction,
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            mut current: StructuralTypeId,
            path: &[StructuralPathSegment],
            maximum: usize,
        ) -> Option<Vec<Vec<StructuralPathSegment>>> {
            if maximum == 0 {
                return None;
            }
            let mut candidates = vec![Vec::new()];
            let path = abstract_operations::values::resolve_constant_runtime_indexes(
                function
                    .blocks
                    .iter()
                    .flat_map(|block| block.nodes.iter().map(|node| &node.operation)),
                path,
            );
            for segment in &path {
                if matches!(segment, StructuralPathSegment::RuntimeIndex { .. }) {
                    let terminal_psi::StructuralTypeShape::FixedArray {
                        element,
                        length: terminal_psi::ArrayExtent::Literal(length),
                    } = types.get(&current)?.shape
                    else {
                        return None;
                    };
                    let length = usize::try_from(length).ok()?;
                    let count = candidates.len().checked_mul(length)?;
                    if count == 0 || count > maximum {
                        return None;
                    }
                    let mut expanded = Vec::with_capacity(count);
                    for candidate in candidates {
                        for element_index in 0..length {
                            let mut selected = candidate.clone();
                            selected.push(StructuralPathSegment::FixedIndex(
                                u64::try_from(element_index).ok()?,
                            ));
                            expanded.push(selected);
                        }
                    }
                    candidates = expanded;
                    current = element;
                } else {
                    current = leaf_type(types, current, std::slice::from_ref(segment))?;
                    for candidate in &mut candidates {
                        candidate.push(segment.clone());
                    }
                }
            }
            Some(candidates)
        }

        pub(super) fn descriptor_storage_available(
            function: &PsiOptimizationFunction,
            structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            live: &[LiveReference],
            place: PlaceId,
            path: &[StructuralPathSegment],
            ancestors: &[ReferenceIdentity],
            requested: StructuralAccess,
        ) -> bool {
            let storage = ReferenceOrigin::IngressLeaf(
                ReferenceIdentity {
                    place,
                    path: path.to_vec(),
                }
                .into(),
            );
            live.iter().all(|loan| {
                if origin_place(&loan.root) != place
                    || ancestors.contains(&loan.identity)
                    || shared_source_compatible(function, structural_types, loan, requested)
                {
                    return true;
                }
                let Some(reference_type) = structural_source_contract(function, loan.carrier)
                    .and_then(|signature| {
                        leaf_type(
                            structural_types,
                            signature.structural_type,
                            &loan.carrier_path,
                        )
                    })
                else {
                    return false;
                };
                let Some(referent) =
                    structural_types
                        .get(&reference_type)
                        .and_then(|declaration| match declaration.shape {
                            terminal_psi::StructuralTypeShape::Reference { referent, .. } => {
                                Some(referent)
                            }
                            _ => None,
                        })
                else {
                    return false;
                };
                let conflicts = |root_place,
                                 root_path: &[terminal_psi::ReferenceOriginProjection],
                                 extent| {
                    (extent == semantic_vocabulary::ReferenceOriginExtent::WithinRegion
                        || structural_source_contract(function, root_place).and_then(|signature| {
                            terminal_semantics::reference_origin_type(
                                structural_types.values().copied(),
                                signature.structural_type,
                                root_path,
                            )
                        }) == Some(referent))
                        && origins_overlap(
                            &ReferenceOrigin::IngressLeaf(ReferenceOriginIdentity {
                                place: root_place,
                                path: root_path.to_vec(),
                                extent,
                            }),
                            &storage,
                        )
                };
                !match &loan.root {
                    ReferenceOrigin::Invocation { .. } => false,
                    ReferenceOrigin::Primitive(place) => conflicts(
                        *place,
                        &[],
                        semantic_vocabulary::ReferenceOriginExtent::Projected,
                    ),
                    ReferenceOrigin::IngressLeaf(identity) => {
                        conflicts(identity.place, &identity.path, identity.extent)
                    }
                    ReferenceOrigin::Alternatives { origins, .. } => origins
                        .iter()
                        .any(|origin| conflicts(origin.place, &origin.path, origin.extent)),
                }
            })
        }

        pub(super) fn call_dependency_conflict(
            dependencies: &[ReferenceOrigin],
            argument: &StructuralArgument,
        ) -> bool {
            if argument.access == StructuralAccess::SharedBorrow
                || argument.path.contains(&StructuralPathSegment::Referent)
            {
                return false;
            }
            let storage = ReferenceOrigin::IngressLeaf(
                ReferenceIdentity {
                    place: argument.place,
                    path: argument.path.clone(),
                }
                .into(),
            );
            dependencies
                .iter()
                .any(|dependency| origins_overlap(dependency, &storage))
        }
    }
    pub(super) mod call_effects {
        use super::*;
        use std::borrow::Cow;
        use terminal_psi::{BoundaryMachineDeclaration, StructuralPathSegment};
        use terminal_semantics::descriptor_lifetimes::descriptor_path;
        use terminal_semantics::descriptor_results::{DescriptorAnalysis, DescriptorStoredOrigin};

        struct SlotEffect {
            destination: StructuralArgument,
            sources: Vec<SlotSource>,
        }

        enum SlotSource {
            Incoming(StructuralArgument),
            BorrowedStorage(StructuralArgument),
        }

        pub(crate) struct ReferenceCallEffects {
            functions: BTreeMap<MachineId, Option<Vec<SlotEffect>>>,
            declarations: Vec<StructuralTypeDeclaration>,
        }

        impl ReferenceCallEffects {
            pub(crate) fn new(
                functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
                boundaries: &[&BoundaryMachineDeclaration],
                dynamic_dispatch: &terminal_psi::TerminalDynamicDispatchCatalog,
                types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            ) -> Self {
                let declarations = types
                    .values()
                    .map(|value| (**value).clone())
                    .collect::<Vec<_>>();
                let graph = crate::descriptor_results::optimized_reference_catalog_graph(
                    functions.values().copied(),
                    boundaries.iter().copied(),
                    &declarations,
                    dynamic_dispatch,
                    |_, node| Some(Cow::Borrowed(&node.operation)),
                );
                let mut analysis = graph
                    .as_ref()
                    .map(|graph| DescriptorAnalysis::with_reference_slots(&declarations, graph));
                let functions = functions
            .iter()
            .map(|(machine, function)| {
                let effects = (|| {
                    analysis
                        .as_mut()?
                        .stored_effects(*machine)?
                        .into_iter()
                        .map(|effect| {
                            let parameter = function.structural_parameters.get(effect.parameter)?;
                            let path = descriptor_path(
                                &declarations,
                                parameter.structural_type,
                                &effect.exact_path()?,
                            )?;
                            let access = if let Some(crossing) = path
                                .iter()
                                .rposition(|segment| *segment == StructuralPathSegment::Referent)
                            {
                                let reference = access_path_type(
                                    types,
                                    parameter.structural_type,
                                    &path[..crossing],
                                )?;
                                reference_access(types, reference)?
                            } else {
                                parameter.access
                            };
                            let destination = StructuralArgument {
                                place: parameter.place,
                                path,
                                access,
                            };
                            let sources = effect
                                .origins
                                .into_iter()
                                .map(|origin| {
                                    match origin {
                                        DescriptorStoredOrigin::IncomingSlot { position, path } => {
                                            let parameter = function.structural_parameters.get(position)?;
                                            Some(SlotSource::Incoming(StructuralArgument {
                                                place: parameter.place,
                                                path: descriptor_path(&declarations, parameter.structural_type, &path)?,
                                                access: parameter.access,
                                            }))
                                        }
                                        DescriptorStoredOrigin::Parameter { position, projection } => {
                                            if !projection.exact || projection.indirect
                                                || projection != terminal_semantics::borrowed_storage::Projection::new(&projection.path) {
                                                return None;
                                            }
                                            let source_parameter = function.structural_parameters.get(position)?;
                                            let reference = access_path_type(types, parameter.structural_type, &destination.path)?;
                                            let source = StructuralArgument {
                                                place: source_parameter.place,
                                                path: projection.path,
                                                access: reference_access(types, reference)?,
                                            };
                                            if reference_source_type(function, types, &source)
                                                != leaf_referent(types, reference, &[]) {
                                                return None;
                                            }
                                            Some(SlotSource::BorrowedStorage(source))
                                        }
                                        DescriptorStoredOrigin::Invocation { .. }
                                        | DescriptorStoredOrigin::IncomingContents { .. }
                                        | DescriptorStoredOrigin::Provider(_)
                                        | DescriptorStoredOrigin::Empty | DescriptorStoredOrigin::Literal { .. } | DescriptorStoredOrigin::AccessFormation { .. } => None,
                                    }
                                })
                                .collect::<Option<Vec<_>>>()?;
                            (!sources.is_empty()).then_some(SlotEffect {
                                destination,
                                sources,
                            })
                        })
                        .collect::<Option<Vec<_>>>()
                })();
                (*machine, effects)
            })
            .collect();
                Self {
                    functions,
                    declarations,
                }
            }

            pub(super) fn remaining_at_return<'live>(
                &self,
                function: &PsiOptimizationFunction,
                types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
                block: BlockId,
                expected: &mut Vec<LiveReference>,
                live: &'live [LiveReference],
            ) -> Result<Vec<&'live LiveReference>, OptimizationUnitValidationError> {
                let Some(effects) = self
                    .functions
                    .get(&function.machine)
                    .and_then(Option::as_ref)
                else {
                    return Ok(live.iter().collect());
                };
                let fail = || {
                    invalid(
                        function,
                        block,
                        None,
                        "normal completion differs from current stored reference effects",
                    )
                };
                let incoming = entry_references(function, types)?;
                let mut remaining = live.iter().collect::<Vec<_>>();
                for effect in effects {
                    let destination = ReferenceIdentity {
                        place: effect.destination.place,
                        path: effect.destination.path.clone(),
                    };
                    let expected_type =
                        slot_type(function, types, &destination).ok_or_else(fail)?;
                    let mut candidates = Vec::new();
                    for source in &effect.sources {
                        let (root, parent, identity) = match source {
                            SlotSource::Incoming(source) => {
                                let source_slot = ReferenceIdentity {
                                    place: source.place,
                                    path: source.path.clone(),
                                };
                                if slot_type(function, types, &source_slot) != Some(expected_type) {
                                    return Err(fail());
                                }
                                let mut matching = incoming.iter().filter(|reference| {
                                    reference.carrier == source.place
                                        && reference.carrier_path == source.path
                                });
                                let source = matching.next().ok_or_else(fail)?;
                                if matching.next().is_some() {
                                    return Err(fail());
                                }
                                (
                                    source.root.clone(),
                                    source.parent.clone(),
                                    (reference_access(types, expected_type)
                                        != Some(StructuralAccess::SharedBorrow))
                                    .then(|| source.identity.clone()),
                                )
                            }
                            SlotSource::BorrowedStorage(source) => {
                                if reference_source_type(function, types, source)
                                    != leaf_referent(types, expected_type, &[])
                                    || !leaf_access_within(types, expected_type, &[], source.access)
                                {
                                    return Err(fail());
                                }
                                let (root, parent) = normalized_direct_source(
                                    function, types, block, 0, &incoming, source,
                                )?;
                                (root, parent, None)
                            }
                        };
                        if !candidates.contains(&(root.clone(), parent.clone(), identity.clone())) {
                            candidates.push((root, parent, identity));
                        }
                    }
                    let mut matching = remaining.iter().filter(|reference| {
                        reference.carrier == destination.place
                            && reference.carrier_path == destination.path
                    });
                    let actual = *matching.next().ok_or_else(fail)?;
                    if matching.next().is_some() {
                        return Err(fail());
                    }
                    let roots = match &actual.root {
                        ReferenceOrigin::Alternatives { origins, .. } => origins
                            .iter()
                            .cloned()
                            .map(ReferenceOrigin::IngressLeaf)
                            .collect::<Vec<_>>(),
                        root => vec![root.clone()],
                    };
                    if roots.is_empty()
                        || roots.iter().any(|root| {
                            let parent = if roots.len() == 1 {
                                actual.parent.clone()
                            } else {
                                ReferenceParent::Root(root.clone())
                            };
                            !candidates.iter().any(
                                |(expected_root, expected_parent, expected_identity)| {
                                    expected_root == root
                                        && *expected_parent == parent
                                        && expected_identity
                                            .as_ref()
                                            .is_none_or(|identity| *identity == actual.identity)
                                },
                            )
                        })
                        || (roots.len() > 1
                            && actual.parent != ReferenceParent::Root(actual.root.clone()))
                    {
                        return Err(fail());
                    }
                    expected.retain(|reference| {
                        reference.carrier != destination.place
                            || reference.carrier_path != destination.path
                    });
                    remaining.retain(|reference| {
                        reference.carrier != destination.place
                            || reference.carrier_path != destination.path
                    });
                }
                Ok(remaining)
            }

            #[allow(clippy::too_many_arguments)]
            pub(super) fn stage(
                &self,
                function: &PsiOptimizationFunction,
                functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
                types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
                borrowed_sources: &BorrowedReferenceSources,
                block: BlockId,
                node: u32,
                operation: &O,
                live: &[LiveReference],
                moved: &BTreeSet<ReferenceIdentity>,
            ) -> Result<StoredReferences, OptimizationUnitValidationError> {
                let O::CallStructural {
                    callee,
                    structural_arguments,
                    ..
                } = operation
                else {
                    return Ok(StoredReferences::default());
                };
                let fail = |reason| invalid(function, block, Some(node), reason);
                let effects = callee
                    .machine()
                    .and_then(|machine| self.functions.get(&machine))
                    .and_then(Option::as_ref);
                let Some(effects) = effects else {
                    if structural_arguments.iter().any(|argument| {
                        structural_source_contract(function, argument.place)
                    .and_then(|signature| {
                        access_path_type(types, signature.structural_type, &argument.path)
                    })
                    .is_none_or(|selected| {
                        terminal_semantics::descriptor_results::DescriptorSlotKind::References
                            .writable_slots(
                                &self.declarations,
                                argument.place,
                                selected,
                                argument.access,
                            )
                            .is_none_or(|slots| !slots.is_empty())
                    })
                    }) {
                        return Err(fail(
                            "ordinary call has no complete stored reference effects",
                        ));
                    }
                    return Ok(StoredReferences::default());
                };
                if effects.is_empty() {
                    return Ok(StoredReferences::default());
                }
                let callee = callee
                    .machine()
                    .and_then(|machine| functions.get(&machine))
                    .ok_or_else(|| fail("stored reference callee is absent"))?;
                let mut replacements = Vec::new();
                for effect in effects {
                    let destination = call_source(
                        &callee.structural_parameters,
                        structural_arguments,
                        &effect.destination,
                    )
                    .ok_or_else(|| fail("stored reference destination argument is absent"))?;
                    if !matches!(
                        destination.access,
                        StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
                    ) {
                        return Err(fail(
                            "stored reference destination lacks exclusive carrier access",
                        ));
                    }
                    let destination =
                        resolve_slot(function, types, block, node, live, &destination)?;
                    let residents = live
                        .iter()
                        .filter(|reference| {
                            reference.carrier == destination.place
                                && reference.carrier_path == destination.path
                        })
                        .count();
                    if residents > 1
                        || (residents == 0
                            && !(destination.path.contains(&StructuralPathSegment::Referent)
                                && function.structural_parameter(destination.place).is_some()))
                    {
                        return Err(fail("stored reference destination is absent or ambiguous"));
                    }
                    let expected = slot_type(function, types, &destination)
                        .ok_or_else(|| fail("stored reference destination type is absent"))?;
                    let mut alternatives = Vec::new();
                    for source in &effect.sources {
                        let formal = match source {
                            SlotSource::Incoming(formal) | SlotSource::BorrowedStorage(formal) => {
                                formal
                            }
                        };
                        let actual = call_source(
                            &callee.structural_parameters,
                            structural_arguments,
                            formal,
                        )
                        .ok_or_else(|| fail("stored reference source argument is absent"))?;
                        let reference = match source {
                            SlotSource::Incoming(_) => {
                                let source_slot =
                                    resolve_slot(function, types, block, node, live, &actual)?;
                                if slot_type(function, types, &source_slot) != Some(expected) {
                                    return Err(fail(
                                        "stored reference transfer changes descriptor type",
                                    ));
                                }
                                let mut matching = live.iter().filter(|reference| {
                                    reference.carrier == source_slot.place
                                        && reference.carrier_path == source_slot.path
                                });
                                let mut reference = matching
                                    .next()
                                    .cloned()
                                    .ok_or_else(|| fail("stored reference source is not live"))?;
                                if matching.next().is_some() {
                                    return Err(fail("stored reference source is ambiguous"));
                                }
                                if source_slot != destination
                                    && !moved.contains(&reference.identity)
                                {
                                    if reference_access(types, expected)
                                        != Some(StructuralAccess::SharedBorrow)
                                    {
                                        return Err(fail(
                                            "stored exclusive reference is not transferred by its argument",
                                        ));
                                    }
                                    reference.identity = destination.clone();
                                }
                                reference.carrier = destination.place;
                                reference.carrier_path = destination.path.clone();
                                reference
                            }
                            SlotSource::BorrowedStorage(_) => {
                                if reference_source_type(function, types, &actual)
                                    != leaf_referent(types, expected, &[])
                                    || !leaf_access_within(types, expected, &[], actual.access)
                                {
                                    return Err(fail(
                                        "stored reference birth changes source type or permission",
                                    ));
                                }
                                let (root, parent) = normalized_source(
                                    function,
                                    types,
                                    borrowed_sources,
                                    block,
                                    node,
                                    live,
                                    &actual,
                                )?;
                                LiveReference {
                                    identity: destination.clone(),
                                    carrier: destination.place,
                                    carrier_path: destination.path.clone(),
                                    root,
                                    parent,
                                }
                            }
                        };
                        if !alternatives.contains(&reference) {
                            alternatives.push(reference);
                        }
                    }
                    let replacement =
                        combine_alternative_references(function, block, node, alternatives)?;
                    if replacements.iter().any(|existing: &LiveReference| {
                        existing.carrier == replacement.carrier
                            && existing.carrier_path == replacement.carrier_path
                    }) {
                        return Err(fail(
                            "ordinary call writes one reference slot more than once",
                        ));
                    }
                    if live.iter().any(|old| old.identity == replacement.identity && old != &replacement
                && !moved.contains(&old.identity)
                && live.iter().any(|child| matches!(&child.parent, ReferenceParent::Reference(identity) if identity == &old.identity))) {
                return Err(fail("stored reference replacement reuses a historical parent identity"));
            }
                    replacements.push(replacement);
                }
                Ok(StoredReferences {
                    replacements,
                    retained_identities: live
                        .iter()
                        .filter(|reference| !moved.contains(&reference.identity))
                        .map(|reference| reference.identity.clone())
                        .collect(),
                })
            }
        }

        fn slot_type(
            function: &PsiOptimizationFunction,
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            slot: &ReferenceIdentity,
        ) -> Option<StructuralTypeId> {
            let selected = leaf_type(
                types,
                structural_source_contract(function, slot.place)?.structural_type,
                &slot.path,
            )?;
            reference_access(types, selected)?;
            Some(selected)
        }

        fn resolve_slot(
            function: &PsiOptimizationFunction,
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            block: BlockId,
            node: u32,
            live: &[LiveReference],
            argument: &StructuralArgument,
        ) -> Result<ReferenceIdentity, OptimizationUnitValidationError> {
            let Some(crossing) = argument
                .path
                .iter()
                .rposition(|segment| *segment == StructuralPathSegment::Referent)
            else {
                return Ok(ReferenceIdentity {
                    place: argument.place,
                    path: argument.path.clone(),
                });
            };
            let reject = |reason| invalid(function, block, Some(node), reason);
            let custody = access_reference_custody(function, types, block, node, live, argument)?;
            let root = structural_source_contract(function, argument.place)
                .ok_or_else(|| reject("stored reference root is absent"))?;
            let reference =
                access_path_type(types, root.structural_type, &argument.path[..crossing])
                    .ok_or_else(|| reject("stored reference crossing is untyped"))?;
            let referent = leaf_referent(types, reference, &[])
                .ok_or_else(|| reject("stored reference crossing has no referent"))?;
            let mut slots = Vec::new();
            for origin in custody.origins {
                let mut slot = match origin {
                    ReferenceOrigin::Primitive(place) => ReferenceIdentity {
                        place,
                        path: Vec::new(),
                    },
                    ReferenceOrigin::IngressLeaf(identity) => {
                        identity.exact_identity().ok_or_else(|| {
                            reject("stored reference carrier has no exact storage identity")
                        })?
                    }
                    ReferenceOrigin::Invocation { .. } | ReferenceOrigin::Alternatives { .. } => {
                        return Err(reject("stored reference carrier has alternative storage"));
                    }
                };
                slot = storage_cursor(function, types, slot, referent)
                    .ok_or_else(|| reject("stored reference origin has no exact storage cursor"))?;
                slot.path.extend_from_slice(&argument.path[crossing + 1..]);
                if !slots.contains(&slot) {
                    slots.push(slot);
                }
            }
            if slots.len() != 1 {
                return Err(reject("stored reference requires one exact carrier slot"));
            }
            Ok(slots.remove(0))
        }

        #[derive(Default)]
        pub(super) struct StoredReferences {
            replacements: Vec<LiveReference>,
            retained_identities: BTreeSet<ReferenceIdentity>,
        }

        impl StoredReferences {
            pub(super) fn commit(
                self,
                function: &PsiOptimizationFunction,
                types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
                block: BlockId,
                node: u32,
                live: &mut Vec<LiveReference>,
            ) -> Result<(), OptimizationUnitValidationError> {
                if self.replacements.is_empty() {
                    return Ok(());
                }
                let replaced = |reference: &LiveReference| {
                    self.replacements.iter().any(|replacement| {
                        replacement.carrier == reference.carrier
                            && replacement.carrier_path == reference.carrier_path
                    })
                };
                let mut staged = live
                    .iter()
                    .filter(|reference| !replaced(reference))
                    .cloned()
                    .collect::<Vec<_>>();
                let mut publications = staged
                    .iter()
                    .filter(|reference| !self.retained_identities.contains(&reference.identity))
                    .cloned()
                    .collect::<Vec<_>>();
                for replacement in self.replacements {
                    let permission = |reference: &LiveReference| {
                        slot_type(
                            function,
                            types,
                            &ReferenceIdentity {
                                place: reference.carrier,
                                path: reference.carrier_path.clone(),
                            },
                        )
                        .and_then(|selected| reference_access(types, selected))
                    };
                    if publications.iter().any(|reference| {
                        origins_overlap(&reference.root, &replacement.root)
                            && (permission(reference) != Some(StructuralAccess::SharedBorrow)
                                || permission(&replacement) != Some(StructuralAccess::SharedBorrow))
                    }) {
                        return Err(invalid(
                            function,
                            block,
                            Some(node),
                            "stored and returned reference effects alias an exclusive origin",
                        ));
                    }

                    if staged
                        .iter()
                        .any(|reference| reference.identity == replacement.identity)
                    {
                        return Err(invalid(
                            function,
                            block,
                            Some(node),
                            "stored and returned reference effects duplicate an identity",
                        ));
                    }
                    publications.push(replacement.clone());
                    staged.push(replacement);
                }
                staged.sort_by(|left, right| left.identity.cmp(&right.identity));
                *live = staged;
                Ok(())
            }
        }
    }
    mod windows {
        use super::owned_resident;
        use super::*;
        use terminal_psi::StructuralPathSegment;
        use terminal_semantics::structural_paths_may_overlap;

        #[allow(clippy::too_many_arguments)]
        pub(super) fn transfer(
            function: &PsiOptimizationFunction,
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            block: BlockId,
            node: u32,
            live: &mut Vec<LiveReference>,
            root: &abstract_operations::StructuralWindowRoot,
            path: &[StructuralPathSegment],
            field: Option<semantic_vocabulary::StructuralFieldId>,
            access: StructuralAccess,
            value_type: StructuralTypeId,
            value: PlaceId,
            extracting: bool,
            replace_existing: bool,
        ) -> Result<(), OptimizationUnitValidationError> {
            let fail = |reason| invalid(function, block, Some(node), reason);
            let hole = window_hole(
                function,
                types,
                root.place(),
                root.structural_type(),
                path,
                field,
                access,
            )
            .ok_or_else(|| fail("restoration window does not resolve to a declared field"))?;
            let crossing = hole
                .iter()
                .rposition(|segment| *segment == StructuralPathSegment::Referent);
            let (origins, dependencies, suffix) = if let Some(position) = crossing {
                let custody = access_reference_custody(
                    function,
                    types,
                    block,
                    node,
                    live,
                    &StructuralArgument {
                        place: root.place(),
                        path: hole.clone(),
                        access,
                    },
                )?;
                if !terminal_semantics::structural_window_selects_child(path, field)
                    && !terminal_semantics::structural_window_origins(
                        function.structural_parameters.iter().chain(
                            function
                                .blocks
                                .iter()
                                .flat_map(|block| &block.structural_parameters),
                        ),
                        custody.origins.iter().map(origin_place),
                        access,
                        |place| {
                            crate::structural_place_supports_window_restoration(function, place)
                        },
                    )
                {
                    return Err(fail(
                        "whole-referent restoration requires live owned or exclusive ingress storage",
                    ));
                }
                (custody.origins, custody.dependencies, &hole[position + 1..])
            } else {
                (
                    vec![ReferenceOrigin::Primitive(root.place())],
                    Vec::new(),
                    hole.as_slice(),
                )
            };
            let mut regions = Vec::new();
            for origin in origins {
                let candidates = match origin {
                    ReferenceOrigin::Invocation { .. } => {
                        return Err(fail("external reference has no owned restoration cursor"));
                    }
                    ReferenceOrigin::Primitive(place) => vec![ReferenceIdentity {
                        place,
                        path: Vec::new(),
                    }],
                    ReferenceOrigin::IngressLeaf(identity) => {
                        vec![identity.exact_identity().ok_or_else(|| {
                            fail("window reference origin has no exact storage cursor")
                        })?]
                    }
                    ReferenceOrigin::Alternatives { origins, .. } => origins
                        .into_iter()
                        .map(|origin| {
                            origin.exact_identity().ok_or_else(|| {
                                fail("window reference origin has no exact storage cursor")
                            })
                        })
                        .collect::<Result<Vec<_>, _>>()?,
                };
                for mut region in candidates {
                    if let Some(position) = crossing {
                        let reference =
                            access_path_type(types, root.structural_type(), &hole[..position])
                                .ok_or_else(|| fail("window reference crossing has no type"))?;
                        let referent = leaf_referent(types, reference, &[])
                            .ok_or_else(|| fail("window reference crossing has no referent"))?;
                        region = super::storage_cursor(function, types, region, referent)
                            .ok_or_else(|| {
                                fail("window reference origin has no exact storage cursor")
                            })?;
                    }
                    region.path.extend_from_slice(suffix);
                    if !regions.contains(&region) {
                        regions.push(region);
                    }
                }
            }
            let expected = leaf_paths(types, value_type, live.len())
                .ok_or_else(|| fail("window reference roster exceeds live custody"))?;
            let is_dependency = |reference: &LiveReference| {
                dependencies.iter().any(|dependency| {
            matches!(dependency, ReferenceOrigin::IngressLeaf(identity)
            if identity.exact_identity().is_some_and(|identity| identity.place == reference.carrier && identity.path == reference.carrier_path))
        })
            };
            let ancestors = live
                .iter()
                .filter(|reference| is_dependency(reference))
                .map(|reference| reference.identity.clone())
                .collect::<Vec<_>>();
            for region in &regions {
                if !access_paths::descriptor_storage_available(
                    function,
                    types,
                    live,
                    region.place,
                    &region.path,
                    &ancestors,
                    access,
                ) {
                    return Err(fail("restoration window accesses suspended storage"));
                }
            }
            if expected.is_empty() {
                if regions.iter().any(|region| {
                    live.iter().any(|reference| {
                        reference.carrier == region.place
                            && (!reference.carrier_path.starts_with(&region.path)
                                || owned_resident(&reference.carrier_path, &region.path))
                            && structural_paths_may_overlap(&reference.carrier_path, &region.path)
                    })
                }) {
                    return Err(fail("resource-free window overlaps a live descriptor"));
                }
                return Ok(());
            }
            let [region] = regions.as_slice() else {
                return Err(fail(
                    "reference-bearing window requires one exact storage region",
                ));
            };
            let whole_value = ReferenceIdentity {
                place: value,
                path: Vec::new(),
            };
            let (source, destination) = if extracting {
                (region, &whole_value)
            } else {
                (&whole_value, region)
            };
            let copies_storage = !extracting
                && structural_source_contract(function, value).is_some_and(|contract| {
                    contract.access == StructuralAccess::Owned
                        && contract.multiplicity
                            == terminal_psi::StructuralMultiplicity::Unrestricted
                });
            let mut staged = live.clone();
            if replace_existing {
                discard_subtree(
                    function,
                    types,
                    block,
                    &mut staged,
                    destination.place,
                    &destination.path,
                )?;
            }
            let staged_live = &mut staged;
            if staged_live.iter().any(|reference| {
                reference.carrier == destination.place
                    && (!reference.carrier_path.starts_with(&destination.path)
                        || owned_resident(&reference.carrier_path, &destination.path))
                    && structural_paths_may_overlap(&reference.carrier_path, &destination.path)
            }) {
                return Err(fail("window destination still owns reference custody"));
            }
            let mut relocations = Vec::new();
            for leaf in expected {
                if copies_storage
                    && leaf_type(types, value_type, &leaf)
                        .and_then(|identity| reference_access(types, identity))
                        != Some(StructuralAccess::SharedBorrow)
                {
                    return Err(fail("window copy requires shared descriptor leaves"));
                }
                let mut source_path = source.path.clone();
                source_path.extend_from_slice(&leaf);
                let mut matching = staged_live.iter().enumerate().filter(|(_, reference)| {
                    reference.carrier == source.place && reference.carrier_path == source_path
                });
                let (position, reference) = matching
                    .next()
                    .ok_or_else(|| fail("window source reference leaf is absent"))?;
                if matching.next().is_some()
            || staged_live.iter().any(|child| matches!(&child.parent, ReferenceParent::Reference(parent) if parent == &reference.identity)) {
            return Err(fail("window source reference leaf is ambiguous or suspended"));
        }
                let mut destination_path = destination.path.clone();
                destination_path.extend(leaf);
                relocations.push((position, destination_path));
            }
            if staged_live.iter().enumerate().any(|(position, reference)| {
                reference.carrier == source.place
                    && owned_resident(&reference.carrier_path, &source.path)
                    && !relocations
                        .iter()
                        .any(|(selected, _)| *selected == position)
            }) {
                return Err(fail("window source reference roster differs from its type"));
            }
            if !access_paths::descriptor_storage_available(
                function,
                types,
                staged_live,
                source.place,
                &source.path,
                &ancestors,
                if copies_storage {
                    StructuralAccess::SharedBorrow
                } else {
                    StructuralAccess::Owned
                },
            ) {
                return Err(fail("window source storage has an outstanding loan"));
            }
            for (position, path) in relocations {
                if copies_storage {
                    let mut copied = staged_live[position].clone();
                    copied.identity = ReferenceIdentity {
                        place: destination.place,
                        path: path.clone(),
                    };
                    if staged_live
                        .iter()
                        .any(|reference| reference.identity == copied.identity)
                    {
                        return Err(fail("window copy destination identity is already live"));
                    }
                    copied.carrier = destination.place;
                    copied.carrier_path = path;
                    staged_live.push(copied);
                } else {
                    staged_live[position].carrier = destination.place;
                    staged_live[position].carrier_path = path;
                }
            }
            *live = staged;
            Ok(())
        }
    }
    use crate::unit_validation::references::access_covers;
    use access_paths::{access_reference_custody, call_dependency_conflict};
    use semantic_vocabulary::StructuralFieldId;
    use terminal_psi::ReferenceOriginProjection;
    use terminal_psi::ReferenceOriginSource;
    use terminal_psi::StructuralOperationResult;
    use terminal_psi::StructuralPathSegment;
    use terminal_psi::StructuralReferenceResultSource;
    use terminal_semantics::ReferenceCallResult;
    use terminal_semantics::ReferenceCallSource;
    use terminal_semantics::reference_origin_paths_may_overlap;
    use terminal_semantics::reference_origin_type;

    fn origin_place(origin: &ReferenceOrigin) -> PlaceId {
        match origin {
            ReferenceOrigin::Invocation { result, .. } => result.place,
            ReferenceOrigin::Primitive(place) => *place,
            ReferenceOrigin::IngressLeaf(identity) => identity.place,
            ReferenceOrigin::Alternatives { place, .. } => *place,
        }
    }

    fn projected_origin(
        origin: &ReferenceOrigin,
        suffix: &[StructuralPathSegment],
    ) -> ReferenceOrigin {
        projected_origin_family(
            origin,
            &suffix
                .iter()
                .cloned()
                .map(ReferenceOriginProjection::Exact)
                .collect::<Vec<_>>(),
            semantic_vocabulary::ReferenceOriginExtent::Projected,
        )
    }

    fn projected_origin_family(
        origin: &ReferenceOrigin,
        suffix: &[ReferenceOriginProjection],
        extent: semantic_vocabulary::ReferenceOriginExtent,
    ) -> ReferenceOrigin {
        if suffix.is_empty() && extent == semantic_vocabulary::ReferenceOriginExtent::Projected {
            return origin.clone();
        }
        match origin {
            ReferenceOrigin::Invocation {
                result,
                carrier_type,
                path,
                extent: previous,
            } => {
                let mut path = path.clone();
                let extent = if *previous == semantic_vocabulary::ReferenceOriginExtent::Projected {
                    path.extend_from_slice(suffix);
                    extent
                } else {
                    *previous
                };
                ReferenceOrigin::Invocation {
                    result: result.clone(),
                    carrier_type: *carrier_type,
                    path,
                    extent,
                }
            }
            ReferenceOrigin::Primitive(place) => {
                ReferenceOrigin::IngressLeaf(ReferenceOriginIdentity {
                    place: *place,
                    path: suffix.to_vec(),
                    extent,
                })
            }
            ReferenceOrigin::IngressLeaf(identity) => {
                let mut identity = identity.clone();
                if identity.extent == semantic_vocabulary::ReferenceOriginExtent::Projected {
                    identity.path.extend_from_slice(suffix);
                    identity.extent = extent;
                }
                ReferenceOrigin::IngressLeaf(identity)
            }
            ReferenceOrigin::Alternatives { place, origins } => {
                let mut origins = origins.clone();
                for origin in &mut origins {
                    if origin.extent == semantic_vocabulary::ReferenceOriginExtent::Projected {
                        origin.path.extend_from_slice(suffix);
                        origin.extent = extent;
                    }
                }
                origins.sort();
                origins.dedup();
                ReferenceOrigin::Alternatives {
                    place: *place,
                    origins,
                }
            }
        }
    }

    fn origins_overlap(left: &ReferenceOrigin, right: &ReferenceOrigin) -> bool {
        if matches!(left, ReferenceOrigin::Invocation { .. })
            || matches!(right, ReferenceOrigin::Invocation { .. })
        {
            return true;
        }
        if origin_place(left) != origin_place(right) {
            return false;
        }
        let overlaps_path = |path: &[ReferenceOriginProjection]| match right {
            ReferenceOrigin::Invocation { .. } | ReferenceOrigin::Primitive(_) => true,
            ReferenceOrigin::IngressLeaf(identity) => {
                reference_origin_paths_may_overlap(path, &identity.path)
            }
            ReferenceOrigin::Alternatives { origins, .. } => origins
                .iter()
                .any(|other| reference_origin_paths_may_overlap(path, &other.path)),
        };
        match left {
            ReferenceOrigin::Invocation { .. } | ReferenceOrigin::Primitive(_) => true,
            ReferenceOrigin::IngressLeaf(identity) => overlaps_path(&identity.path),
            ReferenceOrigin::Alternatives { origins, .. } => {
                origins.iter().any(|origin| overlaps_path(&origin.path))
            }
        }
    }

    fn invalid(
        function: &PsiOptimizationFunction,
        block: BlockId,
        node: Option<u32>,
        reason: &'static str,
    ) -> OptimizationUnitValidationError {
        OptimizationUnitValidationError::CurrentReferenceCustodyViolation {
            machine: function.machine,
            block,
            node,
            reason,
        }
    }

    pub(super) fn owned_resident(
        path: &[StructuralPathSegment],
        prefix: &[StructuralPathSegment],
    ) -> bool {
        path.strip_prefix(prefix).is_some_and(|suffix| {
            !suffix.iter().any(|segment| {
                matches!(
                    segment,
                    StructuralPathSegment::Referent
                        | StructuralPathSegment::RepresentationView { .. }
                )
            })
        })
    }

    pub(super) fn borrows_owned_storage(
        function: &PsiOptimizationFunction,
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        reference: &LiveReference,
        place: PlaceId,
    ) -> bool {
        let ingress = |identity: ReferenceOriginIdentity| {
            if identity.place != place {
                return false;
            }
            let expected =
                structural_source_contract(function, reference.carrier).and_then(|signature| {
                    leaf_referent(types, signature.structural_type, &reference.carrier_path)
                });
            expected
                .and_then(|expected| {
                    identity
                        .exact_identity()
                        .and_then(|identity| storage_cursor(function, types, identity, expected))
                })
                .is_none_or(|cursor| !cursor.path.contains(&StructuralPathSegment::Referent))
        };
        match &reference.root {
            ReferenceOrigin::Invocation { .. } => false,
            ReferenceOrigin::Primitive(root) => *root == place,
            ReferenceOrigin::IngressLeaf(identity) => ingress(identity.clone()),
            ReferenceOrigin::Alternatives { origins, .. } => {
                origins.iter().any(|origin| ingress(origin.clone()))
            }
        }
    }

    fn storage_cursor(
        function: &PsiOptimizationFunction,
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        mut identity: ReferenceIdentity,
        expected: StructuralTypeId,
    ) -> Option<ReferenceIdentity> {
        let signature = structural_source_contract(function, identity.place)?;
        let selected = leaf_type(types, signature.structural_type, &identity.path)?;
        if selected != expected {
            let parameter = function.structural_parameter(identity.place)?;
            if parameter.structural_type != signature.structural_type
                || identity.path.iter().any(|segment| {
                    !matches!(
                        segment,
                        StructuralPathSegment::Field(_)
                            | StructuralPathSegment::Case(_)
                            | StructuralPathSegment::FixedIndex(_)
                    )
                })
                || !matches!(types.get(&selected).map(|declaration| &declaration.shape),
                Some(terminal_psi::StructuralTypeShape::Reference { referent, .. }) if *referent == expected)
            {
                return None;
            }
            identity.path.push(StructuralPathSegment::Referent);
        }
        Some(identity)
    }

    pub(super) fn entry_references(
        function: &PsiOptimizationFunction,
        structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    ) -> Result<Vec<LiveReference>, OptimizationUnitValidationError> {
        let mut live = Vec::new();
        for parameter in &function.structural_parameters {
            if !contains_reference(structural_types, parameter.structural_type) {
                continue;
            }
            let paths = leaf_paths(
                structural_types,
                parameter.structural_type,
                MAX_INCOMING_REFERENCE_LEAVES - live.len(),
            )
            .ok_or_else(|| {
                invalid(
                    function,
                    function.entry,
                    None,
                    "incoming reference leaf reconstruction capacity exceeded",
                )
            })?;
            for path in paths {
                let identity = ReferenceIdentity {
                    place: parameter.place,
                    path: path.clone(),
                };
                let root = ReferenceOrigin::IngressLeaf(identity.clone().into());
                live.push(LiveReference {
                    identity,
                    carrier: parameter.place,
                    carrier_path: path,
                    root: root.clone(),
                    parent: ReferenceParent::Root(root),
                });
            }
        }
        live.sort_by(|left, right| left.identity.cmp(&right.identity));
        Ok(live)
    }

    fn operation_structural_arguments(operation: &O) -> &[StructuralArgument] {
        match operation {
            abstract_operations::call_structural_or_boundary_call_pattern!(
                structural_arguments
            ) => structural_arguments.as_slice(),
            O::StoreStructuralField { value, .. } => std::slice::from_ref(value),
            _ => &[],
        }
    }

    pub(super) fn window_hole(
        function: &PsiOptimizationFunction,
        types: &(impl StructuralTypes + ?Sized),
        place: PlaceId,
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
        field: Option<StructuralFieldId>,
        required_access: StructuralAccess,
    ) -> Option<Vec<StructuralPathSegment>> {
        let (geometry_root, suffix) = if path.contains(&StructuralPathSegment::Referent) {
            crate::unit_validation::references::record_field_destination(
                function,
                types,
                place,
                path,
                required_access,
            )?
        } else {
            (root, path)
        };
        let projection = terminal_semantics::structural_window_projection(
            types.declarations(),
            geometry_root,
            suffix,
            field,
        )?;
        let mut hole = path[..path.len() - suffix.len()].to_vec();
        hole.extend(projection.path);
        Some(hole)
    }

    pub(super) fn check_root_access(
        function: &PsiOptimizationFunction,
        block: BlockId,
        node: u32,
        live: &[LiveReference],
        place: PlaceId,
    ) -> Result<(), OptimizationUnitValidationError> {
        check_region_access(function, block, node, live, place, &[])
    }

    pub(super) fn check_region_access(
        function: &PsiOptimizationFunction,
        block: BlockId,
        node: u32,
        live: &[LiveReference],
        place: PlaceId,
        path: &[StructuralPathSegment],
    ) -> Result<(), OptimizationUnitValidationError> {
        check_region_access_with(function, block, node, live, place, path, |_| false)
    }

    fn check_region_access_with(
        function: &PsiOptimizationFunction,
        block: BlockId,
        node: u32,
        live: &[LiveReference],
        place: PlaceId,
        path: &[StructuralPathSegment],
        compatible_origin: impl Fn(&LiveReference) -> bool,
    ) -> Result<(), OptimizationUnitValidationError> {
        let overlaps = |other: &[StructuralPathSegment]| {
            terminal_semantics::structural_paths_may_overlap(path, other)
        };
        let origin_path = path
            .iter()
            .cloned()
            .map(ReferenceOriginProjection::Exact)
            .collect::<Vec<_>>();
        let overlaps_origin = |other: &[ReferenceOriginProjection]| {
            reference_origin_paths_may_overlap(&origin_path, other)
        };
        if live.iter().any(|reference| {
            (reference.carrier == place && overlaps(&reference.carrier_path))
                || (origin_place(&reference.root) == place
                    && !compatible_origin(reference)
                    && match &reference.root {
                        ReferenceOrigin::Invocation { .. } | ReferenceOrigin::Primitive(_) => true,
                        ReferenceOrigin::IngressLeaf(identity) => overlaps_origin(&identity.path),
                        ReferenceOrigin::Alternatives { origins, .. } => {
                            origins.iter().any(|origin| overlaps_origin(&origin.path))
                        }
                    })
        }) {
            return Err(invalid(
                function,
                block,
                Some(node),
                "operation accesses suspended referent or treats reference carrier as owned data",
            ));
        }
        Ok(())
    }

    fn check_record_field_access(
        function: &PsiOptimizationFunction,
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        block: BlockId,
        node: u32,
        live: &[LiveReference],
        place: PlaceId,
        path: &[StructuralPathSegment],
        field: Option<StructuralFieldId>,
        required_access: StructuralAccess,
    ) -> Result<(), OptimizationUnitValidationError> {
        if !path.contains(&StructuralPathSegment::Referent) {
            let region = super::storage_regions::projected(function, types, place, path, field);
            return check_region_access(function, block, node, live, place, region.direct_path());
        }
        access_reference_custody(
            function,
            types,
            block,
            node,
            live,
            &StructuralArgument {
                place,
                path: path.to_vec(),
                access: required_access,
            },
        )?;
        Ok(())
    }

    fn check_observation_access(
        function: &PsiOptimizationFunction,
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        block: BlockId,
        node: u32,
        live: &[LiveReference],
        place: PlaceId,
        region: &[StructuralPathSegment],
        compatible_origin: impl Fn(&LiveReference) -> bool,
    ) -> Result<(), OptimizationUnitValidationError> {
        let access = structural_source_contract(function, place)
            .and_then(|contract| types.get(&contract.structural_type))
            .and_then(|declaration| match declaration.shape {
                terminal_psi::StructuralTypeShape::Reference { access, .. } => Some(access),
                _ => None,
            });
        let Some(access) = access else {
            return check_region_access_with(
                function,
                block,
                node,
                live,
                place,
                region,
                compatible_origin,
            );
        };
        let mut carriers = live
            .iter()
            .filter(|reference| reference.carrier == place && reference.carrier_path.is_empty());
        let available = carriers.next().is_some_and(|carrier| {
        !live.iter().any(|child| origin_place(&child.root) == place || matches!(&child.parent, ReferenceParent::Reference(parent) if parent == &carrier.identity))
    }) && carriers.next().is_none();
        if !available || !matches!(access, terminal_psi::reading_borrow_pair_pattern!()) {
            return Err(invalid(
                function,
                block,
                Some(node),
                "observation has no active readable reference carrier",
            ));
        }
        Ok(())
    }

    fn shared_source_compatible(
        function: &PsiOptimizationFunction,
        structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        reference: &LiveReference,
        access: StructuralAccess,
    ) -> bool {
        access == StructuralAccess::SharedBorrow
            && structural_source_contract(function, reference.carrier)
                .and_then(|contract| {
                    leaf_type(
                        structural_types,
                        contract.structural_type,
                        &reference.carrier_path,
                    )
                })
                .and_then(|leaf| reference_access(structural_types, leaf))
                == Some(StructuralAccess::SharedBorrow)
    }

    fn shared_borrow_compatibility<'a>(
        function: &'a PsiOptimizationFunction,
        structural_types: &'a BTreeMap<StructuralTypeId, &'a StructuralTypeDeclaration>,
    ) -> impl Fn(&LiveReference) -> bool + 'a {
        move |reference| {
            shared_source_compatible(
                function,
                structural_types,
                reference,
                StructuralAccess::SharedBorrow,
            )
        }
    }

    pub(super) fn copy_reference_carriers(
        function: &PsiOptimizationFunction,
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        block: BlockId,
        node: u32,
        live: &[LiveReference],
        source: PlaceId,
        path: &[StructuralPathSegment],
        selected_type: StructuralTypeId,
        destination: PlaceId,
    ) -> Result<Vec<LiveReference>, OptimizationUnitValidationError> {
        let refusal = |message| invalid(function, block, Some(node), message);
        if structural_source_contract(function, source)
            .and_then(|contract| leaf_type(types, contract.structural_type, path))
            != Some(selected_type)
            || !access_paths::descriptor_storage_available(
                function,
                types,
                live,
                source,
                path,
                &[],
                StructuralAccess::SharedBorrow,
            )
        {
            return Err(refusal("value copy has no available exact source storage"));
        }
        let paths = leaf_paths(types, selected_type, live.len())
            .ok_or_else(|| refusal("reference copy has no complete source leaf roster"))?;
        let mut copied = Vec::with_capacity(paths.len());
        for relative in &paths {
            if leaf_type(types, selected_type, relative)
                .and_then(|leaf| reference_access(types, leaf))
                != Some(StructuralAccess::SharedBorrow)
            {
                return Err(refusal(
                    "reference copy requires available shared descriptor storage",
                ));
            }
            let mut selected = path.to_vec();
            selected.extend_from_slice(relative);
            let mut observation = selected.clone();
            observation.push(StructuralPathSegment::Referent);
            access_reference_custody(
                function,
                types,
                block,
                node,
                live,
                &StructuralArgument {
                    place: source,
                    path: observation,
                    access: StructuralAccess::SharedBorrow,
                },
            )?;
            let mut matching = live.iter().filter(|reference| {
                reference.carrier == source && reference.carrier_path == selected
            });
            let source_leaf = matching
                .next()
                .ok_or_else(|| refusal("reference copy source leaf is absent"))?;
            if matching.next().is_some() {
                return Err(refusal(
                    "reference copy source leaf is ambiguous or suspended",
                ));
            }
            copied.push(LiveReference {
                identity: ReferenceIdentity {
                    place: destination,
                    path: relative.clone(),
                },
                carrier: destination,
                carrier_path: relative.clone(),
                root: source_leaf.root.clone(),
                parent: source_leaf.parent.clone(),
            });
        }
        if live.iter().any(|reference| {
            reference.carrier == source
                && reference
                    .carrier_path
                    .strip_prefix(path)
                    .is_some_and(|relative| {
                        !relative.contains(&StructuralPathSegment::Referent)
                            && !paths.iter().any(|expected| expected == relative)
                    })
        }) {
            return Err(refusal("reference copy source has an unaccounted leaf"));
        }
        Ok(copied)
    }

    fn normalized_source(
        function: &PsiOptimizationFunction,
        structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        borrowed_sources: &BorrowedReferenceSources,
        block: BlockId,
        node: u32,
        live: &[LiveReference],
        source: &StructuralArgument,
    ) -> Result<(ReferenceOrigin, ReferenceParent), OptimizationUnitValidationError> {
        let Some(origins) = borrowed_sources.get(&source.place) else {
            return normalized_direct_source(function, structural_types, block, node, live, source);
        };
        let invalid = || {
            invalid(
                function,
                block,
                Some(node),
                "borrowed reference source has no exact incoming storage",
            )
        };
        let origins = origins
            .as_ref()
            .filter(|origins| !origins.is_empty())
            .ok_or_else(invalid)?;
        let expected =
            storage_projection(function, structural_types, source).ok_or_else(invalid)?;
        let mut alternatives = Vec::with_capacity(origins.len());
        for origin in origins {
            let mut resolved = origin.clone();
            resolved.path.extend_from_slice(&source.path);
            resolved.access = source.access;
            if !crate::unit_validation::references::reference_source_accessible(
                function,
                structural_types,
                &resolved,
            ) || !storage_projection(function, structural_types, &resolved)
                .is_some_and(|actual| actual.matches(structural_types.values().copied(), expected))
            {
                return Err(invalid());
            }
            alternatives.push(normalized_direct_source(
                function,
                structural_types,
                block,
                node,
                live,
                &resolved,
            )?);
        }
        combine_alternative_origins(function, block, node, alternatives)
    }

    fn normalized_direct_source(
        function: &PsiOptimizationFunction,
        structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        block: BlockId,
        node: u32,
        live: &[LiveReference],
        source: &StructuralArgument,
    ) -> Result<(ReferenceOrigin, ReferenceParent), OptimizationUnitValidationError> {
        let fail = |reason| invalid(function, block, Some(node), reason);
        let reject_suspended = |root: &ReferenceOrigin| {
            if live.iter().any(|reference| {
                origins_overlap(&reference.root, root)
                    && !shared_source_compatible(
                        function,
                        structural_types,
                        reference,
                        source.access,
                    )
            }) {
                return Err(fail(
                    "projected referent remains suspended by a live reference",
                ));
            }
            Ok(())
        };
        if let Some(crossing) = source
            .path
            .iter()
            .rposition(|segment| *segment == StructuralPathSegment::Referent)
        {
            let carrier_path = &source.path[..crossing];
            let suffix = &source.path[crossing + 1..];
            if let Some(parent) = live.iter().find(|reference| {
                reference.carrier == source.place && reference.carrier_path == carrier_path
            }) {
                if live
                .iter()
                .any(|reference| matches!(&reference.parent, ReferenceParent::Reference(identity) if identity == &parent.identity)
                    && !shared_source_compatible(function, structural_types, reference, source.access))
            {
                return Err(fail("reference parent remains suspended by a live child"));
            }
                return Ok((
                    projected_origin(&parent.root, suffix),
                    ReferenceParent::Reference(parent.identity.clone()),
                ));
            }
            let leaf = structural_source_contract(function, source.place)
                .and_then(|contract| {
                    leaf_type(structural_types, contract.structural_type, carrier_path)
                })
                .ok_or_else(|| fail("projected reference custody is not yet supported"))?;
            if structural_types.get(&leaf).is_some_and(|declaration| {
                matches!(
                    declaration.shape,
                    terminal_psi::StructuralTypeShape::Reference { .. }
                )
            }) {
                return Err(fail("reference carrier is no longer live"));
            }
            if !suffix.is_empty() {
                return Err(fail(
                    "reference projection has no live supplying descriptor",
                ));
            }
            let root = ReferenceOrigin::IngressLeaf(
                ReferenceIdentity {
                    place: source.place,
                    path: carrier_path.to_vec(),
                }
                .into(),
            );
            reject_suspended(&root)?;
            Ok((root.clone(), ReferenceParent::Root(root)))
        } else if source.path.is_empty() {
            if live.iter().any(|reference| {
                origin_place(&reference.root) == source.place
                    && !shared_source_compatible(
                        function,
                        structural_types,
                        reference,
                        source.access,
                    )
            }) {
                return Err(fail(
                    "original referent remains suspended by a live reference",
                ));
            }
            let root = ReferenceOrigin::Primitive(source.place);
            Ok((root.clone(), ReferenceParent::Root(root)))
        } else if storage_projection(function, structural_types, source).is_some() {
            let root = ReferenceOrigin::IngressLeaf(
                ReferenceIdentity {
                    place: source.place,
                    path: source.path.clone(),
                }
                .into(),
            );
            reject_suspended(&root)?;
            Ok((root.clone(), ReferenceParent::Root(root)))
        } else {
            Err(fail("projected reference custody is not yet supported"))
        }
    }

    pub(super) fn release(
        function: &PsiOptimizationFunction,
        structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        block: BlockId,
        node: Option<u32>,
        live: &mut Vec<LiveReference>,
        source: PlaceId,
        path: &[StructuralPathSegment],
    ) -> Result<(), OptimizationUnitValidationError> {
        if !access_paths::descriptor_storage_available(
            function,
            structural_types,
            live,
            source,
            path,
            &[],
            StructuralAccess::Owned,
        ) {
            return Err(invalid(
                function,
                block,
                node,
                "release carrier storage remains borrowed",
            ));
        }
        let position = live
            .iter()
            .position(|reference| reference.carrier == source && reference.carrier_path == path)
            .ok_or_else(|| {
                invalid(
                    function,
                    block,
                    node,
                    "release requires one live reference carrier",
                )
            })?;
        if live
        .iter()
        .any(|reference| matches!(&reference.parent, ReferenceParent::Reference(identity) if identity == &live[position].identity))
    {
        return Err(invalid(
            function,
            block,
            node,
            "a reference cannot end before its child",
        ));
    }
        live.remove(position);
        Ok(())
    }

    pub(super) fn discard_owned(
        function: &PsiOptimizationFunction,
        structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        block: BlockId,
        live: &mut Vec<LiveReference>,
        source: PlaceId,
    ) -> Result<(), OptimizationUnitValidationError> {
        discard_subtree(function, structural_types, block, live, source, &[])
    }

    pub(super) fn discard_subtree(
        function: &PsiOptimizationFunction,
        structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        block: BlockId,
        live: &mut Vec<LiveReference>,
        source: PlaceId,
        prefix: &[StructuralPathSegment],
    ) -> Result<(), OptimizationUnitValidationError> {
        let Some(signature) = structural_source_contract(function, source) else {
            return Ok(());
        };
        let invalid_discard = || {
            invalid(
                function,
                block,
                None,
                "discard requires an exact live reference subtree",
            )
        };
        let selected = leaf_type(structural_types, signature.structural_type, prefix)
            .ok_or_else(invalid_discard)?;
        let paths =
            leaf_paths(structural_types, selected, live.len()).ok_or_else(invalid_discard)?;
        let mut released = BTreeSet::new();
        let mut released_paths = BTreeSet::new();
        for path in paths.iter().rev() {
            let mut selected_path = prefix.to_vec();
            selected_path.extend_from_slice(path);
            let reference = live
                .iter()
                .find(|reference| {
                    reference.carrier == source && reference.carrier_path == selected_path
                })
                .ok_or_else(invalid_discard)?;
            if live.iter().any(|child| {
                matches!(&child.parent, ReferenceParent::Reference(identity)
                if identity == &reference.identity)
                    && !released.contains(&child.identity)
            }) {
                return Err(invalid(
                    function,
                    block,
                    None,
                    "a reference cannot end before its child",
                ));
            }
            released.insert(reference.identity.clone());
            released_paths.insert(selected_path);
        }
        if live.iter().any(|reference| {
            reference.carrier == source
                && owned_resident(&reference.carrier_path, prefix)
                && !released_paths.contains(&reference.carrier_path)
        }) {
            return Err(invalid_discard());
        }
        live.retain(|reference| {
            reference.carrier != source || !released_paths.contains(&reference.carrier_path)
        });
        Ok(())
    }

    fn establish_record(
        function: &PsiOptimizationFunction,
        structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        block: BlockId,
        node: u32,
        result: &StructuralOperationResult,
        fields: &[terminal_psi::RecordFieldInitializer],
        live: &mut [LiveReference],
    ) -> Result<(), OptimizationUnitValidationError> {
        let declarations = match structural_types
            .get(&result.structural_type)
            .map(|declaration| &declaration.shape)
        {
            Some(terminal_psi::StructuralTypeShape::Record { fields, .. }) => fields,
            _ => {
                return Err(invalid(
                    function,
                    block,
                    Some(node),
                    "record transfer requires a record establishment",
                ));
            }
        };
        let declarations = declarations
            .iter()
            .filter(|field| !field.relevance.is_erased());
        if declarations.clone().count() != fields.len() {
            return Err(invalid(
                function,
                block,
                Some(node),
                "record transfer field roster differs from its type",
            ));
        }
        let operands = declarations.zip(fields).filter_map(|(declaration, field)| {
            let terminal_psi::RecordFieldValue::Structural(argument) = &field.value else {
                return None;
            };
            Some((
                vec![StructuralPathSegment::Field(declaration.identity)],
                argument,
            ))
        });
        establish_constructor(
            function,
            structural_types,
            block,
            node,
            result,
            operands,
            live,
        )
    }

    fn establish_constructor<'a>(
        function: &PsiOptimizationFunction,
        structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        block: BlockId,
        node: u32,
        result: &StructuralOperationResult,
        operands: impl Iterator<Item = (Vec<StructuralPathSegment>, &'a StructuralArgument)>,
        live: &mut [LiveReference],
    ) -> Result<(), OptimizationUnitValidationError> {
        let refusal = |reason| invalid(function, block, Some(node), reason);
        if live.iter().any(|reference| {
            reference.carrier == result.place || reference.identity.place == result.place
        }) {
            return Err(refusal(
                "record destination still owns live reference custody",
            ));
        }
        let mut relocations = Vec::new();
        let mut moved = BTreeSet::new();
        for (destination, argument) in operands {
            let signature = structural_source_contract(function, argument.place)
                .ok_or_else(|| refusal("record operand has no structural source"))?;
            let selected_type =
                leaf_type(structural_types, signature.structural_type, &argument.path)
                    .ok_or_else(|| refusal("constructor operand has no exact selected type"))?;
            let paths =
                leaf_paths(structural_types, selected_type, live.len()).ok_or_else(|| {
                    refusal("constructor operand requires more reference leaves than are live")
                })?;
            for path in paths {
                let mut source_path = argument.path.clone();
                source_path.extend_from_slice(&path);
                let position = live
                    .iter()
                    .position(|reference| {
                        reference.carrier == argument.place && reference.carrier_path == source_path
                    })
                    .ok_or_else(|| refusal("record operand does not own its reference leaf"))?;
                if !moved.insert(position) {
                    return Err(refusal("record cannot duplicate reference custody"));
                }
                let mut destination = destination.clone();
                destination.extend(path);
                relocations.push((position, destination));
            }
            if live.iter().enumerate().any(|(position, reference)| {
                reference.carrier == argument.place
                    && owned_resident(&reference.carrier_path, &argument.path)
                    && !moved.contains(&position)
            }) {
                return Err(refusal(
                    "record operand reference roster differs from its type",
                ));
            }
        }
        for (position, path) in relocations {
            live[position].carrier = result.place;
            live[position].carrier_path = path;
        }
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn apply_operation(
        function: &PsiOptimizationFunction,
        functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
        dynamic_dispatch: &terminal_psi::TerminalDynamicDispatchCatalog,
        structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        borrowed_sources: &BorrowedReferenceSources,
        call_effects: &call_effects::ReferenceCallEffects,
        block: BlockId,
        node: u32,
        operation: &O,
        live: &mut Vec<LiveReference>,
    ) -> Result<(), OptimizationUnitValidationError> {
        let refusal = |reason| invalid(function, block, Some(node), reason);
        if let Some(operation) = operation.psi_operation()
        && live.iter().any(|reference| {
            [origin_place(&reference.root), reference.carrier, reference.identity.place]
                .into_iter()
                .any(|place| function.structural_places.iter().any(|declaration| {
                    declaration.id == place && matches!(declaration.kind,
                        semantic_vocabulary::StructuralPlaceKind::OperationResult { producer, .. } if producer == operation)
                }))
        })
    {
        return Err(refusal("operation result remains pinned by live reference custody"));
    }
        match operation {
            O::AtomicEvent { event, .. } => {
                if let Some(location) = event.location() {
                    if location.path.contains(&StructuralPathSegment::Referent) {
                        let path = match location.path.split_last() {
                            Some((StructuralPathSegment::Field(_), prefix)) => prefix,
                            _ => &location.path,
                        };
                        let path = match path.split_last() {
                            Some((StructuralPathSegment::Case(_), prefix)) => prefix,
                            _ => path,
                        };
                        access_reference_custody(
                            function,
                            structural_types,
                            block,
                            node,
                            live,
                            &StructuralArgument {
                                place: location.root,
                                path: path.to_vec(),
                                access: StructuralAccess::SharedBorrow,
                            },
                        )?;
                    } else {
                        let region = super::storage_regions::projected(
                            function,
                            structural_types,
                            location.root,
                            &location.path,
                            None,
                        );
                        check_observation_access(
                            function,
                            structural_types,
                            block,
                            node,
                            live,
                            location.root,
                            region.direct_path(),
                            shared_borrow_compatibility(function, structural_types),
                        )?;
                    }
                }
            }
            O::ReleaseReference { source, path, .. } => {
                return release(
                    function,
                    structural_types,
                    block,
                    Some(node),
                    live,
                    *source,
                    path,
                );
            }
            O::EstablishCase { fields, .. } => {
                for field in fields {
                    if let terminal_psi::RecordFieldValue::Structural(argument) = &field.value {
                        check_root_access(function, block, node, live, argument.place)?;
                    }
                }
            }
            O::EstablishStructuralArray {
                result, elements, ..
            } => {
                return establish_constructor(
                    function,
                    structural_types,
                    block,
                    node,
                    result,
                    elements.iter().enumerate().map(|(index, argument)| {
                        (
                            vec![StructuralPathSegment::FixedIndex(index as u64)],
                            argument,
                        )
                    }),
                    live,
                );
            }
            O::MoveStructuralValue { result, source, .. }
            | O::TransformContent { result, source, .. } => {
                if let O::TransformContent { parents, .. } = operation {
                    for parent in parents {
                        check_region_access(
                            function,
                            block,
                            node,
                            live,
                            parent.place,
                            &parent.path,
                        )?;
                    }
                }
                return establish_constructor(
                    function,
                    structural_types,
                    block,
                    node,
                    result,
                    std::iter::once((Vec::new(), source)),
                    live,
                );
            }
            O::EstablishRecord { result, fields, .. } => {
                return establish_record(
                    function,
                    structural_types,
                    block,
                    node,
                    result,
                    fields,
                    live,
                );
            }
            O::StorageAddress { source, path, .. }
            | O::PrimitiveScalarRead { source, path, .. }
                if path.contains(&StructuralPathSegment::Referent) =>
            {
                access_reference_custody(
                    function,
                    structural_types,
                    block,
                    node,
                    live,
                    &StructuralArgument {
                        place: *source,
                        path: path.clone(),
                        access: StructuralAccess::SharedBorrow,
                    },
                )?;
            }
            abstract_operations::primitive_storage_observation_pattern!(source, path, ..) => {
                let region = super::storage_regions::projected(
                    function,
                    structural_types,
                    *source,
                    path,
                    None,
                );
                check_observation_access(
                    function,
                    structural_types,
                    block,
                    node,
                    live,
                    *source,
                    region.direct_path(),
                    shared_borrow_compatibility(function, structural_types),
                )?;
            }
            O::StructuralScalarFieldRead {
                source,
                path,
                field,
                ..
            } => {
                let region = super::storage_regions::canonical_field(
                    function,
                    structural_types,
                    *source,
                    path,
                    *field,
                );
                check_observation_access(
                    function,
                    structural_types,
                    block,
                    node,
                    live,
                    *source,
                    &region,
                    shared_borrow_compatibility(function, structural_types),
                )?;
            }
            O::StructuralCase { source, path, .. } => {
                check_observation_access(
                    function,
                    structural_types,
                    block,
                    node,
                    live,
                    *source,
                    path,
                    |_| false,
                )?;
            }
            O::PrimitiveLocalStore { destination, .. } => {
                check_root_access(function, block, node, live, *destination)?
            }
            abstract_operations::byte_len_or_field_read_pattern!(source, path, field, ..) => {
                let region = super::storage_regions::projected(
                    function,
                    structural_types,
                    *source,
                    path,
                    Some(*field),
                );
                check_observation_access(
                    function,
                    structural_types,
                    block,
                    node,
                    live,
                    *source,
                    region.direct_path(),
                    shared_borrow_compatibility(function, structural_types),
                )?;
            }
            O::EstablishByteSequenceConcatenation { .. }
            | O::ByteSequenceEqual { .. }
            | O::ByteSequencePredicate { .. } => {
                let operands = match operation {
                    O::EstablishByteSequenceConcatenation { left, right, .. }
                    | O::ByteSequenceEqual { left, right, .. } => [Some(left), Some(right)],
                    O::ByteSequencePredicate { operand, .. } => [Some(operand), None],
                    _ => unreachable!(),
                };
                for operand in operands.into_iter().flatten() {
                    match operand {
                        terminal_psi::control_flow::ByteSequenceOperand::Whole {
                            source, ..
                        } => {
                            check_record_field_access(
                                function,
                                structural_types,
                                block,
                                node,
                                live,
                                source.place,
                                &source.path,
                                None,
                                StructuralAccess::SharedBorrow,
                            )?;
                        }
                        terminal_psi::control_flow::ByteSequenceOperand::Field {
                            source,
                            field,
                            ..
                        } => check_record_field_access(
                            function,
                            structural_types,
                            block,
                            node,
                            live,
                            source.place,
                            &source.path,
                            Some(*field),
                            StructuralAccess::MutableBorrow,
                        )?,
                    }
                }
            }
            O::WriteOnlyPrimitiveStore {
                destination, path, ..
            } => {
                if path.contains(&StructuralPathSegment::Referent) {
                    access_reference_custody(
                        function,
                        structural_types,
                        block,
                        node,
                        live,
                        &StructuralArgument {
                            place: destination.place(),
                            path: path.clone(),
                            access: StructuralAccess::WriteOnlyBorrow,
                        },
                    )?;
                    return Ok(());
                }
                let region = super::storage_regions::projected(
                    function,
                    structural_types,
                    destination.place(),
                    path,
                    None,
                );
                check_region_access(
                    function,
                    block,
                    node,
                    live,
                    destination.place(),
                    region.direct_path(),
                )?;
            }
            O::StructuralScalarFieldStore {
                destination,
                path,
                field,
                ..
            } => {
                check_record_field_access(
                    function,
                    structural_types,
                    block,
                    node,
                    live,
                    destination.place(),
                    path,
                    Some(*field),
                    StructuralAccess::WriteOnlyBorrow,
                )?;
            }
            O::StructuralByteSequenceFieldStore {
                destination,
                path,
                field,
                source,
                ..
            } => {
                check_record_field_access(
                    function,
                    structural_types,
                    block,
                    node,
                    live,
                    *destination,
                    path,
                    Some(*field),
                    StructuralAccess::MutableBorrow,
                )?;
                check_root_access(function, block, node, live, *source)?;
            }
            O::StructuralByteSequenceFieldByteStore {
                destination,
                path,
                field,
                ..
            } => {
                check_record_field_access(
                    function,
                    structural_types,
                    block,
                    node,
                    live,
                    *destination,
                    path,
                    Some(*field),
                    StructuralAccess::MutableBorrow,
                )?;
            }
            O::MoveStructuralField {
                source,
                path,
                field,
                result,
                ..
            } => {
                return windows::transfer(
                    function,
                    structural_types,
                    block,
                    node,
                    live,
                    source,
                    path,
                    *field,
                    StructuralAccess::MutableBorrow,
                    result.structural_type,
                    result.place,
                    true,
                    false,
                );
            }
            O::StoreStructuralField {
                disposition,
                destination,
                path,
                field,
                value,
                ..
            } => {
                let value_type = structural_source_contract(function, value.place)
                    .ok_or_else(|| refusal("window store value has no declaration"))?
                    .structural_type;
                return windows::transfer(
                    function,
                    structural_types,
                    block,
                    node,
                    live,
                    destination,
                    path,
                    *field,
                    disposition.required_access(),
                    value_type,
                    value.place,
                    false,
                    *disposition == terminal_psi::StructuralStoreDisposition::ReplaceDiscardable,
                );
            }
            O::StructuralLeafCopy {
                result,
                source,
                path,
                ..
            } if contains_reference(structural_types, result.structural_type) => {
                let abstract_operations::StructuralCopyPath::Ordinary(path) = path else {
                    return Err(refusal(
                        "reference copy needs an exact ordinary carrier path",
                    ));
                };
                let shared_descriptor = terminal_semantics::is_copyable_carrier(
                    structural_types.values().copied(),
                    result.structural_type,
                );
                let source_type =
                    structural_source_contract(function, *source).and_then(|signature| {
                        leaf_type(structural_types, signature.structural_type, path)
                    });
                if source_type != Some(result.structural_type)
                    || (result.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
                        && !(result.multiplicity == terminal_psi::StructuralMultiplicity::Affine
                            && shared_descriptor))
                    || !result.claims.is_empty()
                    || !result.qualification_establishments.is_empty()
                    || live.iter().any(|reference| {
                        reference.carrier == result.place
                            || reference.identity.place == result.place
                    })
                {
                    return Err(refusal(
                        "reference copy has no exact shared source or fresh destination",
                    ));
                }
                let copied = copy_reference_carriers(
                    function,
                    structural_types,
                    block,
                    node,
                    live,
                    *source,
                    path,
                    result.structural_type,
                    result.place,
                )?;
                live.extend(copied);
            }
            _ => {}
        }
        let mut normalized_arguments = Vec::new();
        let mut moved = BTreeSet::new();
        let arguments = operation_structural_arguments(operation);
        for (argument_position, argument) in arguments.iter().enumerate() {
            if argument.path.contains(&StructuralPathSegment::Referent) {
                let custody = access_reference_custody(
                    function,
                    structural_types,
                    block,
                    node,
                    live,
                    argument,
                )?;
                if arguments.iter().enumerate().any(|(position, other)| {
                    position != argument_position
                        && call_dependency_conflict(&custody.dependencies, other)
                }) {
                    return Err(refusal(
                        "call consumes or mutates an intermediate reference carrier",
                    ));
                }
                normalized_arguments.push((
                    custody.origins,
                    argument.access,
                    CallRegion::Referent,
                    custody.dependencies,
                    argument_position,
                ));
                continue;
            }
            if structural_source_contract(function, argument.place)
                .is_some_and(|source| contains_reference(structural_types, source.structural_type))
            {
                if matches!(operation, O::BoundaryCall { .. }) {
                    return Err(refusal(
                        "reference-bearing owned calls require a whole internal argument",
                    ));
                }
                let signature = structural_source_contract(function, argument.place)
                    .ok_or_else(|| refusal("owned reference argument has no source"))?;
                if !access_covers(signature.access, argument.access) {
                    return Err(refusal("carrier borrow exceeds source authority"));
                }
                let projected_type = argument
                    .path
                    .iter()
                    .all(|segment| matches!(segment, StructuralPathSegment::Field(_)))
                    .then(|| {
                        terminal_semantics::runtime_structural_path_tip(
                            structural_types.values().copied(),
                            signature.structural_type,
                            &argument.path,
                        )
                    })
                    .flatten()
                    .ok_or_else(|| {
                        refusal("projected reference argument path is not a declared record edge")
                    })?;
                let paths = leaf_paths(structural_types, projected_type.id, live.len())
                    .ok_or_else(|| {
                        refusal("owned argument reference roster exceeds live custody")
                    })?;
                if paths.len()
                    != live
                        .iter()
                        .filter(|reference| {
                            reference.carrier == argument.place
                                && owned_resident(&reference.carrier_path, &argument.path)
                        })
                        .count()
                {
                    return Err(refusal(
                        "owned argument reference roster differs from its type",
                    ));
                }
                let storage = ReferenceOrigin::IngressLeaf(
                    ReferenceIdentity {
                        place: argument.place,
                        path: argument.path.clone(),
                    }
                    .into(),
                );
                if live.iter().any(|reference| {
                    !(reference.carrier == argument.place
                        && owned_resident(&reference.carrier_path, &argument.path))
                        && origins_overlap(&storage, &reference.root)
                }) {
                    return Err(refusal("carrier storage has an outstanding loan"));
                }
                normalized_arguments.push((
                    vec![storage],
                    argument.access,
                    CallRegion::CarrierStorage,
                    Vec::new(),
                    argument_position,
                ));
                if argument.access == StructuralAccess::WriteOnlyBorrow {
                    continue;
                }
                for path in paths {
                    let permission = leaf_type(structural_types, projected_type.id, &path)
                        .and_then(|leaf| reference_access(structural_types, leaf))
                        .ok_or_else(|| refusal("carrier leaf lacks declared permission"))?;
                    let mut carrier_path = argument.path.clone();
                    carrier_path.extend(path);
                    let reference = live
                        .iter()
                        .find(|reference| {
                            reference.carrier == argument.place
                                && reference.carrier_path == carrier_path
                        })
                        .ok_or_else(|| refusal("owned argument reference leaf is not live"))?;
                    if live.iter().any(|child| matches!(&child.parent, ReferenceParent::Reference(identity) if identity == &reference.identity))
                    || (argument.access == StructuralAccess::Owned && !moved.insert(reference.identity.clone()))
                {
                    return Err(refusal("owned reference argument duplicates or suspends a leaf"));
                }
                    normalized_arguments.push((
                        vec![reference.root.clone()],
                        if access_covers(argument.access, permission) {
                            permission
                        } else {
                            argument.access
                        },
                        CallRegion::Referent,
                        Vec::new(),
                        argument_position,
                    ));
                }
                continue;
            }
            let carrier_or_suspended = structural_source_contract(function, argument.place)
                .is_some_and(|source| contains_reference(structural_types, source.structural_type))
                || live
                    .iter()
                    .any(|reference| origin_place(&reference.root) == argument.place);
            if carrier_or_suspended || borrowed_sources.contains_key(&argument.place) {
                if carrier_or_suspended
                    && !is_reference_projection(function, structural_types, argument)
                {
                    return Err(refusal(
                        "call cannot access a suspended root or move its referent",
                    ));
                }
                let (root, _) = normalized_source(
                    function,
                    structural_types,
                    borrowed_sources,
                    block,
                    node,
                    live,
                    argument,
                )?;
                normalized_arguments.push((
                    vec![root],
                    argument.access,
                    CallRegion::Referent,
                    Vec::new(),
                    argument_position,
                ));
            } else {
                normalized_arguments.push((
                    vec![ReferenceOrigin::Primitive(argument.place)],
                    argument.access,
                    CallRegion::OrdinaryStorage,
                    Vec::new(),
                    argument_position,
                ));
            }
        }
        for (position, (roots, access, reference_access, dependencies, argument_position)) in
            normalized_arguments.iter().enumerate()
        {
            if normalized_arguments[position + 1..].iter().any(
                |(
                    others,
                    other_access,
                    other_reference_access,
                    other_dependencies,
                    other_position,
                )| {
                    ((*reference_access != CallRegion::OrdinaryStorage
                        || *other_reference_access != CallRegion::OrdinaryStorage)
                        && (argument_position != other_position
                            || (*reference_access == CallRegion::Referent
                                && *other_reference_access == CallRegion::Referent))
                        && (*access != StructuralAccess::SharedBorrow
                            || *other_access != StructuralAccess::SharedBorrow)
                        && roots
                            .iter()
                            .any(|root| others.iter().any(|other| origins_overlap(root, other))))
                        || (*other_access != StructuralAccess::SharedBorrow
                            && dependencies.iter().any(|root| {
                                others.iter().any(|other| origins_overlap(root, other))
                            }))
                        || (*access != StructuralAccess::SharedBorrow
                            && roots.iter().any(|root| {
                                other_dependencies
                                    .iter()
                                    .any(|other| origins_overlap(root, other))
                            }))
                },
            ) {
                return Err(refusal(
                    "call arguments overlap through reference origins or descriptor dependencies",
                ));
            }
        }
        let stored = call_effects.stage(
            function,
            functions,
            structural_types,
            borrowed_sources,
            block,
            node,
            operation,
            live,
            &moved,
        )?;
        let sources = match operation {
            O::EstablishReference { source, .. } | O::EstablishAccessReference { source, .. } => {
                let transferred =
                    transferred_ingress_leaf(function, structural_types, live, source);
                if let Some(identity) = &transferred {
                    moved.insert(identity.clone());
                }
                vec![ReferenceCallResult {
                    path: Vec::new(),
                    sources: vec![ReferenceCallSource {
                        origin: terminal_semantics::ReferenceCallOrigin::Parameter {
                            source: source.clone(),
                            transfers_existing: transferred.is_some(),
                        },
                        suffix: Vec::new(),
                        extent: semantic_vocabulary::ReferenceOriginExtent::Projected,
                        endpoint: reference_source_type(function, structural_types, source)
                            .ok_or_else(|| refusal("reference formation has no source type"))?,
                    }],
                }]
            }
            O::CallStructural {
                callee,
                structural_arguments,
                result: AbstractStructuralCallResult::Structural { result, .. },
                ..
            } if contains_reference(structural_types, result.structural_type) => {
                let contract = match callee {
                    abstract_operations::AbstractCallTarget::Machine(machine) => {
                        let callee = functions
                            .get(machine)
                            .ok_or_else(|| refusal("reference call target is absent"))?;
                        let declaration = callee.result.structural().ok_or_else(|| {
                            refusal("reference call target has no structural result")
                        })?;
                        terminal_semantics::ReferenceCallContract::Machine {
                            parameters: &callee.structural_parameters,
                            sources: &declaration.reference_sources,
                        }
                    }
                    abstract_operations::AbstractCallTarget::Dynamic {
                        interface, slot, ..
                    } => {
                        let requirement = dynamic_dispatch
                            .interface(*interface)
                            .and_then(|interface| interface.slots.get(*slot as usize))
                            .ok_or_else(|| refusal("reference call requirement is absent"))?;
                        terminal_semantics::ReferenceCallContract::Requirement(requirement)
                    }
                };
                terminal_semantics::bind_reference_call_results(
                    structural_types.values().copied(),
                    contract,
                    structural_arguments,
                )
                .map_err(&refusal)?
            }
            _ => Vec::new(),
        };
        if !sources.is_empty() {
            let result = match operation {
                O::EstablishReference { result, .. }
                | O::EstablishAccessReference { result, .. }
                | O::CallStructural {
                    result: AbstractStructuralCallResult::Structural { result, .. },
                    ..
                } => result,
                _ => {
                    return Err(refusal("reference establishment result is absent"));
                }
            };
            if live.iter().any(|reference| {
                reference.carrier == result.place || reference.identity.place == result.place
            }) {
                return Err(refusal("reference carrier is already live"));
            }
            let mut established = Vec::with_capacity(sources.len());
            let mut roots = Vec::new();
            let mut identities = BTreeSet::new();
            for ReferenceCallResult {
                path,
                sources: candidates,
            } in sources
            {
                let expected = leaf_referent(structural_types, result.structural_type, &path)
                    .ok_or_else(|| refusal("reference call result leaf is unsupported"))?;
                let access = leaf_type(structural_types, result.structural_type, &path)
                    .and_then(|leaf| reference_access(structural_types, leaf))
                    .ok_or_else(|| refusal("reference call result access is absent"))?;
                let mut alternatives = Vec::with_capacity(candidates.len());
                for ReferenceCallSource {
                    origin,
                    suffix,
                    extent,
                    endpoint,
                } in candidates
                {
                    let (source, transfers_existing) = match origin {
                        terminal_semantics::ReferenceCallOrigin::Parameter {
                            source,
                            transfers_existing,
                        } => (source, transfers_existing),
                        terminal_semantics::ReferenceCallOrigin::Invocation {
                            carrier_type,
                            path: origin_path,
                            access: supplied,
                        } => {
                            if endpoint != expected
                                || extent != semantic_vocabulary::ReferenceOriginExtent::Projected
                                || !leaf_access_within(
                                    structural_types,
                                    result.structural_type,
                                    &path,
                                    supplied,
                                )
                            {
                                return Err(refusal(
                                    "invocation result changes admitted reference geometry",
                                ));
                            }
                            let identity = ReferenceIdentity {
                                place: result.place,
                                path: path.clone(),
                            };
                            let root = ReferenceOrigin::Invocation {
                                result: identity.clone(),
                                carrier_type,
                                path: origin_path,
                                extent,
                            };
                            alternatives.push(LiveReference {
                                identity,
                                carrier: result.place,
                                carrier_path: path.clone(),
                                parent: ReferenceParent::Root(root.clone()),
                                root,
                            });
                            continue;
                        }
                    };
                    let compatible = reference_source_type(function, structural_types, &source)
                        .and_then(|root| {
                            if suffix.is_empty() {
                                Some(root)
                            } else {
                                reference_origin_type(
                                    structural_types.values().copied(),
                                    root,
                                    &suffix,
                                )
                            }
                        })
                        .is_some_and(|actual| match operation {
                            O::EstablishAccessReference { correspondence, .. } => {
                                access_reference_source_compatible(
                                    structural_types,
                                    actual,
                                    result.structural_type,
                                    correspondence,
                                )
                            }
                            O::EstablishReference {
                                storage: Some(storage),
                                ..
                            } => terminal_semantics::reference_storage_source_compatible(
                                structural_types.values().copied(),
                                actual,
                                result.structural_type,
                                storage.stored_scalar,
                            ),
                            O::EstablishReference { storage: None, .. } => {
                                terminal_semantics::reference_source_compatible(
                                    structural_types.values().copied(),
                                    actual,
                                    result.structural_type,
                                )
                            }
                            _ => {
                                extent == semantic_vocabulary::ReferenceOriginExtent::WithinRegion
                                    || actual == expected
                            }
                        });
                    if !compatible
                        || !leaf_access_within(
                            structural_types,
                            result.structural_type,
                            &path,
                            source.access,
                        )
                    {
                        return Err(refusal("reference result changes its mapped source type"));
                    }
                    let reference = if transfers_existing {
                        let Some((StructuralPathSegment::Referent, carrier_path)) =
                            source.path.split_last()
                        else {
                            return Err(refusal("owned result mapping has no incoming leaf"));
                        };
                        let mut reference = live
                            .iter()
                            .find(|reference| {
                                reference.carrier == source.place
                                    && reference.carrier_path == carrier_path
                            })
                            .filter(|reference| moved.contains(&reference.identity))
                            .cloned()
                            .ok_or_else(|| {
                                refusal("returned reference is not among the transferred leaves")
                            })?;
                        reference.carrier = result.place;
                        reference.carrier_path = path.clone();
                        reference
                    } else {
                        let (mut root, mut parent) = normalized_source(
                            function,
                            structural_types,
                            borrowed_sources,
                            block,
                            node,
                            live,
                            &source,
                        )?;
                        if !suffix.is_empty()
                            || extent == semantic_vocabulary::ReferenceOriginExtent::WithinRegion
                        {
                            let projected = projected_origin_family(&root, &suffix, extent);
                            if parent == ReferenceParent::Root(root.clone()) {
                                parent = ReferenceParent::Root(projected.clone());
                            }
                            root = projected;
                        }
                        if let O::EstablishReference {
                            storage: Some(storage),
                            ..
                        } = operation
                        {
                            let mut offsets = function.block_nodes().filter_map(|node| match node
                                .operation
                            {
                                O::IntegerConstant { result, value, .. }
                                    if result == storage.byte_offset =>
                                {
                                    Some(value)
                                }
                                _ => None,
                            });
                            if offsets.next()
                                != Some(semantic_vocabulary::IntegerValue::Unsigned(0))
                                || offsets.next().is_some()
                                || terminal_semantics::representation_view_declaration(
                                    structural_types.values().copied(),
                                    expected,
                                    storage.stored_scalar,
                                )
                                .is_none()
                            {
                                return Err(refusal(
                                    "returned interpretation differs from actual view formation",
                                ));
                            }
                            let interpreted = projected_origin(
                                &root,
                                &[StructuralPathSegment::RepresentationView {
                                    structural_type: expected,
                                    stored_scalar: storage.stored_scalar,
                                }],
                            );
                            if parent == ReferenceParent::Root(root.clone()) {
                                parent = ReferenceParent::Root(interpreted.clone());
                            }
                            root = interpreted;
                        }
                        LiveReference {
                            identity: ReferenceIdentity {
                                place: result.place,
                                path: path.clone(),
                            },
                            carrier: result.place,
                            carrier_path: path.clone(),
                            root,
                            parent,
                        }
                    };
                    alternatives.push(reference);
                }
                let reference =
                    combine_alternative_references(function, block, node, alternatives)?;
                if !identities.insert(reference.identity.clone())
                    || roots.iter().any(|(root, prior_access)| {
                        origins_overlap(root, &reference.root)
                            && (access != StructuralAccess::SharedBorrow
                                || *prior_access != StructuralAccess::SharedBorrow)
                    })
                {
                    return Err(refusal("reference result aliases its mapped sources"));
                }
                roots.push((reference.root.clone(), access));
                established.push(reference);
            }
            live.retain(|reference| !moved.contains(&reference.identity));
            live.extend(established);
            live.sort_by(|left, right| left.identity.cmp(&right.identity));
        } else {
            live.retain(|reference| !moved.contains(&reference.identity));
        }
        stored.commit(function, structural_types, block, node, live)?;
        Ok(())
    }

    pub(super) fn transferred_ingress_leaf(
        function: &PsiOptimizationFunction,
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        live: &[LiveReference],
        source: &StructuralArgument,
    ) -> Option<ReferenceIdentity> {
        let parameter = function
            .structural_parameter(source.place)
            .filter(|parameter| parameter.access == StructuralAccess::Owned)?;
        if !matches!(
            formal_origin(types, function, source),
            Some(ReferenceOrigin::IngressLeaf(_))
        ) || matches!(
            types.get(&parameter.structural_type)?.shape,
            terminal_psi::StructuralTypeShape::Reference { .. }
        ) {
            return None;
        }
        let (StructuralPathSegment::Referent, path) = source.path.split_last()? else {
            return None;
        };
        let paths = leaf_paths(types, parameter.structural_type, live.len())?;
        if paths.as_slice() != [path.to_vec()] {
            return None;
        }
        let mut leaves = live.iter().filter(|reference| {
            reference.carrier == source.place && owned_resident(&reference.carrier_path, &[])
        });
        let leaf = leaves.next()?;
        (leaves.next().is_none()
            && leaf.carrier_path == path
            && leaf.parent == ReferenceParent::Root(leaf.root.clone())
            && !live.iter().any(|child| {
                matches!(&child.parent,
            ReferenceParent::Reference(identity) if identity == &leaf.identity)
            }))
        .then(|| leaf.identity.clone())
    }

    fn combine_alternative_references(
        function: &PsiOptimizationFunction,
        block: BlockId,
        node: u32,
        alternatives: Vec<LiveReference>,
    ) -> Result<LiveReference, OptimizationUnitValidationError> {
        let invalid = || {
            invalid(
                function,
                block,
                Some(node),
                "reference result alternatives change carrier or ingress custody",
            )
        };
        let mut alternatives = alternatives.into_iter();
        let mut reference = alternatives.next().ok_or_else(invalid)?;
        let mut origins = vec![(reference.root.clone(), reference.parent.clone())];
        for alternative in alternatives {
            if reference.identity != alternative.identity
                || reference.carrier != alternative.carrier
                || reference.carrier_path != alternative.carrier_path
            {
                return Err(invalid());
            }
            origins.push((alternative.root, alternative.parent));
        }
        (reference.root, reference.parent) =
            combine_alternative_origins(function, block, node, origins)?;
        Ok(reference)
    }

    fn combine_alternative_origins(
        function: &PsiOptimizationFunction,
        block: BlockId,
        node: u32,
        alternatives: Vec<(ReferenceOrigin, ReferenceParent)>,
    ) -> Result<(ReferenceOrigin, ReferenceParent), OptimizationUnitValidationError> {
        let invalid = || {
            invalid(
                function,
                block,
                Some(node),
                "reference result alternatives change ingress custody",
            )
        };
        let mut alternatives = alternatives.into_iter();
        let (root, parent) = alternatives.next().ok_or_else(invalid)?;
        if alternatives.len() == 0 {
            return Ok((root, parent));
        }
        let place = origin_place(&root);
        let mut origins = Vec::new();
        for (origin, origin_parent) in std::iter::once((root, parent)).chain(alternatives) {
            if origin_parent != ReferenceParent::Root(origin.clone())
                || origin_place(&origin) != place
            {
                return Err(invalid());
            }
            match origin {
                ReferenceOrigin::IngressLeaf(identity) => origins.push(identity),
                ReferenceOrigin::Alternatives {
                    origins: alternatives,
                    ..
                } => origins.extend(alternatives),
                ReferenceOrigin::Invocation { .. } | ReferenceOrigin::Primitive(_) => {
                    return Err(invalid());
                }
            }
        }
        origins.sort();
        if origins.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(invalid());
        }
        let root = ReferenceOrigin::Alternatives { place, origins };
        Ok((root.clone(), ReferenceParent::Root(root)))
    }

    pub(super) fn transfer_return(
        function: &PsiOptimizationFunction,
        structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        block: BlockId,
        source: PlaceId,
        live: &mut Vec<LiveReference>,
        observed: &mut Vec<StructuralReferenceResultSource>,
        cleanup_actions: &[terminal_psi::TerminalAffineCleanupAction],
    ) -> Result<bool, OptimizationUnitValidationError> {
        let Some(signature) = structural_source_contract(function, source) else {
            return Ok(false);
        };
        if !contains_reference(structural_types, signature.structural_type) {
            return Ok(false);
        }
        let result = function
            .result
            .structural()
            .ok_or_else(|| invalid(function, block, None, "reference return has no declaration"))?;
        if signature.structural_type != result.structural_type
            || live
                .iter()
                .filter(|reference| {
                    reference.carrier == source && owned_resident(&reference.carrier_path, &[])
                })
                .count()
                != result.reference_sources.len()
        {
            return Err(invalid(
                function,
                block,
                None,
                "returned reference leaf roster differs from its declaration",
            ));
        }
        for mapping in &result.reference_sources {
            let reference = live
                .iter()
                .find(|reference| {
                    reference.carrier == source && reference.carrier_path == mapping.path
                })
                .ok_or_else(|| {
                    invalid(
                        function,
                        block,
                        None,
                        "returned reference carrier is not live",
                    )
                })?;
            let access = leaf_type(
                structural_types,
                signature.structural_type,
                &reference.carrier_path,
            )
            .and_then(|leaf| reference_access(structural_types, leaf))
            .ok_or_else(|| {
                invalid(
                    function,
                    block,
                    None,
                    "returned carrier has no reference access",
                )
            })?;
            let referent = leaf_referent(
                structural_types,
                signature.structural_type,
                &reference.carrier_path,
            )
            .ok_or_else(|| {
                invalid(
                    function,
                    block,
                    None,
                    "returned carrier has no referent type",
                )
            })?;
            if let ReferenceOrigin::Invocation {
                carrier_type,
                path: origin_path,
                extent,
                ..
            } = &reference.root
            {
                let source = terminal_psi::StructuralReferenceOrigin::Invocation {
                    carrier_type: *carrier_type,
                    path: origin_path.clone(),
                    extent: *extent,
                    access,
                };
                if !mapping.sources.contains(&source)
                || !matches!(&reference.parent, ReferenceParent::Root(root) if root == &reference.root)
                || live.iter().any(|child| matches!(&child.parent, ReferenceParent::Reference(identity) if identity == &reference.identity)) {
                return Err(invalid(function, block, None, "invocation reference return differs from completed call custody"));
            }
                observed.push(StructuralReferenceResultSource {
                    path: mapping.path.clone(),
                    sources: vec![source],
                });
                continue;
            }
            let sources = return_origin_family(
                function,
                structural_types,
                &reference.root,
                referent,
                access,
            )
            .ok_or_else(|| {
                invalid(
                    function,
                    block,
                    None,
                    "reference return origin family is unavailable",
                )
            })?;
            if sources.is_empty() || sources.iter().any(|source| {
            let Some(parameter) = function.structural_parameter(source.place) else {
                return true;
            };
            let supplying = mapping.sources.iter()
                .filter_map(terminal_psi::StructuralReferenceOrigin::parameter)
                .filter(|allowed| access_covers(
                    allowed.access, source.access,
                ))
                .map(|allowed| ReferenceOriginSource {
                    access: source.access,
                    ..allowed.clone()
                })
                .collect::<Vec<_>>();
            !terminal_semantics::reference_origin_is_covered(
                structural_types.values().copied(), parameter.structural_type, source, &supplying,
            )
        })
            || matches!(&reference.parent, ReferenceParent::Root(root) if *root != reference.root)
            || live.iter().any(|child| matches!(&child.parent, ReferenceParent::Reference(identity) if identity == &reference.identity))
        {
            return Err(invalid(
                function,
                block,
                None,
                "reference return escapes local custody or differs from its formal source",
            ));
        }
            observed.push(StructuralReferenceResultSource {
                path: mapping.path.clone(),
                sources: sources
                    .into_iter()
                    .map(terminal_psi::StructuralReferenceOrigin::Parameter)
                    .collect(),
            });
        }
        let occurrences = live
            .iter()
            .map(
                |reference| terminal_semantics::ReferenceCompletionOccurrence {
                    identity: &reference.identity,
                    parents: match &reference.parent {
                        ReferenceParent::Root(_) => Vec::new(),
                        ReferenceParent::Reference(parent) => vec![parent],
                    },
                    returned: reference.carrier == source
                        && owned_resident(&reference.carrier_path, &[]),
                    in_caller_storage: function
                        .structural_parameter(reference.carrier)
                        .and_then(|parameter| {
                            terminal_semantics::borrowed_entry_reference_referent(
                                structural_types.values().copied(),
                                parameter,
                                &reference.carrier_path,
                            )
                        })
                        .is_some(),
                    disposed: terminal_semantics::cleanup_disposes_reference(
                        cleanup_actions,
                        reference.carrier,
                        &reference.carrier_path,
                    ),
                },
            )
            .collect::<Vec<_>>();
        terminal_semantics::validate_reference_result_completion(&occurrences)
            .map_err(|reason| invalid(function, block, None, reason.description()))?;
        live.retain(|reference| {
            reference.carrier != source || !owned_resident(&reference.carrier_path, &[])
        });
        Ok(true)
    }

    fn return_origin_family(
        function: &PsiOptimizationFunction,
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        origin: &ReferenceOrigin,
        expected: StructuralTypeId,
        access: StructuralAccess,
    ) -> Option<Vec<ReferenceOriginSource>> {
        use terminal_psi::ReferenceOriginProjection::{Element, Exact};
        use terminal_psi::StructuralPathSegment::{Referent, RuntimeIndex};
        let primitive = ReferenceOriginIdentity {
            place: origin_place(origin),
            path: Vec::new(),
            extent: semantic_vocabulary::ReferenceOriginExtent::Projected,
        };
        let origins = match origin {
            ReferenceOrigin::Invocation { .. } => return None,
            ReferenceOrigin::IngressLeaf(identity) => std::slice::from_ref(identity),
            ReferenceOrigin::Alternatives { origins, .. } => origins.as_slice(),
            ReferenceOrigin::Primitive(_) => std::slice::from_ref(&primitive),
        };
        let mut sources = Vec::with_capacity(origins.len());
        for origin in origins {
            let parameter = function.structural_parameter(origin.place)?;
            let mut path = origin
                .path
                .iter()
                .map(|segment| match segment {
                    Exact(RuntimeIndex { .. }) => Element,
                    _ => segment.clone(),
                })
                .collect::<Vec<_>>();
            let endpoint =
                reference_origin_type(types.values().copied(), parameter.structural_type, &path)?;
            if origin.extent == semantic_vocabulary::ReferenceOriginExtent::Projected
                && endpoint != expected
            {
                if !matches!(types.get(&endpoint).map(|declaration| &declaration.shape),
                Some(terminal_psi::StructuralTypeShape::Reference { referent, access: supplied })
                    if *referent == expected && access_covers(*supplied, access))
                {
                    return None;
                }
                path.push(Exact(Referent));
            }
            sources.push(ReferenceOriginSource {
                place: origin.place,
                path,
                extent: origin.extent,
                access,
            });
        }
        sources.sort();
        sources.dedup();
        Some(sources)
    }

    pub(super) fn validate_return_coverage(
        function: &PsiOptimizationFunction,
        observed: &[StructuralReferenceResultSource],
    ) -> Result<(), OptimizationUnitValidationError> {
        let Some(result) = function.result.structural() else {
            return Ok(());
        };
        for mapping in &result.reference_sources {
            let sources = observed
                .iter()
                .filter(|returned| returned.path == mapping.path)
                .flat_map(|returned| returned.sources.iter().cloned())
                .collect::<BTreeSet<_>>();
            if mapping.sources.iter().any(|source| {
                source.extent() == semantic_vocabulary::ReferenceOriginExtent::Projected
                    && !sources.contains(source)
            }) {
                return Err(invalid(
                    function,
                    function.entry,
                    None,
                    "reference result alternatives differ from reachable return origins",
                ));
            }
        }
        Ok(())
    }

    pub(super) fn validate_normal_return(
        function: &PsiOptimizationFunction,
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        effects: &call_effects::ReferenceCallEffects,
        block: BlockId,
        live: &[LiveReference],
    ) -> Result<(), OptimizationUnitValidationError> {
        let mut expected = entry_references(function, types)?;
        expected.retain(|reference| {
            function.structural_parameters.iter().any(|parameter| {
                parameter.place == reference.carrier && parameter.access != StructuralAccess::Owned
            })
        });
        let mut ordered =
            effects.remaining_at_return(function, types, block, &mut expected, live)?;
        expected.sort_by(|left, right| left.identity.cmp(&right.identity));
        ordered.sort_by(|left, right| left.identity.cmp(&right.identity));
        if expected.len() == ordered.len()
            && expected
                .iter()
                .zip(ordered)
                .all(|(expected, actual)| expected == actual)
        {
            Ok(())
        } else {
            Err(invalid(
                function,
                block,
                None,
                "normal completion does not restore borrowed reference custody",
            ))
        }
    }
}
mod replay {
    use crate::current_ownership::cleanup::validate_return_cleanup_actions;
    use crate::current_ownership::mutations::{insert_owned_result, reject_live_linear_claim};
    use crate::current_ownership::structural::projected_root_is_fully_consumed;
    use crate::current_ownership::{
        CurrentOwnership, LiveClaim, consume_edge_sources, establish_edge_parameters,
    };
    use crate::unit_validation::function_structure::membership_on_every_path;
    use crate::unit_validation::operation_contracts::structural_source_contract;
    use crate::{OperationSite, OptimizationUnitValidationError};
    use abstract_operations::AbstractOperation as O;
    use abstract_operations::establish_record_or_case_pattern;
    use optimization_unit::{
        OptimizationBlock, OwnershipFrontierFact, OwnershipFrontierSite, OwnershipFrontierSnapshot,
        PsiOptimizationFunction,
    };
    use semantic_vocabulary::{BlockId, MachineId, StructuralTypeId};
    use std::collections::{BTreeMap, BTreeSet};
    use terminal_psi::StructuralArgument;
    use terminal_psi::{
        BoundaryMachineDeclaration, StructuralAccess, StructuralMultiplicity,
        StructuralPathSegment, StructuralTypeDeclaration,
    };
    use terminal_semantics::structural_paths_may_overlap;

    fn is_borrowed_member_move(
        function: &PsiOptimizationFunction,
        argument: &StructuralArgument,
    ) -> bool {
        !argument.path.is_empty()
            && structural_source_contract(function, argument.place)
                .is_some_and(|source| source.access == StructuralAccess::MutableBorrow)
    }

    fn validate_atomic_access(
        function: &PsiOptimizationFunction,
        structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        block: BlockId,
        node: u32,
        operation: &O,
        frontier: &CurrentOwnership,
    ) -> Result<(), OptimizationUnitValidationError> {
        let O::AtomicEvent { event, .. } = operation else {
            return Ok(());
        };
        let Some(location) = event.location() else {
            return Ok(());
        };
        let invalid = || OptimizationUnitValidationError::CurrentReferenceCustodyViolation {
            machine: function.machine,
            block,
            node: Some(node),
            reason: "atomic event has no compatible current storage authority",
        };
        let signature = function
            .arrival_parameter(location.root)
            .ok_or_else(invalid)?;
        terminal_semantics::atomic_cell_type(
            structural_types.values().copied(),
            signature.structural_type,
            &location.path,
        )
        .ok_or_else(invalid)?;
        if !signature.qualifications.is_empty()
            || !signature.projected_qualifications.is_empty()
            || (event.observes_resident() && signature.access == StructuralAccess::WriteOnlyBorrow)
        {
            return Err(invalid());
        }
        if signature.access == StructuralAccess::Owned
            && ((signature.multiplicity != StructuralMultiplicity::Unrestricted
                && frontier.owned_places.get(&location.root) != Some(&signature.multiplicity))
                || frontier.partial_custody_paths.contains_key(&location.root))
        {
            return Err(OptimizationUnitValidationError::CurrentOwnedPlaceNotLive(
                OperationSite::new(function.machine, block, node),
                location.root,
            ));
        }
        Ok(())
    }

    fn claims_sit_at_owned_arguments<'path>(
        frontier: &CurrentOwnership,
        arguments: &[StructuralArgument],
        pairs: impl IntoIterator<
            Item = (
                semantic_vocabulary::ClaimId,
                u32,
                &'path [StructuralPathSegment],
            ),
        >,
    ) -> bool {
        let mut seen = BTreeSet::new();
        pairs.into_iter().all(|(claim, index, path)| {
            let (Some(argument), Some(live)) =
                (arguments.get(index as usize), frontier.claims.get(&claim))
            else {
                return false;
            };
            seen.insert(claim)
                && argument.access == StructuralAccess::Owned
                && live.input == Some(argument.place)
                && live.path.starts_with(&argument.path)
                && path == &live.path[argument.path.len()..]
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn claim_path_is_inactive(
        function: &PsiOptimizationFunction,
        structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
        block: BlockId,
        node: usize,
        argument: &StructuralArgument,
        path: &[StructuralPathSegment],
    ) -> bool {
        let Some(block_position) = function
            .blocks
            .iter()
            .position(|candidate| candidate.id == block)
        else {
            return false;
        };
        let mut projection = argument.path.clone();
        for segment in path {
            if let StructuralPathSegment::Case(case) = segment
                && membership_on_every_path(
                    function,
                    structural_types,
                    predecessors,
                    (block_position, node),
                    argument.place,
                    &projection,
                    *case,
                    false,
                )
            {
                return true;
            }
            projection.push(segment.clone());
        }
        false
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn validate_current_ownership_cfg(
        function: &PsiOptimizationFunction,
        unit: &optimization_unit::PsiOptimizationUnit,
        blocks: &BTreeMap<BlockId, &OptimizationBlock>,
        predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
        functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
        boundary_machines: &BTreeMap<
            semantic_vocabulary::BoundaryMachineId,
            &BoundaryMachineDeclaration,
        >,
        structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        call_accesses: &mut super::CallAccesses,
        entry: CurrentOwnership,
        facts: &mut Vec<OwnershipFrontierFact>,
    ) -> Result<
        Vec<terminal_semantics::descriptor_results::DescriptorCompatibilityRequirement>,
        OptimizationUnitValidationError,
    > {
        let identities = &unit.identities;
        let dynamic_dispatch = &unit.dynamic_dispatch;
        let psi = unit.psi;
        let pins = super::address_joins::AddressJoinPins::new(
            function,
            functions,
            structural_types,
            call_accesses,
        );
        let unrestricted =
            super::cleanup::unrestricted_owned_cleanup_roots(function, structural_types);
        let mut ready = BTreeSet::from([function.entry]);
        let mut incoming = BTreeMap::<BlockId, CurrentOwnership>::new();
        incoming.insert(function.entry, entry);
        let mut returned_reference_sources = BTreeMap::new();
        let mut snapshots = BTreeMap::<OwnershipFrontierSite, OwnershipFrontierSnapshot>::new();

        while let Some(block_id) = ready.pop_first() {
            let mut returned_here = Vec::new();
            let mut frontier = incoming
                .get(&block_id)
                .expect("reachable block has a current ownership frontier")
                .clone();
            snapshots.insert(
                OwnershipFrontierSite::BlockEntry(block_id),
                frontier.snapshot(),
            );

            let block = blocks[&block_id];
            let block_position = function
                .blocks
                .iter()
                .position(|candidate| candidate.id == block_id)
                .expect("reachable block belongs to the function");
            let membership_at = |node, source, path: &[StructuralPathSegment], case, expected| {
                membership_on_every_path(
                    function,
                    structural_types,
                    predecessors,
                    (block_position, node),
                    source,
                    path,
                    case,
                    expected,
                )
            };
            for (node_index, node) in block.nodes.iter().enumerate() {
                let node_index =
                    u32::try_from(node_index).expect("optimization-unit node position fits u32");
                let psi_operation = node.operation.psi_operation();
                let admitted_case = |place, path: &[StructuralPathSegment], case| {
                    !membership_at(node_index as usize, place, path, case, false)
                };
                let value_use = super::value_custody::ValueUse {
                    function,
                    types: &unit.structural_types,
                    block: block_id,
                    node: node_index,
                    admitted_case: &admitted_case,
                };
                if let Some(operation) = psi_operation {
                    snapshots
                        .entry(OwnershipFrontierSite::OperationEntry(operation))
                        .or_insert_with(|| frontier.snapshot());
                }

                if let O::StoreStructuralField {
                    disposition,
                    destination,
                    path,
                    field,
                    ..
                } = &node.operation
                {
                    value_use.receiving_store(
                        &frontier,
                        destination,
                        path,
                        *field,
                        *disposition,
                    )?;
                }
                if let O::MoveStructuralField {
                    source: root,
                    path,
                    field,
                    ..
                } = &node.operation
                    && let Some(consumed) = frontier.partial_custody_paths.get(&root.place())
                {
                    let available = super::references::window_hole(
                        function,
                        structural_types,
                        root.place(),
                        root.structural_type(),
                        path,
                        *field,
                        terminal_psi::StructuralAccess::MutableBorrow,
                    )
                    .is_some_and(|selected| {
                        consumed
                            .iter()
                            .all(|absent| !structural_paths_may_overlap(absent, &selected))
                    });
                    if !available {
                        return Err(OptimizationUnitValidationError::CurrentOwnedPlaceNotLive(
                            OperationSite::new(function.machine, block_id, node_index),
                            root.place(),
                        ));
                    }
                }

                let moved_reference_owner = match &node.operation {
                    O::EstablishReference { source, .. }
                    | O::EstablishAccessReference { source, .. } => {
                        super::references::transferred_ingress_leaf(
                            function,
                            structural_types,
                            &frontier.live_references,
                            source,
                        )
                        .map(|_| source.place)
                    }
                    _ => None,
                };
                value_use.observations(&frontier, &node.operation)?;
                super::references::apply_operation(
                    function,
                    functions,
                    dynamic_dispatch,
                    structural_types,
                    pins.reference_sources(),
                    &call_accesses.references,
                    block_id,
                    node_index,
                    &node.operation,
                    &mut frontier.live_references,
                )?;
                validate_atomic_access(
                    function,
                    structural_types,
                    block_id,
                    node_index,
                    &node.operation,
                    &frontier,
                )?;

                if let O::StructuralScalarFieldStore {
                destination:
                    destination @ (abstract_operations::StructuralScalarStoreDestination::OwnedResult {
                        ..
                    }
                    | abstract_operations::StructuralScalarStoreDestination::OwnedBlockParameter {
                        ..
                    }),
                ..
            } = &node.operation
                && ((destination.multiplicity() != StructuralMultiplicity::Unrestricted
                    && frontier.owned_places.get(&destination.place())
                        != Some(&destination.multiplicity()))
                    || frontier
                        .partial_custody_paths
                        .contains_key(&destination.place()))
            {
                return Err(
                    OptimizationUnitValidationError::CurrentOwnedPlaceNotLive(
                        OperationSite::new(function.machine, block_id, node_index),
                        destination.place(),
                    ),
                );
            }

                if let O::StructuralCase { source, .. }
                | O::StructuralCaseMembership { source, .. }
                | O::StructuralScalarFieldRead { source, .. } = &node.operation
                {
                    let signature = structural_source_contract(function, *source).ok_or(
                        OptimizationUnitValidationError::InvalidStructuralCaseDispatch {
                            machine: function.machine,
                            source: *source,
                        },
                    )?;
                    let partially_unavailable = frontier
                        .partial_custody_paths
                        .get(source)
                        .is_some_and(|consumed| match &node.operation {
                            O::StructuralScalarFieldRead { path, field, .. } => {
                                let observed = terminal_semantics::canonical_field_storage_region(
                                    structural_types.values().copied(),
                                    signature.structural_type,
                                    path,
                                    *field,
                                );
                                consumed
                                    .iter()
                                    .any(|path| structural_paths_may_overlap(path, &observed))
                            }
                            O::StructuralCase { path: observed, .. }
                            | O::StructuralCaseMembership { path: observed, .. } => consumed
                                .iter()
                                .any(|path| structural_paths_may_overlap(path, observed)),
                            _ => true,
                        });
                    if signature.access == StructuralAccess::Owned
                        && signature.multiplicity != StructuralMultiplicity::Unrestricted
                        && ((!frontier.owned_places.contains_key(source)
                            && !projected_root_is_fully_consumed(
                                function,
                                structural_types,
                                &frontier,
                                *source,
                            ))
                            || partially_unavailable)
                    {
                        return Err(OptimizationUnitValidationError::CurrentOwnedPlaceNotLive(
                            OperationSite::new(function.machine, block_id, node_index),
                            *source,
                        ));
                    }
                }

                if let O::EstablishTrivialAffineLocal { place, .. } = &node.operation {
                    insert_owned_result(
                        function,
                        block_id,
                        node_index,
                        &mut frontier,
                        place.id,
                        StructuralMultiplicity::Affine,
                    )?;
                }
                if let O::ReturnStructural {
                    trivial_affine_locals,
                    ..
                } = &node.operation
                {
                    for (_, place, _) in trivial_affine_locals {
                        insert_owned_result(
                            function,
                            block_id,
                            node_index,
                            &mut frontier,
                            place.id,
                            StructuralMultiplicity::Affine,
                        )?;
                    }
                }

                let mut record_arguments =
                    super::structural::constructor_arguments(&node.operation)
                        .cloned()
                        .collect::<Vec<_>>();
                if let O::TransformContent {
                    source, parents, ..
                } = &node.operation
                {
                    record_arguments.extend(std::iter::once(source).chain(parents).cloned());
                }
                let structural_arguments = match &node.operation {
                    establish_record_or_case_pattern!(..)
                    | O::TransformContent { .. }
                    | O::EstablishStructuralArray { .. } => record_arguments.as_slice(),
                    abstract_operations::call_structural_or_boundary_call_pattern!(
                        structural_arguments
                    ) => structural_arguments.as_slice(),
                    O::StoreStructuralField { value, .. }
                    | O::ReleaseActivationClaim { source: value, .. }
                    | O::MoveStructuralValue { source: value, .. } => std::slice::from_ref(value),
                    _ => &[],
                };
                let parameter_multiplicities = match &node.operation {
                    establish_record_or_case_pattern!(..)
                    | O::TransformContent { .. }
                    | O::EstablishStructuralArray { .. } => record_arguments
                        .iter()
                        .map(|argument| {
                            structural_source_contract(function, argument.place)
                                .map(|source| source.multiplicity)
                        })
                        .collect::<Option<Vec<_>>>()
                        .ok_or(
                            OptimizationUnitValidationError::StructuralCallContractMismatch(
                                OperationSite::new(function.machine, block_id, node_index),
                            ),
                        )?,
                    O::StoreStructuralField { value, .. }
                    | O::ReleaseActivationClaim { source: value, .. }
                    | O::MoveStructuralValue { source: value, .. } => {
                        vec![
                            structural_source_contract(function, value.place)
                                .map(|source| source.multiplicity)
                                .ok_or(
                                    OptimizationUnitValidationError::StructuralCallContractMismatch(
                                        OperationSite::new(function.machine, block_id, node_index),
                                    ),
                                )?,
                        ]
                    }
                    O::CallStructural {
                        callee: abstract_operations::AbstractCallTarget::Machine(callee),
                        ..
                    }
                    | O::InvokeNoNormal { callee, .. } => functions[callee]
                        .structural_parameters
                        .iter()
                        .map(|parameter| parameter.multiplicity)
                        .collect::<Vec<_>>(),
                    O::CallStructural {
                        callee:
                            abstract_operations::AbstractCallTarget::Dynamic {
                                interface, slot, ..
                            },
                        ..
                    } => dynamic_dispatch
                        .interface(*interface)
                        .and_then(|interface| {
                            usize::try_from(*slot)
                                .ok()
                                .and_then(|slot| interface.slots.get(slot))
                        })
                        .map(|slot| {
                            slot.structural_parameters
                                .iter()
                                .map(|parameter| parameter.multiplicity)
                                .collect::<Vec<_>>()
                        })
                        .ok_or(
                            OptimizationUnitValidationError::StructuralCallContractMismatch(
                                OperationSite::new(function.machine, block_id, node_index),
                            ),
                        )?,
                    O::BoundaryCall { boundary, .. } => boundary_machines[boundary]
                        .structural_parameters
                        .iter()
                        .map(|parameter| parameter.multiplicity)
                        .collect(),
                    _ => Vec::new(),
                };
                for argument in structural_arguments {
                    if argument.access == StructuralAccess::Owned {
                        value_use.moving(&frontier, argument)?;
                    } else {
                        value_use.observing(&frontier, argument.place, &argument.path)?;
                    }
                }
                let mut consumed_places = structural_arguments
                    .iter()
                    .zip(&parameter_multiplicities)
                    .filter_map(|(argument, multiplicity)| {
                        (argument.path.is_empty()
                            && argument.access == StructuralAccess::Owned
                            && *multiplicity != StructuralMultiplicity::Unrestricted)
                            .then_some(argument.place)
                    })
                    .collect::<Vec<_>>();
                consumed_places.extend(moved_reference_owner);
                if let O::ReleaseReference { source, path, .. } = &node.operation
                    && path.is_empty()
                    && structural_source_contract(function, *source).is_some_and(|signature| {
                        signature.multiplicity != StructuralMultiplicity::Unrestricted
                    })
                {
                    consumed_places.push(*source);
                }
                pins.check_operation(
                    function,
                    block_id,
                    node_index,
                    &node.operation,
                    consumed_places.iter().copied().chain(
                        structural_arguments
                            .iter()
                            .filter(|argument| {
                                !argument.path.is_empty()
                                    && argument.access == StructuralAccess::Owned
                            })
                            .map(|argument| argument.place),
                    ),
                )?;
                for place in &consumed_places {
                    if frontier.partial_custody_paths.contains_key(place) {
                        return Err(
                            OptimizationUnitValidationError::CurrentWholePlacePartiallyMoved(
                                OperationSite::new(function.machine, block_id, node_index),
                                *place,
                            ),
                        );
                    }
                }

                let no_normal_target;
                let invocation = match &node.operation {
                    O::CallStructural {
                        callee,
                        structural_arguments,
                        claim_transfers,
                        ..
                    } => Some((callee, structural_arguments, claim_transfers)),
                    O::InvokeNoNormal {
                        callee,
                        structural_arguments,
                        claim_transfers,
                        ..
                    } => {
                        no_normal_target =
                            abstract_operations::AbstractCallTarget::Machine(*callee);
                        Some((&no_normal_target, structural_arguments, claim_transfers))
                    }
                    _ => None,
                };
                if let Some((callee, structural_arguments, claim_transfers)) = invocation {
                    let invalid = || OptimizationUnitValidationError::CurrentCleanupMismatch {
                        machine: function.machine,
                        block: block_id,
                    };
                    let expected_by_argument = match callee {
                        abstract_operations::AbstractCallTarget::Machine(callee) => {
                            let target = functions.get(callee).ok_or_else(invalid)?;
                            target
                                .structural_parameters
                                .iter()
                                .map(|parameter| {
                                    let mut expected = target
                                        .entry_claim_declarations
                                        .iter()
                                        .filter(|claim| claim.input == parameter.place)
                                        .map(|claim| (claim.claim, claim.path.as_slice()))
                                        .collect::<BTreeMap<_, _>>();
                                    for claim in &target.content_entry_claims {
                                        if claim.input.root == parameter.place {
                                            expected.entry(claim.claim).or_insert(&[]);
                                        }
                                    }
                                    expected.into_values().collect::<Vec<_>>()
                                })
                                .collect::<Vec<_>>()
                        }
                        abstract_operations::AbstractCallTarget::Dynamic {
                            interface,
                            slot,
                            ..
                        } => {
                            let signature = dynamic_dispatch
                                .interface(*interface)
                                .and_then(|interface| interface.slots.get(*slot as usize))
                                .ok_or_else(invalid)?;
                            signature
                                .structural_parameters
                                .iter()
                                .enumerate()
                                .map(|(argument_index, _)| {
                                    signature
                                        .entry_claim_paths
                                        .iter()
                                        .filter(|claim| {
                                            claim.argument_index as usize == argument_index
                                        })
                                        .map(|claim| claim.path.as_slice())
                                        .collect::<Vec<_>>()
                                })
                                .collect::<Vec<_>>()
                        }
                    };
                    if expected_by_argument.len() != structural_arguments.len() {
                        return Err(invalid());
                    }
                    for (argument_index, (argument, expected_paths)) in structural_arguments
                        .iter()
                        .zip(expected_by_argument)
                        .enumerate()
                    {
                        if argument.access != StructuralAccess::Owned {
                            continue;
                        }
                        let mut expected_paths = expected_paths
                            .into_iter()
                            .filter(|expected| {
                                !claim_path_is_inactive(
                                    function,
                                    structural_types,
                                    predecessors,
                                    block_id,
                                    node_index as usize,
                                    argument,
                                    expected,
                                )
                            })
                            .collect::<Vec<_>>();
                        let mut actual_paths = Vec::new();
                        for (identity, claim) in &frontier.claims {
                            if claim.input == Some(argument.place)
                                && claim.path.starts_with(&argument.path)
                            {
                                if claim_transfers
                                    .iter()
                                    .filter(|transfer| {
                                        transfer.claim == *identity
                                            && transfer.argument_index as usize == argument_index
                                            && transfer.path == claim.path[argument.path.len()..]
                                    })
                                    .count()
                                    != 1
                                {
                                    return Err(invalid());
                                }
                                actual_paths.push(&claim.path[argument.path.len()..]);
                            }
                        }
                        expected_paths.sort();
                        actual_paths.sort();
                        if expected_paths != actual_paths {
                            return Err(invalid());
                        }
                    }
                    if !claims_sit_at_owned_arguments(
                        &frontier,
                        structural_arguments,
                        claim_transfers.iter().map(|transfer| {
                            (
                                transfer.claim,
                                transfer.argument_index,
                                transfer.path.as_slice(),
                            )
                        }),
                    ) {
                        return Err(invalid());
                    }
                }
                if let O::BoundaryCall {
                    structural_arguments,
                    completion_receipts,
                    ..
                } = &node.operation
                {
                    let receipts_match = claims_sit_at_owned_arguments(
                        &frontier,
                        structural_arguments,
                        completion_receipts.iter().map(|receipt| {
                            (
                                receipt.claim,
                                receipt.argument_index,
                                receipt.path.as_slice(),
                            )
                        }),
                    );
                    let complete = structural_arguments.iter().enumerate().all(
                        |(argument_index, argument)| {
                            argument.access != StructuralAccess::Owned
                                || frontier.claims.iter().all(|(identity, claim)| {
                                    claim.input != Some(argument.place)
                                        || !claim.path.starts_with(&argument.path)
                                        || completion_receipts
                                            .iter()
                                            .filter(|receipt| {
                                                receipt.claim == *identity
                                                    && receipt.argument_index as usize
                                                        == argument_index
                                                    && receipt.path
                                                        == claim.path[argument.path.len()..]
                                            })
                                            .count()
                                            == 1
                                })
                        },
                    );
                    if !receipts_match || !complete {
                        return Err(OptimizationUnitValidationError::CurrentCleanupMismatch {
                            machine: function.machine,
                            block: block_id,
                        });
                    }
                }
                let transformed_parents = if let O::TransformContent {
                    psi_operation,
                    parents,
                    derivations,
                    ..
                } = &node.operation
                {
                    let invalid = || OptimizationUnitValidationError::CurrentCleanupMismatch {
                        machine: function.machine,
                        block: block_id,
                    };
                    let mut expected = BTreeSet::new();
                    for input in derivations.iter().flat_map(|derived| &derived.inputs) {
                        let root = structural_source_contract(function, input.subject.root)
                            .ok_or_else(invalid)?;
                        let path = terminal_semantics::content_transformations::structural_path(
                            structural_types.values().copied(),
                            identities,
                            root.structural_type,
                            &input.subject.segments,
                        )
                        .ok_or_else(invalid)?;
                        let claim = frontier.claims.get(&input.claim).ok_or_else(invalid)?;
                        if claim.input != Some(input.subject.root)
                            || claim.path != path
                            || !parents.iter().any(|parent| {
                                parent.place == input.subject.root && path.starts_with(&parent.path)
                            })
                        {
                            return Err(invalid());
                        }
                        expected.insert(input.claim);
                    }
                    let mut actual = BTreeSet::new();
                    let boundaries = structural_types
                        .values()
                        .filter_map(|declaration| {
                            declaration.owns_linear_claim.then_some(declaration.id)
                        })
                        .chain(
                            functions
                                .values()
                                .filter_map(|function| function.nominal_cleanup_subject),
                        )
                        .collect::<BTreeSet<_>>();
                    for parent in parents {
                        let root = structural_source_contract(function, parent.place)
                            .ok_or_else(invalid)?;
                        let selected = terminal_semantics::runtime_structural_path_tip(
                            structural_types.values().copied(),
                            root.structural_type,
                            &parent.path,
                        )
                        .ok_or_else(invalid)?;
                        let accounted = expected
                            .iter()
                            .filter_map(|identity| frontier.claims.get(identity))
                            .filter(|claim| claim.input == Some(parent.place))
                            .map(|claim| claim.path.clone())
                            .collect::<Vec<_>>();
                        terminal_semantics::owned_structure::validate_accounted_descendants(
                            structural_types,
                            &boundaries,
                            selected.id,
                            &parent.path,
                            &accounted,
                            &|path, case| {
                                membership_at(node_index as usize, parent.place, path, case, true)
                            },
                        )
                        .map_err(|reason| {
                            OptimizationUnitValidationError::InvalidOwnedValueTransfer {
                                machine: function.machine,
                                operation: *psi_operation,
                                reason,
                            }
                        })?;
                        for (identity, claim) in &frontier.claims {
                            if claim.input != Some(parent.place) {
                                continue;
                            }
                            if claim.path.starts_with(&parent.path) {
                                if !actual.insert(*identity) {
                                    return Err(invalid());
                                }
                            } else if parent.path.starts_with(&claim.path) {
                                return Err(invalid());
                            }
                        }
                    }
                    if expected.is_empty() || expected != actual {
                        return Err(invalid());
                    }
                    expected.into_iter().collect::<Vec<_>>()
                } else {
                    Vec::new()
                };
                if let O::ReleaseActivationClaim { source, claim, .. } = &node.operation {
                    let exact = frontier.claims.get(claim).is_some_and(|live| {
                        live.input == Some(source.place) && live.path == source.path
                    }) && frontier
                        .claims
                        .iter()
                        .filter(|(_, live)| {
                            live.input == Some(source.place) && live.path.starts_with(&source.path)
                        })
                        .count()
                        == 1;
                    if !exact || source.access != StructuralAccess::Owned {
                        return Err(OptimizationUnitValidationError::CurrentCleanupMismatch {
                            machine: function.machine,
                            block: block_id,
                        });
                    }
                }
                let transferred = match &node.operation {
                    O::ReleaseActivationClaim { claim, .. } => vec![*claim],
                    O::TransformContent { .. } => transformed_parents,
                    O::CallStructural {
                        claim_transfers, ..
                    }
                    | O::InvokeNoNormal {
                        claim_transfers, ..
                    } => claim_transfers
                        .iter()
                        .map(|transfer| transfer.claim)
                        .collect::<Vec<_>>(),
                    O::BoundaryCall {
                        completion_receipts,
                        ..
                    } => completion_receipts
                        .iter()
                        .map(|receipt| receipt.claim)
                        .collect(),
                    _ => Vec::new(),
                };
                for claim in transferred {
                    if frontier.claims.remove(&claim).is_none() {
                        return Err(OptimizationUnitValidationError::CurrentClaimNotLive(
                            OperationSite::new(function.machine, block_id, node_index),
                            claim,
                        ));
                    }
                }
                for (argument, _) in structural_arguments
                    .iter()
                    .zip(&parameter_multiplicities)
                    .filter(|(argument, multiplicity)| {
                        (argument.access == StructuralAccess::Owned
                            && **multiplicity != StructuralMultiplicity::Unrestricted)
                            || (argument.access != StructuralAccess::Owned
                                && structural_source_contract(function, argument.place)
                                    .is_some_and(|source| {
                                        source.access == StructuralAccess::Owned
                                            && source.multiplicity == StructuralMultiplicity::Affine
                                    }))
                    })
                {
                    if !frontier.owned_places.contains_key(&argument.place)
                        && !is_borrowed_member_move(function, argument)
                    {
                        return Err(OptimizationUnitValidationError::CurrentOwnedPlaceNotLive(
                            OperationSite::new(function.machine, block_id, node_index),
                            argument.place,
                        ));
                    }
                    if argument.path.is_empty()
                        && frontier.partial_custody_paths.contains_key(&argument.place)
                    {
                        return Err(
                            OptimizationUnitValidationError::CurrentWholePlacePartiallyMoved(
                                OperationSite::new(function.machine, block_id, node_index),
                                argument.place,
                            ),
                        );
                    }
                    if !argument.path.is_empty()
                        && frontier
                            .partial_custody_paths
                            .get(&argument.place)
                            .is_some_and(|moved| {
                                moved.iter().any(|existing| {
                                    existing.starts_with(&argument.path)
                                        || argument.path.starts_with(existing)
                                })
                            })
                    {
                        return Err(
                            OptimizationUnitValidationError::CurrentProjectedMoveOverlap(
                                OperationSite::new(function.machine, block_id, node_index),
                                argument.place,
                            ),
                        );
                    }
                }
                for place in consumed_places {
                    if frontier.owned_places.remove(&place).is_none() {
                        return Err(OptimizationUnitValidationError::CurrentOwnedPlaceNotLive(
                            OperationSite::new(function.machine, block_id, node_index),
                            place,
                        ));
                    }
                }

                for argument in structural_arguments.iter().filter(|argument| {
                    !argument.path.is_empty() && argument.access == StructuralAccess::Owned
                }) {
                    if !frontier.owned_places.contains_key(&argument.place) {
                        if is_borrowed_member_move(function, argument) {
                            continue;
                        }
                        return Err(OptimizationUnitValidationError::CurrentOwnedPlaceNotLive(
                            OperationSite::new(function.machine, block_id, node_index),
                            argument.place,
                        ));
                    }
                    let moved = frontier
                        .partial_custody_paths
                        .entry(argument.place)
                        .or_default();
                    if moved.iter().any(|existing| {
                        existing.starts_with(&argument.path) || argument.path.starts_with(existing)
                    }) || !moved.insert(argument.path.clone())
                    {
                        return Err(
                            OptimizationUnitValidationError::CurrentProjectedMoveOverlap(
                                OperationSite::new(function.machine, block_id, node_index),
                                argument.place,
                            ),
                        );
                    }
                    if projected_root_is_fully_consumed(
                        function,
                        structural_types,
                        &frontier,
                        argument.place,
                    ) {
                        frontier.owned_places.remove(&argument.place);
                    }
                }

                if let O::ReleaseReference { source, path, .. } = &node.operation
                    && !path.is_empty()
                {
                    if frontier.owned_places.get(source) != Some(&StructuralMultiplicity::Affine) {
                        return Err(OptimizationUnitValidationError::CurrentOwnedPlaceNotLive(
                            OperationSite::new(function.machine, block_id, node_index),
                            *source,
                        ));
                    }
                    let consumed = frontier.partial_custody_paths.entry(*source).or_default();
                    if consumed
                        .iter()
                        .any(|existing| existing.starts_with(path) || path.starts_with(existing))
                        || !consumed.insert(path.clone())
                    {
                        return Err(
                            OptimizationUnitValidationError::CurrentProjectedMoveOverlap(
                                OperationSite::new(function.machine, block_id, node_index),
                                *source,
                            ),
                        );
                    }
                    if projected_root_is_fully_consumed(
                        function,
                        structural_types,
                        &frontier,
                        *source,
                    ) {
                        frontier.owned_places.remove(source);
                    }
                }

                if let O::StoreStructuralField {
                    disposition,
                    destination,
                    path,
                    field,
                    value,
                    ..
                } = &node.operation
                {
                    value_use.install_store(
                        &mut frontier,
                        destination,
                        path,
                        *field,
                        value,
                        disposition.required_access(),
                    )?;
                }

                let result_contract =
                crate::unit_validation::operation_contracts::structural_operation_result_contract(
                    function,
                    &node.operation,
                );
                let structural_result = result_contract
                    .as_ref()
                    .and_then(|contract| contract.operation_result());
                if let Some(result) = structural_result {
                    let transferred_claims = if let O::MoveStructuralValue { source, .. }
                    | O::TransformContent { source, .. } =
                        &node.operation
                    {
                        let invalid = || OptimizationUnitValidationError::CurrentCleanupMismatch {
                            machine: function.machine,
                            block: block_id,
                        };
                        let mut selected = BTreeMap::new();
                        for (identity, claim) in &frontier.claims {
                            if claim.input != Some(source.place) {
                                continue;
                            }
                            if claim.path.starts_with(&source.path) {
                                selected.insert(*identity, claim.clone());
                            } else if source.path.starts_with(&claim.path) {
                                return Err(invalid());
                            }
                        }
                        let mut transferred = BTreeMap::new();
                        for binding in &result.claims {
                            if claim_path_is_inactive(
                                function,
                                structural_types,
                                predecessors,
                                block_id,
                                node_index as usize,
                                source,
                                &binding.path,
                            ) {
                                if selected.contains_key(&binding.claim) {
                                    return Err(invalid());
                                }
                                continue;
                            }
                            let mut claim = selected.remove(&binding.claim).ok_or_else(invalid)?;
                            let expected_multiplicity = if binding.path.is_empty() {
                                result.multiplicity
                            } else {
                                StructuralMultiplicity::Linear
                            };
                            if claim.path[source.path.len()..] != binding.path
                                || claim.multiplicity != Some(expected_multiplicity)
                                || result.multiplicity == StructuralMultiplicity::Unrestricted
                            {
                                return Err(invalid());
                            }
                            claim.input = Some(result.place);
                            claim.path = binding.path.clone();
                            if transferred.insert(binding.claim, claim).is_some() {
                                return Err(invalid());
                            }
                        }
                        if !selected.is_empty() {
                            return Err(invalid());
                        }
                        for identity in transferred.keys() {
                            frontier.claims.remove(identity);
                        }
                        Some(transferred)
                    } else {
                        None
                    };
                    insert_owned_result(
                        function,
                        block_id,
                        node_index,
                        &mut frontier,
                        result.place,
                        result.multiplicity,
                    )?;
                    if matches!(&node.operation, O::EstablishStructuralLocal { .. })
                        && result.multiplicity != StructuralMultiplicity::Unrestricted
                    {
                        frontier
                            .partial_custody_paths
                            .entry(result.place)
                            .or_default()
                            .insert(Vec::new());
                    }
                    for binding in &result.claims {
                        let claim = if let Some(transferred) = &transferred_claims {
                            let Some(claim) = transferred.get(&binding.claim) else {
                                continue;
                            };
                            claim.clone()
                        } else {
                            LiveClaim {
                                input: Some(result.place),
                                path: binding.path.clone(),
                                multiplicity: Some(if binding.path.is_empty() {
                                    result.multiplicity
                                } else {
                                    StructuralMultiplicity::Linear
                                }),
                            }
                        };
                        if frontier.claims.insert(binding.claim, claim).is_some() {
                            return Err(OptimizationUnitValidationError::CurrentClaimAlreadyLive(
                                OperationSite::new(function.machine, block_id, node_index),
                                binding.claim,
                            ));
                        }
                    }
                }

                let membership = |source, path: &[StructuralPathSegment], case, expected| {
                    membership_at(node_index as usize, source, path, case, expected)
                };
                let cleanup_frontier = |actions, frontier: &CurrentOwnership| {
                    validate_return_cleanup_actions(
                        function,
                        functions,
                        structural_types,
                        &call_accesses.references,
                        block_id,
                        &unrestricted,
                        frontier,
                        actions,
                        &membership,
                    )
                };
                match &node.operation {
                    O::Return {
                        cleanup_actions, ..
                    } => {
                        frontier = cleanup_frontier(cleanup_actions, &frontier)?;
                        reject_live_linear_claim(function, block_id, &frontier)?;
                    }
                    O::ReturnUnit {
                        cleanup_actions, ..
                    } => {
                        cleanup_frontier(cleanup_actions, &frontier)?;
                    }
                    O::ReturnStructural {
                        source,
                        returned_claims,
                        cleanup_actions,
                        ..
                    } => {
                        value_use.moving(
                            &frontier,
                            &StructuralArgument {
                                place: *source,
                                path: Vec::new(),
                                access: StructuralAccess::Owned,
                            },
                        )?;
                        super::references::transfer_return(
                            function,
                            structural_types,
                            block_id,
                            *source,
                            &mut frontier.live_references,
                            &mut returned_here,
                            cleanup_actions,
                        )?;
                        if frontier.partial_custody_paths.contains_key(source) {
                            return Err(
                            OptimizationUnitValidationError::CurrentStructuralReturnSourcePartiallyMoved {
                                machine: function.machine,
                                block: block_id,
                                place: *source,
                            },
                        );
                        }
                        let unrestricted_result =
                            function.result.structural().is_some_and(|result| {
                                result.multiplicity == StructuralMultiplicity::Unrestricted
                            });
                        if !unrestricted_result && frontier.owned_places.remove(source).is_none() {
                            return Err(OptimizationUnitValidationError::CurrentOwnedPlaceNotLive(
                                OperationSite::new(function.machine, block_id, node_index),
                                *source,
                            ));
                        }
                        let root = structural_source_contract(function, *source).ok_or(
                            OptimizationUnitValidationError::CurrentOwnedPlaceNotLive(
                                OperationSite::new(function.machine, block_id, node_index),
                                *source,
                            ),
                        )?;
                        terminal_semantics::owned_structure::validate_complete_claim_frontier(
                            |identity| structural_types.get(&identity).copied(),
                            root.structural_type,
                            &[],
                            frontier.claims.values().filter_map(|claim| {
                                (claim.input == Some(*source)
                                    && claim.multiplicity == Some(StructuralMultiplicity::Linear))
                                .then_some(claim.path.as_slice())
                            }),
                            &|path, case| {
                                membership_at(node_index as usize, *source, path, case, true)
                            },
                        )
                        .map_err(|reason| {
                            OptimizationUnitValidationError::IncompleteValueClaimFrontier {
                                site: OperationSite::new(function.machine, block_id, node_index),
                                place: *source,
                                reason,
                            }
                        })?;
                        let expected = frontier
                            .claims
                            .iter()
                            .filter(|(_, live)| live.input == Some(*source))
                            .map(|(claim, live)| terminal_psi::StructuralResultClaimBinding {
                                claim: *claim,
                                path: live.path.clone(),
                            })
                            .collect::<Vec<_>>();
                        if returned_claims != &expected {
                            return Err(
                            OptimizationUnitValidationError::CurrentStructuralReturnClaimSetMismatch {
                                machine: function.machine,
                                block: block_id,
                            },
                        );
                        }
                        for binding in returned_claims {
                            frontier.claims.remove(&binding.claim);
                        }
                        frontier = cleanup_frontier(cleanup_actions, &frontier)?;
                        if let Some(claim) = frontier.claims.keys().next().copied() {
                            return Err(
                            OptimizationUnitValidationError::CurrentClaimLiveAfterStructuralReturn {
                                machine: function.machine,
                                block: block_id,
                                claim,
                            },
                        );
                        }
                    }
                    O::Crash {
                        frontier_lower_bound,
                        ..
                    } if frontier_lower_bound
                        != &frontier.claims.keys().copied().collect::<Vec<_>>() =>
                    {
                        return Err(
                            OptimizationUnitValidationError::CurrentCrashClaimFrontierMismatch {
                                machine: function.machine,
                                block: block_id,
                            },
                        );
                    }
                    _ => {}
                }
                if let Some(operation) = psi_operation {
                    snapshots.insert(
                        OwnershipFrontierSite::OperationExit(operation),
                        frontier.snapshot(),
                    );
                }
            }

            // A tightened join may replay a block. Replace that block's
            // observations rather than retaining evidence from an earlier pass.
            returned_reference_sources.insert(block_id, returned_here);
            for edge in &block
                .nodes
                .last()
                .expect("validated block is nonempty")
                .successors
            {
                snapshots.insert(
                    OwnershipFrontierSite::EdgeEntry(edge.psi_edge),
                    frontier.snapshot(),
                );
                pins.check_edge(
                    function,
                    block_id,
                    u32::try_from(block.nodes.len() - 1)
                        .expect("optimization-unit node position fits u32"),
                    edge,
                    &blocks[&edge.target].structural_parameters,
                    &frontier,
                )?;
                let mut outgoing = frontier.clone();
                if let O::StructuralCase {
                    source,
                    path,
                    cases,
                } = &block
                    .nodes
                    .last()
                    .ok_or(OptimizationUnitValidationError::CurrentCleanupMismatch {
                        machine: function.machine,
                        block: block_id,
                    })?
                    .operation
                {
                    let selected = cases
                        .iter()
                        .find(|case| case.psi_edge == edge.psi_edge)
                        .ok_or(OptimizationUnitValidationError::CurrentCleanupMismatch {
                            machine: function.machine,
                            block: block_id,
                        })?;
                    outgoing.claims.retain(|_, claim| {
                        if claim.input != Some(*source) {
                            return true;
                        }
                        match claim
                            .path
                            .strip_prefix(path.as_slice())
                            .and_then(|suffix| suffix.first())
                        {
                            Some(StructuralPathSegment::Case(case)) => *case == selected.case,
                            _ => true,
                        }
                    });
                }
                let membership = |source, path: &[StructuralPathSegment], case, expected| {
                    let terminator = &block.nodes[block.nodes.len() - 1].operation;
                    if let O::StructuralCase {
                        source: observed,
                        path: observed_path,
                        cases,
                    } = terminator
                        && *observed == source
                        && observed_path == path
                        && let Some(selected) = cases
                            .iter()
                            .find(|selected| selected.psi_edge == edge.psi_edge)
                    {
                        return (selected.case == case) == expected;
                    }
                    membership_at(block.nodes.len() - 1, source, path, case, expected)
                };
                let transfer = consume_edge_sources(
                    function,
                    structural_types,
                    block_id,
                    &mut outgoing,
                    &edge.structural_bindings,
                    blocks[&edge.target],
                    |source, path, case| membership(source, path, case, false),
                )?;
                super::continuation::apply_edge_cleanup_actions(
                    function,
                    functions,
                    structural_types,
                    block_id,
                    &unrestricted,
                    &mut outgoing,
                    &edge.cleanup_actions,
                    &membership,
                )?;
                establish_edge_parameters(
                    function,
                    block_id,
                    &mut outgoing,
                    blocks[&edge.target],
                    transfer,
                )?;
                snapshots.insert(
                    OwnershipFrontierSite::EdgeExit(edge.psi_edge),
                    outgoing.snapshot(),
                );
                if let Some(existing) = incoming.get_mut(&edge.target) {
                    if existing.claims != outgoing.claims {
                        return Err(OptimizationUnitValidationError::CurrentClaimJoinMismatch {
                            machine: function.machine,
                            block: edge.target,
                        });
                    }
                    let changed = super::value_custody::join_owned_frontier(
                        function,
                        &unit.structural_types,
                        existing,
                        &outgoing,
                    )
                    .ok_or(
                        OptimizationUnitValidationError::CurrentOwnedPlaceJoinMismatch {
                            machine: function.machine,
                            block: edge.target,
                        },
                    )?;
                    if changed {
                        ready.insert(edge.target);
                    }
                } else {
                    incoming.insert(edge.target, outgoing);
                    ready.insert(edge.target);
                }
            }
        }

        if let Some(block) = blocks.keys().find(|block| !incoming.contains_key(block)) {
            return Err(OptimizationUnitValidationError::UnreachableBlock {
                machine: function.machine,
                block: *block,
            });
        }

        super::references::validate_return_coverage(
            function,
            &returned_reference_sources
                .into_values()
                .flatten()
                .collect::<Vec<_>>(),
        )?;
        facts.extend(snapshots.into_iter().map(|(site, snapshot)| {
            OwnershipFrontierFact::new(psi, function.machine, site, snapshot)
        }));
        Ok(pins.descriptor_requirements())
    }
}
mod residuals {
    use crate::StructuralTypes;
    use semantic_vocabulary::{StructuralCaseId, StructuralTypeId};
    use std::collections::BTreeSet;
    use terminal_psi::{
        StructuralFieldType, StructuralPathSegment, StructuralTypeShape, find_structural_case,
    };

    pub(super) fn partial_affine_residuals(
        structural_types: &(impl StructuralTypes + ?Sized),
        root_type: StructuralTypeId,
        moved_paths: &BTreeSet<Vec<StructuralPathSegment>>,
        max_residuals: usize,
    ) -> Option<Vec<(Vec<StructuralPathSegment>, StructuralTypeId)>> {
        selected_affine_residuals(
            structural_types,
            root_type,
            moved_paths,
            max_residuals,
            &|_, _| false,
        )
    }

    pub(super) fn selected_affine_residuals(
        structural_types: &(impl StructuralTypes + ?Sized),
        root_type: StructuralTypeId,
        moved_paths: &BTreeSet<Vec<StructuralPathSegment>>,
        max_residuals: usize,
        selected: &dyn Fn(&[StructuralPathSegment], StructuralCaseId) -> bool,
    ) -> Option<Vec<(Vec<StructuralPathSegment>, StructuralTypeId)>> {
        if moved_paths.is_empty()
            || moved_paths.iter().any(|path| {
                path.is_empty()
                    || crate::unit_validation::structural_catalog::resolve_structural_path(
                        structural_types,
                        root_type,
                        path,
                    )
                    .is_none()
            })
            || moved_paths.iter().enumerate().any(|(index, path)| {
                moved_paths
                    .iter()
                    .enumerate()
                    .any(|(other_index, other)| index != other_index && path.starts_with(other))
            })
        {
            return None;
        }
        let moved_paths = moved_paths.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let mut residuals = Vec::new();
        collect_partial_affine_residuals(
            structural_types,
            root_type,
            &moved_paths,
            &mut Vec::new(),
            max_residuals,
            selected,
            &mut residuals,
        )?;
        Some(residuals)
    }

    fn collect_partial_affine_residuals(
        structural_types: &(impl StructuralTypes + ?Sized),
        structural_type: StructuralTypeId,
        moved_paths: &[&[StructuralPathSegment]],
        prefix: &mut Vec<StructuralPathSegment>,
        max_residuals: usize,
        selected: &dyn Fn(&[StructuralPathSegment], StructuralCaseId) -> bool,
        residuals: &mut Vec<(Vec<StructuralPathSegment>, StructuralTypeId)>,
    ) -> Option<()> {
        match &structural_types.declaration(structural_type)?.shape {
            StructuralTypeShape::Record { fields, .. } => {
                if fields.is_empty() {
                    return None;
                }
                collect_field_residuals(
                    structural_types,
                    fields,
                    moved_paths,
                    prefix,
                    max_residuals,
                    selected,
                    residuals,
                )?;
            }
            StructuralTypeShape::FixedArray {
                element,
                length: terminal_psi::ArrayExtent::Literal(length),
            } if *length != 0 => {
                let remaining = max_residuals.checked_sub(residuals.len())?;
                if u128::from(*length) > remaining as u128 + moved_paths.len() as u128 {
                    return None;
                }
                for index in (0..*length).rev() {
                    collect_child_residuals(
                        structural_types,
                        *element,
                        StructuralPathSegment::FixedIndex(index),
                        moved_paths,
                        prefix,
                        max_residuals,
                        selected,
                        residuals,
                    )?;
                }
            }
            StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } => {
                let selected_case = moved_paths
                    .iter()
                    .find_map(|path| match path.first() {
                        Some(StructuralPathSegment::Case(case)) => Some(*case),
                        _ => None,
                    })
                    .or_else(|| {
                        let mut matches = cases.iter().filter(|case| selected(prefix, case.id));
                        let case = matches.next()?;
                        matches.next().is_none().then_some(case.id)
                    })?;
                let selected_case_declaration = find_structural_case(cases, selected_case)?;
                let common = structural_types
                    .declaration(structural_type)?
                    .shape
                    .direct_fields();
                if moved_paths.iter().any(|path| match path.first() {
                    Some(StructuralPathSegment::Case(case)) => *case != selected_case,
                    Some(StructuralPathSegment::Field(_)) => common.is_none(),
                    _ => true,
                }) {
                    return None;
                }
                let payload_moves = moved_paths
                    .iter()
                    .filter_map(|path| {
                        matches!(path.first(), Some(StructuralPathSegment::Case(_)))
                            .then_some(&path[1..])
                    })
                    .collect::<Vec<_>>();
                prefix.push(StructuralPathSegment::Case(selected_case));
                collect_field_residuals(
                    structural_types,
                    &selected_case_declaration.fields,
                    &payload_moves,
                    prefix,
                    max_residuals,
                    selected,
                    residuals,
                )?;
                prefix.pop();
                if let Some(common) = common {
                    let common_moves = moved_paths
                        .iter()
                        .copied()
                        .filter(|path| {
                            matches!(path.first(), Some(StructuralPathSegment::Field(_)))
                        })
                        .collect::<Vec<_>>();
                    collect_field_residuals(
                        structural_types,
                        common,
                        &common_moves,
                        prefix,
                        max_residuals,
                        selected,
                        residuals,
                    )?;
                }
            }
            _ => return None,
        }
        Some(())
    }

    fn collect_field_residuals(
        structural_types: &(impl StructuralTypes + ?Sized),
        fields: &[terminal_psi::StructuralFieldDeclaration],
        moved_paths: &[&[StructuralPathSegment]],
        prefix: &mut Vec<StructuralPathSegment>,
        max_residuals: usize,
        selected: &dyn Fn(&[StructuralPathSegment], StructuralCaseId) -> bool,
        residuals: &mut Vec<(Vec<StructuralPathSegment>, StructuralTypeId)>,
    ) -> Option<()> {
        for field in fields.iter().rev() {
            if field.relevance.is_erased() {
                return None;
            }
            let child_type = match &field.field_type {
                StructuralFieldType::Structural(child) => Some(*child),
                scalar if scalar.scalar_type().is_some() => None,
                bytes => match bytes.byte_sequence_carrier()? {
                    terminal_psi::ByteSequenceCarrier::BoundedOwned { .. } => None,
                    _ => Some(
                        terminal_semantics::inline_carrier_declaration(
                            structural_types.declarations(),
                            field,
                        )?
                        .id,
                    ),
                },
            };
            if let Some(child_type) = child_type {
                collect_child_residuals(
                    structural_types,
                    child_type,
                    StructuralPathSegment::Field(field.identity),
                    moved_paths,
                    prefix,
                    max_residuals,
                    selected,
                    residuals,
                )?;
            }
        }
        Some(())
    }

    fn collect_child_residuals(
        structural_types: &(impl StructuralTypes + ?Sized),
        child_type: StructuralTypeId,
        segment: StructuralPathSegment,
        moved_paths: &[&[StructuralPathSegment]],
        prefix: &mut Vec<StructuralPathSegment>,
        max_residuals: usize,
        selected: &dyn Fn(&[StructuralPathSegment], StructuralCaseId) -> bool,
        residuals: &mut Vec<(Vec<StructuralPathSegment>, StructuralTypeId)>,
    ) -> Option<()> {
        let descendants = moved_paths
            .iter()
            .filter_map(|path| {
                let (head, tail) = path.split_first()?;
                (head == &segment).then_some(tail)
            })
            .collect::<Vec<_>>();
        prefix.push(segment);
        if descendants.is_empty() {
            if residuals.len() == max_residuals {
                return None;
            }
            residuals.push((prefix.clone(), child_type));
        } else if descendants.iter().all(|path| !path.is_empty()) {
            collect_partial_affine_residuals(
                structural_types,
                child_type,
                &descendants,
                prefix,
                max_residuals,
                selected,
                residuals,
            )?;
        } else if descendants.len() != 1 {
            return None;
        }
        prefix.pop();
        Some(())
    }
}
mod storage_regions {
    use crate::StructuralTypes;
    use crate::unit_validation::operation_contracts::structural_source_contract;
    use abstract_operations::AbstractOperation as O;
    use optimization_unit::PsiOptimizationFunction;
    use semantic_vocabulary::{
        CanonicalStructuralPathSegment, PlaceId, StructuralFieldId, StructuralTypeId,
    };
    use std::collections::BTreeMap;
    use terminal_psi::{
        StructuralFieldDeclaration, StructuralFieldType, StructuralPathSegment,
        StructuralTypeDeclaration, StructuralTypeShape,
    };

    pub(super) fn projected(
        function: &PsiOptimizationFunction,
        types: &(impl StructuralTypes + ?Sized),
        place: PlaceId,
        path: &[StructuralPathSegment],
        field: Option<StructuralFieldId>,
    ) -> terminal_semantics::StorageRegion {
        let path = abstract_operations::values::resolve_constant_runtime_indexes(
            function
                .blocks
                .iter()
                .flat_map(|block| block.nodes.iter().map(|node| &node.operation)),
            path,
        );
        structural_source_contract(function, place).map_or_else(
            || terminal_semantics::StorageRegion {
                path: Vec::new(),
                indirect: true,
            },
            |root| {
                let mut region = terminal_semantics::projected_storage_region(
                    types.declarations(),
                    root.structural_type,
                    &path,
                    field,
                );
                if !region.indirect
                    && let Some(position) = path.iter().position(|segment| {
                        matches!(segment, StructuralPathSegment::RuntimeIndex { .. })
                    })
                {
                    region.path = path[..=position].to_vec();
                }
                region
            },
        )
    }

    pub(super) fn canonical_field(
        function: &PsiOptimizationFunction,
        types: &(impl StructuralTypes + ?Sized),
        place: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        field: StructuralFieldId,
    ) -> Vec<StructuralPathSegment> {
        structural_source_contract(function, place).map_or_else(Vec::new, |root| {
            terminal_semantics::canonical_field_storage_region(
                types.declarations(),
                root.structural_type,
                path,
                field,
            )
        })
    }

    pub(super) fn written_or_exclusive_regions(
        function: &PsiOptimizationFunction,
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        operation: &O,
    ) -> Vec<(PlaceId, terminal_semantics::StorageRegion)> {
        operation
            .storage_mutations()
            .into_iter()
            .map(|region| {
                (
                    region.place,
                    projected(function, types, region.place, region.path, region.field),
                )
            })
            .collect()
    }

    pub(crate) fn storage_cannot_retain_loan(
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        structural_type: StructuralTypeId,
        established: &mut BTreeMap<StructuralTypeId, bool>,
    ) -> bool {
        if let Some(result) = established.get(&structural_type) {
            return *result;
        }
        established.insert(structural_type, false);
        let Some(declaration) = types.get(&structural_type) else {
            return false;
        };
        let result = match &declaration.shape {
            StructuralTypeShape::PrimitiveScalar(_) | StructuralTypeShape::AtomicCell(_) => true,
            StructuralTypeShape::ByteSequence(carrier) => !carrier.is_borrowed(),
            StructuralTypeShape::Record { fields, .. } => {
                fields_cannot_retain_loan(types, fields, established)
            }
            StructuralTypeShape::FixedArray { element, .. } => {
                storage_cannot_retain_loan(types, *element, established)
            }
            StructuralTypeShape::Sum { cases } => cases
                .iter()
                .all(|case| fields_cannot_retain_loan(types, &case.fields, established)),
            StructuralTypeShape::Mixed { fields, cases } => {
                fields_cannot_retain_loan(types, fields, established)
                    && cases
                        .iter()
                        .all(|case| fields_cannot_retain_loan(types, &case.fields, established))
            }
            StructuralTypeShape::Reference { .. } | StructuralTypeShape::ElementView { .. } => {
                false
            }
        };
        established.insert(structural_type, result);
        result
    }

    fn fields_cannot_retain_loan(
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        fields: &[StructuralFieldDeclaration],
        established: &mut BTreeMap<StructuralTypeId, bool>,
    ) -> bool {
        fields.iter().all(|field| match &field.field_type {
            StructuralFieldType::Scalar(_)
            | StructuralFieldType::Atomic(_)
            | StructuralFieldType::AtomicBoundedInteger(_)
            | StructuralFieldType::RefinedScalar(_)
            | StructuralFieldType::BoundedInteger(_)
            | StructuralFieldType::IeeeFloat(_) => true,
            StructuralFieldType::ByteSequence(carrier)
            | StructuralFieldType::PredicateByteSequence { carrier, .. } => !carrier.is_borrowed(),
            StructuralFieldType::Structural(child) => {
                storage_cannot_retain_loan(types, *child, established)
            }
            StructuralFieldType::Erased { .. } => field.relevance.is_erased(),
        })
    }
}
pub(crate) use storage_regions::storage_cannot_retain_loan;
mod structural {
    use crate::StructuralTypes;
    use crate::current_ownership::CurrentOwnership;
    use crate::current_ownership::residuals::partial_affine_residuals;
    use abstract_operations::AbstractOperation as O;
    use optimization_unit::PsiOptimizationFunction;
    use semantic_vocabulary::{PlaceId, StructuralTypeId};
    use std::collections::BTreeSet;
    use terminal_psi::{StructuralMultiplicity, StructuralPathSegment, StructuralTypeShape};

    pub(super) fn place_structural_type(
        function: &PsiOptimizationFunction,
        place: PlaceId,
    ) -> Option<StructuralTypeId> {
        function
            .structural_parameters
            .iter()
            .chain(function.block_structural_parameters())
            .find_map(|parameter| (parameter.place == place).then_some(parameter.structural_type))
            .or_else(|| {
                function
                    .result
                    .structural()
                    .filter(|result| result.place == place)
                    .map(|result| result.structural_type)
            })
            .or_else(|| {
                function
                    .structural_places
                    .iter()
                    .find_map(|candidate| (candidate.id == place).then_some(candidate.kind))
                    .and_then(|kind| match kind {
                        semantic_vocabulary::StructuralPlaceKind::OperationResult {
                            structural_type,
                            ..
                        }
                        | semantic_vocabulary::StructuralPlaceKind::ByteSequenceLiteral {
                            structural_type,
                            ..
                        }
                        | semantic_vocabulary::StructuralPlaceKind::TrivialAffineLocal {
                            structural_type,
                            ..
                        } => Some(structural_type),
                        semantic_vocabulary::StructuralPlaceKind::ProviderAttachment {
                            attachment,
                            ..
                        } => Some(attachment),
                        semantic_vocabulary::StructuralPlaceKind::Parameter { .. }
                        | semantic_vocabulary::StructuralPlaceKind::BlockParameter { .. }
                        | semantic_vocabulary::StructuralPlaceKind::Result => None,
                    })
            })
    }

    pub(crate) fn partial_affine_root_type(
        function: &PsiOptimizationFunction,
        place: PlaceId,
    ) -> Option<StructuralTypeId> {
        if function.entry_claim_declared(place) || function.content_entry_claim_declared(place) {
            return None;
        }
        if let Some(parameter) = function.arrival_parameter(place) {
            return (parameter.is_unqualified_owned_affine()).then_some(parameter.structural_type);
        }
        let semantic_vocabulary::StructuralPlaceKind::OperationResult {
            producer,
            structural_type,
        } = function.structural_place(place)?.kind
        else {
            return None;
        };
        function.block_nodes().find_map(|node| {
            let (operation, result) = node.operation.structural_result()?;
            let source =
                crate::unit_validation::operation_contracts::structural_operation_result_contract(
                    function,
                    &node.operation,
                )?;
            (operation == producer
                && result.place == place
                && result.structural_type == structural_type
                && source.access == terminal_psi::StructuralAccess::Owned
                && result.multiplicity == StructuralMultiplicity::Affine
                && result.qualifications.is_empty()
                && result.projected_qualifications.is_empty()
                && result.claims.is_empty())
            .then_some(structural_type)
        })
    }

    pub(crate) fn partial_affine_root_is_drained(
        function: &PsiOptimizationFunction,
        structural_types: &(impl StructuralTypes + ?Sized),
        place: PlaceId,
        moved: &BTreeSet<Vec<StructuralPathSegment>>,
    ) -> bool {
        partial_affine_root_type(function, place).is_some_and(|root_type| {
            partial_affine_residuals(structural_types, root_type, moved, 0)
                .is_some_and(|residuals| residuals.is_empty())
        })
    }

    pub(super) fn projected_root_is_fully_consumed(
        function: &PsiOptimizationFunction,
        structural_types: &(impl StructuralTypes + ?Sized),
        frontier: &CurrentOwnership,
        place: PlaceId,
    ) -> bool {
        if frontier
            .claims
            .values()
            .any(|claim| claim.input == Some(place))
            || function.content_entry_claim_declared(place)
        {
            return false;
        }
        let Some(moved) = frontier.partial_custody_paths.get(&place) else {
            return false;
        };
        projected_paths_are_fully_consumed(function, structural_types, place, moved)
    }

    pub(crate) fn projected_paths_are_fully_consumed(
        function: &PsiOptimizationFunction,
        structural_types: &(impl StructuralTypes + ?Sized),
        place: PlaceId,
        moved: &BTreeSet<Vec<StructuralPathSegment>>,
    ) -> bool {
        if function.content_entry_claim_declared(place) {
            return false;
        }
        if partial_affine_root_type(function, place).is_some() {
            return partial_affine_root_is_drained(function, structural_types, place, moved);
        }
        let Some(parameter) = function.structural_parameter(place) else {
            return false;
        };
        if parameter.multiplicity != StructuralMultiplicity::Linear {
            return false;
        }
        let Some(StructuralTypeShape::FixedArray {
            length: terminal_psi::ArrayExtent::Literal(length),
            ..
        }) = structural_types
            .declaration(parameter.structural_type)
            .map(|declaration| &declaration.shape)
        else {
            return false;
        };
        let Some(length) = usize::try_from(*length).ok() else {
            return false;
        };
        moved.len() == length
            && (0..length).all(|index| {
                moved.contains(&vec![StructuralPathSegment::FixedIndex(
                    u64::try_from(index).expect("a usize index fits u64"),
                )])
            })
    }

    pub(crate) fn constructor_arguments(
        operation: &O,
    ) -> impl Iterator<Item = &terminal_psi::StructuralArgument> {
        let fields = match operation {
            abstract_operations::establish_case_or_record_fields_pattern!(fields) => {
                fields.as_slice()
            }
            _ => &[],
        };
        let elements = match operation {
            O::EstablishStructuralArray { elements, .. } => elements.as_slice(),
            _ => &[],
        };
        fields
            .iter()
            .filter_map(|initializer| match &initializer.value {
                terminal_psi::RecordFieldValue::Structural(argument) => Some(argument),
                terminal_psi::RecordFieldValue::Scalar { .. } => None,
            })
            .chain(elements)
    }
}
mod value_custody {
    use super::{CurrentOwnership, LiveClaim};
    use crate::StructuralTypes;
    use crate::unit_validation::operation_contracts::structural_source_contract;
    use crate::{OperationSite, OptimizationUnitValidationError};
    use abstract_operations::AbstractOperation as O;
    use abstract_operations::StructuralWindowRoot;
    use optimization_unit::PsiOptimizationFunction;
    use semantic_vocabulary::{BlockId, PlaceId, StructuralCaseId, StructuralTypeId};
    use std::collections::BTreeSet;
    use terminal_psi::{
        StructuralAccess, StructuralArgument, StructuralMultiplicity, StructuralPathSegment,
        StructuralStoreDisposition, StructuralTypeDeclaration,
    };

    pub(super) struct ValueUse<'a> {
        pub function: &'a PsiOptimizationFunction,
        pub types: &'a [StructuralTypeDeclaration],
        pub block: BlockId,
        pub node: u32,
        pub admitted_case: &'a dyn Fn(PlaceId, &[StructuralPathSegment], StructuralCaseId) -> bool,
    }

    pub(super) fn uninstalled_home(current: &CurrentOwnership, place: PlaceId) -> bool {
        current.owned_places.contains_key(&place)
            && current
                .partial_custody_paths
                .get(&place)
                .is_some_and(|missing| {
                    missing.len() == 1 && missing.first().is_some_and(Vec::is_empty)
                })
    }

    /// Equivalent absence can be recorded as one parent hole or as its
    /// exhausted children. Keep the coarsest paths actually supplied by the
    /// arrivals, so receiving authority cannot depend on which arrived first.
    /// Only a real finite affine home permits this typed comparison; claims,
    /// allocated owners and references retain their existing exact joins.
    pub(super) fn join_owned_frontier(
        function: &PsiOptimizationFunction,
        types: &[StructuralTypeDeclaration],
        left: &mut CurrentOwnership,
        right: &CurrentOwnership,
    ) -> Option<bool> {
        if left.owned_places != right.owned_places
            || left.live_references != right.live_references
            || left.partial_custody_paths.len() != right.partial_custody_paths.len()
        {
            return None;
        }
        let mut replacements = Vec::new();
        for ((place, mine), (other_place, theirs)) in left
            .partial_custody_paths
            .iter()
            .zip(&right.partial_custody_paths)
        {
            if place != other_place {
                return None;
            }
            if mine == theirs {
                continue;
            }
            let root = super::structural::partial_affine_root_type(function, *place)?;
            let declaration = types.iter().find(|declaration| declaration.id == root)?;
            if declaration.copyable
                || declaration.owns_linear_claim
                || !declaration.no_code_affine_cleanup
                || !declaration.descriptor_cleanup
            {
                return None;
            }
            let covered = |missing, regions| {
                terminal_semantics::partial_affine_residuals_in_regions(
                    types, root, missing, regions, 0,
                )
                .is_some_and(|residuals| residuals.is_empty())
            };
            if !covered(mine, theirs) || !covered(theirs, mine) {
                return None;
            }
            let mut coarsened: BTreeSet<Vec<StructuralPathSegment>> = BTreeSet::new();
            // Lexical path order places an ancestor before all its children.
            // Every kept path comes from an actual incoming frontier.
            for path in mine.union(theirs) {
                if coarsened
                    .last()
                    .is_none_or(|parent| !path.starts_with(parent))
                {
                    coarsened.insert(path.clone());
                }
            }
            if coarsened != *mine {
                replacements.push((*place, coarsened));
            }
        }
        let changed = !replacements.is_empty();
        for (place, missing) in replacements {
            left.partial_custody_paths.insert(place, missing);
        }
        Some(changed)
    }

    impl ValueUse<'_> {
        pub(super) fn receiving_store(
            &self,
            current: &CurrentOwnership,
            destination: &StructuralWindowRoot,
            path: &[StructuralPathSegment],
            field: Option<semantic_vocabulary::StructuralFieldId>,
            disposition: StructuralStoreDisposition,
        ) -> Result<(), OptimizationUnitValidationError> {
            let hole = super::references::window_hole(
                self.function,
                self.types,
                destination.place(),
                destination.structural_type(),
                path,
                field,
                disposition.required_access(),
            )
            .ok_or_else(|| {
                self.refusal(destination.place(), "store has no exact receiving window")
            })?;
            if destination.parameter().is_some()
                || destination.multiplicity() == StructuralMultiplicity::Unrestricted
                || !terminal_semantics::projection_stays_in_inline_storage(
                    self.types.iter(),
                    destination.structural_type(),
                    &hole,
                )
            {
                return Ok(());
            }
            let missing = current.partial_custody_paths.get(&destination.place());
            if !current.owned_places.contains_key(&destination.place()) && missing.is_none() {
                return Err(self.refusal(destination.place(), "store targets an ended owned home"));
            }
            let ready = match disposition {
                StructuralStoreDisposition::Vacant => missing.is_some_and(|missing| {
                    terminal_semantics::structural_region_is_vacant(
                        self.types,
                        destination.structural_type(),
                        missing,
                        &hole,
                        &|path, case| (self.admitted_case)(destination.place(), path, case),
                    )
                }),
                StructuralStoreDisposition::ReplaceDiscardable => missing.is_none_or(|missing| {
                    missing.iter().all(|absent| {
                        !terminal_semantics::structural_paths_may_overlap(absent, &hole)
                    })
                }),
            };
            if !ready {
                return Err(self.refusal_at(
                    destination.place(),
                    &hole,
                    "store disposition does not match the owned receiving region",
                ));
            }
            Ok(())
        }

        fn refusal(&self, place: PlaceId, reason: &'static str) -> OptimizationUnitValidationError {
            self.refusal_at(place, &[], reason)
        }

        fn refusal_at(
            &self,
            place: PlaceId,
            path: &[StructuralPathSegment],
            reason: &'static str,
        ) -> OptimizationUnitValidationError {
            OptimizationUnitValidationError::CurrentOwnedValueUnavailable {
                site: OperationSite::new(self.function.machine, self.block, self.node),
                place,
                path_length: path.len(),
                path_prefix: path.iter().take(4).cloned().collect(),
                reason,
            }
        }

        fn frontier(
            &self,
            current: &CurrentOwnership,
            place: PlaceId,
            selected: StructuralTypeId,
            path: &[StructuralPathSegment],
        ) -> Result<(), OptimizationUnitValidationError> {
            let claims = current.claims.values().filter(|claim| {
                claim.input == Some(place)
                    && (claim.path.starts_with(path) || path.starts_with(&claim.path))
            });
            if claims
                .clone()
                .any(|claim| claim.multiplicity != Some(StructuralMultiplicity::Linear))
            {
                return Err(
                    self.refusal(place, "owned value claim has no exact linear multiplicity")
                );
            }
            terminal_semantics::owned_structure::validate_owned_claim_frontier(
                &|identity| self.types.declaration(identity),
                selected,
                path,
                claims.map(|claim| claim.path.clone()),
                &|path, case| (self.admitted_case)(place, path, case),
            )
            .map_err(|reason| self.refusal_at(place, path, reason))
        }

        pub(super) fn moving(
            &self,
            current: &CurrentOwnership,
            argument: &StructuralArgument,
        ) -> Result<(), OptimizationUnitValidationError> {
            let root =
                structural_source_contract(self.function, argument.place).ok_or_else(|| {
                    self.refusal(argument.place, "owned operand has no exact source contract")
                })?;
            if root.access != StructuralAccess::Owned
                || !terminal_semantics::projection_stays_in_inline_storage(
                    self.types.iter(),
                    root.structural_type,
                    &argument.path,
                )
            {
                return Ok(());
            }
            if current
                .partial_custody_paths
                .get(&argument.place)
                .is_some_and(|missing| {
                    missing.iter().any(|path| {
                        terminal_semantics::structural_paths_may_overlap(path, &argument.path)
                    })
                })
            {
                return Err(self.refusal_at(
                    argument.place,
                    &argument.path,
                    "owned operand is unestablished or partially unavailable",
                ));
            }
            let selected = terminal_semantics::runtime_structural_path_tip(
                self.types.iter(),
                root.structural_type,
                &argument.path,
            )
            .ok_or_else(|| {
                self.refusal(argument.place, "owned operand path has no selected carrier")
            })?;
            self.frontier(current, argument.place, selected.id, &argument.path)
        }

        pub(super) fn observing(
            &self,
            current: &CurrentOwnership,
            place: PlaceId,
            path: &[StructuralPathSegment],
        ) -> Result<(), OptimizationUnitValidationError> {
            let root = structural_source_contract(self.function, place).ok_or_else(|| {
                self.refusal(place, "observed value has no exact source contract")
            })?;
            if root.access != StructuralAccess::Owned {
                return Ok(());
            }
            if (root.multiplicity != StructuralMultiplicity::Unrestricted
                && current.owned_places.get(&place) != Some(&root.multiplicity)
                && !super::structural::projected_root_is_fully_consumed(
                    self.function,
                    self.types,
                    current,
                    place,
                ))
                || current
                    .partial_custody_paths
                    .get(&place)
                    .is_some_and(|absent| {
                        absent.iter().any(|absent| {
                            terminal_semantics::structural_paths_may_overlap(absent, path)
                        })
                    })
            {
                return Err(self.refusal_at(
                    place,
                    path,
                    "observed owned value is moved or partially unavailable",
                ));
            }
            for length in 0..=path.len() {
                let prefix = &path[..length];
                if matches!(prefix.last(), Some(StructuralPathSegment::Case(_))) {
                    continue;
                }
                if !terminal_semantics::projection_stays_in_inline_storage(
                    self.types.iter(),
                    root.structural_type,
                    prefix,
                ) {
                    return Ok(());
                }
                let Some(selected) = terminal_semantics::runtime_structural_path_tip(
                    self.types.iter(),
                    root.structural_type,
                    prefix,
                ) else {
                    return Ok(());
                };
                if selected.owns_linear_claim || length == path.len() {
                    return self.frontier(current, place, selected.id, prefix);
                }
            }
            Ok(())
        }

        pub(super) fn observations(
            &self,
            current: &CurrentOwnership,
            operation: &O,
        ) -> Result<(), OptimizationUnitValidationError> {
            match operation {
                O::EstablishReference { source, .. }
                | O::EstablishAccessReference { source, .. }
                | O::EstablishElementView { source, .. }
                | O::ElementViewLength { source, .. }
                | O::ElementViewSubslice { source, .. }
                | O::ByteSequenceLength { source, .. }
                | O::ByteSequenceSubslice { source, .. }
                | O::GlobalDescriptorTable { source, .. } => {
                    self.observing(current, source.place, &source.path)
                }
                abstract_operations::primitive_storage_observation_pattern!(source, path, ..) => {
                    let region = super::storage_regions::projected(
                        self.function,
                        self.types,
                        *source,
                        path,
                        None,
                    );
                    self.observing(current, *source, region.direct_path())
                }
                O::StructuralScalarFieldRead {
                    source,
                    path,
                    field,
                    ..
                } => {
                    let region = super::storage_regions::canonical_field(
                        self.function,
                        self.types,
                        *source,
                        path,
                        *field,
                    );
                    self.observing(current, *source, &region)
                }
                O::StructuralCase { source, path, .. } => self.observing(current, *source, path),
                O::StructuralLeafCopy {
                    source,
                    path: abstract_operations::StructuralCopyPath::Ordinary(path),
                    ..
                } => self.observing(current, *source, path),
                O::MoveStructuralField {
                    source,
                    path,
                    field,
                    ..
                } => {
                    let selected = super::references::window_hole(
                        self.function,
                        self.types,
                        source.place(),
                        source.structural_type(),
                        path,
                        *field,
                        StructuralAccess::MutableBorrow,
                    )
                    .ok_or_else(|| {
                        self.refusal(source.place(), "move has no selected storage window")
                    })?;
                    self.observing(current, source.place(), &selected)
                }
                abstract_operations::byte_len_or_field_read_pattern!(source, path, field, ..) => {
                    let region = super::storage_regions::projected(
                        self.function,
                        self.types,
                        *source,
                        path,
                        Some(*field),
                    );
                    self.observing(current, *source, region.direct_path())
                }
                O::ElementViewRead { source, .. } | O::ByteSequenceRead { source, .. } => {
                    self.observing(current, *source, &[])
                }
                O::EstablishByteSequenceConcatenation { left, right, .. }
                | O::ByteSequenceEqual { left, right, .. } => {
                    self.byte_operand(current, left)?;
                    self.byte_operand(current, right)
                }
                O::ByteSequencePredicate { operand, .. } => self.byte_operand(current, operand),
                O::ReloadSegmentDescriptors { gdt, scratch, .. } => {
                    self.observing(current, gdt.place, &gdt.path)?;
                    self.observing(current, scratch.place, &scratch.path)
                }
                O::ActivateTaskRegister {
                    gdt,
                    task_state,
                    scratch,
                    ..
                } => {
                    self.observing(current, gdt.place, &gdt.path)?;
                    self.observing(current, task_state.place, &task_state.path)?;
                    self.observing(current, scratch.place, &scratch.path)
                }
                _ => Ok(()),
            }
        }

        fn byte_operand(
            &self,
            current: &CurrentOwnership,
            operand: &terminal_psi::control_flow::ByteSequenceOperand,
        ) -> Result<(), OptimizationUnitValidationError> {
            use terminal_psi::control_flow::ByteSequenceOperand;
            let (source, field) = match operand {
                ByteSequenceOperand::Whole { source, .. } => (source, None),
                ByteSequenceOperand::Field { source, field, .. } => (source, Some(*field)),
            };
            let region = super::storage_regions::projected(
                self.function,
                self.types,
                source.place,
                &source.path,
                field,
            );
            self.observing(current, source.place, region.direct_path())
        }

        pub(super) fn install_store(
            &self,
            current: &mut CurrentOwnership,
            destination: &StructuralWindowRoot,
            path: &[StructuralPathSegment],
            field: Option<semantic_vocabulary::StructuralFieldId>,
            value: &StructuralArgument,
            access: StructuralAccess,
        ) -> Result<(), OptimizationUnitValidationError> {
            let hole = super::references::window_hole(
                self.function,
                self.types,
                destination.place(),
                destination.structural_type(),
                path,
                field,
                access,
            )
            .ok_or_else(|| {
                self.refusal(destination.place(), "store has no exact receiving window")
            })?;
            let owned = destination.parameter().is_none()
                && terminal_semantics::projection_stays_in_inline_storage(
                    self.types.iter(),
                    destination.structural_type(),
                    &hole,
                );
            let moved = current
                .claims
                .iter()
                .filter(|(_, claim)| claim.input == Some(value.place))
                .map(|(identity, claim)| (*identity, claim.clone()))
                .collect::<Vec<_>>();
            if owned
                && current.claims.values().any(|claim| {
                    claim.input == Some(destination.place())
                        && terminal_semantics::structural_paths_may_overlap(&claim.path, &hole)
                })
            {
                return Err(self.refusal(
                    destination.place(),
                    "store would overwrite a live owned claim",
                ));
            }
            for (identity, claim) in moved {
                current.claims.remove(&identity);
                if owned {
                    let mut restored = hole.clone();
                    restored.extend_from_slice(&claim.path);
                    let multiplicity = if restored.is_empty() {
                        destination.multiplicity()
                    } else {
                        StructuralMultiplicity::Linear
                    };
                    current.claims.insert(
                        identity,
                        LiveClaim {
                            input: Some(destination.place()),
                            path: restored,
                            multiplicity: Some(multiplicity),
                        },
                    );
                }
            }
            if owned
                && let Some(missing) = current.partial_custody_paths.get_mut(&destination.place())
            {
                missing.retain(|absent| !absent.starts_with(&hole));
                if missing.is_empty() {
                    current.partial_custody_paths.remove(&destination.place());
                }
            }
            if owned && destination.multiplicity() != StructuralMultiplicity::Unrestricted {
                current
                    .owned_places
                    .entry(destination.place())
                    .or_insert(destination.multiplicity());
            }
            Ok(())
        }
    }

    #[cfg(test)]
    mod store_tests {
        use super::*;
        use semantic_vocabulary::{PsiSemanticId, StructuralPlaceKind};
        use std::collections::{BTreeMap, BTreeSet};
        use terminal_psi::{
            ArrayExtent, StructuralParameterDeclaration, StructuralPlaceDeclaration,
            StructuralRecordStorage, StructuralTypeShape, TerminalIdentityIndex,
        };

        fn id<T: PsiSemanticId>(raw: u64) -> T {
            T::new(raw).unwrap()
        }

        fn context() -> (
            PsiOptimizationFunction,
            Vec<StructuralTypeDeclaration>,
            StructuralWindowRoot,
        ) {
            let types = (1..=3)
                .map(|raw| StructuralTypeDeclaration {
                    id: id(raw),
                    identity: TerminalIdentityIndex(raw as u32),
                    quotient_carrier: None,
                    owns_linear_claim: false,
                    copyable: false,
                    no_code_affine_cleanup: true,
                    descriptor_cleanup: true,
                    scalar_restriction: None,
                    requires: Vec::new(),
                    shape: if raw == 1 {
                        StructuralTypeShape::Record {
                            fields: Vec::new(),
                            storage: StructuralRecordStorage::Packed,
                        }
                    } else {
                        StructuralTypeShape::FixedArray {
                            element: id(raw - 1),
                            length: ArrayExtent::Literal(2),
                        }
                    },
                })
                .collect();
            let parameter = StructuralParameterDeclaration {
                place: id(1),
                position: 0,
                structural_type: id(3),
                multiplicity: StructuralMultiplicity::Affine,
                access: StructuralAccess::Owned,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                additional_input_qualifications: Vec::new(),
            };
            let function = PsiOptimizationFunction {
                machine: id(1),
                attachment: None,
                nominal_cleanup_subject: None,
                entry: id(1),
                parameters: Vec::new(),
                structural_parameters: vec![parameter.clone()],
                structural_places: vec![StructuralPlaceDeclaration {
                    id: id(1),
                    kind: StructuralPlaceKind::Parameter { position: 0 },
                }],
                result: abstract_operations::AbstractFunctionResult::Unit,
                declared_places: BTreeSet::from([id(1)]),
                entry_claim_declarations: Vec::new(),
                block_entry_claims: Vec::new(),
                content_entry_claims: Vec::new(),
                verified_contract: None,
                evidence_contract_lanes: Vec::new(),
                entry_claims: BTreeSet::new(),
                published_service_ceiling: Vec::new(),
                declared_service_reach: Vec::new(),
                facts: Vec::new(),
                blocks: Vec::new(),
            };
            (function, types, StructuralWindowRoot::Parameter(parameter))
        }

        fn current(missing: Vec<Vec<StructuralPathSegment>>) -> CurrentOwnership {
            CurrentOwnership {
                claims: BTreeMap::new(),
                owned_places: BTreeMap::from([(id(1), StructuralMultiplicity::Affine)]),
                partial_custody_paths: if missing.is_empty() {
                    BTreeMap::new()
                } else {
                    BTreeMap::from([(id(1), missing.into_iter().collect())])
                },
                live_references: Vec::new(),
            }
        }

        fn path(indices: &[u64]) -> Vec<StructuralPathSegment> {
            indices
                .iter()
                .map(|index| StructuralPathSegment::FixedIndex(*index))
                .collect()
        }

        #[test]
        fn optional_copy_cleanup_requires_its_actual_no_code_owner() {
            use terminal_psi::TerminalAffineCleanupAction as Action;
            let (mut function, mut types, _) = context();
            function.structural_parameters[0].multiplicity = StructuralMultiplicity::Unrestricted;
            for declaration in &mut types {
                declaration.copyable = true;
            }
            let cleanup = |function: &PsiOptimizationFunction,
                           types: &[StructuralTypeDeclaration],
                           current: &mut CurrentOwnership,
                           actions: &[Action]| {
                let types = types
                    .iter()
                    .map(|declaration| (declaration.id, declaration))
                    .collect();
                let unrestricted =
                    super::super::cleanup::unrestricted_owned_cleanup_roots(function, &types);
                super::super::continuation::apply_edge_cleanup_actions(
                    function,
                    &BTreeMap::new(),
                    &types,
                    id(1),
                    &unrestricted,
                    current,
                    actions,
                    &|_, _, _, _| false,
                )
            };
            let empty = || CurrentOwnership {
                claims: BTreeMap::new(),
                owned_places: BTreeMap::new(),
                partial_custody_paths: BTreeMap::new(),
                live_references: Vec::new(),
            };
            for action in [Action::RetireHome(id(1)), Action::DiscardRoot(id(1))] {
                let mut frontier = empty();
                cleanup(&function, &types, &mut frontier, &[action]).unwrap();
                assert_eq!(
                    frontier,
                    empty(),
                    "a copy has no finite obligation to invent"
                );
            }
            for borrowed in [
                StructuralAccess::SharedBorrow,
                StructuralAccess::MutableBorrow,
            ] {
                let mut borrowed_function = function.clone();
                borrowed_function.structural_parameters[0].access = borrowed;
                assert!(
                    cleanup(
                        &borrowed_function,
                        &types,
                        &mut empty(),
                        &[Action::RetireHome(id(1))]
                    )
                    .is_err()
                );
            }
            for changed in 0..4 {
                let mut declarations = types.clone();
                match changed {
                    0 => declarations[2].copyable = false,
                    1 => declarations[2].owns_linear_claim = true,
                    2 => declarations[2].no_code_affine_cleanup = false,
                    _ => declarations[2].descriptor_cleanup = false,
                }
                assert!(
                    cleanup(
                        &function,
                        &declarations,
                        &mut empty(),
                        &[Action::RetireHome(id(1))]
                    )
                    .is_err()
                );
            }
            let mut claimed = empty();
            claimed.claims.insert(
                id(1),
                super::super::LiveClaim {
                    input: Some(id(1)),
                    path: Vec::new(),
                    multiplicity: Some(StructuralMultiplicity::Linear),
                },
            );
            assert!(
                cleanup(
                    &function,
                    &types,
                    &mut claimed,
                    &[Action::RetireHome(id(1))]
                )
                .is_err()
            );
            for actions in [
                vec![Action::RetireHome(id(2))],
                vec![Action::RetireHome(id(1)), Action::RetireHome(id(1))],
            ] {
                assert!(cleanup(&function, &types, &mut empty(), &actions).is_err());
            }
            let mut unresolved = function.clone();
            unresolved.structural_parameters.clear();
            unresolved.structural_places[0].kind = StructuralPlaceKind::OperationResult {
                producer: id(1),
                structural_type: id(3),
            };
            assert!(
                cleanup(
                    &unresolved,
                    &types,
                    &mut empty(),
                    &[Action::RetireHome(id(1))]
                )
                .is_err(),
                "a type declaration cannot invent its missing producer"
            );
            use crate::unit_validation::references::{
                LiveReference, ReferenceIdentity, ReferenceOrigin, ReferenceParent,
            };
            let mut borrowed = empty();
            borrowed.live_references.push(LiveReference {
                identity: ReferenceIdentity {
                    place: id(2),
                    path: Vec::new(),
                },
                carrier: id(2),
                carrier_path: Vec::new(),
                root: ReferenceOrigin::Primitive(id(1)),
                parent: ReferenceParent::Root(ReferenceOrigin::Primitive(id(1))),
            });
            let before = borrowed.clone();
            assert!(
                cleanup(
                    &function,
                    &types,
                    &mut borrowed,
                    &[Action::RetireHome(id(1))]
                )
                .is_err()
            );
            assert_eq!(
                borrowed, before,
                "retirement leaves an outstanding loan intact"
            );
        }

        #[test]
        fn an_allocated_local_uses_its_real_result_contract_at_the_join() {
            let (mut function, types, _) = context();
            function.structural_parameters.clear();
            function.structural_places[0].kind = StructuralPlaceKind::OperationResult {
                producer: id(1),
                structural_type: id(3),
            };
            function.blocks.push(optimization_unit::OptimizationBlock {
                id: id(1),
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                nodes: vec![optimization_unit::OptimizationNode {
                    operation: abstract_operations::AbstractOperation::EstablishStructuralLocal {
                        psi_operation: id(1),
                        result: terminal_psi::StructuralOperationResult {
                            place: id(1),
                            structural_type: id(3),
                            multiplicity: StructuralMultiplicity::Affine,
                            qualifications: Vec::new(),
                            projected_qualifications: Vec::new(),
                            additional_input_qualifications: Vec::new(),
                            qualification_establishments: Vec::new(),
                            claims: Vec::new(),
                        },
                    },
                    provenance: Vec::new(),
                    work: Vec::new(),
                    effect: optimization_unit::EffectLink {
                        input: 0,
                        output: 1,
                    },
                    definitions: Vec::new(),
                    uses: Vec::new(),
                    successors: Vec::new(),
                    ownership: Vec::new(),
                }],
            });
            let children = current(vec![path(&[0, 0]), path(&[0, 1])]);
            let parent = current(vec![path(&[0])]);
            for (mut left, right) in [
                (children.clone(), parent.clone()),
                (parent.clone(), children.clone()),
            ] {
                join_owned_frontier(&function, &types, &mut left, &right).unwrap();
                assert_eq!(left, parent);
            }
            for (producer, structural_type) in [(id(2), id(3)), (id(1), id(2))] {
                let mut wrong = function.clone();
                wrong.structural_places[0].kind = StructuralPlaceKind::OperationResult {
                    producer,
                    structural_type,
                };
                let mut left = children.clone();
                assert!(join_owned_frontier(&wrong, &types, &mut left, &parent).is_none());
                assert_eq!(
                    left, children,
                    "a mismatched declaration grants no local home authority"
                );
            }
        }

        #[test]
        fn a_join_compares_missing_regions_instead_of_their_partition_spelling() {
            let (function, types, destination) = context();
            let children = current(vec![path(&[0, 0]), path(&[0, 1])]);
            let parent = current(vec![path(&[0])]);
            for (mut first, second) in [
                (children.clone(), parent.clone()),
                (parent.clone(), children.clone()),
            ] {
                join_owned_frontier(&function, &types, &mut first, &second)
                    .expect("both arrivals have the same home and exact absent subtree");
                assert_eq!(first, parent);
                assert_eq!(
                    join_owned_frontier(&function, &types, &mut first, &second),
                    Some(false)
                );
                let use_value = ValueUse {
                    function: &function,
                    types: &types,
                    block: id(1),
                    node: 0,
                    admitted_case: &|_, _, _| false,
                };
                assert!(
                    use_value
                        .receiving_store(
                            &first,
                            &destination,
                            &path(&[0, 0]),
                            None,
                            StructuralStoreDisposition::Vacant
                        )
                        .is_err(),
                    "predecessor order cannot grant child-reconstruction authority"
                );
                use_value
                    .receiving_store(
                        &first,
                        &destination,
                        &path(&[0]),
                        None,
                        StructuralStoreDisposition::Vacant,
                    )
                    .unwrap();
                use_value
                    .install_store(
                        &mut first,
                        &destination,
                        &path(&[0]),
                        None,
                        &StructuralArgument::whole(id(2), StructuralAccess::Owned),
                        StructuralAccess::MutableBorrow,
                    )
                    .unwrap();
                assert_eq!(
                    first,
                    current(Vec::new()),
                    "common refill restores the same home in either order"
                );
            }
        }

        #[test]
        fn a_join_cannot_hide_a_live_sibling_or_change_its_home_authority() {
            let (function, types, _) = context();
            let parent = current(vec![path(&[0])]);
            let children = current(vec![path(&[0, 0]), path(&[0, 1])]);
            let half = current(vec![path(&[0, 0])]);
            let sibling = current(vec![path(&[1])]);
            let mut ended = children.clone();
            ended.owned_places.clear();
            for other in [half, sibling, ended] {
                let mut held = parent.clone();
                assert!(join_owned_frontier(&function, &types, &mut held, &other).is_none());
                assert_eq!(held, parent, "a rejected join changes no custody");
            }
            for linear in [false, true] {
                let mut unavailable = types.clone();
                if linear {
                    unavailable[2].owns_linear_claim = true;
                } else {
                    unavailable[2].no_code_affine_cleanup = false;
                }
                let mut held = children.clone();
                assert!(
                    join_owned_frontier(&function, &unavailable, &mut held, &parent).is_none(),
                    "region equivalence does not authorize linear or custom cleanup"
                );
                assert_eq!(held, children);
            }
        }

        #[test]
        fn vacancy_keeps_the_admitted_case_context_when_only_common_fields_moved() {
            let (_, mut types, _) = context();
            let field = TerminalIdentityIndex(10);
            let mut mixed = types[0].clone();
            mixed.id = id(4);
            mixed.identity = TerminalIdentityIndex(4);
            mixed.shape = StructuralTypeShape::Mixed {
                fields: vec![terminal_psi::StructuralFieldDeclaration {
                    inline_carrier: None,
                    id: id(1),
                    identity: field,
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: terminal_psi::StructuralFieldType::Structural(id(1)),
                    storage: None,
                }],
                cases: [1, 2]
                    .map(|raw| terminal_psi::StructuralCaseDeclaration {
                        id: id(raw),
                        identity: TerminalIdentityIndex(10 + raw as u32),
                        fields: Vec::new(),
                    })
                    .to_vec(),
            };
            types.push(mixed);
            let missing = BTreeSet::from([vec![StructuralPathSegment::Field(field)]]);
            let vacant = |admitted: &dyn Fn(&[StructuralPathSegment], StructuralCaseId) -> bool| {
                terminal_semantics::structural_region_is_vacant(
                    &types,
                    id(4),
                    &missing,
                    &[],
                    admitted,
                )
            };
            assert!(
                !vacant(&|_, _| false),
                "an absent payload is not evidence of its case"
            );
            assert!(vacant(&|path, case| path.is_empty() && case == id(1)));
            assert!(
                !vacant(&|_, _| true),
                "two admitted cases do not select one payload"
            );
        }

        #[test]
        fn a_receiving_region_needs_full_vacancy_and_an_allocated_home() {
            let (function, types, destination) = context();
            let use_value = ValueUse {
                function: &function,
                types: &types,
                block: id(1),
                node: 0,
                admitted_case: &|_, _, _| false,
            };
            let vacant = StructuralStoreDisposition::Vacant;
            let replace = StructuralStoreDisposition::ReplaceDiscardable;
            let check = |current: &CurrentOwnership, region: &[u64], mode| {
                use_value.receiving_store(current, &destination, &path(region), None, mode)
            };
            let complete = current(vec![path(&[0, 0]), path(&[0, 1])]);
            assert!(
                check(&complete, &[0], vacant).is_ok(),
                "both children cover the receiving subtree"
            );
            assert!(check(&complete, &[0], replace).is_err());
            assert!(
                check(&complete, &[1], vacant).is_err(),
                "a missing sibling is not the receiving region"
            );
            assert!(check(&complete, &[1], replace).is_ok());
            assert!(
                check(&current(vec![path(&[0, 0])]), &[0], vacant).is_err(),
                "one child remains live"
            );
            assert!(
                check(&current(vec![path(&[0])]), &[0, 0], vacant).is_err(),
                "an ancestor hole cannot lose its other missing children"
            );
            let mut ended = current(Vec::new());
            assert!(
                check(&ended, &[0], vacant).is_err(),
                "occupied storage is not vacant"
            );
            ended.owned_places.clear();
            assert!(check(&ended, &[0], vacant).is_err());
            assert!(
                check(&ended, &[0], replace).is_err(),
                "a declaration cannot recreate an ended home"
            );
        }

        #[test]
        fn installing_a_subtree_preserves_other_holes_and_rejoins_its_home() {
            let (function, types, destination) = context();
            let use_value = ValueUse {
                function: &function,
                types: &types,
                block: id(1),
                node: 0,
                admitted_case: &|_, _, _| false,
            };
            let mut frontier = current(vec![path(&[0]), path(&[1])]);
            frontier.owned_places.clear(); // All owned members moved out; the home remains allocated.
            use_value
                .receiving_store(
                    &frontier,
                    &destination,
                    &path(&[0]),
                    None,
                    StructuralStoreDisposition::Vacant,
                )
                .unwrap();
            use_value
                .install_store(
                    &mut frontier,
                    &destination,
                    &path(&[0]),
                    None,
                    &StructuralArgument::whole(id(2), StructuralAccess::Owned),
                    StructuralAccess::MutableBorrow,
                )
                .unwrap();
            assert_eq!(
                frontier.owned_places.get(&id(1)),
                Some(&StructuralMultiplicity::Affine)
            );
            assert_eq!(
                frontier.partial_custody_paths.get(&id(1)),
                Some(&BTreeSet::from([path(&[1])]))
            );
            assert!(use_value.observing(&frontier, id(1), &path(&[0])).is_ok());
            assert!(use_value.observing(&frontier, id(1), &path(&[1])).is_err());
            assert!(use_value.observing(&frontier, id(1), &[]).is_err());
            let empty = current(vec![Vec::new()]);
            assert!(
                uninstalled_home(&empty, id(1)),
                "a vacant affine local has no invented value"
            );
        }
    }
}
pub(crate) use structural::{
    constructor_arguments, partial_affine_root_is_drained, partial_affine_root_type,
    projected_paths_are_fully_consumed,
};

pub(crate) use block_parameters::parameter_establishment_order;
use block_parameters::{consume_edge_sources, establish_edge_parameters};

pub(super) fn validate_current_ownership_frontier_collecting(
    unit: &optimization_unit::PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    blocks: &BTreeMap<BlockId, &OptimizationBlock>,
    predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
    functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
    boundary_machines: &BTreeMap<
        semantic_vocabulary::BoundaryMachineId,
        &BoundaryMachineDeclaration,
    >,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    call_accesses: &mut CallAccesses,
    facts: &mut Vec<OwnershipFrontierFact>,
) -> Result<
    Vec<terminal_semantics::descriptor_results::DescriptorCompatibilityRequirement>,
    OptimizationUnitValidationError,
> {
    let entry = reconstruct_entry_ownership(function, structural_types)?;
    replay::validate_current_ownership_cfg(
        function,
        unit,
        blocks,
        predecessors,
        functions,
        boundary_machines,
        structural_types,
        call_accesses,
        entry,
        facts,
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LiveClaim {
    input: Option<PlaceId>,
    path: Vec<StructuralPathSegment>,
    multiplicity: Option<StructuralMultiplicity>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CurrentOwnership {
    claims: BTreeMap<ClaimId, LiveClaim>,
    owned_places: BTreeMap<PlaceId, StructuralMultiplicity>,
    partial_custody_paths: BTreeMap<PlaceId, BTreeSet<Vec<StructuralPathSegment>>>,
    live_references: Vec<LiveReference>,
}

impl CurrentOwnership {
    fn snapshot(&self) -> OwnershipFrontierSnapshot {
        OwnershipFrontierSnapshot {
            claims: self
                .claims
                .iter()
                .map(|(claim, live)| OwnershipFrontierLiveClaim {
                    claim: *claim,
                    input: live.input,
                    path: live.path.clone(),
                    multiplicity: live.multiplicity,
                })
                .collect(),
            owned_places: self
                .owned_places
                .iter()
                .map(|(place, multiplicity)| OwnershipFrontierOwnedPlace {
                    place: *place,
                    multiplicity: *multiplicity,
                })
                .collect(),
            partial_custody: self
                .partial_custody_paths
                .iter()
                .map(|(place, moved_paths)| OwnershipFrontierPartialCustody {
                    place: *place,
                    moved_paths: moved_paths.iter().cloned().collect(),
                })
                .collect(),
        }
    }
}

fn reconstruct_entry_ownership(
    function: &PsiOptimizationFunction,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<CurrentOwnership, OptimizationUnitValidationError> {
    let mut claims = BTreeMap::<ClaimId, LiveClaim>::new();
    for claim in &function.entry_claim_declarations {
        let parameter = function
            .structural_parameter(claim.input)
            .expect("structural signature validation precedes current ownership replay");
        claims.insert(
            claim.claim,
            LiveClaim {
                input: Some(claim.input),
                path: claim.path.clone(),
                multiplicity: Some(if claim.path.is_empty() {
                    parameter.multiplicity
                } else {
                    StructuralMultiplicity::Linear
                }),
            },
        );
    }
    for claim in &function.content_entry_claims {
        let parameter = function.structural_parameter(claim.input.root);
        claims.entry(claim.claim).or_insert(LiveClaim {
            input: parameter.map(|_| claim.input.root),
            path: Vec::new(),
            multiplicity: parameter.map(|parameter| parameter.multiplicity),
        });
    }
    Ok(CurrentOwnership {
        claims,
        owned_places: function
            .structural_parameters
            .iter()
            .filter_map(|parameter| {
                (parameter.access == StructuralAccess::Owned
                    && parameter.multiplicity != StructuralMultiplicity::Unrestricted)
                    .then_some((parameter.place, parameter.multiplicity))
            })
            .collect(),
        partial_custody_paths: BTreeMap::new(),
        live_references: references::entry_references(function, structural_types)?,
    })
}

pub(crate) use address_joins::validate_compatibility as validate_descriptor_compatibility;
pub(crate) use cleanup::unrestricted_owned_cleanup_roots;
