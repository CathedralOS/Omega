//! Exact AArch64 ABI preservation-storage groups.

use register_model::{
    PreservationStorageCatalog, PreservationStorageCatalogValidationError,
    PreservationStorageGroup, PreservationStorageGroupId, ValidatedPhysicalRegisterModel,
    ValidatedPreservationStorageCatalog, validate_preservation_storage_catalog,
};
use target::NativeTarget;

use crate::register_model::{
    aarch64_physical_register_model, aarch64_preservation_convention_for_target,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Aarch64PreservationStorageCatalogError {
    UnsupportedTarget,
    PhysicalRegisterModelMismatch,
    InvalidCatalog(PreservationStorageCatalogValidationError),
}

impl std::fmt::Display for Aarch64PreservationStorageCatalogError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid AArch64 preservation-storage catalog: {self:?}"
        )
    }
}

impl std::error::Error for Aarch64PreservationStorageCatalogError {}

pub fn aarch64_preservation_storage_catalog(
    model: &ValidatedPhysicalRegisterModel,
    target: NativeTarget,
) -> Result<ValidatedPreservationStorageCatalog, Aarch64PreservationStorageCatalogError> {
    if model.model() != &aarch64_physical_register_model() {
        return Err(Aarch64PreservationStorageCatalogError::PhysicalRegisterModelMismatch);
    }
    if target != NativeTarget::linux_arm64() && target != NativeTarget::macos_arm64() {
        return Err(Aarch64PreservationStorageCatalogError::UnsupportedTarget);
    }
    let convention = aarch64_preservation_convention_for_target(model, target)
        .ok_or(Aarch64PreservationStorageCatalogError::UnsupportedTarget)?;
    let group_names = (19..=29)
        .map(|index| format!("x{index}"))
        .chain((8..=15).map(|index| format!("d{index}")))
        .collect::<Vec<_>>();
    let groups = group_names
        .iter()
        .enumerate()
        .map(|(index, name)| {
            let view = model
                .model()
                .view_named(name)
                .expect("canonical AArch64 preservation view exists");
            PreservationStorageGroup {
                id: PreservationStorageGroupId(
                    u16::try_from(index).expect("AArch64 preservation group id fits u16"),
                ),
                name: name.clone(),
                storage_view: view.id,
                preserved_units: view.units.clone(),
                size_bytes: 8,
                alignment_bytes: 8,
            }
        })
        .collect();
    validate_preservation_storage_catalog(
        PreservationStorageCatalog {
            physical_register_model: model.identity(),
            convention: convention.name.clone(),
            groups,
        },
        model,
    )
    .map_err(Aarch64PreservationStorageCatalogError::InvalidCatalog)
}

#[cfg(test)]
mod tests {
    use super::*;
    use register_model::validate_physical_register_model;
    use target::{Architecture, ObjectFormat};

    fn model() -> ValidatedPhysicalRegisterModel {
        validate_physical_register_model(aarch64_physical_register_model()).unwrap()
    }

    fn assert_exact_groups(catalog: &ValidatedPreservationStorageCatalog) {
        let groups = &catalog.catalog().groups;
        assert_eq!(groups.len(), 19);
        assert_eq!(
            groups
                .iter()
                .map(|group| group.name.as_str())
                .collect::<Vec<_>>(),
            [
                "x19", "x20", "x21", "x22", "x23", "x24", "x25", "x26", "x27", "x28", "x29", "d8",
                "d9", "d10", "d11", "d12", "d13", "d14", "d15",
            ]
        );
        assert!(groups.iter().all(|group| {
            group.preserved_units.len() == 1 && group.size_bytes == 8 && group.alignment_bytes == 8
        }));
        assert_eq!(
            groups.iter().map(|group| group.size_bytes).sum::<u64>(),
            152
        );
    }

    #[test]
    fn aapcs_and_darwin_have_exact_distinct_catalogs() {
        let model = model();
        let aapcs =
            aarch64_preservation_storage_catalog(&model, NativeTarget::linux_arm64()).unwrap();
        let darwin =
            aarch64_preservation_storage_catalog(&model, NativeTarget::macos_arm64()).unwrap();
        assert_exact_groups(&aapcs);
        assert_exact_groups(&darwin);
        assert_eq!(aapcs.catalog().convention, "aapcs64");
        assert_eq!(darwin.catalog().convention, "darwin-aapcs64");
        assert_ne!(aapcs.identity(), darwin.identity());
        assert_eq!(aapcs.catalog().groups, darwin.catalog().groups);
    }

    #[test]
    fn vector_preservation_uses_only_d8_through_d15_low_halves() {
        let model = model();
        let catalog =
            aarch64_preservation_storage_catalog(&model, NativeTarget::linux_arm64()).unwrap();
        for group in &catalog.catalog().groups[11..] {
            let index = group.name.trim_start_matches('d');
            let q = model.model().view_named(&format!("q{index}")).unwrap();
            assert_eq!(group.preserved_units, vec![q.units[0]]);
            assert!(!group.preserved_units.contains(&q.units[1]));
        }
        let convention = model
            .model()
            .conventions
            .iter()
            .find(|row| row.name == "aapcs64")
            .unwrap();
        assert_eq!(
            catalog
                .catalog()
                .groups
                .iter()
                .flat_map(|group| group.preserved_units.iter().copied())
                .collect::<Vec<_>>(),
            convention.callee_saved
        );
    }

    #[test]
    fn exact_target_and_canonical_model_are_mandatory() {
        let model = model();
        assert_eq!(
            aarch64_preservation_storage_catalog(&model, NativeTarget::linux_x64()),
            Err(Aarch64PreservationStorageCatalogError::UnsupportedTarget)
        );
        assert_eq!(
            aarch64_preservation_storage_catalog(
                &model,
                NativeTarget {
                    architecture: Architecture::Aarch64,
                    object_format: ObjectFormat::Elf,
                    pointer_size: 4,
                    pointer_alignment: 4,
                },
            ),
            Err(Aarch64PreservationStorageCatalogError::UnsupportedTarget)
        );

        let mut changed = aarch64_physical_register_model();
        changed.units[0].name.push_str(".changed");
        let changed = validate_physical_register_model(changed).unwrap();
        assert_eq!(
            aarch64_preservation_storage_catalog(&changed, NativeTarget::linux_arm64()),
            Err(Aarch64PreservationStorageCatalogError::PhysicalRegisterModelMismatch)
        );
    }
}
