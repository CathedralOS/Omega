use crate::*;

/// Physical members hidden while checking guest operations. An opaque value
/// keeps its own identity and ownership throughout the check.
#[must_use = "restore physical storage before publishing the checked module"]
pub struct OpaqueStorageShapes(Vec<OpaqueStorageShape>);

struct OpaqueStorageShape {
    index: usize,
    id: semantic_vocabulary::StructuralTypeId,
    shape: crate::StructuralTypeShape,
    restriction: Option<crate::StructuralFieldType>,
}

impl OpaqueStorageShapes {
    pub fn restore(self, module: &mut Psi) -> Result<(), &'static str> {
        if self.0.iter().any(|saved| {
            module.types.structural_types.get(saved.index).is_none_or(|row| {
                row.id != saved.id
                    || row.shape != crate::StructuralTypeShape::Opaque
                    || row.scalar_restriction.is_some()
            })
        }) {
            return Err("opaque declaration changed while checking guest operations");
        }
        for saved in self.0 {
            let row = &mut module.types.structural_types[saved.index];
            row.shape = saved.shape;
            row.scalar_restriction = saved.restriction;
        }
        Ok(())
    }
}


impl Psi {
    /// The wire retains physical storage on both members of a selected pair.
    /// The guest may transport the opaque member, but only the carrier's own
    /// type grants access to its fields. Move the physical shapes out while
    /// checking guest code so every constructor, projection and proof sees
    /// the nominal declaration, including through nested aggregates.
    pub fn take_opaque_storage_shapes(&mut self) -> Result<OpaqueStorageShapes, &'static str> {
        let mut shapes = Vec::with_capacity(self.types.opaque_representations.len());
        for relation in &self.types.opaque_representations {
            let index = self
                .types.structural_types
                .binary_search_by_key(&relation.opaque, |row| row.id)
                .map_err(|_| "opaque representation lost its storage declaration")?;
            let row = &self.types.structural_types[index];
            if shapes
                .last()
                .is_some_and(|previous: &OpaqueStorageShape| previous.index >= index)
            {
                return Err("opaque representations repeat or reorder a storage declaration");
            }
            if !row.requires.is_empty() || row.quotient_carrier.is_some() {
                return Err(
                    "opaque data cannot supply a visible carrier or authored value predicate",
                );
            }
            shapes.push(OpaqueStorageShape {
                index,
                id: row.id,
                shape: crate::StructuralTypeShape::Opaque,
                restriction: None,
            });
        }
        for saved in &mut shapes {
            let row = &mut self.types.structural_types[saved.index];
            std::mem::swap(&mut row.shape, &mut saved.shape);
            saved.restriction = row.scalar_restriction.take();
        }
        Ok(OpaqueStorageShapes(shapes))
    }
}
