//! Possible backing regions shared by address joins and descriptor loans.
//!
//! Reconstruct the borrowed alias graph once. Every possible origin can cause
//! interference; only exact prefixes compose with later projections. Unknown
//! references, dynamic selectors and unresolved cycles remain conservative.

use super::range_relations::RangeRelations;
use abstract_operations::AbstractOperation as O;
use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::{BlockId, PlaceId, StructuralTypeId, ValueId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{StructuralAccess, StructuralPathSegment, StructuralTypeDeclaration};

pub(in crate::current_ownership) type Position = (BlockId, u32);
type AliasSources = BTreeMap<PlaceId, Vec<(PlaceId, Projection)>>;

#[derive(Clone)]
pub(in crate::current_ownership) struct Projection {
    pub(in crate::current_ownership) path: Vec<StructuralPathSegment>,
    pub(in crate::current_ownership) exact: bool,
    pub(in crate::current_ownership) indirect: bool,
    pub(super) window: Option<(ValueId, ValueId)>,
    pub(super) captured_at: Option<Position>,
}

impl Projection {
    pub(in crate::current_ownership) fn new(path: &[StructuralPathSegment]) -> Self {
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
                Some(
                    StructuralPathSegment::FixedByteRange { .. }
                        | StructuralPathSegment::RuntimeIndex { .. }
                )
            ));
        Self {
            path: path[..bounded_length].to_vec(),
            exact: length == path.len(),
            indirect: path.contains(&StructuralPathSegment::Referent),
            window: None,
            captured_at: None,
        }
    }

    pub(super) fn at(mut self, position: Position) -> Self {
        if self.window.is_some()
            || self
                .path
                .iter()
                .any(|segment| segment.runtime_index().is_some())
        {
            self.captured_at = Some(position);
        }
        self
    }

    pub(super) fn window(start: ValueId, end: ValueId, position: Position) -> Self {
        Self {
            path: Vec::new(),
            exact: false,
            indirect: false,
            window: Some((start, end)),
            captured_at: Some(position),
        }
    }

    /// A callee summary has no caller scalar namespace. Preserve its static
    /// enclosing storage, but never reinterpret a callee selector or capture
    /// coordinate as an identically numbered caller value.
    pub(in crate::current_ownership) fn without_runtime_coordinates(mut self) -> Self {
        if let Some(position) = self
            .path
            .iter()
            .position(|segment| matches!(segment, StructuralPathSegment::RuntimeIndex { .. }))
        {
            self.path.truncate(position);
            self.exact = false;
        }
        self.window = None;
        self.captured_at = None;
        self
    }

    pub(in crate::current_ownership) fn then(&self, suffix: &Self) -> Self {
        let mut composed = self.clone();
        composed.indirect |= suffix.indirect;
        if composed.exact {
            composed.path.extend_from_slice(&suffix.path);
            composed.exact = suffix.exact;
            composed.window = suffix.window;
            composed.captured_at = suffix.captured_at;
        }
        composed
    }
}

#[derive(Clone)]
pub(in crate::current_ownership) struct Region {
    pub(in crate::current_ownership) root: Option<PlaceId>,
    pub(in crate::current_ownership) projection: Projection,
}

impl Region {
    pub(in crate::current_ownership) fn overlaps(&self, other: &Self) -> bool {
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

#[derive(Default)]
pub(in crate::current_ownership) struct BorrowedRegions {
    aliases: AliasSources,
    relations: Option<RangeRelations>,
    writes: BTreeMap<Position, Vec<(PlaceId, Vec<Region>)>>,
}

impl BorrowedRegions {
    pub(in crate::current_ownership) fn new(
        function: &PsiOptimizationFunction,
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        call_accesses: &crate::current_ownership::CallAccesses,
    ) -> Self {
        let mut result = Self::origins(function);
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
            .then(|| RangeRelations::new(function));
        result.writes = writes;
        result
    }

    pub(in crate::current_ownership) fn origins(function: &PsiOptimizationFunction) -> Self {
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
                    _ => None,
                };
                if let Some((result, source, projection)) = descriptor {
                    aliases
                        .entry(result)
                        .or_default()
                        .push((source, projection));
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
            relations: None,
            writes: BTreeMap::new(),
        }
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

    /// Refine possible overlap only from current, all-path scalar relations.
    /// Captured descriptor coordinates must still denote their original values;
    /// a later loop iteration cannot reuse their names as fresh evidence.
    pub(super) fn regions_overlap_at(
        &self,
        left: &Region,
        right: &Region,
        position: Position,
    ) -> bool {
        if !left.overlaps(right) {
            return false;
        }
        if left.projection.indirect
            || right.projection.indirect
            || left.root.is_none()
            || left.root != right.root
        {
            return true;
        }
        let Some(relations) = &self.relations else {
            return true;
        };
        !window_excludes_index(&left.projection, &right.projection, relations, position)
            && !window_excludes_index(&right.projection, &left.projection, relations, position)
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

fn window_excludes_index(
    window: &Projection,
    selected: &Projection,
    relations: &RangeRelations,
    position: Position,
) -> bool {
    let (Some((start, end)), Some(birth)) = (window.window, window.captured_at) else {
        return false;
    };
    let Some(StructuralPathSegment::RuntimeIndex { index, .. }) =
        selected.path.get(window.path.len())
    else {
        return false;
    };
    if selected.path[..window.path.len()] != window.path
        || !relations.values_unchanged_since(birth, position, &[start, end])
        || selected
            .captured_at
            .is_none_or(|captured| !relations.values_unchanged_since(captured, position, &[*index]))
    {
        return false;
    }
    relations.proves_less_than(position, *index, start)
        || relations.proves_less_or_equal(position, end, *index)
}

fn resolve(aliases: &AliasSources, root: PlaceId, projection: Projection) -> Vec<Region> {
    let mut result = Vec::new();
    let mut pending = vec![(root, projection, BTreeSet::new())];
    // A pair expands once: alias diamonds reach the same (place, projection)
    // through every path and would re-emit identical subtrees. Per-path
    // `visited` stays the cycle cut — a cycle whose projection drifts each
    // lap produces distinct pairs, and only the path-local cut ends it.
    let mut expanded = BTreeSet::new();
    while let Some((place, projection, mut visited)) = pending.pop() {
        if !visited.insert(place) {
            continue;
        }
        let key = (
            place,
            projection.path.clone(),
            projection.exact,
            projection.indirect,
            projection.window,
            projection.captured_at,
        );
        if !expanded.insert(key) {
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

#[cfg(test)]
mod tests;
