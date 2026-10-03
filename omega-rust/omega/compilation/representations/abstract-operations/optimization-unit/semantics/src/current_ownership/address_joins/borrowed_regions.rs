//! Possible backing regions shared by address joins and descriptor loans.
//!
//! Reconstruct the borrowed alias graph once. Every possible origin can cause
//! interference; only exact prefixes compose with later projections. Unknown
//! references, dynamic selectors and unresolved cycles remain conservative.

use super::range_relations::RangeRelations;
use abstract_operations::AbstractOperation as O;
use optimization_unit::PsiOptimizationFunction;
#[cfg(test)]
use semantic_vocabulary::{BlockId, ValueId};
use semantic_vocabulary::{MachineId, PlaceId, StructuralTypeId};
use std::collections::BTreeMap;
use terminal_psi::{StructuralAccess, StructuralPathSegment, StructuralTypeDeclaration};

use terminal_semantics::borrowed_storage::{AliasSources, resolve};
pub(in crate::current_ownership) use terminal_semantics::borrowed_storage::{
    Position, Projection, Region,
};
use terminal_semantics::descriptor_results::DescriptorCallOrigin;

#[derive(Default)]
pub(in crate::current_ownership) struct BorrowedRegions {
    aliases: AliasSources,
    returned: BTreeMap<PlaceId, Vec<DescriptorCallOrigin>>,
    relations: Option<RangeRelations>,
    writes: BTreeMap<Position, Vec<(PlaceId, Vec<Region>)>>,
}

impl BorrowedRegions {
    pub(in crate::current_ownership) fn new(
        function: &PsiOptimizationFunction,
        functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        call_accesses: &crate::current_ownership::CallAccesses,
    ) -> Self {
        let mut result = Self::origins(function, functions, types);
        let aliases = &result.aliases;
        if aliases.is_empty() {
            return result;
        }
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
                        (place, resolve(aliases, place, projection))
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
        result.relations = aliases
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
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    ) -> Self {
        // Prepare the current graph once for this receiving function. No source
        // producer annotation or stale optimization snapshot supplies origins.
        let calls =
            function
                .blocks
                .iter()
                .flat_map(|block| &block.nodes)
                .filter_map(|node| match &node.operation {
                    O::CallStructural {
                        result:
                            abstract_operations::AbstractStructuralCallResult::Structural {
                                result, ..
                            },
                        ..
                    } if result.multiplicity
                        == terminal_psi::StructuralMultiplicity::Unrestricted
                        && terminal_semantics::descriptor_results::is_shared_descriptor(
                            types.values().copied(),
                            result.structural_type,
                        ) =>
                    {
                        Some(result.place)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
        let mut returned = BTreeMap::new();
        if !calls.is_empty() {
            let declarations = types.values().copied().cloned().collect::<Vec<_>>();
            if let Some(graph) = crate::descriptor_results::optimized_catalog_graph(
                functions.values().copied(),
                &declarations,
            ) {
                for place in calls {
                    if let Some(origins) =
                        terminal_semantics::descriptor_results::reconstruct_call_origins(
                            &declarations,
                            &graph,
                            function.machine,
                            place,
                        )
                    {
                        returned.insert(place, origins);
                    }
                }
            }
        }
        let mut aliases = AliasSources::new();
        for block in &function.blocks {
            for (index, node) in block.nodes.iter().enumerate() {
                let position = (block.id, index as u32);
                let descriptor = match &node.operation {
                    O::EstablishElementView { result, source, .. } => Some((
                        result.place,
                        source.place,
                        Projection::new(&source.path).at(position),
                    )),
                    O::ElementViewSubslice {
                        result,
                        source,
                        start,
                        end,
                        ..
                    }
                    | O::ByteSequenceSubslice {
                        result,
                        source,
                        start,
                        end,
                        ..
                    } => Some((
                        result.place,
                        *source,
                        Projection::window(*start, *end, position),
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
                    aliases
                        .entry(result)
                        .or_default()
                        .push((source, projection));
                }
                if let Some((_, result)) = node.operation.structural_result()
                    && let Some(origins) = returned.get(&result.place)
                {
                    for origin in origins {
                        if let DescriptorCallOrigin::Argument { source, projection } = origin {
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
            aliases,
            returned,
            relations: None,
            writes: BTreeMap::new(),
        }
    }

    pub(super) fn call_origins(&self, result: PlaceId) -> Option<&[DescriptorCallOrigin]> {
        self.returned.get(&result).map(Vec::as_slice)
    }

    pub(super) fn direct_aliases(&self, result: PlaceId) -> Option<&[(PlaceId, Projection)]> {
        self.aliases.get(&result).map(Vec::as_slice)
    }

    pub(in crate::current_ownership) fn resolve(
        &self,
        place: PlaceId,
        projection: Projection,
    ) -> Vec<Region> {
        resolve(&self.aliases, place, projection)
    }

    pub(in crate::current_ownership) fn writes_at(
        &self,
        position: Position,
    ) -> &[(PlaceId, Vec<Region>)] {
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
                self.writes_at(position)
                    .iter()
                    .find_map(|(place, regions)| {
                        regions
                            .iter()
                            .any(|region| pinned.iter().any(|loan| loan.overlaps(region)))
                            .then_some(*place)
                    })
            })
    }
}

#[cfg(test)]
mod tests;
