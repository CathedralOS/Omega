//! Exact target-owned grouping of ABI-preserved register storage.
//!
//! These catalogs describe abstract save-storage carriers only. They choose no
//! instruction, stack coordinate, prologue, epilogue, or unwind operation.

mod identity;
mod validation;

use super::{PhysicalRegisterModelIdentity, RegisterUnitId, RegisterViewId};
pub use identity::*;
pub use validation::*;

#[cfg(test)]
mod tests;

/// Exact target-owned preservation convention selected for frame planning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FrameAbiPreservationConvention {
    SystemVAMD64,
    MicrosoftX64,
    Aapcs64,
    DarwinAapcs64,
}

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
    identity: PreservationStorageCatalogIdentity,
}

impl ValidatedPreservationStorageCatalog {
    pub const fn catalog(&self) -> &PreservationStorageCatalog {
        &self.catalog
    }

    pub const fn identity(&self) -> PreservationStorageCatalogIdentity {
        self.identity
    }

    pub const fn physical_identity(&self) -> PhysicalRegisterModelIdentity {
        self.catalog.physical_register_model
    }

    pub fn into_catalog(self) -> PreservationStorageCatalog {
        self.catalog
    }

    const fn new(
        catalog: PreservationStorageCatalog,
        identity: PreservationStorageCatalogIdentity,
    ) -> Self {
        Self { catalog, identity }
    }
}
