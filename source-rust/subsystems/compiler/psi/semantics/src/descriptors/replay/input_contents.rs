//! The borrowed contents that an opaque call can retain from an input.
//! A shared array with a symbolic length contributes its element origins
//! without claiming that any particular index exists.

use super::*;
use crate::descriptor_lifetimes::descriptor_location_type;
use terminal_psi::StructuralParameterDeclaration;

impl DescriptorSlotKind {
    pub(crate) fn input_contents_origins(
        self,
        types: &[StructuralTypeDeclaration],
        formal: &StructuralParameterDeclaration,
        position: usize,
    ) -> Option<Vec<DescriptorStoredOrigin>> {
        let mut origins = Vec::new();
        let mut pending = vec![(formal.structural_type, Vec::new(), Vec::new())];
        while let Some((ty, prefix, mut ancestors)) = pending.pop() {
            if ancestors.contains(&ty) {
                return None;
            }
            ancestors.push(ty);
            let inventory =
                slot_inventory(self, DescriptorStorageScope::Owned, types, formal.place, ty)?;
            let mut append = |path: Vec<DescriptorPathSegment>| {
                origins.push(if prefix.is_empty() {
                    DescriptorStoredOrigin::IncomingSlot { position, path }
                } else {
                    let mut steps = prefix.clone();
                    steps.extend(path.into_iter().map(DescriptorContentsStep::Address));
                    DescriptorStoredOrigin::IncomingContents {
                        position,
                        steps,
                        projection: Projection::new(&[]),
                    }
                });
            };
            for slot in inventory.slots {
                append(slot.path);
            }
            for unknown in inventory.unknown_contents {
                let mut path = unknown.location.path;
                if unknown.kind != UnknownContentsKind::SymbolicArray {
                    append(path);
                    continue;
                }
                if formal.access != StructuralAccess::SharedBorrow
                    || path.pop()? != DescriptorPathSegment::UnknownContents
                {
                    return None;
                }
                let array = descriptor_location_type(types, ty, &path)?;
                let StructuralTypeShape::FixedArray { element, .. } =
                    find_structural_type(types, array)?.shape
                else {
                    return None;
                };
                let mut steps = prefix.clone();
                steps.extend(path.into_iter().map(DescriptorContentsStep::Address));
                steps.push(DescriptorContentsStep::any_element(element));
                pending.push((element, steps, ancestors.clone()));
            }
            if origins.len().checked_add(pending.len())? > 4096 {
                return None;
            }
        }
        Some(origins)
    }
}
