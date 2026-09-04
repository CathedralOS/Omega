use omega_register_model::ValidatedPreservationStorageCatalog;

use crate::FunctionAllocatedCalleeSavedRequirements;

use super::super::{
    FunctionNonAuthoritativeCalleeSaveStorage, NonAuthoritativeCalleeSaveSlot,
    NonAuthoritativeCalleeSaveSlotId, NonAuthoritativeCalleeSaveStorageError,
};

pub(super) fn derive_function(
    source: &FunctionAllocatedCalleeSavedRequirements,
    catalog: &ValidatedPreservationStorageCatalog,
) -> Result<FunctionNonAuthoritativeCalleeSaveStorage, NonAuthoritativeCalleeSaveStorageError> {
    let mut slots = Vec::new();
    let mut extent = 0_u64;
    let mut max_alignment = 1_u64;
    for group in &catalog.catalog().groups {
        let modified_units = source
            .modified_units
            .iter()
            .filter(|requirement| group.preserved_units.contains(&requirement.unit))
            .cloned()
            .collect::<Vec<_>>();
        if modified_units.is_empty() {
            continue;
        }
        let alignment = group.alignment_bytes;
        let offset = align_up(extent, alignment)?;
        extent = offset
            .checked_add(group.size_bytes)
            .ok_or(NonAuthoritativeCalleeSaveStorageError::StorageGeometryOverflow)?;
        max_alignment = max_alignment.max(group.alignment_bytes);
        slots.push(NonAuthoritativeCalleeSaveSlot {
            id: NonAuthoritativeCalleeSaveSlotId(
                u16::try_from(slots.len())
                    .map_err(|_| NonAuthoritativeCalleeSaveStorageError::StorageGeometryOverflow)?,
            ),
            storage_group: group.id,
            storage_view: group.storage_view,
            preserved_units: group.preserved_units.clone(),
            modified_units,
            abstract_offset_bytes: offset,
            size_bytes: group.size_bytes,
            alignment_bytes: group.alignment_bytes,
        });
    }
    if slots
        .iter()
        .map(|slot| slot.modified_units.len())
        .sum::<usize>()
        != source.modified_units.len()
    {
        let missing = source
            .modified_units
            .iter()
            .find(|requirement| {
                !slots
                    .iter()
                    .any(|slot| slot.modified_units.contains(requirement))
            })
            .map(|requirement| requirement.unit)
            .unwrap_or(omega_register_model::RegisterUnitId(u16::MAX));
        return Err(NonAuthoritativeCalleeSaveStorageError::UnknownPreservedUnit(missing));
    }
    Ok(FunctionNonAuthoritativeCalleeSaveStorage {
        machine: source.machine,
        kind: source.kind,
        abstract_area_bytes: extent,
        abstract_area_alignment: max_alignment,
        slots,
    })
}

fn align_up(value: u64, alignment: u64) -> Result<u64, NonAuthoritativeCalleeSaveStorageError> {
    let mask = alignment
        .checked_sub(1)
        .ok_or(NonAuthoritativeCalleeSaveStorageError::StorageGeometryOverflow)?;
    value
        .checked_add(mask)
        .map(|rounded| rounded & !mask)
        .ok_or(NonAuthoritativeCalleeSaveStorageError::StorageGeometryOverflow)
}
