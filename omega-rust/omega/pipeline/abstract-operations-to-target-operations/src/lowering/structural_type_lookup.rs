//! One prepared module lookup and shared declaration catalog for all functions.

use std::collections::BTreeMap;
use std::ops::Deref;

use abstract_operations::StructuralTypeCatalog;
use semantic_vocabulary::StructuralTypeId;
use terminal_psi::StructuralTypeDeclaration;

pub(crate) struct StructuralTypeLookup<'a> {
    declarations: BTreeMap<StructuralTypeId, &'a StructuralTypeDeclaration>,
    catalog: StructuralTypeCatalog,
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
            declarations.values().map(|row| (*row).clone()).collect()
        };
        Self {
            declarations,
            catalog,
        }
    }

    pub(super) fn catalog(&self) -> &StructuralTypeCatalog {
        &self.catalog
    }
}

impl<'a> Deref for StructuralTypeLookup<'a> {
    type Target = BTreeMap<StructuralTypeId, &'a StructuralTypeDeclaration>;

    fn deref(&self) -> &Self::Target {
        &self.declarations
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terminal_psi::StructuralTypeShape;

    fn declaration(number: u64, identity: &str) -> StructuralTypeDeclaration {
        StructuralTypeDeclaration {
            id: StructuralTypeId::new(number).unwrap(),
            identity: identity.into(),
            shape: StructuralTypeShape::PrimitiveScalar(semantic_vocabulary::ScalarType::Boolean),
        }
    }

    #[test]
    fn canonical_catalog_is_retained_without_copying() {
        let source: StructuralTypeCatalog =
            vec![declaration(1, "first"), declaration(2, "second")].into();
        let lookup = StructuralTypeLookup::new(&source);
        assert!(lookup.catalog().shares_storage_with(&source));
        assert!(std::ptr::eq(
            *lookup.get(&source[0].id).unwrap(),
            &source[0]
        ));
    }

    #[test]
    fn unchecked_catalog_keeps_the_existing_ordered_last_row_projection() {
        let source: StructuralTypeCatalog = vec![
            declaration(2, "old"),
            declaration(1, "first"),
            declaration(2, "last"),
        ]
        .into();
        let lookup = StructuralTypeLookup::new(&source);
        assert!(!lookup.catalog().shares_storage_with(&source));
        assert_eq!(
            lookup.catalog().as_slice(),
            &[declaration(1, "first"), declaration(2, "last")]
        );
        assert_eq!(source.len(), 3);
        assert!(
            lookup
                .catalog()
                .shares_storage_with(&lookup.catalog().clone())
        );
    }
}
