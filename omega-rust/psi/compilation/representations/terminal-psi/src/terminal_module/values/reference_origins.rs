//! A returned reference describes a relation to formal input storage, not an
//! address computation. An element family avoids enumerating a fixed array and
//! also describes runtime-length views. The concrete returned descriptor still
//! owns its captured selector, checked bounds and loan lineage.
//!
//! Keep this vocabulary separate from executable structural arguments: a family
//! projection must never become a wildcard load, store or loan formation.

use super::{StructuralArgument, StructuralPathSegment};
use crate::StructuralAccess;
use semantic_vocabulary::PlaceId;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ReferenceOriginProjection {
    /// A declaration-relative projection. Catalog validation excludes runtime
    /// selectors and call-scoped byte windows even though executable paths use
    /// the same declaration vocabulary for their other steps.
    Exact(StructuralPathSegment),
    /// One element of the selected collection. This states possible provenance,
    /// not that the collection is nonempty or that any access is in bounds.
    Element,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReferenceOriginSource {
    pub place: PlaceId,
    pub path: Vec<ReferenceOriginProjection>,
    pub extent: semantic_vocabulary::ReferenceOriginExtent,
    pub access: StructuralAccess,
}

impl From<StructuralArgument> for ReferenceOriginSource {
    fn from(argument: StructuralArgument) -> Self {
        Self {
            place: argument.place,
            extent: semantic_vocabulary::ReferenceOriginExtent::Projected,
            path: argument
                .path
                .into_iter()
                .map(ReferenceOriginProjection::Exact)
                .collect(),
            access: argument.access,
        }
    }
}

impl ReferenceOriginSource {
    /// Exact origins can be resolved as ordinary projections. A family has no
    /// single address and must instead be joined to a captured returned loan.
    pub fn exact_argument(&self) -> Option<StructuralArgument> {
        if self.extent != semantic_vocabulary::ReferenceOriginExtent::Projected {
            return None;
        }
        Some(StructuralArgument {
            place: self.place,
            path: self
                .path
                .iter()
                .map(|projection| match projection {
                    ReferenceOriginProjection::Exact(segment) => Some(segment.clone()),
                    ReferenceOriginProjection::Element => None,
                })
                .collect::<Option<Vec<_>>>()?,
            access: self.access,
        })
    }
}
