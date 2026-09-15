use super::{instruction_key, miniature_catalog, miniature_model, validated_miniature_model};
use crate::{
    RegisterClassId, RegisterConstraintCatalogValidationError, RegisterConstraintId,
    RegisterUnitId, RegisterViewId, validate_physical_register_model,
    validate_register_constraint_catalog,
};
use target::Architecture;

#[test]
fn constraint_catalog_accepts_a_closed_required_inventory() {
    let validated =
        validate_register_constraint_catalog(miniature_catalog(), &validated_miniature_model())
            .expect("closed catalog must validate");

    assert_eq!(validated.architecture(), Architecture::X86_64);
    assert_eq!(validated.catalog().required, vec![instruction_key(7)]);
}

#[test]
fn constraint_catalog_rejects_missing_and_unexpected_inventory_rows() {
    let model = validated_miniature_model();
    let mut missing = miniature_catalog();
    missing.required.push(instruction_key(8));
    assert_eq!(
        validate_register_constraint_catalog(missing, &model),
        Err(
            RegisterConstraintCatalogValidationError::MissingRequiredConstraint(instruction_key(8))
        )
    );

    let mut unexpected = miniature_catalog();
    let mut row = unexpected.constraints[0].clone();
    row.id = RegisterConstraintId(1);
    row.key = instruction_key(8);
    unexpected.constraints.push(row);
    assert_eq!(
        validate_register_constraint_catalog(unexpected, &model),
        Err(RegisterConstraintCatalogValidationError::UnexpectedConstraint(instruction_key(8)))
    );
}

#[test]
fn constraint_catalog_rejects_noncanonical_ids_keys_and_operands() {
    let model = validated_miniature_model();
    let mut bad_id = miniature_catalog();
    bad_id.constraints[0].id = RegisterConstraintId(1);
    assert_eq!(
        validate_register_constraint_catalog(bad_id, &model),
        Err(RegisterConstraintCatalogValidationError::NonCanonicalConstraintIds)
    );

    let mut duplicate_required = miniature_catalog();
    duplicate_required.required.push(instruction_key(7));
    assert_eq!(
        validate_register_constraint_catalog(duplicate_required, &model),
        Err(RegisterConstraintCatalogValidationError::NonCanonicalRequiredKeys)
    );

    let mut duplicate_key = miniature_catalog();
    let mut second_row = duplicate_key.constraints[0].clone();
    second_row.id = RegisterConstraintId(1);
    duplicate_key.constraints.push(second_row);
    assert_eq!(
        validate_register_constraint_catalog(duplicate_key, &model),
        Err(RegisterConstraintCatalogValidationError::NonCanonicalConstraintKeys)
    );

    let mut duplicate_operand = miniature_catalog();
    duplicate_operand.constraints[0].operands[1].operand = 0;
    assert_eq!(
        validate_register_constraint_catalog(duplicate_operand, &model),
        Err(
            RegisterConstraintCatalogValidationError::NonCanonicalOperands(RegisterConstraintId(0))
        )
    );
}

#[test]
fn constraint_catalog_rejects_fixed_view_class_corruption() {
    let model = validated_miniature_model();
    let mut corrupted = miniature_catalog();
    corrupted.constraints[0].operands[1].class = RegisterClassId(1);

    assert_eq!(
        validate_register_constraint_catalog(corrupted, &model),
        Err(
            RegisterConstraintCatalogValidationError::FixedViewClassMismatch {
                constraint: RegisterConstraintId(0),
                operand: 1,
            }
        )
    );
}

#[test]
fn constraint_catalog_rejects_unknown_or_unallocatable_operand_domains() {
    let model = validated_miniature_model();
    let mut unknown_class = miniature_catalog();
    unknown_class.constraints[0].operands[0].class = RegisterClassId(u16::MAX);
    assert_eq!(
        validate_register_constraint_catalog(unknown_class, &model),
        Err(RegisterConstraintCatalogValidationError::UnknownClass {
            constraint: RegisterConstraintId(0),
            class: RegisterClassId(u16::MAX),
        })
    );

    let mut unknown_view = miniature_catalog();
    unknown_view.constraints[0].operands[1].fixed_view = Some(RegisterViewId(u16::MAX));
    assert_eq!(
        validate_register_constraint_catalog(unknown_view, &model),
        Err(RegisterConstraintCatalogValidationError::UnknownFixedView {
            constraint: RegisterConstraintId(0),
            view: RegisterViewId(u16::MAX),
        })
    );

    let mut physical = miniature_model();
    physical.views[1].allocatable = false;
    let physical = validate_physical_register_model(physical).unwrap();
    let mut unallocatable = miniature_catalog();
    unallocatable.constraints[0].operands[0].class = RegisterClassId(1);
    assert_eq!(
        validate_register_constraint_catalog(unallocatable, &physical),
        Err(
            RegisterConstraintCatalogValidationError::UnallocatableOperandClass {
                constraint: RegisterConstraintId(0),
                operand: 0,
            }
        )
    );

    let mut empty = miniature_catalog();
    let row = &mut empty.constraints[0];
    row.operands.clear();
    row.implicit_uses.clear();
    row.implicit_defs.clear();
    row.clobbers.clear();
    assert_eq!(
        validate_register_constraint_catalog(empty, &model),
        Err(RegisterConstraintCatalogValidationError::EmptyConstraint(
            RegisterConstraintId(0)
        ))
    );
}

#[test]
fn constraint_catalog_rejects_malformed_ties_and_early_clobbers() {
    let model = validated_miniature_model();
    let mut self_tie = miniature_catalog();
    self_tie.constraints[0].operands[1].tied_to = Some(1);
    assert_eq!(
        validate_register_constraint_catalog(self_tie, &model),
        Err(
            RegisterConstraintCatalogValidationError::InvalidOperandTie {
                constraint: RegisterConstraintId(0),
                operand: 1,
            }
        )
    );

    let mut dangling_tie = miniature_catalog();
    dangling_tie.constraints[0].operands[1].operand = 2;
    dangling_tie.constraints[0].operands[1].tied_to = Some(1);
    assert_eq!(
        validate_register_constraint_catalog(dangling_tie, &model),
        Err(
            RegisterConstraintCatalogValidationError::InvalidOperandTie {
                constraint: RegisterConstraintId(0),
                operand: 2,
            }
        )
    );

    let mut incompatible_tie = miniature_catalog();
    incompatible_tie.constraints[0].operands[1].class = RegisterClassId(1);
    incompatible_tie.constraints[0].operands[1].fixed_view = None;
    assert_eq!(
        validate_register_constraint_catalog(incompatible_tie, &model),
        Err(
            RegisterConstraintCatalogValidationError::IncompatibleOperandTie {
                constraint: RegisterConstraintId(0),
                operand: 1,
                tied_to: 0,
            }
        )
    );

    let mut early_use = miniature_catalog();
    early_use.constraints[0].operands[0].early_clobber = true;
    assert_eq!(
        validate_register_constraint_catalog(early_use, &model),
        Err(
            RegisterConstraintCatalogValidationError::InvalidEarlyClobber {
                constraint: RegisterConstraintId(0),
                operand: 0,
            }
        )
    );
}

#[test]
fn constraint_catalog_rejects_implicit_effect_corruption() {
    let model = validated_miniature_model();
    let mut duplicate_use = miniature_catalog();
    duplicate_use.constraints[0]
        .implicit_uses
        .push(RegisterUnitId(0));
    assert_eq!(
        validate_register_constraint_catalog(duplicate_use, &model),
        Err(
            RegisterConstraintCatalogValidationError::NonCanonicalImplicitUses(
                RegisterConstraintId(0)
            )
        )
    );

    let mut unknown_clobber = miniature_catalog();
    unknown_clobber.constraints[0].clobbers = vec![RegisterUnitId(2)];
    assert_eq!(
        validate_register_constraint_catalog(unknown_clobber, &model),
        Err(RegisterConstraintCatalogValidationError::UnknownUnit {
            constraint: RegisterConstraintId(0),
            unit: RegisterUnitId(2),
        })
    );

    let mut contradictory_write = miniature_catalog();
    contradictory_write.constraints[0].implicit_defs = vec![RegisterUnitId(1)];
    assert_eq!(
        validate_register_constraint_catalog(contradictory_write, &model),
        Err(
            RegisterConstraintCatalogValidationError::DefClobberOverlap {
                constraint: RegisterConstraintId(0),
                unit: RegisterUnitId(1),
            }
        )
    );
}

#[test]
fn constraint_catalog_is_bound_to_the_validated_model_architecture() {
    let mut wrong_architecture = miniature_catalog();
    wrong_architecture.architecture = Architecture::Aarch64;

    assert_eq!(
        validate_register_constraint_catalog(wrong_architecture, &validated_miniature_model()),
        Err(RegisterConstraintCatalogValidationError::ArchitectureMismatch)
    );
}
