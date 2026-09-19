use super::{
    CatalogMutation, ModelMutation, instruction_key, miniature_catalog, miniature_model,
    validated_miniature_model,
};
use crate::{
    RegisterClassId, RegisterConstraintFamily, RegisterConstraintId, RegisterConstraintKey,
    RegisterOperandAccess, RegisterReservationProfile, RegisterUnitId, RegisterUnitKind,
    RegisterViewId, RegisterWriteSemantics, ReservationReason,
    TargetRegisterEnvironmentConstraintKeys, identities, target_register_environment_identity,
    validate_physical_register_model, validate_register_constraint_catalog,
    validate_register_reservation_profile,
};
use target::Architecture;

#[test]
fn physical_identity_is_deterministic_and_binds_every_declaration_family() {
    let baseline = validated_miniature_model().identity();
    assert_eq!(baseline, validated_miniature_model().identity());

    let mutations: Vec<ModelMutation> = vec![
        Box::new(|model| model.architecture = Architecture::Aarch64),
        Box::new(|model| model.units.swap(0, 1)),
        Box::new(|model| model.units[0].id = RegisterUnitId(9)),
        Box::new(|model| model.units[0].name.push_str(".changed")),
        Box::new(|model| model.units[0].bits = 128),
        Box::new(|model| model.units[0].kind = RegisterUnitKind::Flags),
        Box::new(|model| model.views.swap(0, 1)),
        Box::new(|model| model.views[0].id = RegisterViewId(9)),
        Box::new(|model| model.views[0].name.push_str(".changed")),
        Box::new(|model| model.views[0].class = RegisterClassId(1)),
        Box::new(|model| model.views[0].units.push(RegisterUnitId(1))),
        Box::new(|model| model.views[0].write_units.push(RegisterUnitId(1))),
        Box::new(|model| model.views[0].bits = 32),
        Box::new(|model| {
            model.views[0].write_semantics = RegisterWriteSemantics::InstructionDefined
        }),
        Box::new(|model| model.views[0].allocatable = false),
        Box::new(|model| model.classes.swap(0, 1)),
        Box::new(|model| model.classes[0].id = RegisterClassId(9)),
        Box::new(|model| model.classes[0].name.push_str(".changed")),
        Box::new(|model| model.classes[0].views.push(RegisterViewId(1))),
        Box::new(|model| model.conventions[0].name.push_str(".changed")),
        Box::new(|model| model.conventions[0].argument_views.push(RegisterViewId(1))),
        Box::new(|model| model.conventions[0].result_views.push(RegisterViewId(1))),
        Box::new(|model| model.conventions[0].caller_saved.clear()),
        Box::new(|model| model.conventions[0].callee_saved.clear()),
        Box::new(|model| model.conventions[0].fixed.push(RegisterUnitId(0))),
        Box::new(|model| model.conventions[0].stack_alignment = 32),
        Box::new(|model| model.conventions[0].red_zone_bytes = 64),
        Box::new(|model| model.reservations.swap(0, 1)),
        Box::new(|model| model.reservations[0].name.push_str(".changed")),
        Box::new(|model| model.reservations[0].reason = ReservationReason::FramePointer),
        Box::new(|model| model.reservations[0].units.push(RegisterUnitId(1))),
    ];
    for mutate in mutations {
        let mut model = miniature_model();
        mutate(&mut model);
        let identity = identities::physical_register_model_identity(&model);
        assert_ne!(identity, baseline);
    }
}

#[test]
fn catalog_identity_binds_physical_identity_and_every_constraint_family() {
    let model = validated_miniature_model();
    let baseline = validate_register_constraint_catalog(miniature_catalog(), &model)
        .unwrap()
        .identity();
    assert_eq!(
        baseline,
        validate_register_constraint_catalog(miniature_catalog(), &model)
            .unwrap()
            .identity()
    );

    let mut changed_physical = miniature_model();
    changed_physical.units[0].name.push_str(".changed");
    let changed_physical = validate_physical_register_model(changed_physical).unwrap();
    assert_ne!(
        baseline,
        validate_register_constraint_catalog(miniature_catalog(), &changed_physical)
            .unwrap()
            .identity()
    );

    let mutations: Vec<CatalogMutation> = vec![
        Box::new(|catalog| catalog.architecture = Architecture::Aarch64),
        Box::new(|catalog| catalog.required[0].family = RegisterConstraintFamily::Return),
        Box::new(|catalog| {
            catalog.required[0].variant += 1;
            catalog.constraints[0].key.variant += 1;
        }),
        Box::new(|catalog| catalog.constraints[0].id = RegisterConstraintId(9)),
        Box::new(|catalog| catalog.constraints[0].key.family = RegisterConstraintFamily::Return),
        Box::new(|catalog| catalog.constraints[0].key.variant += 1),
        Box::new(|catalog| catalog.constraints[0].operands.swap(0, 1)),
        Box::new(|catalog| catalog.constraints[0].operands[0].operand = 9),
        Box::new(|catalog| {
            catalog.constraints[0].operands[0].access = RegisterOperandAccess::UseDef
        }),
        Box::new(|catalog| catalog.constraints[0].operands[0].class = RegisterClassId(1)),
        Box::new(|catalog| catalog.constraints[0].operands[0].fixed_view = Some(RegisterViewId(0))),
        Box::new(|catalog| catalog.constraints[0].operands[1].fixed_view = None),
        Box::new(|catalog| catalog.constraints[0].operands[1].tied_to = None),
        Box::new(|catalog| catalog.constraints[0].operands[1].early_clobber = false),
        Box::new(|catalog| catalog.constraints[0].implicit_uses.clear()),
        Box::new(|catalog| catalog.constraints[0].implicit_defs.push(RegisterUnitId(0))),
        Box::new(|catalog| catalog.constraints[0].clobbers.clear()),
    ];
    for mutate in mutations {
        let mut catalog = miniature_catalog();
        mutate(&mut catalog);
        let identity = identities::register_constraint_catalog_identity(model.identity(), &catalog);
        assert_ne!(identity, baseline);
    }
}

#[test]
fn environment_identity_binds_target_components_and_named_selected_keys() {
    let target = target::NativeTarget::linux_x64();
    let physical = validated_miniature_model();
    let constraints = validate_register_constraint_catalog(miniature_catalog(), &physical).unwrap();
    let reservations = validate_register_reservation_profile(
        RegisterReservationProfile {
            name: "test.policy".into(),
            active_overlays: vec!["test.reserve-r0".into()],
        },
        target,
        &physical,
    )
    .unwrap();
    let keys = TargetRegisterEnvironmentConstraintKeys {
        call_aggregate: Vec::new(),
        return_aggregate: Vec::new(),
        load64: Some(instruction_key(30)),
        load_packed: Some(instruction_key(36)),
        store_packed: Some(instruction_key(37)),
        load8: None,
        load16: None,
        load32: None,
        load8_indexed: None,
        copy_bytes: None,
        store: Some(instruction_key(34)),
        address_offset: Some(instruction_key(35)),
        store64: Some(instruction_key(31)),
        frame_address: Some(instruction_key(32)),
        hosted_read_byte: None,
        hosted_write_byte_i32: None,
        hosted_exit_process_i32: None,
        call_unit: vec![RegisterConstraintKey {
            family: RegisterConstraintFamily::Call,
            variant: 2,
        }],
        call_unit_mixed: Vec::new(),
        call_scalar: vec![RegisterConstraintKey {
            family: RegisterConstraintFamily::Call,
            variant: 3,
        }],
        call_normalized_foreign: vec![RegisterConstraintKey {
            family: RegisterConstraintFamily::Call,
            variant: 3000,
        }],
        materialize_i64: instruction_key(1),
        materialize_boolean: instruction_key(732),
        copy_i64: instruction_key(5),
        float32_to_bits: None,
        float64_to_bits: None,
        bits_to_float32: None,
        bits_to_float64: None,
        add_i64: instruction_key(6),
        add_i64_immediate: instruction_key(7),
        subtract_i64: instruction_key(8),
        multiply_i64: instruction_key(46),
        saturating_subtract_unsigned: instruction_key(8),
        saturating_add_u64: instruction_key(8),
        divide_u64: instruction_key(8),
        remainder_i64: instruction_key(38),
        saturating_add_clamped: instruction_key(40),
        saturating_subtract_clamped: instruction_key(41),
        saturating_divide_signed: instruction_key(42),
        subtract_i64_immediate: instruction_key(9),
        compare_i64_zero: instruction_key(2),
        compare_i64: instruction_key(20),
        compare_i64_immediate: instruction_key(24),
        conditional_branch: instruction_key(3),
        jump: instruction_key(22),
        return_float: Vec::new(),
        return_i64: instruction_key(4),
        return_unit: instruction_key(5),
    };
    let identity =
        target_register_environment_identity(target, &physical, &constraints, &reservations, &keys);
    assert_eq!(
        identity,
        target_register_environment_identity(target, &physical, &constraints, &reservations, &keys,)
    );

    for changed_target in [
        target::NativeTarget {
            architecture: Architecture::Aarch64,
            ..target
        },
        target::NativeTarget {
            object_format: target::ObjectFormat::Coff,
            ..target
        },
        target::NativeTarget {
            pointer_size: 4,
            ..target
        },
        target::NativeTarget {
            pointer_alignment: 4,
            ..target
        },
    ] {
        assert_ne!(
            identity,
            target_register_environment_identity(
                changed_target,
                &physical,
                &constraints,
                &reservations,
                &keys,
            )
        );
    }

    for changed_keys in [
        TargetRegisterEnvironmentConstraintKeys {
            load_packed: None,
            ..keys.clone()
        },
        TargetRegisterEnvironmentConstraintKeys {
            store_packed: None,
            ..keys.clone()
        },
        TargetRegisterEnvironmentConstraintKeys {
            load_packed: Some(instruction_key(38)),
            ..keys.clone()
        },
        TargetRegisterEnvironmentConstraintKeys {
            store_packed: Some(instruction_key(38)),
            ..keys.clone()
        },
        TargetRegisterEnvironmentConstraintKeys {
            load_packed: keys.store_packed,
            store_packed: keys.load_packed,
            ..keys.clone()
        },
        TargetRegisterEnvironmentConstraintKeys {
            hosted_read_byte: Some(keys.materialize_i64),
            ..keys.clone()
        },
        TargetRegisterEnvironmentConstraintKeys {
            store: None,
            ..keys.clone()
        },
        TargetRegisterEnvironmentConstraintKeys {
            address_offset: None,
            ..keys.clone()
        },
        TargetRegisterEnvironmentConstraintKeys {
            call_unit: Vec::new(),
            call_unit_mixed: Vec::new(),
            ..keys.clone()
        },
        TargetRegisterEnvironmentConstraintKeys {
            load64: None,
            ..keys.clone()
        },
        TargetRegisterEnvironmentConstraintKeys {
            store64: None,
            ..keys.clone()
        },
        TargetRegisterEnvironmentConstraintKeys {
            frame_address: None,
            ..keys.clone()
        },
        TargetRegisterEnvironmentConstraintKeys {
            load64: keys.store64,
            store64: keys.load64,
            ..keys.clone()
        },
        TargetRegisterEnvironmentConstraintKeys {
            call_scalar: Vec::new(),
            ..keys.clone()
        },
        TargetRegisterEnvironmentConstraintKeys {
            call_aggregate: vec![instruction_key(30)],
            ..keys.clone()
        },
        TargetRegisterEnvironmentConstraintKeys {
            call_normalized_foreign: Vec::new(),
            ..keys.clone()
        },
        TargetRegisterEnvironmentConstraintKeys {
            call_normalized_foreign: vec![instruction_key(30)],
            ..keys.clone()
        },
        TargetRegisterEnvironmentConstraintKeys {
            return_aggregate: vec![instruction_key(31)],
            ..keys.clone()
        },
        TargetRegisterEnvironmentConstraintKeys {
            materialize_i64: instruction_key(11),
            materialize_boolean: instruction_key(732),
            ..keys.clone()
        },
        TargetRegisterEnvironmentConstraintKeys {
            copy_i64: instruction_key(15),
            float32_to_bits: None,
            float64_to_bits: None,
            bits_to_float32: None,
            bits_to_float64: None,
            ..keys.clone()
        },
        TargetRegisterEnvironmentConstraintKeys {
            add_i64: instruction_key(16),
            ..keys.clone()
        },
        TargetRegisterEnvironmentConstraintKeys {
            add_i64_immediate: instruction_key(17),
            ..keys.clone()
        },
        TargetRegisterEnvironmentConstraintKeys {
            subtract_i64: instruction_key(18),
            ..keys.clone()
        },
        TargetRegisterEnvironmentConstraintKeys {
            multiply_i64: instruction_key(47),
            ..keys.clone()
        },
        TargetRegisterEnvironmentConstraintKeys {
            saturating_subtract_unsigned: instruction_key(18),
            ..keys.clone()
        },
        TargetRegisterEnvironmentConstraintKeys {
            saturating_add_u64: instruction_key(18),
            ..keys.clone()
        },
        TargetRegisterEnvironmentConstraintKeys {
            divide_u64: instruction_key(18),
            ..keys.clone()
        },
        TargetRegisterEnvironmentConstraintKeys {
            remainder_i64: instruction_key(39),
            ..keys.clone()
        },
        TargetRegisterEnvironmentConstraintKeys {
            saturating_add_clamped: instruction_key(43),
            ..keys.clone()
        },
        TargetRegisterEnvironmentConstraintKeys {
            saturating_subtract_clamped: instruction_key(44),
            ..keys.clone()
        },
        TargetRegisterEnvironmentConstraintKeys {
            saturating_divide_signed: instruction_key(45),
            ..keys.clone()
        },
        TargetRegisterEnvironmentConstraintKeys {
            subtract_i64_immediate: instruction_key(19),
            ..keys.clone()
        },
        TargetRegisterEnvironmentConstraintKeys {
            compare_i64_zero: instruction_key(12),
            ..keys.clone()
        },
        TargetRegisterEnvironmentConstraintKeys {
            compare_i64: instruction_key(21),
            ..keys.clone()
        },
        TargetRegisterEnvironmentConstraintKeys {
            conditional_branch: instruction_key(13),
            jump: instruction_key(23),
            ..keys.clone()
        },
        TargetRegisterEnvironmentConstraintKeys {
            return_float: Vec::new(),
            return_i64: instruction_key(14),
            ..keys.clone()
        },
        TargetRegisterEnvironmentConstraintKeys {
            return_unit: instruction_key(15),
            ..keys.clone()
        },
    ] {
        assert_ne!(
            identity,
            target_register_environment_identity(
                target,
                &physical,
                &constraints,
                &reservations,
                &changed_keys,
            )
        );
    }
}
