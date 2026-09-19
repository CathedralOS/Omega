//! Register model tests: the physical model layout, the constraint catalog and its validation.

use super::{
    AARCH64_AAPCS64_CALL, AARCH64_AAPCS64_CALL_I64_PAIR_TO_I64, AARCH64_AAPCS64_RETURN,
    AARCH64_ADD_I64, AARCH64_ADD_I64_IMMEDIATE, AARCH64_COMPARE_I64_ZERO,
    AARCH64_CONDITIONAL_BRANCH, AARCH64_COPY_I64, AARCH64_LINUX_SYSTEM_CALL,
    AARCH64_MATERIALIZE_I64, AARCH64_REQUIRED_REGISTER_CONSTRAINTS, AARCH64_SUBTRACT_I64,
    Aarch64RegisterConstraintCatalogValidationError, aarch64_aapcs64_mixed_unit_call_keys,
    aarch64_aapcs64_normalized_foreign_call_keys, aarch64_darwin_mixed_unit_call_keys,
    aarch64_darwin_normalized_foreign_call_keys, aarch64_fixed_register_view,
    aarch64_float_scalar_call_keys, aarch64_float_scalar_return_keys,
    aarch64_indirect_aggregate_call_keys, aarch64_mixed_aggregate_call_keys,
    aarch64_physical_register_model, aarch64_preservation_convention_for_target,
    aarch64_register_aggregate_call_keys, aarch64_register_aggregate_return_keys,
    aarch64_register_constraint_catalog, validate_aarch64_register_constraint_catalog,
};
use crate::register_model::physical_model::GPR64;
use calling_conventions::MachineRegister;
use register_model::{
    RegisterConstraintCatalog, RegisterConstraintId, RegisterConstraintKey,
    RegisterInstructionConstraint, RegisterOperandAccess, RegisterUnit, RegisterUnitId,
    RegisterUnitKind,
};
use register_model::{
    RegisterConstraintCatalogValidationError, RegisterModelValidationError,
    validate_physical_register_model,
};
use target::NativeTarget;

fn row(
    catalog: &RegisterConstraintCatalog,
    key: RegisterConstraintKey,
) -> &RegisterInstructionConstraint {
    catalog
        .constraints
        .iter()
        .find(|row| row.key == key)
        .unwrap()
}

fn row_mut(
    catalog: &mut RegisterConstraintCatalog,
    key: RegisterConstraintKey,
) -> &mut RegisterInstructionConstraint {
    catalog
        .constraints
        .iter_mut()
        .find(|row| row.key == key)
        .unwrap()
}

#[test]
fn aggregate_result_fragments_are_definitions_not_unknown_clobbers() {
    let model = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    let catalog = aarch64_register_constraint_catalog(&model);
    let second = model.model().view_named("x1").unwrap();
    let scratch = model.model().view_named("x2").unwrap();
    for (ordinal, key) in [false, true]
        .into_iter()
        .flat_map(aarch64_register_aggregate_call_keys)
        .enumerate()
    {
        let fragments = ordinal % 18 / 9 + 1;
        let call = row(&catalog, key);
        assert_eq!(
            call.operands
                .iter()
                .filter(|operand| operand.access == RegisterOperandAccess::Def)
                .count(),
            fragments
        );
        for unit in &second.write_units {
            assert_eq!(call.clobbers.contains(unit), fragments == 1, "{key:?}");
        }
        assert!(
            scratch
                .write_units
                .iter()
                .all(|unit| call.clobbers.contains(unit))
        );
        let mut altered = catalog.clone();
        row_mut(&mut altered, key)
            .clobbers
            .retain(|unit| !scratch.write_units.contains(unit));
        assert!(validate_aarch64_register_constraint_catalog(altered, &model).is_err());
    }
}

#[test]
fn fixed_machine_register_views_are_target_owned() {
    let model = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    assert_eq!(
        aarch64_fixed_register_view(&model, MachineRegister::Aarch64X(0)),
        model.model().view_named("x0").map(|view| view.id)
    );
    assert_eq!(
        aarch64_fixed_register_view(&model, MachineRegister::X86Rdi),
        None
    );
}

#[test]
fn preservation_convention_is_selected_by_exact_target_policy() {
    let model = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    assert_eq!(
        aarch64_preservation_convention_for_target(&model, NativeTarget::linux_arm64())
            .unwrap()
            .name,
        "aapcs64"
    );
    assert_eq!(
        aarch64_preservation_convention_for_target(&model, NativeTarget::macos_arm64())
            .unwrap()
            .name,
        "darwin-aapcs64"
    );
    assert!(
        aarch64_preservation_convention_for_target(&model, NativeTarget::windows_x64()).is_none()
    );
}

#[test]
fn model_validates_without_collapsing_stack_and_zero_registers() {
    let validated = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    let model = validated.model();
    let view = |name| model.view_named(name).unwrap().id;
    assert!(model.aliases(view("x0"), view("w0")));
    assert!(model.aliases(view("q0"), view("d0")));
    assert!(!model.aliases(view("sp"), view("xzr")));
    let aapcs = model
        .conventions
        .iter()
        .find(|row| row.name == "aapcs64")
        .unwrap();
    let d8 = model.view_named("d8").unwrap();
    let q8 = model.view_named("q8").unwrap();
    assert!(aapcs.callee_saved.contains(&d8.units[0]));
    assert!(!aapcs.callee_saved.contains(&q8.units[1]));
    assert!(!model.view_named("sp").unwrap().allocatable);
    assert!(!model.view_named("xzr").unwrap().allocatable);
}

#[test]
fn register_constraint_catalog_closes_the_required_aarch64_inventory() {
    let model = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    let validated = validate_aarch64_register_constraint_catalog(
        aarch64_register_constraint_catalog(&model),
        &model,
    )
    .unwrap();
    let catalog = validated.catalog();
    assert!(
        AARCH64_REQUIRED_REGISTER_CONSTRAINTS
            .iter()
            .all(|key| catalog.required.contains(key))
    );
    assert_eq!(
        catalog.required.len(),
        AARCH64_REQUIRED_REGISTER_CONSTRAINTS.len()
            + aarch64_indirect_aggregate_call_keys(false).len()
            + aarch64_indirect_aggregate_call_keys(true).len()
            + aarch64_float_scalar_call_keys(false).len()
            + aarch64_float_scalar_call_keys(true).len()
            + aarch64_float_scalar_return_keys(false).len()
            + aarch64_float_scalar_return_keys(true).len()
            + aarch64_aapcs64_mixed_unit_call_keys().len()
            + aarch64_darwin_mixed_unit_call_keys().len()
            + aarch64_register_aggregate_call_keys(false).len()
            + aarch64_register_aggregate_call_keys(true).len()
            + aarch64_mixed_aggregate_call_keys(false).len()
            + aarch64_mixed_aggregate_call_keys(true).len()
            + aarch64_register_aggregate_return_keys(false).len()
            + aarch64_register_aggregate_return_keys(true).len()
            + aarch64_aapcs64_normalized_foreign_call_keys().len()
            + aarch64_darwin_normalized_foreign_call_keys().len()
    );

    let call = row(catalog, AARCH64_AAPCS64_CALL);
    assert_eq!(call.key, AARCH64_AAPCS64_CALL);
    assert_eq!(call.operands.len(), 9);
    assert_eq!(call.operands[8].access, RegisterOperandAccess::Def);
    for state in ["x30", "pc"] {
        assert!(
            model
                .model()
                .view_named(state)
                .unwrap()
                .units
                .iter()
                .all(|unit| call.implicit_defs.contains(unit))
        );
    }

    let scalar_call = row(catalog, AARCH64_AAPCS64_CALL_I64_PAIR_TO_I64);
    assert_eq!(scalar_call.key, AARCH64_AAPCS64_CALL_I64_PAIR_TO_I64);
    assert_eq!(scalar_call.operands.len(), 3);
    assert_eq!(
        scalar_call.operands[0].fixed_view,
        Some(model.model().view_named("x0").unwrap().id)
    );
    assert_eq!(
        scalar_call.operands[1].fixed_view,
        Some(model.model().view_named("x1").unwrap().id)
    );
    assert_eq!(
        scalar_call.operands[2].fixed_view,
        Some(model.model().view_named("x0").unwrap().id)
    );

    let returned = row(catalog, AARCH64_AAPCS64_RETURN);
    assert!(
        model
            .model()
            .view_named("x30")
            .unwrap()
            .units
            .iter()
            .all(|unit| returned.implicit_uses.contains(unit))
    );
    assert!(
        model
            .model()
            .view_named("pc")
            .unwrap()
            .units
            .iter()
            .all(|unit| returned.implicit_defs.contains(unit))
    );

    let syscall = row(catalog, AARCH64_LINUX_SYSTEM_CALL);
    assert_eq!(syscall.key, AARCH64_LINUX_SYSTEM_CALL);
    assert_eq!(syscall.operands[1].access, RegisterOperandAccess::UseDef);
    assert_eq!(
        syscall.operands[0].fixed_view,
        Some(model.model().view_named("x8").unwrap().id)
    );
    assert!(
        model
            .model()
            .view_named("nzcv")
            .unwrap()
            .units
            .iter()
            .all(|unit| syscall.clobbers.contains(unit))
    );

    let materialize = row(catalog, AARCH64_MATERIALIZE_I64);
    assert_eq!(materialize.key, AARCH64_MATERIALIZE_I64);
    assert_eq!(materialize.operands.len(), 1);
    assert_eq!(materialize.operands[0].access, RegisterOperandAccess::Def);
    assert_eq!(materialize.operands[0].class, GPR64);

    let copy = row(catalog, AARCH64_COPY_I64);
    assert_eq!(copy.key, AARCH64_COPY_I64);
    assert_eq!(copy.operands[0].access, RegisterOperandAccess::Use);
    assert_eq!(copy.operands[1].access, RegisterOperandAccess::Def);

    let compare = row(catalog, AARCH64_COMPARE_I64_ZERO);
    assert_eq!(compare.key, AARCH64_COMPARE_I64_ZERO);
    assert_eq!(compare.operands[0].class, GPR64);
    assert_eq!(
        compare.implicit_defs,
        model.model().view_named("nzcv").unwrap().units
    );

    let branch = row(catalog, AARCH64_CONDITIONAL_BRANCH);
    assert_eq!(branch.key, AARCH64_CONDITIONAL_BRANCH);
    for state in ["nzcv", "pc"] {
        assert!(
            model
                .model()
                .view_named(state)
                .unwrap()
                .units
                .iter()
                .all(|unit| branch.implicit_uses.contains(unit))
        );
    }

    let add = row(catalog, AARCH64_ADD_I64);
    assert_eq!(add.key, AARCH64_ADD_I64);
    assert_eq!(add.operands.len(), 3);
    assert_eq!(add.operands[0].access, RegisterOperandAccess::Use);
    assert_eq!(add.operands[1].access, RegisterOperandAccess::Use);
    assert_eq!(add.operands[2].access, RegisterOperandAccess::Def);
    assert!(add.operands.iter().all(|operand| operand.class == GPR64));
    assert!(add.operands.iter().all(|operand| operand.tied_to.is_none()));
    assert!(add.implicit_uses.is_empty());
    assert!(add.implicit_defs.is_empty());
    assert!(add.clobbers.is_empty());

    let add_immediate = row(catalog, AARCH64_ADD_I64_IMMEDIATE);
    assert_eq!(add_immediate.key, AARCH64_ADD_I64_IMMEDIATE);
    assert_eq!(add_immediate.operands.len(), 2);
    assert_eq!(add_immediate.operands[0].access, RegisterOperandAccess::Use);
    assert_eq!(add_immediate.operands[1].access, RegisterOperandAccess::Def);
    assert!(
        add_immediate
            .operands
            .iter()
            .all(|operand| operand.class == GPR64
                && operand.fixed_view.is_none()
                && operand.tied_to.is_none()
                && !operand.early_clobber)
    );
    assert!(add_immediate.implicit_uses.is_empty());
    assert!(add_immediate.implicit_defs.is_empty());
    assert!(add_immediate.clobbers.is_empty());

    let subtract = row(catalog, AARCH64_SUBTRACT_I64);
    assert_eq!(subtract.key, AARCH64_SUBTRACT_I64);
    assert_eq!(subtract.operands.len(), 3);
    assert_eq!(subtract.operands[0].access, RegisterOperandAccess::Use);
    assert_eq!(subtract.operands[1].access, RegisterOperandAccess::Use);
    assert_eq!(subtract.operands[2].access, RegisterOperandAccess::Def);
    assert!(
        subtract
            .operands
            .iter()
            .all(|operand| operand.class == GPR64
                && operand.fixed_view.is_none()
                && operand.tied_to.is_none()
                && !operand.early_clobber)
    );
    assert!(subtract.implicit_uses.is_empty());
    assert!(subtract.implicit_defs.is_empty());
    assert!(subtract.clobbers.is_empty());
    assert_eq!(
        branch.implicit_defs,
        model.model().view_named("pc").unwrap().units
    );
}

#[test]
fn every_missing_required_aarch64_constraint_rejects() {
    let model = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    for (position, expected) in aarch64_register_constraint_catalog(&model)
        .required
        .into_iter()
        .enumerate()
    {
        let mut catalog = aarch64_register_constraint_catalog(&model);
        catalog.constraints.remove(position);
        for (id, constraint) in catalog.constraints.iter_mut().enumerate() {
            constraint.id = RegisterConstraintId(u16::try_from(id).unwrap());
        }
        assert_eq!(
            validate_aarch64_register_constraint_catalog(catalog, &model),
            Err(Aarch64RegisterConstraintCatalogValidationError::Structural(
                RegisterConstraintCatalogValidationError::MissingRequiredConstraint(expected)
            ))
        );
    }
}

#[test]
fn aarch64_target_semantics_reject_class_compatible_corruption() {
    let model = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    let mut wrong_syscall_register = aarch64_register_constraint_catalog(&model);
    row_mut(&mut wrong_syscall_register, AARCH64_LINUX_SYSTEM_CALL).operands[5].fixed_view =
        Some(model.model().view_named("x3").unwrap().id);
    assert_eq!(
        validate_aarch64_register_constraint_catalog(wrong_syscall_register, &model),
        Err(
            Aarch64RegisterConstraintCatalogValidationError::TargetSemantics(
                AARCH64_LINUX_SYSTEM_CALL
            )
        )
    );

    let mut missing_nzcv = aarch64_register_constraint_catalog(&model);
    row_mut(&mut missing_nzcv, AARCH64_LINUX_SYSTEM_CALL)
        .clobbers
        .clear();
    assert_eq!(
        validate_aarch64_register_constraint_catalog(missing_nzcv, &model),
        Err(
            Aarch64RegisterConstraintCatalogValidationError::TargetSemantics(
                AARCH64_LINUX_SYSTEM_CALL
            )
        )
    );

    let mut missing_link_state = aarch64_register_constraint_catalog(&model);
    let x30 = model.model().view_named("x30").unwrap().units[0];
    row_mut(&mut missing_link_state, AARCH64_AAPCS64_CALL)
        .implicit_defs
        .retain(|unit| *unit != x30);
    assert_eq!(
        validate_aarch64_register_constraint_catalog(missing_link_state, &model),
        Err(Aarch64RegisterConstraintCatalogValidationError::TargetSemantics(AARCH64_AAPCS64_CALL))
    );
}

#[test]
fn aarch64_compare_rejects_one_field_missing_flags_definition() {
    let model = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    let mut catalog = aarch64_register_constraint_catalog(&model);
    row_mut(&mut catalog, AARCH64_COMPARE_I64_ZERO)
        .implicit_defs
        .clear();
    assert_eq!(
        validate_aarch64_register_constraint_catalog(catalog, &model),
        Err(
            Aarch64RegisterConstraintCatalogValidationError::TargetSemantics(
                AARCH64_COMPARE_I64_ZERO,
            )
        )
    );

    let mut immediate = aarch64_register_constraint_catalog(&model);
    row_mut(&mut immediate, AARCH64_ADD_I64_IMMEDIATE).operands[0].access =
        RegisterOperandAccess::Def;
    assert_eq!(
        validate_aarch64_register_constraint_catalog(immediate, &model),
        Err(
            Aarch64RegisterConstraintCatalogValidationError::TargetSemantics(
                AARCH64_ADD_I64_IMMEDIATE,
            )
        )
    );
}

#[test]
fn aarch64_add_rejects_one_field_operand_role_change() {
    let model = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    let mut catalog = aarch64_register_constraint_catalog(&model);
    row_mut(&mut catalog, AARCH64_ADD_I64).operands[1].access = RegisterOperandAccess::Def;
    assert_eq!(
        validate_aarch64_register_constraint_catalog(catalog, &model),
        Err(Aarch64RegisterConstraintCatalogValidationError::TargetSemantics(AARCH64_ADD_I64,))
    );

    let mut subtract = aarch64_register_constraint_catalog(&model);
    row_mut(&mut subtract, AARCH64_SUBTRACT_I64).operands[1].access = RegisterOperandAccess::Def;
    assert_eq!(
        validate_aarch64_register_constraint_catalog(subtract, &model),
        Err(
            Aarch64RegisterConstraintCatalogValidationError::TargetSemantics(AARCH64_SUBTRACT_I64,)
        )
    );

    let mut flag_clobber = aarch64_register_constraint_catalog(&model);
    row_mut(&mut flag_clobber, AARCH64_SUBTRACT_I64).clobbers =
        model.model().view_named("nzcv").unwrap().units.clone();
    assert_eq!(
        validate_aarch64_register_constraint_catalog(flag_clobber, &model),
        Err(
            Aarch64RegisterConstraintCatalogValidationError::TargetSemantics(AARCH64_SUBTRACT_I64,)
        )
    );
}

#[test]
fn aarch64_catalog_validation_rejects_same_architecture_forged_physical_model() {
    let canonical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    let catalog = aarch64_register_constraint_catalog(&canonical);
    let mut forged = aarch64_physical_register_model();
    forged.views[0].name = "forged.x0".into();
    let forged = validate_physical_register_model(forged).unwrap();
    assert_eq!(
        aarch64_fixed_register_view(&forged, MachineRegister::Aarch64X(0)),
        None
    );
    assert_eq!(
        validate_aarch64_register_constraint_catalog(catalog, &forged),
        Err(Aarch64RegisterConstraintCatalogValidationError::NonCanonicalPhysicalModel)
    );
}

#[test]
fn unknown_and_omitted_units_reject() {
    let mut unknown = aarch64_physical_register_model();
    unknown.views[0].units[0] = RegisterUnitId(u16::MAX);
    assert_eq!(
        validate_physical_register_model(unknown),
        Err(RegisterModelValidationError::UnknownUnit(RegisterUnitId(
            u16::MAX
        )))
    );

    let mut omitted = aarch64_physical_register_model();
    let omitted_id = RegisterUnitId(u16::try_from(omitted.units.len()).unwrap());
    omitted.units.push(RegisterUnit {
        id: omitted_id,
        name: "omitted.storage".into(),
        bits: 1,
        kind: RegisterUnitKind::Flags,
    });
    assert_eq!(
        validate_physical_register_model(omitted),
        Err(RegisterModelValidationError::UnitNotCovered(omitted_id))
    );
}
