use sha2::{Digest, Sha256};

use super::PreservationStorageCatalog;

const IDENTITY_WIDTH: usize = 32;
const IDENTITY_DOMAIN: &[u8] = b"omega.preservation-storage-catalog-identity.v1\0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PreservationStorageCatalogIdentity([u8; IDENTITY_WIDTH]);

impl PreservationStorageCatalogIdentity {
    pub const fn from_bytes(bytes: [u8; IDENTITY_WIDTH]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; IDENTITY_WIDTH] {
        self.0
    }
}

pub fn preservation_storage_catalog_identity(
    catalog: &PreservationStorageCatalog,
) -> PreservationStorageCatalogIdentity {
    let mut canonical = Vec::new();
    canonical.extend_from_slice(&catalog.physical_register_model.bytes());
    string(&mut canonical, &catalog.convention);
    length(&mut canonical, catalog.groups.len());
    for group in &catalog.groups {
        canonical.extend_from_slice(&group.id.0.to_le_bytes());
        string(&mut canonical, &group.name);
        canonical.extend_from_slice(&group.storage_view.0.to_le_bytes());
        length(&mut canonical, group.preserved_units.len());
        for unit in &group.preserved_units {
            canonical.extend_from_slice(&unit.0.to_le_bytes());
        }
        canonical.extend_from_slice(&group.size_bytes.to_le_bytes());
        canonical.extend_from_slice(&group.alignment_bytes.to_le_bytes());
    }

    let mut digest = Sha256::new();
    digest.update(IDENTITY_DOMAIN);
    digest.update(
        u64::try_from(canonical.len())
            .expect("canonical preservation-storage identity length fits u64")
            .to_le_bytes(),
    );
    digest.update(canonical);
    PreservationStorageCatalogIdentity(digest.finalize().into())
}

fn string(bytes: &mut Vec<u8>, value: &str) {
    length(bytes, value.len());
    bytes.extend_from_slice(value.as_bytes());
}

fn length(bytes: &mut Vec<u8>, value: usize) {
    bytes.extend_from_slice(
        &u64::try_from(value)
            .expect("canonical preservation-storage identity length fits u64")
            .to_le_bytes(),
    );
}
