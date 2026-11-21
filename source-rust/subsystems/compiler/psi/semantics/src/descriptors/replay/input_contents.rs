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
                return { eprintln!("ARRAY STOP {}:{}", file!(), line!()); None };
            }
            ancestors.push(ty);
            let inventory =
                slot_inventory(self, DescriptorStorageScope::Owned, types, formal.place, ty).array_trace(line!())?;
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
                    || path.pop().array_trace(line!())? != DescriptorPathSegment::UnknownContents
                {
                    return { eprintln!("ARRAY STOP {}:{}", file!(), line!()); None };
                }
                let array = descriptor_location_type(types, ty, &path).array_trace(line!())?;
                let StructuralTypeShape::FixedArray { element, .. } =
                    find_structural_type(types, array).array_trace(line!())?.shape
                else {
                    return { eprintln!("ARRAY STOP {}:{}", file!(), line!()); None };
                };
                let mut steps = prefix.clone();
                steps.extend(path.into_iter().map(DescriptorContentsStep::Address));
                steps.push(DescriptorContentsStep::any_element(element));
                pending.push((element, steps, ancestors.clone()));
            }
            if origins.len().checked_add(pending.len()).array_trace(line!())? > 4096 {
                return { eprintln!("ARRAY STOP {}:{}", file!(), line!()); None };
            }
        }
        Some(origins)
    }
}

trait ArrayTrace: Sized { fn array_trace(self, line: u32) -> Self; }
impl<T> ArrayTrace for Option<T> {
 fn array_trace(self, line: u32) -> Self { if self.is_none() {eprintln!("ARRAY STOP {}:{}",file!(),line);} self }
}
impl<T,E> ArrayTrace for Result<T,E> {
 fn array_trace(self, line: u32) -> Self { if self.is_err() {eprintln!("ARRAY STOP {}:{}",file!(),line);} self }
}
