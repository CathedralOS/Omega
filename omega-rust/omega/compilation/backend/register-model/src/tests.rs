//! Fixtures shared by the register-model tests: a miniature physical
//! register model and constraint catalog.

mod constraint_catalog;
mod declared_identities;
mod model_validation;
mod reservation_profiles;

use crate::{
    PhysicalRegisterModel, PreservationConvention, RegisterClass, RegisterClassId,
    RegisterConstraintCatalog, RegisterConstraintCatalogValidationError, RegisterConstraintId,
    RegisterConstraintKey, RegisterInstructionConstraint, RegisterModelValidationError,
    RegisterOperandAccess, RegisterOperandConstraint, RegisterReservationOverlay, RegisterUnit,
    RegisterUnitId, RegisterUnitKind, RegisterView, RegisterViewId, RegisterWriteSemantics,
    ReservationReason, ValidatedPhysicalRegisterModel, validate_physical_register_model,
    validate_register_constraint_catalog,
};
use target::Architecture;

type ModelMutation = Box<dyn Fn(&mut PhysicalRegisterModel)>;

type CatalogMutation = Box<dyn Fn(&mut RegisterConstraintCatalog)>;

/// Assert that a fresh fixture, corrupted by `mutate`, fails `validate` with
/// exactly `expected`. Every negative fixture test in this module tree spells
/// its expectation through this one owner.
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

/// The miniature catalog rejects the row `mutate` corrupts with `expected`.
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

/// The miniature model rejects the declaration `mutate` corrupts with `expected`.
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
