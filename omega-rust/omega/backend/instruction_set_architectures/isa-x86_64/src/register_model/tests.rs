//! Register model tests: the physical model layout, the constraint catalog and its validation.

use super::{
    X86_64_ADD_I64, X86_64_ADD_I64_IMMEDIATE, X86_64_COMPARE_I64_ZERO, X86_64_CONDITIONAL_BRANCH,
    X86_64_COPY_I64, X86_64_LINUX_SYSTEM_CALL, X86_64_MATERIALIZE_I64, X86_64_MICROSOFT_CALL_UNIT,
    X86_64_MICROSOFT_RETURN, X86_64_REMAINDER_I64, X86_64_REQUIRED_REGISTER_CONSTRAINTS,
    X86_64_SUBTRACT_I64, X86_64_SYSTEM_V_CALL, X86_64_SYSTEM_V_CALL_I64_PAIR_TO_I64,
    X86_64RegisterConstraintCatalogValidationError, validate_x86_64_register_constraint_catalog,
    x86_64_fixed_register_view, x86_64_float_scalar_call_keys, x86_64_float_scalar_return_keys,
    x86_64_indirect_aggregate_call_keys, x86_64_microsoft_aggregate_call_keys,
    x86_64_microsoft_aggregate_return_keys, x86_64_microsoft_mixed_aggregate_call_keys,
    x86_64_microsoft_mixed_unit_call_keys, x86_64_microsoft_normalized_foreign_call_keys,
    x86_64_physical_register_model, x86_64_preservation_convention_for_target,
    x86_64_register_constraint_catalog, x86_64_system_v_aggregate_call_keys,
    x86_64_system_v_aggregate_return_keys, x86_64_system_v_mixed_aggregate_call_keys,
    x86_64_system_v_mixed_unit_call_keys, x86_64_system_v_normalized_foreign_call_keys,
};
use crate::register_model::physical_model::GPR64;
use crate::register_model::physical_model::VECTOR128;
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
    let model = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let catalog = x86_64_register_constraint_catalog(&model);
    let second = model.model().view_named("rdx").unwrap();
    let scratch = model.model().view_named("rcx").unwrap();
    for (ordinal, key) in x86_64_system_v_aggregate_call_keys()
        .into_iter()
        .enumerate()
    {
        let fragments = ordinal % 14 / 7 + 1;
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
        assert!(validate_x86_64_register_constraint_catalog(altered, &model).is_err());
    }
}

#[test]
fn fixed_machine_register_views_are_target_owned() {
    let model = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    assert_eq!(
        x86_64_fixed_register_view(&model, MachineRegister::X86Rdi),
        model.model().view_named("rdi").map(|view| view.id)
    );
    assert_eq!(
        x86_64_fixed_register_view(&model, MachineRegister::Aarch64X(0)),
        None
    );
}

#[test]
fn microsoft_direct_aggregate_rows_preserve_positional_arguments_and_volatile_units() {
    let model = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let catalog = x86_64_register_constraint_catalog(&model);
    let rax = model.model().view_named("rax").unwrap();
    let rdx = model.model().view_named("rdx").unwrap();
    let rsi = model.model().view_named("rsi").unwrap();
    for (arity, key) in x86_64_microsoft_aggregate_call_keys()
        .into_iter()
        .enumerate()
    {
        let call = row(&catalog, key);
        let expected = ["rcx", "rdx", "r8", "r9"]
            .into_iter()
            .take(arity)
            .chain(["rax"])
            .map(|name| model.model().view_named(name).unwrap().id)
            .collect::<Vec<_>>();
        assert_eq!(
            call.operands
                .iter()
                .map(|operand| operand.fixed_view.unwrap())
                .collect::<Vec<_>>(),
            expected
        );
        assert_eq!(
            call.operands.last().unwrap().access,
            RegisterOperandAccess::Def
        );
        assert!(
            rax.write_units
                .iter()
                .all(|unit| !call.clobbers.contains(unit))
        );
        assert!(
            rdx.write_units
                .iter()
                .all(|unit| call.clobbers.contains(unit))
        );
        assert!(
            rsi.write_units
                .iter()
                .all(|unit| !call.clobbers.contains(unit))
        );
        let mut changed = catalog.clone();
        row_mut(&mut changed, key)
            .operands
            .last_mut()
            .unwrap()
            .fixed_view = Some(rdx.id);
        assert!(validate_x86_64_register_constraint_catalog(changed, &model).is_err());
    }
    let returns = x86_64_microsoft_aggregate_return_keys();
    assert_eq!(returns.len(), 1);
    assert_eq!(row(&catalog, returns[0]).operands.len(), 1);
    assert_eq!(
        row(&catalog, returns[0]).operands[0].fixed_view,
        Some(rax.id)
    );
}

#[test]
fn preservation_convention_is_selected_by_exact_target_policy() {
    let model = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let system_v =
        x86_64_preservation_convention_for_target(&model, NativeTarget::linux_x64()).unwrap();
    let microsoft =
        x86_64_preservation_convention_for_target(&model, NativeTarget::windows_x64()).unwrap();
    assert_eq!(system_v.name, "system-v-amd64");
    assert_eq!(microsoft.name, "microsoft-x64");
    let rsi = model.model().view_named("rsi").unwrap().units[0];
    assert!(!system_v.callee_saved.contains(&rsi));
    assert!(microsoft.callee_saved.contains(&rsi));
    assert!(
        x86_64_preservation_convention_for_target(&model, NativeTarget::macos_arm64()).is_none()
    );
}

#[test]
fn model_validates_and_partial_register_aliases_are_exact() {
    let validated = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let model = validated.model();
    let view = |name| model.view_named(name).unwrap().id;
    assert!(model.aliases(view("rax"), view("eax")));
    assert!(model.aliases(view("rax"), view("ah")));
    assert!(!model.aliases(view("al"), view("ah")));
    assert!(!model.aliases(view("rax"), view("xmm0")));
    assert_eq!(
        model.view_named("eax").unwrap().write_units,
        model.view_named("rax").unwrap().units
    );
    assert!(!model.view_named("ah").unwrap().allocatable);
    assert!(!model.view_named("rsp").unwrap().allocatable);
}

#[test]
fn register_constraint_catalog_closes_the_required_x86_64_inventory() {
    let model = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let validated = validate_x86_64_register_constraint_catalog(
        x86_64_register_constraint_catalog(&model),
        &model,
    )
    .unwrap();
    let catalog = validated.catalog();
    assert!(
        X86_64_REQUIRED_REGISTER_CONSTRAINTS
            .iter()
            .all(|key| catalog.required.contains(key))
    );
    assert_eq!(
        catalog.required.len(),
        X86_64_REQUIRED_REGISTER_CONSTRAINTS.len()
            + x86_64_indirect_aggregate_call_keys(false).len()
            + x86_64_indirect_aggregate_call_keys(true).len()
            + x86_64_float_scalar_call_keys(false).len()
            + x86_64_float_scalar_call_keys(true).len()
            + x86_64_float_scalar_return_keys(false).len()
            + x86_64_float_scalar_return_keys(true).len()
            + 2
            + x86_64_system_v_mixed_unit_call_keys().len()
            + x86_64_microsoft_mixed_unit_call_keys().len()
            + x86_64_system_v_mixed_aggregate_call_keys().len()
            + x86_64_microsoft_mixed_aggregate_call_keys().len()
            + x86_64_system_v_aggregate_call_keys().len()
            + x86_64_system_v_aggregate_return_keys().len()
            + x86_64_microsoft_aggregate_call_keys().len()
            + x86_64_microsoft_aggregate_return_keys().len()
            + x86_64_system_v_normalized_foreign_call_keys().len()
            + x86_64_microsoft_normalized_foreign_call_keys().len()
    );

    let sysv_call = row(catalog, X86_64_SYSTEM_V_CALL);
    assert_eq!(sysv_call.key, X86_64_SYSTEM_V_CALL);
    assert_eq!(sysv_call.operands.len(), 7);
    assert_eq!(sysv_call.operands[6].access, RegisterOperandAccess::Def);
    assert_eq!(
        sysv_call.operands[6].fixed_view,
        Some(model.model().view_named("rax").unwrap().id)
    );
    assert!(
        model
            .model()
            .view_named("rsp")
            .unwrap()
            .units
            .iter()
            .all(|unit| sysv_call.implicit_uses.contains(unit)
                && sysv_call.implicit_defs.contains(unit))
    );

    let unit_call = row(catalog, X86_64_MICROSOFT_CALL_UNIT);
    assert_eq!(unit_call.key, X86_64_MICROSOFT_CALL_UNIT);
    assert_eq!(unit_call.operands.len(), 2);
    for (operand, name) in unit_call.operands.iter().zip(["rcx", "rdx"]) {
        assert_eq!(operand.access, RegisterOperandAccess::Use);
        assert_eq!(
            operand.fixed_view,
            Some(model.model().view_named(name).unwrap().id)
        );
    }
    for used in ["rsp", "rip"] {
        assert!(
            model
                .model()
                .view_named(used)
                .unwrap()
                .units
                .iter()
                .all(|unit| unit_call.implicit_uses.contains(unit))
        );
    }
    for clobbered in [
        "rax", "rcx", "rdx", "r8", "r9", "r10", "r11", "xmm0", "xmm1", "xmm2", "xmm3", "xmm4",
        "xmm5", "rflags",
    ] {
        assert!(
            model
                .model()
                .view_named(clobbered)
                .unwrap()
                .units
                .iter()
                .all(|unit| unit_call.clobbers.contains(unit))
        );
    }
    let scalar_call = row(catalog, X86_64_SYSTEM_V_CALL_I64_PAIR_TO_I64);
    assert_eq!(scalar_call.key, X86_64_SYSTEM_V_CALL_I64_PAIR_TO_I64);
    assert_eq!(scalar_call.operands.len(), 3);
    assert_eq!(
        scalar_call.operands[0].fixed_view,
        Some(model.model().view_named("rdi").unwrap().id)
    );
    assert_eq!(
        scalar_call.operands[1].fixed_view,
        Some(model.model().view_named("rsi").unwrap().id)
    );
    assert_eq!(
        scalar_call.operands[2].fixed_view,
        Some(model.model().view_named("rax").unwrap().id)
    );

    let syscall = row(catalog, X86_64_LINUX_SYSTEM_CALL);
    assert_eq!(syscall.key, X86_64_LINUX_SYSTEM_CALL);
    assert_eq!(syscall.operands[0].access, RegisterOperandAccess::UseDef);
    assert_eq!(
        syscall.operands[0].fixed_view,
        Some(model.model().view_named("rax").unwrap().id)
    );
    for clobbered in ["rcx", "r11", "rflags"] {
        assert!(
            model
                .model()
                .view_named(clobbered)
                .unwrap()
                .units
                .iter()
                .all(|unit| syscall.clobbers.contains(unit))
        );
    }

    let materialize = row(catalog, X86_64_MATERIALIZE_I64);
    assert_eq!(materialize.key, X86_64_MATERIALIZE_I64);
    assert_eq!(materialize.operands.len(), 1);
    assert_eq!(materialize.operands[0].access, RegisterOperandAccess::Def);
    assert_eq!(materialize.operands[0].class, GPR64);

    let copy = row(catalog, X86_64_COPY_I64);
    assert_eq!(copy.key, X86_64_COPY_I64);
    assert_eq!(copy.operands[0].access, RegisterOperandAccess::Use);
    assert_eq!(copy.operands[1].access, RegisterOperandAccess::Def);

    let compare = row(catalog, X86_64_COMPARE_I64_ZERO);
    assert_eq!(compare.key, X86_64_COMPARE_I64_ZERO);
    assert_eq!(compare.operands[0].class, GPR64);
    assert_eq!(
        compare.implicit_defs,
        model.model().view_named("rflags").unwrap().units
    );

    let branch = row(catalog, X86_64_CONDITIONAL_BRANCH);
    assert_eq!(branch.key, X86_64_CONDITIONAL_BRANCH);
    for state in ["rflags", "rip"] {
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

    let add = row(catalog, X86_64_ADD_I64);
    assert_eq!(add.key, X86_64_ADD_I64);
    assert_eq!(add.operands.len(), 3);
    assert_eq!(add.operands[0].access, RegisterOperandAccess::Use);
    assert_eq!(add.operands[1].access, RegisterOperandAccess::Use);
    assert_eq!(add.operands[2].access, RegisterOperandAccess::Def);
    assert!(add.operands.iter().all(|operand| operand.class == GPR64));
    assert!(add.operands.iter().all(|operand| operand.tied_to.is_none()));
    assert!(add.implicit_uses.is_empty());
    assert!(add.implicit_defs.is_empty());
    assert!(add.clobbers.is_empty());

    let add_immediate = row(catalog, X86_64_ADD_I64_IMMEDIATE);
    assert_eq!(add_immediate.key, X86_64_ADD_I64_IMMEDIATE);
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

    let subtract = row(catalog, X86_64_SUBTRACT_I64);
    assert_eq!(subtract.key, X86_64_SUBTRACT_I64);
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
    assert_eq!(
        subtract.clobbers,
        model.model().view_named("rflags").unwrap().units
    );
    assert_eq!(
        branch.implicit_defs,
        model.model().view_named("rip").unwrap().units
    );
}

#[test]
fn signed_remainder_row_keeps_the_divisor_out_of_rdx() {
    let model = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let catalog = x86_64_register_constraint_catalog(&model);
    let rax = model.model().view_named("rax").unwrap().id;
    let rcx = model.model().view_named("rcx").unwrap().id;
    let rdx = model.model().view_named("rdx").unwrap();
    let remainder = row(&catalog, X86_64_REMAINDER_I64);
    assert_eq!(remainder.key, X86_64_REMAINDER_I64);
    assert_eq!(remainder.operands.len(), 4);
    for (operand, access, fixed) in [
        (0, RegisterOperandAccess::Use, Some(rax)),
        (1, RegisterOperandAccess::Use, Some(rcx)),
        (2, RegisterOperandAccess::Def, Some(rax)),
        (3, RegisterOperandAccess::Def, Some(rdx.id)),
    ] {
        assert_eq!(remainder.operands[operand].operand, operand as u16);
        assert_eq!(remainder.operands[operand].access, access);
        assert_eq!(remainder.operands[operand].fixed_view, fixed);
        assert!(
            !remainder.operands[operand].early_clobber,
            "operand {operand} must not be early-clobber"
        );
        assert_eq!(remainder.operands[operand].class, GPR64);
    }
    assert!(remainder.implicit_uses.is_empty());
    assert!(remainder.implicit_defs.is_empty());
    assert_eq!(
        remainder.clobbers,
        model.model().view_named("rflags").unwrap().units
    );

    // A fixed early-clobber definition cannot participate in the allocation
    // model: restating operand 3 as an early-clobber Def must reject even
    // though it matches the old row this contract replaced.
    let mut changed = x86_64_register_constraint_catalog(&model);
    let operand = &mut row_mut(&mut changed, X86_64_REMAINDER_I64).operands[3];
    operand.early_clobber = true;
    assert_eq!(
        validate_x86_64_register_constraint_catalog(changed, &model),
        Err(
            X86_64RegisterConstraintCatalogValidationError::TargetSemanticMismatch(
                X86_64_REMAINDER_I64,
            )
        )
    );

    // The divisor pin is load-bearing: an allocatable divisor could be homed
    // in RDX, whose zeroing precedes the divisor read in the realized form.
    let mut changed = x86_64_register_constraint_catalog(&model);
    let operand = &mut row_mut(&mut changed, X86_64_REMAINDER_I64).operands[1];
    operand.fixed_view = None;
    assert_eq!(
        validate_x86_64_register_constraint_catalog(changed, &model),
        Err(
            X86_64RegisterConstraintCatalogValidationError::TargetSemanticMismatch(
                X86_64_REMAINDER_I64,
            )
        )
    );
}

#[test]
fn no_fixed_view_operand_is_early_clobber() {
    let model = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let catalog = x86_64_register_constraint_catalog(&model);
    for constraint in &catalog.constraints {
        for operand in &constraint.operands {
            assert!(
                !(operand.early_clobber && operand.fixed_view.is_some()),
                "{:?} operand {} pins an early-clobber fixed view, which \
                 fixed-precolored interval validation rejects",
                constraint.key,
                operand.operand,
            );
        }
    }
}

#[test]
fn missing_required_x86_64_constraint_rejects() {
    let model = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let mut catalog = x86_64_register_constraint_catalog(&model);
    catalog
        .constraints
        .retain(|constraint| constraint.key != X86_64_MICROSOFT_RETURN);
    for (id, constraint) in catalog.constraints.iter_mut().enumerate() {
        constraint.id = RegisterConstraintId(u16::try_from(id).unwrap());
    }
    assert_eq!(
        validate_x86_64_register_constraint_catalog(catalog, &model),
        Err(X86_64RegisterConstraintCatalogValidationError::Structural(
            RegisterConstraintCatalogValidationError::MissingRequiredConstraint(
                X86_64_MICROSOFT_RETURN,
            ),
        )),
    );
}

#[test]
fn target_inventory_cannot_erase_a_required_key_and_its_row_together() {
    let model = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let mut catalog = x86_64_register_constraint_catalog(&model);
    catalog
        .required
        .retain(|key| *key != X86_64_MICROSOFT_RETURN);
    catalog
        .constraints
        .retain(|constraint| constraint.key != X86_64_MICROSOFT_RETURN);
    for (id, constraint) in catalog.constraints.iter_mut().enumerate() {
        constraint.id = RegisterConstraintId(u16::try_from(id).unwrap());
    }
    assert_eq!(
        validate_x86_64_register_constraint_catalog(catalog, &model),
        Err(
            X86_64RegisterConstraintCatalogValidationError::TargetSemanticMismatch(
                X86_64_MICROSOFT_RETURN,
            )
        )
    );
}

#[test]
fn x86_64_constraint_semantic_corruption_rejects() {
    let model = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let mut wrong_class = x86_64_register_constraint_catalog(&model);
    let call_id = row(&wrong_class, X86_64_SYSTEM_V_CALL).id;
    row_mut(&mut wrong_class, X86_64_SYSTEM_V_CALL).operands[6].class = VECTOR128;
    assert_eq!(
        validate_x86_64_register_constraint_catalog(wrong_class, &model),
        Err(X86_64RegisterConstraintCatalogValidationError::Structural(
            RegisterConstraintCatalogValidationError::FixedViewClassMismatch {
                constraint: call_id,
                operand: 6,
            },
        )),
    );

    let mut contradictory_post_state = x86_64_register_constraint_catalog(&model);
    let unit = row_mut(&mut contradictory_post_state, X86_64_SYSTEM_V_CALL).implicit_defs[0];
    row_mut(&mut contradictory_post_state, X86_64_SYSTEM_V_CALL)
        .clobbers
        .push(unit);
    row_mut(&mut contradictory_post_state, X86_64_SYSTEM_V_CALL)
        .clobbers
        .sort_unstable();
    assert_eq!(
        validate_x86_64_register_constraint_catalog(contradictory_post_state, &model),
        Err(X86_64RegisterConstraintCatalogValidationError::Structural(
            RegisterConstraintCatalogValidationError::DefClobberOverlap {
                constraint: call_id,
                unit,
            },
        )),
    );
}

#[test]
fn x86_64_target_semantics_reject_compatible_substitution_and_missing_clobbers() {
    let model = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let mut wrong_syscall_register = x86_64_register_constraint_catalog(&model);
    row_mut(&mut wrong_syscall_register, X86_64_LINUX_SYSTEM_CALL).operands[4].fixed_view =
        Some(model.model().view_named("r11").unwrap().id);
    assert_eq!(
        validate_x86_64_register_constraint_catalog(wrong_syscall_register, &model),
        Err(
            X86_64RegisterConstraintCatalogValidationError::TargetSemanticMismatch(
                X86_64_LINUX_SYSTEM_CALL,
            )
        )
    );

    for clobber in ["rcx", "r11", "rflags"] {
        let mut missing_clobber = x86_64_register_constraint_catalog(&model);
        let omitted = model.model().view_named(clobber).unwrap().units[0];
        row_mut(&mut missing_clobber, X86_64_LINUX_SYSTEM_CALL)
            .clobbers
            .retain(|unit| *unit != omitted);
        assert_eq!(
            validate_x86_64_register_constraint_catalog(missing_clobber, &model),
            Err(
                X86_64RegisterConstraintCatalogValidationError::TargetSemanticMismatch(
                    X86_64_LINUX_SYSTEM_CALL,
                )
            ),
            "omitting {clobber} state must reject",
        );
    }

    let mut wrong_add_role = x86_64_register_constraint_catalog(&model);
    row_mut(&mut wrong_add_role, X86_64_ADD_I64).operands[1].access = RegisterOperandAccess::Def;
    assert_eq!(
        validate_x86_64_register_constraint_catalog(wrong_add_role, &model),
        Err(
            X86_64RegisterConstraintCatalogValidationError::TargetSemanticMismatch(X86_64_ADD_I64,)
        )
    );

    let mut wrong_immediate_role = x86_64_register_constraint_catalog(&model);
    row_mut(&mut wrong_immediate_role, X86_64_ADD_I64_IMMEDIATE).operands[0].access =
        RegisterOperandAccess::Def;
    assert_eq!(
        validate_x86_64_register_constraint_catalog(wrong_immediate_role, &model),
        Err(
            X86_64RegisterConstraintCatalogValidationError::TargetSemanticMismatch(
                X86_64_ADD_I64_IMMEDIATE,
            )
        )
    );

    let mut wrong_subtract_role = x86_64_register_constraint_catalog(&model);
    row_mut(&mut wrong_subtract_role, X86_64_SUBTRACT_I64).operands[1].access =
        RegisterOperandAccess::Def;
    assert_eq!(
        validate_x86_64_register_constraint_catalog(wrong_subtract_role, &model),
        Err(
            X86_64RegisterConstraintCatalogValidationError::TargetSemanticMismatch(
                X86_64_SUBTRACT_I64,
            )
        )
    );

    let mut missing_subtract_flags = x86_64_register_constraint_catalog(&model);
    row_mut(&mut missing_subtract_flags, X86_64_SUBTRACT_I64)
        .clobbers
        .clear();
    assert_eq!(
        validate_x86_64_register_constraint_catalog(missing_subtract_flags, &model),
        Err(
            X86_64RegisterConstraintCatalogValidationError::TargetSemanticMismatch(
                X86_64_SUBTRACT_I64,
            )
        )
    );
}

#[test]
fn x86_64_branch_rejects_one_field_missing_flags_use() {
    let model = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let mut catalog = x86_64_register_constraint_catalog(&model);
    let flags = model.model().view_named("rflags").unwrap().units[0];
    row_mut(&mut catalog, X86_64_CONDITIONAL_BRANCH)
        .implicit_uses
        .retain(|unit| *unit != flags);
    assert_eq!(
        validate_x86_64_register_constraint_catalog(catalog, &model),
        Err(
            X86_64RegisterConstraintCatalogValidationError::TargetSemanticMismatch(
                X86_64_CONDITIONAL_BRANCH,
            )
        )
    );
}

#[test]
fn x86_64_catalog_validation_rejects_same_architecture_forged_physical_model() {
    let canonical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let catalog = x86_64_register_constraint_catalog(&canonical);
    let mut forged = x86_64_physical_register_model();
    forged.views[0].name = "forged.rax".into();
    let forged = validate_physical_register_model(forged).unwrap();
    assert_eq!(
        x86_64_fixed_register_view(&forged, MachineRegister::X86Rax),
        None
    );
    assert_eq!(
        validate_x86_64_register_constraint_catalog(catalog, &forged),
        Err(X86_64RegisterConstraintCatalogValidationError::NonCanonicalPhysicalModel)
    );
}

#[test]
fn omitted_and_overlapping_units_reject() {
    let mut omitted = x86_64_physical_register_model();
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

    let mut overlap = x86_64_physical_register_model();
    let unit = overlap.conventions[0].caller_saved[0];
    overlap.conventions[0].callee_saved.push(unit);
    overlap.conventions[0].callee_saved.sort_unstable();
    assert_eq!(
        validate_physical_register_model(overlap),
        Err(RegisterModelValidationError::ConventionPartitionOverlap(
            unit
        ))
    );
}
