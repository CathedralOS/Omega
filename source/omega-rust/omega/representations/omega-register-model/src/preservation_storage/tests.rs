use super::*;
use crate::{
    PhysicalRegisterModel, PreservationConvention, RegisterClass, RegisterClassId, RegisterUnit,
    RegisterUnitId, RegisterUnitKind, RegisterView, RegisterViewId, RegisterWriteSemantics,
    validate_physical_register_model,
};
use omega_target::Architecture;

fn validated_model() -> crate::ValidatedPhysicalRegisterModel {
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

fn catalog(model: &crate::ValidatedPhysicalRegisterModel) -> PreservationStorageCatalog {
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
    let model = validated_model();

    let mut wrong_model = catalog(&model);
    wrong_model.physical_register_model = crate::PhysicalRegisterModelIdentity::from_bytes([7; 32]);
    assert_eq!(
        validate_preservation_storage_catalog(wrong_model, &model),
        Err(PreservationStorageCatalogValidationError::PhysicalRegisterModelMismatch)
    );

    let mut unknown_convention = catalog(&model);
    unknown_convention.convention = "unknown".into();
    assert_eq!(
        validate_preservation_storage_catalog(unknown_convention, &model),
        Err(PreservationStorageCatalogValidationError::UnknownConvention("unknown".into()))
    );

    let mut bad_id = catalog(&model);
    bad_id.groups[0].id = PreservationStorageGroupId(1);
    assert_eq!(
        validate_preservation_storage_catalog(bad_id, &model),
        Err(PreservationStorageCatalogValidationError::NonCanonicalGroupIds)
    );

    let mut bad_view = catalog(&model);
    bad_view.groups[0].storage_view = RegisterViewId(u16::MAX);
    assert_eq!(
        validate_preservation_storage_catalog(bad_view, &model),
        Err(
            PreservationStorageCatalogValidationError::UnknownStorageView {
                group: PreservationStorageGroupId(0),
                view: RegisterViewId(u16::MAX),
            }
        )
    );

    let mut wrong_size = catalog(&model);
    wrong_size.groups[0].size_bytes = 4;
    assert_eq!(
        validate_preservation_storage_catalog(wrong_size, &model),
        Err(
            PreservationStorageCatalogValidationError::StorageSizeMismatch(
                PreservationStorageGroupId(0)
            )
        )
    );

    let mut bad_alignment = catalog(&model);
    bad_alignment.groups[0].alignment_bytes = 3;
    assert_eq!(
        validate_preservation_storage_catalog(bad_alignment, &model),
        Err(
            PreservationStorageCatalogValidationError::InvalidStorageAlignment(
                PreservationStorageGroupId(0)
            )
        )
    );

    let mut missing = catalog(&model);
    missing.groups[0].preserved_units.pop();
    assert_eq!(
        validate_preservation_storage_catalog(missing, &model),
        Err(
            PreservationStorageCatalogValidationError::StorageViewUnitMismatch(
                PreservationStorageGroupId(0)
            )
        )
    );
}

#[test]
fn catalog_rejects_overlap_omission_and_noncanonical_group_order() {
    let model = validated_model();
    let one_unit_group = |id, name: &str, unit| PreservationStorageGroup {
        id: PreservationStorageGroupId(id),
        name: name.into(),
        storage_view: RegisterViewId(0),
        preserved_units: vec![unit],
        size_bytes: 8,
        alignment_bytes: 8,
    };

    let mut overlap = catalog(&model);
    overlap.groups.push(PreservationStorageGroup {
        id: PreservationStorageGroupId(1),
        name: "duplicate".into(),
        ..overlap.groups[0].clone()
    });
    assert_eq!(
        validate_preservation_storage_catalog(overlap, &model),
        Err(PreservationStorageCatalogValidationError::OverlappingPreservedUnit(RegisterUnitId(0)))
    );

    let mut omitted = catalog(&model);
    omitted.groups.clear();
    assert_eq!(
        validate_preservation_storage_catalog(omitted, &model),
        Err(PreservationStorageCatalogValidationError::MissingPreservedUnit(RegisterUnitId(0)))
    );

    // The view mismatch is checked before order for split groups; exact view
    // equality prevents a catalog from inventing partial storage carriers.
    let mut partial = catalog(&model);
    partial.groups = vec![
        one_unit_group(0, "high", RegisterUnitId(1)),
        one_unit_group(1, "low", RegisterUnitId(0)),
    ];
    assert_eq!(
        validate_preservation_storage_catalog(partial, &model),
        Err(
            PreservationStorageCatalogValidationError::StorageViewUnitMismatch(
                PreservationStorageGroupId(0)
            )
        )
    );
}
