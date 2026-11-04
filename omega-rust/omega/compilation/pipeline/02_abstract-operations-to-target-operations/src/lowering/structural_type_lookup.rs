//! One prepared module lookup and shared declaration catalog for all functions.

use std::ops::Deref;

use abstract_operations::StructuralTypeCatalog;
use calling_conventions::terminal_layout::StructuralLayouts;
use semantic_vocabulary::{NonDenseIdentities, StructuralTypeId};
use terminal_psi::StructuralPathSegment;

pub(crate) struct StructuralTypeLookup<'a> {
    layouts: StructuralLayouts<'a>,
    catalog: &'a StructuralTypeCatalog,
}

impl<'a> StructuralTypeLookup<'a> {
    /// Every structural type's native shape is computed here, once per module.
    pub(crate) fn new(source: &'a StructuralTypeCatalog) -> Result<Self, NonDenseIdentities> {
        Ok(Self {
            layouts: StructuralLayouts::new(source.iter())?,
            catalog: source,
        })
    }

    pub(super) fn catalog(&self) -> &StructuralTypeCatalog {
        self.catalog
    }

    /// Resolve storage geometry, not canonical ownership. Inline descriptors
    /// retain their owning field and access; a case namespace does not change
    /// their geometry into owned backing. Bounds and disposal authority belong
    /// to the consuming operation. Reference crossings remain excluded here.
    pub(super) fn subtree(
        &self,
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
    ) -> Option<StructuralTypeId> {
        if path.iter().any(|segment| {
            !matches!(
                segment,
                StructuralPathSegment::Field(_)
                    | StructuralPathSegment::FixedIndex(_)
                    | StructuralPathSegment::RuntimeIndex { .. }
                    | StructuralPathSegment::Case(_)
            )
        }) {
            return None;
        }
        terminal_semantics::runtime_structural_path_tip(self.layouts.values().copied(), root, path)
            .map(|declaration| declaration.id)
    }
}

impl<'a> Deref for StructuralTypeLookup<'a> {
    type Target = StructuralLayouts<'a>;

    fn deref(&self) -> &Self::Target {
        &self.layouts
    }
}
