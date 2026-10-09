use super::PreservationStorageCatalog;
use crate::register_model::identities::{length, string};

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

pub(crate) fn preservation_storage_catalog_identity(
    catalog: &PreservationStorageCatalog,
) -> PreservationStorageCatalogIdentity {
    let mut canonical = Vec::new();
    canonical.extend_from_slice(&catalog.physical_register_model.bytes());
    string(&mut canonical, &catalog.convention);
    length(&mut canonical, catalog.groups.len());
    for group in &catalog.groups {
        crate::codec_bytes::push_encoded(&mut canonical, group.id.0.to_le_bytes());
        string(&mut canonical, &group.name);
        crate::codec_bytes::push_encoded(&mut canonical, group.storage_view.0.to_le_bytes());
        length(&mut canonical, group.preserved_units.len());
        for unit in &group.preserved_units {
            crate::codec_bytes::push_encoded(&mut canonical, unit.0.to_le_bytes());
        }
        crate::codec_bytes::push_encoded(&mut canonical, group.size_bytes.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut canonical, group.alignment_bytes.to_le_bytes());
    }

    let mut digest = crate::digest_encoder::DomainDigestEncoder::new(IDENTITY_DOMAIN);
    digest.raw(
        &u64::try_from(canonical.len())
            .expect("canonical preservation-storage identity length fits u64")
            .to_le_bytes(),
    );
    digest.raw(&canonical);
    PreservationStorageCatalogIdentity(digest.finish())
}
