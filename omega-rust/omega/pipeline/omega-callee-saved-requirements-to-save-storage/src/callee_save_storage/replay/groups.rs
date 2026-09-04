use std::collections::BTreeMap;

use omega_register_model::{
    PreservationStorageGroupId, RegisterUnitId, ValidatedPreservationStorageCatalog,
};

use crate::{AllocatedCalleeSavedRequirementPlan, AllocatedCalleeSavedUnitRequirement};

use super::super::{
    FunctionNonAuthoritativeCalleeSaveStorage, NonAuthoritativeCalleeSaveSlot,
    NonAuthoritativeCalleeSaveSlotId, NonAuthoritativeCalleeSaveStorageError,
};

pub(super) fn reconstruct_functions(
    source: &AllocatedCalleeSavedRequirementPlan,
    catalog: &ValidatedPreservationStorageCatalog,
) -> Result<Vec<FunctionNonAuthoritativeCalleeSaveStorage>, NonAuthoritativeCalleeSaveStorageError>
{
    let mut unit_groups = BTreeMap::<RegisterUnitId, PreservationStorageGroupId>::new();
    for group in &catalog.catalog().groups {
        for unit in &group.preserved_units {
            if unit_groups.insert(*unit, group.id).is_some() {
                return Err(NonAuthoritativeCalleeSaveStorageError::NonCanonicalStorage);
            }
        }
    }
    source
        .functions
        .iter()
        .map(|function| {
            let mut modified = BTreeMap::<
                PreservationStorageGroupId,
                Vec<AllocatedCalleeSavedUnitRequirement>,
            >::new();
            for requirement in &function.modified_units {
                let group = unit_groups.get(&requirement.unit).copied().ok_or(
                    NonAuthoritativeCalleeSaveStorageError::UnknownPreservedUnit(requirement.unit),
                )?;
                modified.entry(group).or_default().push(requirement.clone());
            }
            let mut slots = Vec::new();
            let mut extent = 0_u64;
            let mut max_alignment = 1_u64;
            for group in &catalog.catalog().groups {
                let Some(modified_units) = modified.remove(&group.id) else {
                    continue;
                };
                let alignment = group.alignment_bytes;
                let mask = alignment
                    .checked_sub(1)
                    .ok_or(NonAuthoritativeCalleeSaveStorageError::StorageGeometryOverflow)?;
                let offset = extent
                    .checked_add(mask)
                    .map(|rounded| rounded & !mask)
                    .ok_or(NonAuthoritativeCalleeSaveStorageError::StorageGeometryOverflow)?;
                extent = offset
                    .checked_add(group.size_bytes)
                    .ok_or(NonAuthoritativeCalleeSaveStorageError::StorageGeometryOverflow)?;
                max_alignment = max_alignment.max(group.alignment_bytes);
                slots.push(NonAuthoritativeCalleeSaveSlot {
                    id: NonAuthoritativeCalleeSaveSlotId(u16::try_from(slots.len()).map_err(
                        |_| NonAuthoritativeCalleeSaveStorageError::StorageGeometryOverflow,
                    )?),
                    storage_group: group.id,
                    storage_view: group.storage_view,
                    preserved_units: group.preserved_units.clone(),
                    modified_units,
                    abstract_offset_bytes: offset,
                    size_bytes: group.size_bytes,
                    alignment_bytes: group.alignment_bytes,
                });
            }
            if !modified.is_empty() {
                return Err(NonAuthoritativeCalleeSaveStorageError::NonCanonicalStorage);
            }
            Ok(FunctionNonAuthoritativeCalleeSaveStorage {
                machine: function.machine,
                kind: function.kind,
                abstract_area_bytes: extent,
                abstract_area_alignment: max_alignment,
                slots,
            })
        })
        .collect()
}
