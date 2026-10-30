//! Borrowed backing geometry reconstructed by current-program adapters.
//!
//! This module owns analysis scratch, not durable program authority. Adapters
//! supply exact source aliases and current scalar ordering; availability,
//! access, lifetime and loan closure remain independent receiving judgments.
//! All possible origins remain relevant; a relative child cannot refine an
//! inexact parent, and captured scalar coordinates never become live selectors.
use semantic_vocabulary::{BlockId, PlaceId, ValueId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::StructuralPathSegment;

pub type Position = (BlockId, u32);
pub type AliasSources = BTreeMap<PlaceId, Vec<(PlaceId, Projection)>>;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Projection {
    // Exact captured composition is distinct from conservative overlap geometry.
    address: Option<Vec<AddressStep>>,
    pub path: Vec<StructuralPathSegment>,
    pub exact: bool,
    pub indirect: bool,
    pub window: Option<(ValueId, ValueId)>,
    pub captured_at: Option<Position>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum AddressStep {
    Path(Vec<StructuralPathSegment>, Option<Position>),
    Window {
        start: ValueId,
        end: ValueId,
        at: Position,
    },
}

impl Projection {
    pub fn new(path: &[StructuralPathSegment]) -> Self {
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
            address: Some(if path.is_empty() {
                Vec::new()
            } else {
                vec![AddressStep::Path(path.to_vec(), None)]
            }),
            path: path[..bounded_length].to_vec(),
            exact: length == path.len(),
            // `resolve` discharges a leading `Referent` from the place's own
            // backing alias, so only a deeper crossing stays indirect.
            indirect: crate::crosses_unaliased_reference(path),
            window: None,
            captured_at: None,
        }
    }

    pub fn at(mut self, position: Position) -> Self {
        if let Some(steps) = &mut self.address {
            for step in steps {
                if let AddressStep::Path(path, captured @ None) = step
                    && path.iter().any(|segment| segment.runtime_index().is_some())
                {
                    *captured = Some(position);
                }
            }
        }
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

    pub fn window(start: ValueId, end: ValueId, position: Position) -> Self {
        Self::new(&[]).with_window(start, end, position)
    }

    /// Narrow the selected backing region without discarding its containing
    /// path or unresolved reference crossings. Endpoints belong to this
    /// activation's scalar namespace; they never become live path selectors.
    pub fn with_window(mut self, start: ValueId, end: ValueId, position: Position) -> Self {
        if let Some(steps) = &mut self.address {
            steps.push(AddressStep::Window {
                start,
                end,
                at: position,
            });
        }
        self.exact = false;
        self.window = Some((start, end));
        self.captured_at = Some(position);
        self
    }

    /// A callee summary has no caller scalar namespace. Preserve its static
    /// enclosing storage, but never reinterpret a callee selector or capture
    /// coordinate as an identically numbered caller value.
    pub fn without_runtime_coordinates(mut self) -> Self {
        if self.address.as_ref().is_some_and(|steps| {
            steps.iter().any(|step| match step {
                AddressStep::Window { .. } => true,
                AddressStep::Path(path, _) => {
                    path.iter().any(|segment| segment.runtime_index().is_some())
                }
            })
        }) {
            self.address = None;
        }
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

    /// Name the complete direct storage containing this access. A modular
    /// compatibility demand may require disjointness from this larger region
    /// without transporting callee-local selectors. This is not the original
    /// access geometry: never use it to replace a captured loan or bounds check.
    pub fn enclosing_storage(&self) -> Option<Self> {
        if self.indirect {
            return None;
        }
        let length = self
            .path
            .iter()
            .take_while(|segment| {
                matches!(
                    segment,
                    StructuralPathSegment::Field(_) | StructuralPathSegment::FixedIndex(_)
                )
            })
            .count();
        Some(Self::new(&self.path[..length]))
    }

    /// Retain conservative geometry without claiming its erased address history.
    pub fn without_captured_address(mut self) -> Self {
        self.address = None;
        self
    }

    /// A representation change preserves containing storage, not element
    /// coordinates. Neither an exact address nor a refined suffix survives it.
    pub fn forget_address(&mut self) {
        self.exact = false;
        self.address = None;
    }

    /// Resolve only a complete fixed address proved from each selector's own
    /// captured scalar snapshot. This does not prove access, bounds or lifetime.
    /// Unknown symbolic coordinates remain unknown; overlap is never equality.
    pub fn fixed_address(
        &self,
        ordering: &crate::scalar_ordering::ScalarOrdering,
    ) -> Option<Vec<StructuralPathSegment>> {
        self.reference_origin_path(ordering)?
            .into_iter()
            .map(|projection| match projection {
                terminal_psi::ReferenceOriginProjection::Exact(segment) => Some(segment),
                terminal_psi::ReferenceOriginProjection::Element => None,
            })
            .collect()
    }

    /// Describe a complete captured selection for returned-origin coverage.
    /// Unknown coordinates retain their selected collection dimension as an
    /// element family, never an executable address or proof of overlap. Windows
    /// compose with their following index; an unselected window has no projected
    /// endpoint. Conservative interference `path` cannot supply this evidence.
    pub fn reference_origin_path(
        &self,
        ordering: &crate::scalar_ordering::ScalarOrdering,
    ) -> Option<Vec<terminal_psi::ReferenceOriginProjection>> {
        use terminal_psi::ReferenceOriginProjection::{Element, Exact};
        if self.indirect {
            return None;
        }
        let mut result = Vec::new();
        // Outer None means no pending window; Some(None) retains a window
        // whose captured start is unknown. These cannot be conflated: only an
        // actual subsequent element selection consumes the latter dimension.
        let mut offset: Option<Option<u64>> = None;
        let append_offset = |offset: &mut Option<Option<u64>>, start: Option<u64>| {
            *offset = Some(match (offset.unwrap_or(Some(0)), start) {
                (Some(left), Some(right)) => Some(left.checked_add(right)?),
                _ => None,
            });
            Some(())
        };
        for step in self.address.as_ref()? {
            match step {
                AddressStep::Window { start, at, .. } => {
                    append_offset(&mut offset, ordering.known_unsigned(*at, *start))?
                }
                AddressStep::Path(path, at) => {
                    for segment in path {
                        match segment {
                            StructuralPathSegment::FixedByteRange { start, .. } => {
                                append_offset(&mut offset, Some(*start))?
                            }
                            StructuralPathSegment::FixedIndex(_)
                            | StructuralPathSegment::RuntimeIndex { .. } => {
                                let index = match segment {
                                    StructuralPathSegment::FixedIndex(index) => Some(*index),
                                    StructuralPathSegment::RuntimeIndex { index, .. } => {
                                        ordering.known_unsigned((*at)?, *index)
                                    }
                                    _ => unreachable!(),
                                };
                                result.push(match (offset.take().unwrap_or(Some(0)), index) {
                                    (Some(start), Some(index)) => {
                                        Exact(StructuralPathSegment::FixedIndex(
                                            start.checked_add(index)?,
                                        ))
                                    }
                                    _ => Element,
                                });
                            }
                            StructuralPathSegment::Field(_) | StructuralPathSegment::Case(_)
                                if offset.is_none() =>
                            {
                                result.push(Exact(segment.clone()))
                            }
                            _ => return None,
                        }
                    }
                }
            }
        }
        offset.is_none().then_some(result)
    }

    pub fn then(&self, suffix: &Self) -> Self {
        let mut composed = self.clone();
        composed.address =
            self.address
                .as_ref()
                .zip(suffix.address.as_ref())
                .and_then(|(prefix, suffix)| {
                    // Recurrent static producer coordinates do not identify one dynamic
                    // capture instance. Stop exact refinement instead of growing an
                    // unbounded address chain in the existing fixed-point replay.
                    let work = prefix.iter().chain(suffix).try_fold(0usize, |used, step| {
                        used.checked_add(match step {
                            AddressStep::Path(path, _) => path.len(),
                            AddressStep::Window { .. } => 1,
                        })
                    })?;
                    if work > 4096
                        || suffix.iter().any(|step| {
                            prefix.iter().any(|prior| match (prior, step) {
                                (
                                    AddressStep::Window { at: before, .. },
                                    AddressStep::Window { at, .. },
                                ) => before == at,
                                (
                                    AddressStep::Path(previous, Some(before)),
                                    AddressStep::Path(path, Some(at)),
                                ) if before == at => path
                                    .iter()
                                    .filter_map(StructuralPathSegment::runtime_index)
                                    .any(|selector| {
                                        previous.iter().any(|segment| {
                                            segment.runtime_index() == Some(selector)
                                        })
                                    }),
                                _ => false,
                            })
                        })
                    {
                        return None;
                    }
                    let mut steps: Vec<AddressStep> = Vec::new();
                    for step in prefix.iter().chain(suffix) {
                        if let AddressStep::Path(path, at) = step
                            && let Some(AddressStep::Path(previous, captured)) = steps.last_mut()
                            && (captured == at
                                || captured.is_none()
                                    && !previous
                                        .iter()
                                        .any(|segment| segment.runtime_index().is_some())
                                || at.is_none()
                                    && !path
                                        .iter()
                                        .any(|segment| segment.runtime_index().is_some()))
                        {
                            previous.extend_from_slice(path);
                            *captured = (*captured).or(*at);
                        } else {
                            steps.push(step.clone());
                        }
                    }
                    Some(steps)
                });
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
pub struct Region {
    pub root: Option<PlaceId>,
    pub projection: Projection,
}

impl Region {
    pub fn overlaps(&self, other: &Self) -> bool {
        self.projection.indirect
            || other.projection.indirect
            || self.root.is_none()
            || other.root.is_none()
            || (self.root == other.root
                && crate::structural_paths_may_overlap(
                    &self.projection.path,
                    &other.projection.path,
                ))
    }
}

/// Refine possible overlap only from current, all-path scalar relations.
/// Every dynamic coordinate must still denote its captured value. Unknown
/// origins, references and relative child geometry retain conservative overlap.
pub fn regions_overlap_at(
    left: &Region,
    right: &Region,
    position: Position,
    relations: &crate::scalar_ordering::ScalarOrdering,
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
    !window_excludes_index(&left.projection, &right.projection, relations, position)
        && !window_excludes_index(&right.projection, &left.projection, relations, position)
}

fn window_excludes_index(
    window: &Projection,
    selected: &Projection,
    relations: &crate::scalar_ordering::ScalarOrdering,
    position: Position,
) -> bool {
    let (Some((start, end)), Some(birth)) = (window.window, window.captured_at) else {
        return false;
    };
    use crate::scalar_ordering::Term;
    let index = match selected.path.get(window.path.len()) {
        Some(StructuralPathSegment::FixedIndex(index)) => Term::Unsigned(*index),
        Some(StructuralPathSegment::RuntimeIndex { index, .. })
            if selected.captured_at.is_some_and(|captured| {
                relations.values_unchanged_since(captured, position, &[*index])
            }) =>
        {
            Term::Value(*index)
        }
        _ => return false,
    };
    if selected.path[..window.path.len()] != window.path
        || !relations.values_unchanged_since(birth, position, &[start, end])
    {
        return false;
    }
    relations.proves(position, index, Term::Value(start), true)
        || relations.proves(position, Term::Value(end), index, false)
}

pub fn resolve(aliases: &AliasSources, root: PlaceId, projection: Projection) -> Vec<Region> {
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
        let key = (place, projection.clone());
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
