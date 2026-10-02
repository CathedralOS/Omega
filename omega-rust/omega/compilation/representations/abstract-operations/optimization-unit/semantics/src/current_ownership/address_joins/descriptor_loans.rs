//! Shared descriptors freeze their projected backing for their observable life.
//!
//! A descriptor freshly reborrowed from an exclusive state arrival is usable;
//! that does not permit its parent to mutate the same storage while the child
//! remains observable. Reconstruct the interval from current operations and
//! exact descriptor forwarding, including every possible child at a join.
//! Instruction positions matter at both ends: a preceding write did not touch
//! a live child, and a following write cannot disturb a completed scalar read.
//! Unknown escapes conservatively retain the loan until the activation ends.
//! Possible origins establish interference only, never permission or equality.

use abstract_operations::AbstractOperation as O;
use optimization_unit::{OptimizationEdge, PsiOptimizationFunction};
use semantic_vocabulary::{BlockId, PlaceId, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{StructuralAccess, StructuralPathSegment, StructuralTypeDeclaration};

type Position = (BlockId, u32);
type AliasSources = BTreeMap<PlaceId, Vec<(PlaceId, Projection)>>;

#[derive(Clone)]
struct Projection {
    path: Vec<StructuralPathSegment>,
    exact: bool,
    indirect: bool,
}

impl Projection {
    fn new(path: &[StructuralPathSegment]) -> Self {
        let length = path
            .iter()
            .position(|segment| {
                !matches!(
                    segment,
                    StructuralPathSegment::Field(_) | StructuralPathSegment::FixedIndex(_)
                )
            })
            .unwrap_or(path.len());
        // A static window bounds its backing, but later indices are relative
        // to the view. Keep the bound without appending that relative suffix.
        let bounded_length = length
            + usize::from(matches!(
                path.get(length),
                Some(StructuralPathSegment::FixedByteRange { .. })
            ));
        Self {
            path: path[..bounded_length].to_vec(),
            exact: length == path.len(),
            indirect: path.contains(&StructuralPathSegment::Referent),
        }
    }

    fn then(&self, suffix: &Self) -> Self {
        let mut composed = self.clone();
        composed.indirect |= suffix.indirect;
        if composed.exact {
            composed.path.extend_from_slice(&suffix.path);
            composed.exact = suffix.exact;
        }
        composed
    }
}

#[derive(Clone)]
struct Region {
    root: Option<PlaceId>,
    projection: Projection,
}

impl Region {
    fn overlaps(&self, other: &Self) -> bool {
        self.projection.indirect
            || other.projection.indirect
            || self.root.is_none()
            || other.root.is_none()
            || (self.root == other.root
                && terminal_semantics::structural_paths_may_overlap(
                    &self.projection.path,
                    &other.projection.path,
                ))
    }
}

struct DescriptorLoan {
    regions: Vec<Region>,
    live: BTreeSet<Position>,
}

#[derive(Default)]
pub(super) struct DescriptorLoans {
    loans: Vec<DescriptorLoan>,
    aliases: AliasSources,
    writes: BTreeMap<Position, Vec<(PlaceId, Vec<Region>)>>,
}

impl DescriptorLoans {
    pub(super) fn new(
        function: &PsiOptimizationFunction,
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    ) -> Self {
        let mut aliases = AliasSources::new();
        let mut births = Vec::new();
        for block in &function.blocks {
            for (index, node) in block.nodes.iter().enumerate() {
                let position = (
                    block.id,
                    u32::try_from(index).expect("unit node index fits u32"),
                );
                let descriptor = match &node.operation {
                    O::EstablishElementView { result, source, .. } => Some((
                        result.place,
                        source.place,
                        Projection::new(&source.path),
                        source.access == StructuralAccess::SharedBorrow,
                    )),
                    O::ElementViewSubslice { result, source, .. }
                    | O::ByteSequenceSubslice { result, source, .. } => Some((
                        result.place,
                        *source,
                        // Runtime slice endpoints cannot refine the parent footprint.
                        Projection {
                            path: Vec::new(),
                            exact: false,
                            indirect: false,
                        },
                        true,
                    )),
                    _ => None,
                };
                if let Some((result, source, projection, shared)) = descriptor {
                    aliases
                        .entry(result)
                        .or_default()
                        .push((source, projection.clone()));
                    if shared {
                        births.push((position, result, source, projection));
                    }
                }
                for edge in &node.successors {
                    for binding in &edge.structural_bindings {
                        if binding.argument.access != StructuralAccess::Owned {
                            aliases.entry(binding.parameter).or_default().push((
                                binding.argument.place,
                                Projection::new(&binding.argument.path),
                            ));
                        }
                    }
                }
            }
        }
        if births.is_empty() {
            return Self::default();
        }
        let (successors, predecessors) = position_graph(function);
        let mut loans = Vec::with_capacity(births.len());
        for (birth, descriptor, source, projection) in births {
            let names = descendants(descriptor, &aliases);
            let after_birth = reachable(
                successors.get(&birth).into_iter().flatten().copied(),
                &successors,
                None,
            );
            // In a loop, an older descriptor may remain in a carried alias
            // when its producer runs again. Without a complete instance-end
            // proof, reformation cannot retire that earlier cohort.
            let stop = (!after_birth.contains(&birth)).then_some(birth);
            let mut observations = BTreeSet::new();
            let mut escaped = BTreeSet::new();
            for block in &function.blocks {
                if block
                    .structural_parameters
                    .iter()
                    .any(|parameter| names.contains(&parameter.place))
                {
                    observations.insert((block.id, 0));
                }
                for (index, node) in block.nodes.iter().enumerate() {
                    let position = (
                        block.id,
                        u32::try_from(index).expect("unit node index fits u32"),
                    );
                    if position == birth {
                        continue;
                    }
                    if crate::unit_validation::function_structure::operation_place_inputs(
                        &node.operation,
                    )
                    .iter()
                    .any(|place| names.contains(place))
                    {
                        observations.insert(position);
                        if !known_descriptor_use(&node.operation) {
                            escaped.insert(position);
                        }
                    }
                }
            }
            let mut live = reachable(observations, &predecessors, stop);
            // A path reaching the same code before any formation has no child.
            live.retain(|position| after_birth.contains(position));
            // Reformation cannot end an unknown retained alias from an earlier
            // loop iteration. Its pin survives through the whole activation.
            live.extend(
                reachable(escaped, &successors, None)
                    .into_iter()
                    .filter(|position| after_birth.contains(position)),
            );
            loans.push(DescriptorLoan {
                regions: resolve(&aliases, source, projection),
                live,
            });
        }
        let mut writes = BTreeMap::new();
        for block in &function.blocks {
            for (index, node) in block.nodes.iter().enumerate() {
                let regions = super::written_or_exclusive_regions(function, types, &node.operation)
                    .into_iter()
                    .map(|(place, path)| (place, resolve(&aliases, place, Projection::new(&path))))
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
        Self {
            loans,
            aliases,
            writes,
        }
    }

    pub(super) fn conflicted_operation(
        &self,
        _function: &PsiOptimizationFunction,
        block: BlockId,
        node: u32,
        operation: &O,
        moved: &[PlaceId],
    ) -> Option<PlaceId> {
        let position = (block, node);
        // Normal return cleanup closes the activation after its scalar result
        // is observed. It is not a move while an escaped child can still run.
        let moved = if matches!(operation, O::Return { .. } | O::ReturnUnit { .. }) {
            &[][..]
        } else {
            moved
        };
        let moved = moved
            .iter()
            .map(|place| (*place, resolve(&self.aliases, *place, Projection::new(&[]))))
            .collect::<Vec<_>>();
        for loan in self
            .loans
            .iter()
            .filter(|loan| loan.live.contains(&position))
        {
            for (place, regions) in self
                .writes
                .get(&position)
                .into_iter()
                .flatten()
                .chain(&moved)
            {
                if regions
                    .iter()
                    .any(|region| loan.regions.iter().any(|backing| backing.overlaps(region)))
                {
                    return Some(*place);
                }
            }
        }
        None
    }

    pub(super) fn conflicted_edge(
        &self,
        _function: &PsiOptimizationFunction,
        block: BlockId,
        node: u32,
        edge: &OptimizationEdge,
        destination: &[terminal_psi::StructuralParameterDeclaration],
    ) -> Option<PlaceId> {
        for loan in &self.loans {
            if !loan.live.contains(&(block, node)) || !loan.live.contains(&(edge.target, 0)) {
                continue;
            }
            let disturbed = edge
                .structural_bindings
                .iter()
                .zip(destination)
                .filter(|(binding, parameter)| match binding.argument.access {
                    StructuralAccess::SharedBorrow => false,
                    StructuralAccess::Owned => {
                        parameter.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
                    }
                    StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow => true,
                })
                .map(|(binding, _)| {
                    (
                        binding.argument.place,
                        Projection::new(&binding.argument.path),
                    )
                })
                .chain(
                    edge.trivial_affine_discards
                        .iter()
                        .map(|place| (*place, Projection::new(&[]))),
                )
                .chain(
                    edge.residual_affine_discards
                        .iter()
                        .map(|discard| (discard.place, Projection::new(&[]))),
                );
            for (place, projection) in disturbed {
                if resolve(&self.aliases, place, projection)
                    .iter()
                    .any(|region| loan.regions.iter().any(|backing| backing.overlaps(region)))
                {
                    return Some(place);
                }
            }
        }
        None
    }
}

fn descendants(root: PlaceId, aliases: &AliasSources) -> BTreeSet<PlaceId> {
    let mut names = BTreeSet::from([root]);
    loop {
        let before = names.len();
        for (name, sources) in aliases {
            if sources.iter().any(|(source, _)| names.contains(source)) {
                names.insert(*name);
            }
        }
        if names.len() == before {
            return names;
        }
    }
}

fn resolve(aliases: &AliasSources, root: PlaceId, projection: Projection) -> Vec<Region> {
    let mut result = Vec::new();
    let mut pending = vec![(root, projection, BTreeSet::new())];
    while let Some((place, projection, mut visited)) = pending.pop() {
        if !visited.insert(place) {
            continue;
        }
        match aliases.get(&place) {
            Some(sources) => {
                for (source, prefix) in sources {
                    pending.push((*source, prefix.then(&projection), visited.clone()));
                }
            }
            None => result.push(Region {
                root: Some(place),
                projection,
            }),
        }
    }
    if result.is_empty() {
        result.push(Region {
            root: None,
            projection: Projection::new(&[]),
        });
    }
    result
}

fn known_descriptor_use(operation: &O) -> bool {
    if let O::CallStructural {
        result,
        structural_arguments,
        dynamic_arguments,
        claim_transfers,
        ..
    } = operation
    {
        // The checked ordinary call cannot publish a borrowed alias through
        // a scalar/Unit result or a read-only input. Writable captures,
        // descriptor dispatch and explicit claim transfers need their own
        // retained-lineage evidence; those remain conservative escapes here.
        return matches!(
            result,
            abstract_operations::AbstractStructuralCallResult::Unit
                | abstract_operations::AbstractStructuralCallResult::Scalar(_)
        ) && structural_arguments
            .iter()
            .all(|argument| argument.access == StructuralAccess::SharedBorrow)
            && dynamic_arguments.is_empty()
            && claim_transfers.is_empty();
    }
    matches!(
        operation,
        O::ElementViewLength { .. }
            | O::ElementViewRead { .. }
            | O::ByteSequenceLength { .. }
            | O::ByteSequenceRead { .. }
            | O::EstablishElementView { .. }
            | O::ElementViewSubslice { .. }
            | O::ByteSequenceSubslice { .. }
            | O::Jump { .. }
            | O::Conditional { .. }
            | O::Return { .. }
            | O::ReturnUnit { .. }
            | O::Crash { .. }
    )
}

type PositionGraph = BTreeMap<Position, BTreeSet<Position>>;

fn position_graph(function: &PsiOptimizationFunction) -> (PositionGraph, PositionGraph) {
    let mut successors = PositionGraph::new();
    let mut predecessors = PositionGraph::new();
    for block in &function.blocks {
        for (index, node) in block.nodes.iter().enumerate() {
            let position = (
                block.id,
                u32::try_from(index).expect("unit node index fits u32"),
            );
            let targets = if node.successors.is_empty() {
                (index + 1 < block.nodes.len())
                    .then_some((block.id, position.1 + 1))
                    .into_iter()
                    .collect()
            } else {
                node.successors
                    .iter()
                    .map(|edge| (edge.target, 0))
                    .collect::<BTreeSet<_>>()
            };
            for target in &targets {
                predecessors.entry(*target).or_default().insert(position);
            }
            successors.insert(position, targets);
        }
    }
    (successors, predecessors)
}

fn reachable(
    starts: impl IntoIterator<Item = Position>,
    graph: &PositionGraph,
    stop: Option<Position>,
) -> BTreeSet<Position> {
    let mut visited = BTreeSet::new();
    let mut pending = starts.into_iter().collect::<Vec<_>>();
    while let Some(position) = pending.pop() {
        if Some(position) == stop || !visited.insert(position) {
            continue;
        }
        pending.extend(graph.get(&position).into_iter().flatten().copied());
    }
    visited
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn address_join_child_authority_requires_every_incoming_origin() {
        let [current, earlier, left, right, merged] =
            [1, 2, 3, 4, 5].map(|value| PlaceId::new(value).unwrap());
        let mut parents = BTreeMap::from([
            (left, BTreeSet::from([current])),
            (right, BTreeSet::from([current])),
            (merged, BTreeSet::from([left, right])),
        ]);
        assert!(super::super::entirely_descends_from(
            merged,
            current,
            &parents,
            &mut BTreeSet::new(),
        ));
        parents.get_mut(&right).unwrap().insert(earlier);
        assert!(!super::super::entirely_descends_from(
            merged,
            current,
            &parents,
            &mut BTreeSet::new(),
        ));
        parents.insert(right, BTreeSet::from([merged]));
        assert!(!super::super::entirely_descends_from(
            merged,
            current,
            &parents,
            &mut BTreeSet::new(),
        ));
    }
}
