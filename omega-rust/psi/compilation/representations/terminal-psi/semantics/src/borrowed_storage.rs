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
    pub path: Vec<StructuralPathSegment>,
    pub exact: bool,
    pub indirect: bool,
    pub window: Option<(ValueId, ValueId)>,
    pub captured_at: Option<Position>,
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
            path: path[..bounded_length].to_vec(),
            exact: length == path.len(),
            indirect: path.contains(&StructuralPathSegment::Referent),
            window: None,
            captured_at: None,
        }
    }

    pub fn at(mut self, position: Position) -> Self {
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
    pub fn without_runtime_coordinates(mut self) -> Self {
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

    pub fn then(&self, suffix: &Self) -> Self {
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
