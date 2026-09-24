//! One prepared module lookup and shared declaration catalog for all functions.

use std::collections::BTreeMap;
use std::ops::Deref;

use abstract_operations::StructuralTypeCatalog;
use semantic_vocabulary::StructuralTypeId;
use terminal_psi::{
    StructuralFieldType, StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape,
};

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

    /// Resolve a verified projection path to its structural type. Field
    /// segments admit named structural children and the canonical standalone
    /// declaration of a plain leaf field; fixed-index segments stay inside
    /// declared array bounds. Every other segment or missing declaration
    /// fails closed.
    pub(super) fn subtree(
        &self,
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
    ) -> Option<StructuralTypeId> {
        let mut current = root;
        self.get(&current)?;
        for segment in path {
            let declaration = self.get(&current)?;
            current = match (segment, &declaration.shape) {
                (
                    StructuralPathSegment::Field(identity),
                    StructuralTypeShape::Record { fields },
                ) => {
                    let field = fields.iter().find(|field| {
                        field.identity == *identity && !field.relevance.is_erased()
                    })?;
                    match &field.field_type {
                        StructuralFieldType::Structural(child) => *child,
                        leaf => {
                            let shape = leaf.canonical_leaf_shape()?;
                            *self
                                .iter()
                                .find(|(_, declaration)| declaration.shape == shape)
                                .map(|(id, _)| id)?
                        }
                    }
                }
                (
                    StructuralPathSegment::FixedIndex(index),
                    StructuralTypeShape::FixedArray { element, length },
                ) if index < length => *element,
                _ => return None,
            };
        }
        Some(current)
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
    use super::{
        StructuralTypeCatalog, StructuralTypeDeclaration, StructuralTypeId, StructuralTypeLookup,
    };
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
