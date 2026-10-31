use super::{
    CatalogMutation, ModelMutation, instruction_key, miniature_catalog, miniature_model,
    validated_miniature_model,
};
use crate::register_model::identities;
use crate::{
    RegisterClassId, RegisterConstraintFamily, RegisterConstraintId, RegisterConstraintKey,
    RegisterOperandAccess, RegisterReservationProfile, RegisterUnitId, RegisterUnitKind,
    RegisterViewId, RegisterWriteSemantics, ReservationReason,
    TargetRegisterEnvironmentConstraintKeys, target_register_environment_identity,
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
        port_write: None,
        system_register_read: None,
        system_register_write: None,
        control_register_read: None,
        control_register_write: None,
        port_register_read: None,
        port_register_write: None,
        model_specific_read: None,
        model_specific_write: None,
        processor_halt: None,
        interrupt_control: None,
        memory_fence: None,
        atomic_fence: None,
        access_read: None,
        access_write: None,
        atomic_load: None,
        atomic_store: None,
        atomic_update: None,
        atomic_compare_exchange: None,
        atomic_bitwise_update: None,
        crash: instruction_key(780),
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
        scan_byte_sequence: None,
        load_encoded_scalar: None,
        store_encoded_scalar: None,
        compare_byte_sequences: None,
        zero_bytes: None,
        store: Some(instruction_key(34)),
        address_offset: Some(instruction_key(35)),
        store64: Some(instruction_key(31)),
        frame_address: Some(instruction_key(32)),
        byte_literal_image_address: None,
        hosted_read_byte: None,
        hosted_write_byte_i32: None,
        save_floating_control: None,
        restore_floating_control: None,
        hosted_exit_process_i32: None,
        call_unit: vec![RegisterConstraintKey::call(2)],
        call_unit_mixed: Vec::new(),
        call_scalar: vec![RegisterConstraintKey::call(3)],
        call_normalized_foreign: vec![RegisterConstraintKey::call(3000)],
        materialize_i64: instruction_key(1),
        materialize_boolean: instruction_key(732),
        copy_i64: instruction_key(5),
        float32_to_bits: None,
        float64_to_bits: None,
        bits_to_float32: None,
        bits_to_float64: None,
        ieee_float_arithmetic: None,
        ieee_float_directed_binary: None,
        ieee_float_directed_unary: None,
        ieee_float_unary: None,
        ieee_float_conversion_control: None,
        ieee_float_ternary: None,
        add_i64: instruction_key(6),
        add_i64_immediate: instruction_key(7),
        subtract_i64: instruction_key(8),
        multiply_i64: instruction_key(46),
        saturating_subtract_unsigned: instruction_key(8),
        saturating_add_u64: instruction_key(8),
        divide_u64: instruction_key(8),
        remainder_u64: instruction_key(48),
        remainder_i64: instruction_key(38),
        divide_i64: instruction_key(49),
        shift_i64: instruction_key(50),
        saturating_add_clamped: instruction_key(40),
        saturating_subtract_clamped: instruction_key(41),
        saturating_divide_signed: instruction_key(42),
        saturating_multiply_clamped: instruction_key(53),
        saturating_multiply_u64: instruction_key(54),
        trapping_binary: instruction_key(57),
        trapping_fixed_pair: instruction_key(58),
        trapping_shift: instruction_key(59),
        trapping_convert: instruction_key(60),
        float_result_trapping: None,
        float_result_saturating: None,
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

    // One owner for "the environment identity with a named selection changed":
    // each case below names only the fields it overrides.
    let identity_with = |mutate: &dyn Fn(&mut TargetRegisterEnvironmentConstraintKeys)| {
        let mut changed = keys.clone();
        mutate(&mut changed);
        target_register_environment_identity(
            target,
            &physical,
            &constraints,
            &reservations,
            &changed,
        )
    };

    // Both optional positions and their exact key payloads contribute, not
    // merely the presence of some floating policy capability.
    let policy_identities: Vec<_> = [None, Some(instruction_key(65)), Some(instruction_key(66))]
        .into_iter()
        .flat_map(|trapping| {
            [None, Some(instruction_key(65)), Some(instruction_key(66))]
                .into_iter()
                .map(move |saturating| (trapping, saturating))
        })
        .map(|(trapping, saturating)| {
            identity_with(&|row| {
                row.float_result_trapping = trapping;
                row.float_result_saturating = saturating;
            })
        })
        .collect();
    for (position, identity) in policy_identities.iter().enumerate() {
        assert!(!policy_identities[..position].contains(identity));
    }

    let directed_identities = [None, Some(instruction_key(785)), Some(instruction_key(792))]
        .map(|directed| identity_with(&|row| row.ieee_float_directed_binary = directed));
    for (position, identity) in directed_identities.iter().enumerate() {
        assert!(
            !directed_identities[..position].contains(identity),
            "missing, nearest and directed custody must remain distinct"
        );
    }

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

    for changed in [
        identity_with(&|row| row.atomic_load = Some(instruction_key(781))),
        identity_with(&|row| row.atomic_store = Some(instruction_key(782))),
        identity_with(&|row| row.atomic_update = Some(instruction_key(783))),
        identity_with(&|row| row.atomic_compare_exchange = Some(instruction_key(784))),
        identity_with(&|row| row.atomic_bitwise_update = Some(instruction_key(785))),
        identity_with(&|row| row.crash = instruction_key(781)),
        identity_with(&|row| row.load_packed = None),
        identity_with(&|row| row.store_packed = None),
        identity_with(&|row| row.load_packed = Some(instruction_key(38))),
        identity_with(&|row| row.store_packed = Some(instruction_key(38))),
        identity_with(&|row| {
            row.load_packed = keys.store_packed;
            row.store_packed = keys.load_packed;
        }),
        identity_with(&|row| row.hosted_read_byte = Some(keys.materialize_i64)),
        identity_with(&|row| row.store = None),
        identity_with(&|row| row.address_offset = None),
        identity_with(&|row| row.call_unit = Vec::new()),
        identity_with(&|row| row.load64 = None),
        identity_with(&|row| row.store64 = None),
        identity_with(&|row| row.frame_address = None),
        identity_with(&|row| {
            row.load64 = keys.store64;
            row.store64 = keys.load64;
        }),
        identity_with(&|row| row.call_scalar = Vec::new()),
        identity_with(&|row| row.call_aggregate = vec![instruction_key(30)]),
        identity_with(&|row| row.call_normalized_foreign = Vec::new()),
        identity_with(&|row| row.call_normalized_foreign = vec![instruction_key(30)]),
        identity_with(&|row| row.return_aggregate = vec![instruction_key(31)]),
        identity_with(&|row| row.materialize_i64 = instruction_key(11)),
        identity_with(&|row| {
            row.copy_i64 = instruction_key(15);
            row.ieee_float_conversion_control = Some(instruction_key(788));
        }),
        identity_with(&|row| row.add_i64 = instruction_key(16)),
        identity_with(&|row| row.add_i64_immediate = instruction_key(17)),
        identity_with(&|row| row.subtract_i64 = instruction_key(18)),
        identity_with(&|row| row.multiply_i64 = instruction_key(47)),
        identity_with(&|row| row.saturating_subtract_unsigned = instruction_key(18)),
        identity_with(&|row| row.saturating_add_u64 = instruction_key(18)),
        identity_with(&|row| row.divide_u64 = instruction_key(18)),
        identity_with(&|row| row.remainder_u64 = instruction_key(50)),
        identity_with(&|row| row.remainder_i64 = instruction_key(39)),
        identity_with(&|row| row.divide_i64 = instruction_key(51)),
        identity_with(&|row| row.shift_i64 = instruction_key(52)),
        identity_with(&|row| row.saturating_add_clamped = instruction_key(43)),
        identity_with(&|row| row.saturating_subtract_clamped = instruction_key(44)),
        identity_with(&|row| row.saturating_divide_signed = instruction_key(45)),
        identity_with(&|row| row.saturating_multiply_clamped = instruction_key(55)),
        identity_with(&|row| row.saturating_multiply_u64 = instruction_key(56)),
        identity_with(&|row| row.trapping_binary = instruction_key(61)),
        identity_with(&|row| row.trapping_fixed_pair = instruction_key(62)),
        identity_with(&|row| row.trapping_shift = instruction_key(63)),
        identity_with(&|row| row.trapping_convert = instruction_key(64)),
        identity_with(&|row| row.float_result_trapping = Some(instruction_key(65))),
        identity_with(&|row| row.float_result_saturating = Some(instruction_key(65))),
        identity_with(&|row| row.float_result_trapping = Some(instruction_key(66))),
        identity_with(&|row| row.float_result_saturating = Some(instruction_key(66))),
        identity_with(&|row| row.subtract_i64_immediate = instruction_key(19)),
        identity_with(&|row| row.compare_i64_zero = instruction_key(12)),
        identity_with(&|row| row.compare_i64 = instruction_key(21)),
        identity_with(&|row| {
            row.conditional_branch = instruction_key(13);
            row.jump = instruction_key(23);
        }),
        identity_with(&|row| row.return_i64 = instruction_key(14)),
        identity_with(&|row| row.return_unit = instruction_key(15)),
    ] {
        assert_ne!(identity, changed);
    }
}
