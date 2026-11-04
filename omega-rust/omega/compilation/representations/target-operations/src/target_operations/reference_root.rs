//! The referent root of a reference descriptor, shared by target lowering,
//! which produces it, and selection's reference-custody audit, which
//! resolves the same three cases.

use semantic_vocabulary::PlaceId;
use terminal_psi::StructuralPathSegment;

/// The referent root of one reference descriptor: which storage a use reaches.
/// This shape is stated once, here, because the custody replay downstream
/// resolves the same three cases; a second spelling would let the two drift.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReferenceRoot {
    /// A mutable borrowed parameter or activation-local primitive place.
    Place(PlaceId),
    /// A projected storage path. Captured descriptor values use the separate
    /// IncomingContents namespace, not the storage occupied by this path.
    Ingress(PlaceId, Vec<StructuralPathSegment>),
    /// One permission's possible backing paths for a runtime-selected
    /// descriptor; its selected static path is carried by the pointer.
    Alternatives(Vec<crate::TargetReferenceOrigin>),
}

impl ReferenceRoot {
    /// The pointer value carried by an input slot, not the slot's own storage.
    /// Projections below it retain this captured value across later slot writes.
    pub fn incoming_contents(root: PlaceId, slot: Vec<StructuralPathSegment>) -> Self {
        Self::Alternatives(vec![crate::TargetReferenceOrigin {
            base: crate::TargetReferenceOriginBase::IncomingContents { root, slot },
            path: Vec::new(),
            extent: semantic_vocabulary::ReferenceOriginExtent::Projected,
        }])
    }

    /// Compose presentation metadata without selecting a runtime address.
    pub fn projected(&self, suffix: &[StructuralPathSegment]) -> Self {
        self.projected_family(
            &suffix
                .iter()
                .cloned()
                .map(terminal_psi::ReferenceOriginProjection::Exact)
                .collect::<Vec<_>>(),
            semantic_vocabulary::ReferenceOriginExtent::Projected,
        )
    }

    pub fn projected_family(
        &self,
        suffix: &[terminal_psi::ReferenceOriginProjection],
        extent: semantic_vocabulary::ReferenceOriginExtent,
    ) -> Self {
        if suffix.is_empty() && extent == semantic_vocabulary::ReferenceOriginExtent::Projected {
            return self.clone();
        }
        let mut origins = self.origins();
        for origin in &mut origins {
            // A child's actual descriptor supplies its projection. Its public
            // envelope remains the original input region, whose type may be a
            // collection rather than the returned record being projected.
            if origin.extent == semantic_vocabulary::ReferenceOriginExtent::Projected {
                origin.path.extend_from_slice(suffix);
                origin.extent = extent;
            }
        }
        // `exact_path` admits only `Storage`, so one exact origin names a
        // physical place. Incoming contents name a captured pointer value:
        // their slot is not the pointee's storage, so they stay families.
        if origins.len() == 1
            && let Some(path) = origins[0].exact_path()
            && let Some(root) = origins[0].base.root_place()
        {
            Self::Ingress(root, path)
        } else {
            origins.sort();
            origins.dedup();
            Self::Alternatives(origins)
        }
    }

    pub fn origins(&self) -> Vec<crate::TargetReferenceOrigin> {
        match self {
            Self::Place(root) => vec![crate::TargetReferenceOrigin {
                base: crate::TargetReferenceOriginBase::Storage { root: *root },
                extent: semantic_vocabulary::ReferenceOriginExtent::Projected,
                path: Vec::new(),
            }],
            Self::Ingress(root, path) => vec![crate::TargetReferenceOrigin {
                base: crate::TargetReferenceOriginBase::Storage { root: *root },
                extent: semantic_vocabulary::ReferenceOriginExtent::Projected,
                path: path
                    .iter()
                    .cloned()
                    .map(terminal_psi::ReferenceOriginProjection::Exact)
                    .collect(),
            }],
            Self::Alternatives(origins) => origins.clone(),
        }
    }

    pub fn contains_place(&self, place: PlaceId) -> bool {
        match self {
            Self::Place(root) | Self::Ingress(root, _) => *root == place,
            Self::Alternatives(origins) => origins
                .iter()
                .any(|origin| origin.base.root_place() == Some(place)),
        }
    }
}
