/*
We define and validate target-owned preservation-storage catalogs
(`PreservationStorageCatalog`, `ValidatedPreservationStorageCatalog`, and
`PreservationStorageCatalogIdentity`) that partition a convention's
callee-saved register units into byte-sized, power-of-two-aligned storage
carrier views without choosing stack offsets or prologue/epilogue instructions.
*/

use std::collections::BTreeSet;

use crate::identities::{identity, length, string, u16_value, u64_value, unit_ids};
use crate::{
    PhysicalRegisterModelIdentity, RegisterUnitId, RegisterViewId, ValidatedPhysicalRegisterModel,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FrameAbiPreservationConvention {
    SystemVAMD64,
    MicrosoftX64,
    Aapcs64,
    DarwinAapcs64,
}

impl FrameAbiPreservationConvention {
    pub const fn abi_tag(self) -> u8 {
        match self {
            Self::SystemVAMD64 => 0,
            Self::MicrosoftX64 => 1,
            Self::Aapcs64 => 2,
            Self::DarwinAapcs64 => 3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PreservationStorageGroupId(pub u16);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreservationStorageGroup {
    pub id: PreservationStorageGroupId,
    pub name: String,
    pub storage_view: RegisterViewId,
    pub preserved_units: Vec<RegisterUnitId>,
    pub size_bytes: u64,
    pub alignment_bytes: u64,
}

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

    const fn new(
        catalog: PreservationStorageCatalog,
        identity: PreservationStorageCatalogIdentity,
    ) -> Self {
        Self { catalog, identity }
    }
}

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

invalid_display!(
    PreservationStorageCatalogValidationError,
    "invalid preservation-storage catalog"
);

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
            if model.model().unit(*unit).is_none() {
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

        let Some(view) = model.model().view(group.storage_view) else {
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

#[cfg(test)]
mod tests {
    use super::{
        PreservationStorageCatalog, PreservationStorageCatalogValidationError as Rejected,
        PreservationStorageGroup, PreservationStorageGroupId,
        preservation_storage_catalog_identity, validate_preservation_storage_catalog,
    };
    use crate::tests::assert_rejected;
    use crate::{
        PhysicalRegisterModel, PreservationConvention, RegisterClass, RegisterClassId,
        RegisterUnit, RegisterUnitId, RegisterUnitKind, RegisterView, RegisterViewId,
        RegisterWriteSemantics, ValidatedPhysicalRegisterModel, validate_physical_register_model,
    };
    use target::Architecture;

    fn validated_model() -> ValidatedPhysicalRegisterModel {
        validate_physical_register_model(PhysicalRegisterModel {
            architecture: Architecture::X86_64,
            units: vec![
                RegisterUnit {
                    id: RegisterUnitId(0),
                    name: "r0.low".into(),
                    bits: 32,
                    kind: RegisterUnitKind::IntegerLane,
                },
                RegisterUnit {
                    id: RegisterUnitId(1),
                    name: "r0.high".into(),
                    bits: 32,
                    kind: RegisterUnitKind::IntegerLane,
                },
            ],
            views: vec![RegisterView {
                id: RegisterViewId(0),
                name: "r0".into(),
                class: RegisterClassId(0),
                units: vec![RegisterUnitId(0), RegisterUnitId(1)],
                write_units: vec![RegisterUnitId(0), RegisterUnitId(1)],
                bits: 64,
                write_semantics: RegisterWriteSemantics::ExactView,
                allocatable: true,
            }],
            classes: vec![RegisterClass {
                id: RegisterClassId(0),
                name: "gpr64".into(),
                views: vec![RegisterViewId(0)],
            }],
            conventions: vec![PreservationConvention {
                name: "test".into(),
                argument_views: vec![RegisterViewId(0)],
                result_views: vec![RegisterViewId(0)],
                caller_saved: Vec::new(),
                callee_saved: vec![RegisterUnitId(0), RegisterUnitId(1)],
                fixed: Vec::new(),
                stack_alignment: 16,
                red_zone_bytes: 0,
            }],
            reservations: Vec::new(),
        })
        .unwrap()
    }

    fn catalog(model: &ValidatedPhysicalRegisterModel) -> PreservationStorageCatalog {
        PreservationStorageCatalog {
            physical_register_model: model.identity(),
            convention: "test".into(),
            groups: vec![PreservationStorageGroup {
                id: PreservationStorageGroupId(0),
                name: "r0".into(),
                storage_view: RegisterViewId(0),
                preserved_units: vec![RegisterUnitId(0), RegisterUnitId(1)],
                size_bytes: 8,
                alignment_bytes: 8,
            }],
        }
    }

    fn rejection(mutate: impl FnOnce(&mut PreservationStorageCatalog), expected: Rejected) {
        let model = validated_model();
        assert_rejected(
            catalog(&model),
            mutate,
            |catalog| validate_preservation_storage_catalog(catalog, &model),
            expected,
        );
    }

    #[test]
    fn catalog_accepts_one_fragment_coalescing_group_and_binds_every_field() {
        let model = validated_model();
        let validated = validate_preservation_storage_catalog(catalog(&model), &model).unwrap();
        assert_eq!(validated.catalog().groups[0].preserved_units.len(), 2);
        assert_eq!(
            validated.identity(),
            validate_preservation_storage_catalog(catalog(&model), &model)
                .unwrap()
                .identity()
        );

        let baseline = validated.identity();
        let mut mutations: Vec<Box<dyn FnMut(&mut PreservationStorageCatalog)>> = vec![
            Box::new(|catalog| catalog.convention.push_str(".changed")),
            Box::new(|catalog| catalog.groups[0].id = PreservationStorageGroupId(1)),
            Box::new(|catalog| catalog.groups[0].name.push_str(".changed")),
            Box::new(|catalog| catalog.groups[0].storage_view = RegisterViewId(9)),
            Box::new(|catalog| catalog.groups[0].preserved_units.pop().map(|_| ()).unwrap()),
            Box::new(|catalog| catalog.groups[0].size_bytes = 16),
            Box::new(|catalog| catalog.groups[0].alignment_bytes = 16),
        ];
        for mutate in &mut mutations {
            let mut changed = catalog(&model);
            mutate(&mut changed);
            assert_ne!(preservation_storage_catalog_identity(&changed), baseline);
        }

        let mut changed_model = model.clone().into_model();
        changed_model.units[0].name.push_str(".changed");
        let changed_model = validate_physical_register_model(changed_model).unwrap();
        let changed = PreservationStorageCatalog {
            physical_register_model: changed_model.identity(),
            ..catalog(&model)
        };
        assert_ne!(preservation_storage_catalog_identity(&changed), baseline);
    }

    #[test]
    fn catalog_rejects_model_convention_id_view_geometry_and_coverage_corruption() {
        rejection(
            |catalog| {
                catalog.physical_register_model =
                    crate::PhysicalRegisterModelIdentity::from_bytes([7; 32])
            },
            Rejected::PhysicalRegisterModelMismatch,
        );
        rejection(
            |catalog| catalog.convention = "unknown".into(),
            Rejected::UnknownConvention("unknown".into()),
        );
        rejection(
            |catalog| catalog.groups[0].id = PreservationStorageGroupId(1),
            Rejected::NonCanonicalGroupIds,
        );
        rejection(
            |catalog| catalog.groups[0].storage_view = RegisterViewId(u16::MAX),
            Rejected::UnknownStorageView {
                group: PreservationStorageGroupId(0),
                view: RegisterViewId(u16::MAX),
            },
        );
        rejection(
            |catalog| catalog.groups[0].size_bytes = 4,
            Rejected::StorageSizeMismatch(PreservationStorageGroupId(0)),
        );
        rejection(
            |catalog| catalog.groups[0].alignment_bytes = 3,
            Rejected::InvalidStorageAlignment(PreservationStorageGroupId(0)),
        );
        rejection(
            |catalog| {
                catalog.groups[0].preserved_units.pop();
            },
            Rejected::StorageViewUnitMismatch(PreservationStorageGroupId(0)),
        );
    }

    #[test]
    fn catalog_rejects_overlap_omission_and_noncanonical_group_order() {
        let one_unit_group = |id, name: &str, unit| PreservationStorageGroup {
            id: PreservationStorageGroupId(id),
            name: name.into(),
            storage_view: RegisterViewId(0),
            preserved_units: vec![unit],
            size_bytes: 8,
            alignment_bytes: 8,
        };

        rejection(
            |catalog| {
                catalog.groups.push(PreservationStorageGroup {
                    id: PreservationStorageGroupId(1),
                    name: "duplicate".into(),
                    ..catalog.groups[0].clone()
                });
            },
            Rejected::OverlappingPreservedUnit(RegisterUnitId(0)),
        );
        rejection(
            |catalog| catalog.groups.clear(),
            Rejected::MissingPreservedUnit(RegisterUnitId(0)),
        );
        rejection(
            |catalog| {
                catalog.groups = vec![
                    one_unit_group(0, "high", RegisterUnitId(1)),
                    one_unit_group(1, "low", RegisterUnitId(0)),
                ];
            },
            Rejected::StorageViewUnitMismatch(PreservationStorageGroupId(0)),
        );
    }
}
