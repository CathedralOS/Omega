//! One prepared module lookup and shared declaration catalog for all functions.

use std::collections::BTreeMap;
use std::ops::Deref;

use abstract_operations::StructuralTypeCatalog;
use semantic_vocabulary::StructuralTypeId;
use terminal_psi::{StructuralPathSegment, StructuralTypeDeclaration};

pub(crate) struct StructuralTypeLookup<'a> {
    declarations: BTreeMap<StructuralTypeId, &'a StructuralTypeDeclaration>,
    catalog: StructuralTypeCatalog,
    domains: Option<&'a [terminal_psi::StructuralDomainDeclaration]>,
}

impl<'a> StructuralTypeLookup<'a> {
    pub(crate) fn new(source: &'a StructuralTypeCatalog) -> Self {
        let declarations = source
            .iter()
            .map(|row| (row.id, row))
            .collect::<BTreeMap<_, _>>();
        // Preserve the existing ID-ordered projection on unchecked inputs.
        // Valid canonical input retains its storage without copying declarations.
        let catalog = if source
            .iter()
            .map(|row| row.id)
            .eq(declarations.keys().copied())
        {
            source.clone()
        } else {
            let mut ordered = source.clone();
            *ordered.make_mut() = declarations.values().map(|row| (*row).clone()).collect();
            ordered
        };
        Self {
            declarations,
            catalog,
            domains: None,
        }
    }

    pub(super) fn with_verified_domains(
        mut self,
        domains: Option<&'a [terminal_psi::StructuralDomainDeclaration]>,
    ) -> Self {
        self.domains = domains;
        self
    }

    /// Qualifications stay on the retained contracts. This classification
    /// selects ordinary byte storage; it grants neither membership nor access.
    pub(super) fn pure_byte_qualifications(
        &self,
        root: StructuralTypeId,
        whole: &[semantic_vocabulary::StructuralDomainId],
        projected: &[terminal_psi::StructuralPathQualification],
    ) -> bool {
        if whole.is_empty() && projected.is_empty() {
            return true;
        }
        self.domains.is_some_and(|domains| {
            terminal_semantics::pure_byte_qualifications(
                self.values().copied(),
                domains.iter(),
                root,
                whole,
                projected,
            )
        })
    }

    pub(super) fn catalog(&self) -> &StructuralTypeCatalog {
        &self.catalog
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
        terminal_semantics::runtime_structural_path_tip(self.values().copied(), root, path)
            .map(|declaration| declaration.id)
    }
}

impl<'a> Deref for StructuralTypeLookup<'a> {
    type Target = BTreeMap<StructuralTypeId, &'a StructuralTypeDeclaration>;

    fn deref(&self) -> &Self::Target {
        &self.declarations
    }
}
