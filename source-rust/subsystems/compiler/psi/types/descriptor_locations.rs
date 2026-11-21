//! The stored place and path of a descriptor at a specific position in a Psi machine.

use semantic_vocabulary::{BlockId, PlaceId};

pub type DescriptorPosition = (BlockId, u32);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DescriptorLocation {
    pub root: PlaceId,
    pub path: Vec<DescriptorPathSegment>,
}

pub use crate::ResolvedStructuralPathSegment as DescriptorPathSegment;

impl DescriptorLocation {
    /// Retain a captured address without translating its storage steps.
    /// A runtime selector needs its capture point; an extent is metadata.
    pub fn captured(root: PlaceId, path: &[DescriptorPathSegment]) -> Option<Self> {
        if path.iter().any(|step| {
            matches!(
                step,
                DescriptorPathSegment::RuntimeIndex { .. }
                    | DescriptorPathSegment::Extent
                    | DescriptorPathSegment::UnknownContents
            )
        }) {
            return None;
        }
        Some(Self {
            root,
            path: path.to_vec(),
        })
    }

    pub fn whole(root: PlaceId) -> Self {
        Self {
            root,
            path: Vec::new(),
        }
    }
    pub fn overlaps(&self, other: &Self) -> bool {
        self.root == other.root && may_overlap(&self.path, &other.path)
    }
    pub fn contains(&self, other: &Self) -> bool {
        self.root == other.root && other.path.starts_with(&self.path)
    }
    pub fn transported(&self, source: &Self, destination: &Self) -> Option<Self> {
        if !source.contains(self) {
            return None;
        }
        let mut path = destination.path.clone();
        path.extend_from_slice(&self.path[source.path.len()..]);
        Some(Self {
            root: destination.root,
            path,
        })
    }
}

pub fn may_overlap(left: &[DescriptorPathSegment], right: &[DescriptorPathSegment]) -> bool {
    for (left, right) in left.iter().zip(right) {
        if left == right {
            continue;
        }
        return !matches!(
            (left, right),
            (
                DescriptorPathSegment::Field(_),
                DescriptorPathSegment::Field(_)
            ) | (
                DescriptorPathSegment::FixedIndex(_),
                DescriptorPathSegment::FixedIndex(_)
            )
        );
    }
    true
}
