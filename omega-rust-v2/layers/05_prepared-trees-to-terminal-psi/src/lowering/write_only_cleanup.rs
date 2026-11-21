//! Publish cleanup authority only after package construction has selected every
//! nominal cleanup. A reference contributes its own descriptor, never the
//! cleanup of the storage it points to. By-value containment follows every
//! field in every case, since a write-only loan cannot inspect a discriminator.
use crate::{LoweredData, LoweringTypes, Multiplicity, ReferenceAccess, TypeId, TypeShape, WriteOnlyCleanup};
use std::collections::HashMap;

#[derive(Clone, Copy)]
struct Storage {
    cleanup: WriteOnlyCleanup,
    copyable: bool,
}

pub(super) fn retain(types: &LoweringTypes, data: &mut [LoweredData]) {
    let plain = Storage { cleanup: WriteOnlyCleanup::NoCodeAffine, copyable: true };
    let mut proven: HashMap<TypeId, Option<Storage>> = HashMap::new();
    for schema in data.iter() {
        let mut pending = vec![(schema.carrier, false)];
        while let Some((ty, finish)) = pending.pop() {
            if !finish && proven.contains_key(&ty) {
                continue;
            }
            // None is conservative for an active ancestor (a by-value cycle)
            // and for a finished type without authority.
            proven.insert(ty, None);
            let (mut storage, children) = match types.get(ty) {
                Some(TypeShape::PrimitiveScalar(_)) => (plain, Vec::new()),
                Some(TypeShape::ScalarReference { access, .. } | TypeShape::NominalReference { access, .. }) => (
                    Storage { cleanup: WriteOnlyCleanup::DescriptorCleanup, copyable: *access == ReferenceAccess::Shared },
                    Vec::new(),
                ),
                Some(TypeShape::FixedArray { element, .. }) => (plain, vec![*element]),
                Some(TypeShape::Nominal(id)) => {
                    let child = &data[id.get() - 1];
                    if child.carrier != ty || child.multiplicity == Multiplicity::Linear || child.cleanup_target.is_some() {
                        continue;
                    }
                    let fields = child.fields.iter().chain(child.cases.iter().flat_map(|case| &case.fields));
                    (Storage { copyable: child.multiplicity == Multiplicity::Copy, ..plain }, fields.map(|field| field.carrier).collect())
                }
                _ => continue,
            };
            if !finish && !children.is_empty() {
                pending.push((ty, true));
                pending.extend(children.into_iter().map(|child| (child, false)));
                continue;
            }
            let mut valid = true;
            for child in children {
                let Some(Some(field)) = proven.get(&child) else {
                    valid = false;
                    break;
                };
                if matches!(types.get(ty), Some(TypeShape::Nominal(_))) && storage.copyable && !field.copyable {
                    valid = false;
                    break;
                }
                storage.copyable &= field.copyable;
                if field.cleanup == WriteOnlyCleanup::DescriptorCleanup {
                    storage.cleanup = field.cleanup;
                }
            }
            proven.insert(ty, valid.then_some(storage));
        }
    }
    for schema in data {
        schema.write_only_cleanup = proven.get(&schema.carrier).copied().flatten().map(|storage| storage.cleanup);
    }
}
