use super::{
    ReferenceStorage, StructuralAccess, StructuralTypeId, StructuralTypeLookup, StructuralTypeShape,
};

/// The two static presentations of a single checked shared reference field.
#[derive(Clone, Copy)]
pub(crate) struct OpenedReferenceField {
    pub(crate) storage_type: StructuralTypeId,
    opened_type: StructuralTypeId,
    storage_referent: StructuralTypeId,
    opened_referent: StructuralTypeId,
}

impl OpenedReferenceField {
    pub(crate) fn new(
        types: &StructuralTypeLookup<'_>,
        storage_type: StructuralTypeId,
        opened_type: StructuralTypeId,
    ) -> Option<Self> {
        let witness = |carrier| match types.get(carrier)?.shape {
            StructuralTypeShape::Reference {
                referent,
                access: StructuralAccess::SharedBorrow,
            } if types
                .catalog()
                .storage_inputs()
                .is_package_witness(referent) =>
            {
                Some(referent)
            }
            _ => None,
        };
        let storage_referent = witness(storage_type)?;
        let opened_referent = witness(opened_type)?;
        (storage_referent != opened_referent).then_some(Self {
            storage_type,
            opened_type,
            storage_referent,
            opened_referent,
        })
    }

    pub(super) fn open(self, leaf: &mut ReferenceStorage) -> bool {
        if leaf.carrier_type != self.storage_type || leaf.referent != self.storage_referent {
            return false;
        }
        leaf.carrier_type = self.opened_type;
        leaf.referent = self.opened_referent;
        true
    }

    pub(crate) fn seal(self, leaf: &mut ReferenceStorage) -> bool {
        if leaf.carrier_type != self.opened_type || leaf.referent != self.opened_referent {
            return false;
        }
        leaf.carrier_type = self.storage_type;
        leaf.referent = self.storage_referent;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{HOST, machine, place, reference_type, scalar_field, ty, u32_type};
    use abstract_operations::StructuralTypeCatalog;
    use target_operations::ReferenceRoot;
    use terminal_psi::{
        StructuralArgument, TerminalIdentityTableBuilder, TerminalPackageCatalog,
        TerminalPackageOpening, TerminalPackageSchema,
    };

    #[test]
    fn opening_and_storing_a_reference_preserves_its_origin_and_rejects_other_carriers() {
        let mut names = TerminalIdentityTableBuilder::default();
        let mut types = Vec::new();
        for (id, name) in [(1, "schema"), (2, "opening"), (3, "ordinary")] {
            let mut opaque = reference_type(id, names.intern(name), ty(id));
            opaque.shape = StructuralTypeShape::Opaque;
            types.push(opaque);
        }
        for (id, name) in [(4, "stored_ref"), (5, "opened_ref"), (6, "ordinary_ref")] {
            types.push(reference_type(id, names.intern(name), ty(id - 3)));
        }
        let mut schema_field = scalar_field(1, names.intern("stored"), u32_type());
        schema_field.field_type = terminal_psi::StructuralFieldType::Structural(ty(4));
        let mut package = reference_type(7, names.intern("package"), ty(1));
        package.shape = StructuralTypeShape::Record {
            fields: vec![schema_field],
            storage: terminal_psi::StructuralRecordStorage::Packed,
        };
        types.push(package);
        let catalog = StructuralTypeCatalog::new(types, vec![]).with_package_witnesses(
            &TerminalPackageCatalog {
                schemas: vec![TerminalPackageSchema {
                    record: ty(7),
                    witnesses: vec![ty(1)],
                }],
                openings: vec![TerminalPackageOpening {
                    machine: machine(),
                    record: ty(7),
                    subject: StructuralArgument {
                        place: place(1),
                        path: vec![],
                        access: StructuralAccess::SharedBorrow,
                    },
                    witnesses: vec![ty(2)],
                }],
                sites: vec![],
            },
        );
        let lookup = StructuralTypeLookup::new(&catalog, HOST(), None, &[]).unwrap();
        let shape = |id| lookup.projections.shape(ty(id)).unwrap();
        assert_eq!(shape(4), shape(5));
        assert_ne!(shape(4), shape(6));
        let field = OpenedReferenceField::new(&lookup, ty(4), ty(5)).unwrap();
        assert!(OpenedReferenceField::new(&lookup, ty(4), ty(6)).is_none());
        assert!(OpenedReferenceField::new(&lookup, ty(4), ty(4)).is_none());
        let stored = ReferenceStorage {
            root: ReferenceRoot::Place(place(8)),
            carrier_type: ty(4),
            referent: ty(1),
            placed: None,
        };
        let mut leaf = stored.clone();
        assert!(!field.seal(&mut leaf));
        assert!(field.open(&mut leaf));
        assert_eq!(leaf.root, stored.root);
        assert_eq!((leaf.carrier_type, leaf.referent), (ty(5), ty(2)));
        assert!(!field.open(&mut leaf));
        assert!(field.seal(&mut leaf));
        assert_eq!(leaf, stored);
    }
}
