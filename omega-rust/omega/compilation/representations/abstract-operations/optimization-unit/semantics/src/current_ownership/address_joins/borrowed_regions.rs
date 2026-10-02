//! Possible backing regions shared by address joins and descriptor loans.
//!
//! Reconstruct the borrowed alias graph once. Every possible origin can cause
//! interference; only exact prefixes compose with later projections. Unknown
//! references, dynamic selectors and unresolved cycles remain conservative.

use abstract_operations::AbstractOperation as O;
use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::{BlockId, PlaceId, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{StructuralAccess, StructuralPathSegment, StructuralTypeDeclaration};

pub(super) type Position = (BlockId, u32);
type AliasSources = BTreeMap<PlaceId, Vec<(PlaceId, Projection)>>;

#[derive(Clone)]
pub(super) struct Projection {
    pub(super) path: Vec<StructuralPathSegment>,
    pub(super) exact: bool,
    pub(super) indirect: bool,
}

impl Projection {
    pub(super) fn new(path: &[StructuralPathSegment]) -> Self {
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
pub(super) struct Region {
    root: Option<PlaceId>,
    projection: Projection,
}

impl Region {
    pub(super) fn overlaps(&self, other: &Self) -> bool {
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
pub(super) struct BorrowedRegions {
    aliases: AliasSources,
    writes: BTreeMap<Position, Vec<(PlaceId, Vec<Region>)>>,
}

impl BorrowedRegions {
    pub(super) fn new(
        function: &PsiOptimizationFunction,
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    ) -> Self {
        let mut aliases = AliasSources::new();
        for block in &function.blocks {
            for node in &block.nodes {
                let descriptor = match &node.operation {
                    O::EstablishElementView { result, source, .. } => {
                        Some((result.place, source.place, Projection::new(&source.path)))
                    }
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
                                Projection::new(&binding.argument.path),
                            ));
                        }
                    }
                }
            }
        }
        if aliases.is_empty() {
            return Self::default();
        }
        let mut writes = BTreeMap::new();
        for block in &function.blocks {
            for (index, node) in block.nodes.iter().enumerate() {
                let regions = super::written_or_exclusive_regions(function, types, &node.operation)
                    .into_iter()
                    .map(|(place, region)| {
                        let mut projection = Projection::new(&region.path);
                        projection.indirect |= region.indirect;
                        (place, resolve(&aliases, place, projection))
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
        Self { aliases, writes }
    }

    pub(super) fn resolve(&self, place: PlaceId, projection: Projection) -> Vec<Region> {
        resolve(&self.aliases, place, projection)
    }

    pub(super) fn writes_at(&self, position: Position) -> &[(PlaceId, Vec<Region>)] {
        self.writes.get(&position).map(Vec::as_slice).unwrap_or(&[])
    }

    pub(super) fn overlaps(
        &self,
        pinned: &[Region],
        place: PlaceId,
        path: &[StructuralPathSegment],
    ) -> bool {
        self.resolve(place, Projection::new(path))
            .iter()
            .any(|region| pinned.iter().any(|loan| loan.overlaps(region)))
    }

    pub(super) fn conflicted_operation(
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
