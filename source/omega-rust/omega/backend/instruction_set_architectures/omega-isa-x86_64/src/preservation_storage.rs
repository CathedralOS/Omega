//! Exact x86-64 ABI preservation-storage groups.

use omega_register_model::{
    PreservationStorageCatalog, PreservationStorageCatalogValidationError,
    PreservationStorageGroup, PreservationStorageGroupId, ValidatedPhysicalRegisterModel,
    ValidatedPreservationStorageCatalog, validate_preservation_storage_catalog,
};
use omega_target::NativeTarget;

use crate::register_model::{
    x86_64_physical_register_model, x86_64_preservation_convention_for_target,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum X86_64PreservationStorageCatalogError {
    UnsupportedTarget,
    PhysicalRegisterModelMismatch,
    InvalidCatalog(PreservationStorageCatalogValidationError),
}

impl std::fmt::Display for X86_64PreservationStorageCatalogError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid x86-64 preservation-storage catalog: {self:?}"
        )
    }
}

impl std::error::Error for X86_64PreservationStorageCatalogError {}

/// Select and validate the exact preservation-storage catalog for one x86-64
/// target. Windows and UEFI intentionally share the indistinguishable
/// Microsoft-x64 `NativeTarget` contract at this layer.
pub fn x86_64_preservation_storage_catalog(
    model: &ValidatedPhysicalRegisterModel,
    target: NativeTarget,
) -> Result<ValidatedPreservationStorageCatalog, X86_64PreservationStorageCatalogError> {
    if model.model() != &x86_64_physical_register_model() {
        return Err(X86_64PreservationStorageCatalogError::PhysicalRegisterModelMismatch);
    }
    let group_names: Vec<String> = if target == NativeTarget::linux_x64() {
        ["rbx", "rbp", "r12", "r13", "r14", "r15"]
            .into_iter()
            .map(str::to_owned)
            .collect()
    } else if target == NativeTarget::windows_x64() {
        ["rbx", "rsi", "rdi", "rbp", "r12", "r13", "r14", "r15"]
            .into_iter()
            .map(str::to_owned)
            .chain((6..=15).map(|index| format!("xmm{index}")))
            .collect()
    } else {
        return Err(X86_64PreservationStorageCatalogError::UnsupportedTarget);
    };
    let convention = x86_64_preservation_convention_for_target(model, target)
        .ok_or(X86_64PreservationStorageCatalogError::UnsupportedTarget)?;
    let groups = group_names
        .iter()
        .enumerate()
        .map(|(index, name)| {
            let view = model
                .model()
                .view_named(name)
                .expect("canonical x86 preservation view exists");
            let vector = name.starts_with("xmm");
            PreservationStorageGroup {
                id: PreservationStorageGroupId(
                    u16::try_from(index).expect("x86 preservation group id fits u16"),
                ),
                name: name.clone(),
                storage_view: view.id,
                preserved_units: view.units.clone(),
                size_bytes: if vector { 16 } else { 8 },
                alignment_bytes: if vector { 16 } else { 8 },
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
    .map_err(X86_64PreservationStorageCatalogError::InvalidCatalog)
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_register_model::{RegisterUnitId, validate_physical_register_model};
    use omega_target::{Architecture, ObjectFormat};

    fn model() -> ValidatedPhysicalRegisterModel {
        validate_physical_register_model(x86_64_physical_register_model()).unwrap()
    }

    #[test]
    fn system_v_catalog_coalesces_each_fragmented_gpr_into_one_group() {
        let model = model();
        let catalog =
            x86_64_preservation_storage_catalog(&model, NativeTarget::linux_x64()).unwrap();
        let groups = &catalog.catalog().groups;
        assert_eq!(
            groups
                .iter()
                .map(|group| group.name.as_str())
                .collect::<Vec<_>>(),
            ["rbx", "rbp", "r12", "r13", "r14", "r15"]
        );
        assert_eq!(groups.len(), 6);
        assert!(groups.iter().all(|group| {
            group.preserved_units.len() == 4 && group.size_bytes == 8 && group.alignment_bytes == 8
        }));
        assert_eq!(
            groups
                .iter()
                .flat_map(|group| group.preserved_units.iter().copied())
                .collect::<Vec<_>>(),
            model.model().conventions[0].callee_saved
        );
        assert_eq!(groups.iter().map(|group| group.size_bytes).sum::<u64>(), 48);
    }

    #[test]
    fn microsoft_catalog_has_eight_gpr_and_ten_distinct_xmm_groups() {
        let model = model();
        let catalog =
            x86_64_preservation_storage_catalog(&model, NativeTarget::windows_x64()).unwrap();
        let groups = &catalog.catalog().groups;
        assert_eq!(groups.len(), 18);
        assert_eq!(
            groups[..8]
                .iter()
                .map(|group| group.name.as_str())
                .collect::<Vec<_>>(),
            ["rbx", "rsi", "rdi", "rbp", "r12", "r13", "r14", "r15"]
        );
        assert_eq!(
            groups[8..]
                .iter()
                .map(|group| group.name.as_str())
                .collect::<Vec<_>>(),
            [
                "xmm6", "xmm7", "xmm8", "xmm9", "xmm10", "xmm11", "xmm12", "xmm13", "xmm14",
                "xmm15",
            ]
        );
        assert!(groups[..8].iter().all(|group| {
            group.preserved_units.len() == 4 && group.size_bytes == 8 && group.alignment_bytes == 8
        }));
        assert!(groups[8..].iter().all(|group| {
            group.preserved_units.len() == 1
                && group.size_bytes == 16
                && group.alignment_bytes == 16
        }));
        assert_eq!(
            groups.iter().map(|group| group.size_bytes).sum::<u64>(),
            224
        );
        let convention = model
            .model()
            .conventions
            .iter()
            .find(|row| row.name == "microsoft-x64")
            .unwrap();
        assert_eq!(
            groups
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
            x86_64_preservation_storage_catalog(&model, NativeTarget::macos_arm64()),
            Err(X86_64PreservationStorageCatalogError::UnsupportedTarget)
        );
        assert_eq!(
            x86_64_preservation_storage_catalog(
                &model,
                NativeTarget {
                    architecture: Architecture::X86_64,
                    object_format: ObjectFormat::Elf,
                    pointer_size: 4,
                    pointer_alignment: 4,
                },
            ),
            Err(X86_64PreservationStorageCatalogError::UnsupportedTarget)
        );

        let mut changed = x86_64_physical_register_model();
        changed.units[0].name.push_str(".changed");
        let changed = validate_physical_register_model(changed).unwrap();
        assert_eq!(
            x86_64_preservation_storage_catalog(&changed, NativeTarget::linux_x64()),
            Err(X86_64PreservationStorageCatalogError::PhysicalRegisterModelMismatch)
        );
    }

    #[test]
    fn convention_and_group_shape_are_identity_bound() {
        let model = model();
        let system_v =
            x86_64_preservation_storage_catalog(&model, NativeTarget::linux_x64()).unwrap();
        let microsoft =
            x86_64_preservation_storage_catalog(&model, NativeTarget::windows_x64()).unwrap();
        assert_ne!(system_v.identity(), microsoft.identity());
        assert_eq!(
            system_v.identity(),
            x86_64_preservation_storage_catalog(&model, NativeTarget::linux_x64())
                .unwrap()
                .identity()
        );
        assert_ne!(
            system_v.catalog().groups[0].preserved_units,
            vec![RegisterUnitId(u16::MAX)]
        );
    }
}
