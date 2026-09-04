use std::collections::{BTreeMap, BTreeSet};

use super::super::{RegisterUnitId, RegisterViewId, ValidatedPhysicalRegisterModel};
use super::{
    PreservationStorageCatalog, PreservationStorageGroupId, ValidatedPreservationStorageCatalog,
    preservation_storage_catalog_identity,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreservationStorageCatalogValidationError {
    PhysicalRegisterModelMismatch,
    UnknownConvention(String),
    NonCanonicalGroupIds,
    EmptyGroupName(PreservationStorageGroupId),
    DuplicateGroupName(String),
    EmptyGroup(PreservationStorageGroupId),
    NonCanonicalPreservedUnits(PreservationStorageGroupId),
    UnknownPreservedUnit {
        group: PreservationStorageGroupId,
        unit: RegisterUnitId,
    },
    UnknownStorageView {
        group: PreservationStorageGroupId,
        view: RegisterViewId,
    },
    StorageViewUnitMismatch(PreservationStorageGroupId),
    NonByteSizedStorageView(PreservationStorageGroupId),
    StorageSizeMismatch(PreservationStorageGroupId),
    InvalidStorageAlignment(PreservationStorageGroupId),
    OverlappingPreservedUnit(RegisterUnitId),
    UnexpectedPreservedUnit(RegisterUnitId),
    MissingPreservedUnit(RegisterUnitId),
    NonCanonicalCoverage,
}

impl std::fmt::Display for PreservationStorageCatalogValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid preservation-storage catalog: {self:?}")
    }
}

impl std::error::Error for PreservationStorageCatalogValidationError {}

pub fn validate_preservation_storage_catalog(
    catalog: PreservationStorageCatalog,
    model: &ValidatedPhysicalRegisterModel,
) -> Result<ValidatedPreservationStorageCatalog, PreservationStorageCatalogValidationError> {
    if catalog.physical_register_model != model.identity() {
        return Err(PreservationStorageCatalogValidationError::PhysicalRegisterModelMismatch);
    }
    let Some(convention) = model
        .model()
        .conventions
        .iter()
        .find(|candidate| candidate.name == catalog.convention)
    else {
        return Err(
            PreservationStorageCatalogValidationError::UnknownConvention(
                catalog.convention.clone(),
            ),
        );
    };
    if catalog
        .groups
        .iter()
        .enumerate()
        .any(|(expected, group)| usize::from(group.id.0) != expected)
    {
        return Err(PreservationStorageCatalogValidationError::NonCanonicalGroupIds);
    }

    let units = model
        .model()
        .units
        .iter()
        .map(|unit| (unit.id, unit))
        .collect::<BTreeMap<_, _>>();
    let views = model
        .model()
        .views
        .iter()
        .map(|view| (view.id, view))
        .collect::<BTreeMap<_, _>>();
    let mut names = BTreeSet::new();
    let mut covered = BTreeSet::new();
    let mut ordered_coverage = Vec::new();

    for group in &catalog.groups {
        if group.name.is_empty() {
            return Err(PreservationStorageCatalogValidationError::EmptyGroupName(
                group.id,
            ));
        }
        if !names.insert(group.name.clone()) {
            return Err(
                PreservationStorageCatalogValidationError::DuplicateGroupName(group.name.clone()),
            );
        }
        if group.preserved_units.is_empty() {
            return Err(PreservationStorageCatalogValidationError::EmptyGroup(
                group.id,
            ));
        }
        if group
            .preserved_units
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(
                PreservationStorageCatalogValidationError::NonCanonicalPreservedUnits(group.id),
            );
        }
        for unit in &group.preserved_units {
            if !units.contains_key(unit) {
                return Err(
                    PreservationStorageCatalogValidationError::UnknownPreservedUnit {
                        group: group.id,
                        unit: *unit,
                    },
                );
            }
            if convention.callee_saved.binary_search(unit).is_err() {
                return Err(
                    PreservationStorageCatalogValidationError::UnexpectedPreservedUnit(*unit),
                );
            }
            if !covered.insert(*unit) {
                return Err(
                    PreservationStorageCatalogValidationError::OverlappingPreservedUnit(*unit),
                );
            }
        }
        ordered_coverage.extend(group.preserved_units.iter().copied());

        let Some(view) = views.get(&group.storage_view) else {
            return Err(
                PreservationStorageCatalogValidationError::UnknownStorageView {
                    group: group.id,
                    view: group.storage_view,
                },
            );
        };
        if view.units != group.preserved_units {
            return Err(
                PreservationStorageCatalogValidationError::StorageViewUnitMismatch(group.id),
            );
        }
        if view.bits % 8 != 0 {
            return Err(
                PreservationStorageCatalogValidationError::NonByteSizedStorageView(group.id),
            );
        }
        if group.size_bytes != u64::from(view.bits / 8) {
            return Err(PreservationStorageCatalogValidationError::StorageSizeMismatch(group.id));
        }
        if group.alignment_bytes == 0 || !group.alignment_bytes.is_power_of_two() {
            return Err(
                PreservationStorageCatalogValidationError::InvalidStorageAlignment(group.id),
            );
        }
    }

    if let Some(unit) = convention
        .callee_saved
        .iter()
        .find(|unit| !covered.contains(unit))
    {
        return Err(PreservationStorageCatalogValidationError::MissingPreservedUnit(*unit));
    }
    if ordered_coverage != convention.callee_saved {
        return Err(PreservationStorageCatalogValidationError::NonCanonicalCoverage);
    }

    let identity = preservation_storage_catalog_identity(&catalog);
    Ok(ValidatedPreservationStorageCatalog::new(catalog, identity))
}
