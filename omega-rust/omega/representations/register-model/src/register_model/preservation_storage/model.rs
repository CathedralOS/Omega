/// Exact target-owned preservation convention selected for frame planning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FrameAbiPreservationConvention {
    SystemVAMD64,
    MicrosoftX64,
    Aapcs64,
    DarwinAapcs64,
}

use super::super::{PhysicalRegisterModelIdentity, RegisterUnitId, RegisterViewId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PreservationStorageGroupId(pub u16);

/// One target-declared storage carrier for an ABI-preserved register image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreservationStorageGroup {
    pub id: PreservationStorageGroupId,
    pub name: String,
    pub storage_view: RegisterViewId,
    pub preserved_units: Vec<RegisterUnitId>,
    pub size_bytes: u64,
    pub alignment_bytes: u64,
}

/// Canonical preservation-storage grouping for one physical model convention.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreservationStorageCatalog {
    pub physical_register_model: PhysicalRegisterModelIdentity,
    pub convention: String,
    pub groups: Vec<PreservationStorageGroup>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedPreservationStorageCatalog {
    catalog: PreservationStorageCatalog,
    identity: super::PreservationStorageCatalogIdentity,
}

impl ValidatedPreservationStorageCatalog {
    pub const fn catalog(&self) -> &PreservationStorageCatalog {
        &self.catalog
    }

    pub const fn identity(&self) -> super::PreservationStorageCatalogIdentity {
        self.identity
    }

    pub const fn physical_identity(&self) -> PhysicalRegisterModelIdentity {
        self.catalog.physical_register_model
    }

    pub fn into_catalog(self) -> PreservationStorageCatalog {
        self.catalog
    }

    pub(super) const fn new(
        catalog: PreservationStorageCatalog,
        identity: super::PreservationStorageCatalogIdentity,
    ) -> Self {
        Self { catalog, identity }
    }
}
