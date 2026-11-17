/*
We provide our miniature physical-register model and constraint-catalog test
fixtures along with unit tests for physical-model validation and reservation
profile validation.
*/

mod constraint_catalog;
mod declared_identities;

use crate::{
    PhysicalRegisterModel, PreservationConvention, RegisterClass, RegisterClassId,
    RegisterConstraintCatalog, RegisterConstraintCatalogValidationError, RegisterConstraintId,
    RegisterConstraintKey, RegisterInstructionConstraint, RegisterModelValidationError,
    RegisterOperandAccess, RegisterOperandConstraint, RegisterReservationOverlay,
    RegisterReservationProfile, RegisterReservationProfileValidationError, RegisterUnit,
    RegisterUnitId, RegisterUnitKind, RegisterView, RegisterViewId, RegisterWriteSemantics,
    ReservationReason, ValidatedPhysicalRegisterModel, validate_physical_register_model,
    validate_register_constraint_catalog, validate_register_reservation_profile,
};
use target::Architecture;

type ModelMutation = Box<dyn Fn(&mut PhysicalRegisterModel)>;

type CatalogMutation = Box<dyn Fn(&mut RegisterConstraintCatalog)>;

pub(crate) fn assert_rejected<T, V, E>(
    mut fixture: T,
    mutate: impl FnOnce(&mut T),
    validate: impl FnOnce(T) -> Result<V, E>,
    expected: E,
) where
    E: std::fmt::Debug + PartialEq,
    V: std::fmt::Debug + PartialEq,
{
    mutate(&mut fixture);
    assert_eq!(validate(fixture), Err(expected));
}

pub(crate) fn catalog_rejection(
    mutate: impl FnOnce(&mut RegisterConstraintCatalog),
    expected: RegisterConstraintCatalogValidationError,
) {
    assert_rejected(
        miniature_catalog(),
        mutate,
        |catalog| validate_register_constraint_catalog(catalog, &validated_miniature_model()),
        expected,
    );
}

pub(crate) fn model_rejection(
    mutate: impl FnOnce(&mut PhysicalRegisterModel),
    expected: RegisterModelValidationError,
) {
    assert_rejected(
        miniature_model(),
        mutate,
        validate_physical_register_model,
        expected,
    );
}

fn miniature_model() -> PhysicalRegisterModel {
    PhysicalRegisterModel {
        architecture: Architecture::X86_64,
        units: vec![
            RegisterUnit {
                id: RegisterUnitId(0),
                name: "r0.storage".into(),
                bits: 64,
                kind: RegisterUnitKind::IntegerLane,
            },
            RegisterUnit {
                id: RegisterUnitId(1),
                name: "v0.storage".into(),
                bits: 128,
                kind: RegisterUnitKind::VectorLane,
            },
        ],
        views: vec![
            RegisterView {
                id: RegisterViewId(0),
                name: "r0".into(),
                class: RegisterClassId(0),
                units: vec![RegisterUnitId(0)],
                write_units: vec![RegisterUnitId(0)],
                bits: 64,
                write_semantics: RegisterWriteSemantics::ExactView,
                allocatable: true,
            },
            RegisterView {
                id: RegisterViewId(1),
                name: "v0".into(),
                class: RegisterClassId(1),
                units: vec![RegisterUnitId(1)],
                write_units: vec![RegisterUnitId(1)],
                bits: 128,
                write_semantics: RegisterWriteSemantics::ExactView,
                allocatable: true,
            },
        ],
        classes: vec![
            RegisterClass {
                id: RegisterClassId(0),
                name: "integer".into(),
                views: vec![RegisterViewId(0)],
            },
            RegisterClass {
                id: RegisterClassId(1),
                name: "vector".into(),
                views: vec![RegisterViewId(1)],
            },
        ],
        conventions: vec![PreservationConvention {
            name: "test-call".into(),
            argument_views: vec![RegisterViewId(0)],
            result_views: vec![RegisterViewId(0)],
            caller_saved: vec![RegisterUnitId(0)],
            callee_saved: vec![RegisterUnitId(1)],
            fixed: Vec::new(),
            stack_alignment: 16,
            red_zone_bytes: 0,
        }],
        reservations: vec![
            RegisterReservationOverlay {
                name: "test.reserve-r0".into(),
                reason: ReservationReason::Backend,
                units: vec![RegisterUnitId(0)],
            },
            RegisterReservationOverlay {
                name: "test.reserve-v0".into(),
                reason: ReservationReason::InlineAssembly,
                units: vec![RegisterUnitId(1)],
            },
        ],
    }
}

fn instruction_key(variant: u32) -> RegisterConstraintKey {
    RegisterConstraintKey::instruction(variant)
}

fn miniature_catalog() -> RegisterConstraintCatalog {
    let key = instruction_key(7);
    RegisterConstraintCatalog {
        architecture: Architecture::X86_64,
        required: vec![key],
        constraints: vec![RegisterInstructionConstraint {
            id: RegisterConstraintId(0),
            key,
            operands: vec![
                RegisterOperandConstraint {
                    operand: 0,
                    access: RegisterOperandAccess::Use,
                    class: RegisterClassId(0),
                    fixed_view: None,
                    tied_to: None,
                    early_clobber: false,
                },
                RegisterOperandConstraint {
                    operand: 1,
                    access: RegisterOperandAccess::Def,
                    class: RegisterClassId(0),
                    fixed_view: Some(RegisterViewId(0)),
                    tied_to: Some(0),
                    early_clobber: true,
                },
            ],
            implicit_uses: vec![RegisterUnitId(0)],
            implicit_defs: Vec::new(),
            clobbers: vec![RegisterUnitId(1)],
        }],
    }
}

fn validated_miniature_model() -> ValidatedPhysicalRegisterModel {
    validate_physical_register_model(miniature_model()).expect("miniature model must validate")
}

#[test]
fn validator_accepts_a_closed_model_and_rejects_noncanonical_units() {
    assert!(validate_physical_register_model(miniature_model()).is_ok());
    model_rejection(
        |model| model.views[0].units.push(RegisterUnitId(0)),
        RegisterModelValidationError::NonCanonicalUnitSet,
    );
}

#[test]
fn validator_rejects_a_false_zero_extension_footprint() {
    model_rejection(
        |model| model.views[0].write_semantics = RegisterWriteSemantics::ZeroExtendsParent,
        RegisterModelValidationError::WriteFootprintMismatch(RegisterViewId(0)),
    );
}

#[test]
fn active_reservation_profile_is_exact_canonical_and_model_bound() {
    let model = validated_miniature_model();
    let target = target::NativeTarget::linux_x64();
    let profile = RegisterReservationProfile {
        name: "test.policy".into(),
        active_overlays: vec!["test.reserve-r0".into(), "test.reserve-v0".into()],
    };
    let validated = validate_register_reservation_profile(profile.clone(), target, &model)
        .expect("canonical profile must validate");
    assert_eq!(
        validated.reserved_units(),
        &[RegisterUnitId(0), RegisterUnitId(1)]
    );
    assert_eq!(
        validated.identity(),
        validate_register_reservation_profile(profile.clone(), target, &model)
            .unwrap()
            .identity()
    );

    let one_overlay = validate_register_reservation_profile(
        RegisterReservationProfile {
            name: profile.name.clone(),
            active_overlays: vec!["test.reserve-r0".into()],
        },
        target,
        &model,
    )
    .unwrap();
    assert_eq!(one_overlay.reserved_units(), &[RegisterUnitId(0)]);
    assert_ne!(one_overlay.identity(), validated.identity());

    let renamed = validate_register_reservation_profile(
        RegisterReservationProfile {
            name: "test.policy-renamed".into(),
            active_overlays: profile.active_overlays.clone(),
        },
        target,
        &model,
    )
    .unwrap();
    assert_ne!(renamed.identity(), validated.identity());
    let windows = validate_register_reservation_profile(
        profile.clone(),
        target::NativeTarget::windows_x64(),
        &model,
    )
    .unwrap();
    assert_ne!(windows.identity(), validated.identity());
    let mut changed_model = miniature_model();
    changed_model.units[0].name.push_str(".changed");
    let changed_model = validate_physical_register_model(changed_model).unwrap();
    let changed_model_profile =
        validate_register_reservation_profile(profile.clone(), target, &changed_model).unwrap();
    assert_ne!(changed_model_profile.identity(), validated.identity());

    let mut duplicate = profile.clone();
    duplicate.active_overlays[1] = duplicate.active_overlays[0].clone();
    assert_eq!(
        validate_register_reservation_profile(duplicate, target, &model),
        Err(RegisterReservationProfileValidationError::NonCanonicalOverlayNames)
    );
    let unknown = RegisterReservationProfile {
        name: profile.name,
        active_overlays: vec!["unknown".into()],
    };
    assert_eq!(
        validate_register_reservation_profile(unknown, target, &model),
        Err(RegisterReservationProfileValidationError::UnknownOverlay(
            "unknown".into()
        ))
    );
    assert_eq!(
        validate_register_reservation_profile(
            RegisterReservationProfile {
                name: "test.policy".into(),
                active_overlays: Vec::new(),
            },
            target::NativeTarget::linux_arm64(),
            &model,
        ),
        Err(RegisterReservationProfileValidationError::TargetArchitectureMismatch)
    );
}
