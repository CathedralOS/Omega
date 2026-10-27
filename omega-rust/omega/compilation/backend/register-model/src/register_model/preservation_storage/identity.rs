use super::PreservationStorageCatalog;
use crate::register_model::identities::{identity, length, string, u16_value, u64_value, unit_ids};

identity!(
    PreservationStorageCatalogIdentity,
    b"omega.preservation-storage-catalog-identity.v1\0"
);

pub(crate) fn preservation_storage_catalog_identity(
    catalog: &PreservationStorageCatalog,
) -> PreservationStorageCatalogIdentity {
    let mut canonical = Vec::new();
    canonical.extend_from_slice(&catalog.physical_register_model.bytes());
    string(&mut canonical, &catalog.convention);
    length(&mut canonical, catalog.groups.len());
    for group in &catalog.groups {
        u16_value(&mut canonical, group.id.0);
        string(&mut canonical, &group.name);
        u16_value(&mut canonical, group.storage_view.0);
        unit_ids(&mut canonical, &group.preserved_units);
        u64_value(&mut canonical, group.size_bytes);
        u64_value(&mut canonical, group.alignment_bytes);
    }
    PreservationStorageCatalogIdentity::from_canonical_bytes(&canonical)
}
