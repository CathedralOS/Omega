//! Focused normalized foreign-scalar boundary-call lowering tests.

use super::*;

fn declaration(
    boundary: BoundaryMachineId,
    scalar_parameters: Vec<ScalarType>,
) -> psi_terminal::BoundaryMachineDeclaration {
    psi_terminal::BoundaryMachineDeclaration {
        id: boundary,
        identity: "Foreign::leaf".into(),
        attachment: None,
        scalar_parameters,
        structural_parameters: Vec::new(),
        result: None,
        requires: Vec::new(),
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        published_service_ceiling: Vec::new(),
    }
}

fn entry_plan(
    target: NativeTarget,
    scalar_types: &[IntegerType],
) -> omega_calling_conventions::BoundaryEntryPlan {
    omega_calling_conventions::evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: scalar_types
                .iter()
                .map(|scalar_type| {
                    let bytes = scalar_type.bits().div_ceil(8);
                    ValueShape::integer(bytes, bytes.next_power_of_two().min(8))
                })
                .collect(),
            result: None,
        },
    )
    .expect("evaluated entry plan")
    .plan()
    .clone()
}

#[test]
fn fixed_integer_literal_preserves_source_type_value_order_and_register_placement() {
    let boundary = BoundaryMachineId::new(41).expect("boundary");
    let source = ValueId::new(42).expect("source");
    let constant = OperationId::new(43).expect("constant");
    let integer_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    let declaration = declaration(boundary, vec![ScalarType::Integer(integer_type)]);
    let constants = BTreeMap::from([(source, (constant, integer_type, IntegerValue::Signed(-17)))]);

    for (target, expected_register) in [
        (NativeTarget::linux_x64(), MachineRegister::X86Rdi),
        (NativeTarget::linux_arm64(), MachineRegister::Aarch64X(0)),
    ] {
        let plan = entry_plan(target, &[integer_type]);
        let arguments = lower_normalized_foreign_scalar_arguments(
            boundary,
            &declaration,
            &[source],
            &plan,
            &constants,
        )
        .expect("one evaluated literal argument");
        let [argument] = arguments.as_slice() else {
            panic!("one argument")
        };
        assert_eq!(argument.source_value, source);
        assert_eq!(argument.scalar_type, integer_type);
        assert_eq!(argument.immediate, IntegerValue::Signed(-17));
        assert_eq!(argument.parameter_index, 0);
        assert_eq!(argument.placement, plan.call.parameters[0]);
        assert!(matches!(
            argument.placement.locations.as_slice(),
            [ValueLocation::Register { register, .. }] if *register == expected_register
        ));
    }
}

#[test]
fn two_fixed_integer_literals_preserve_ordered_occurrence_custody() {
    let boundary = BoundaryMachineId::new(45).expect("boundary");
    let first = ValueId::new(46).expect("first source");
    let second = ValueId::new(47).expect("second source");
    let i16_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16");
    let i64_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
    let declaration = declaration(
        boundary,
        vec![ScalarType::Integer(i16_type), ScalarType::Integer(i64_type)],
    );
    let constants = BTreeMap::from([
        (
            first,
            (
                OperationId::new(48).expect("first constant"),
                i16_type,
                IntegerValue::Unsigned(513),
            ),
        ),
        (
            second,
            (
                OperationId::new(49).expect("second constant"),
                i64_type,
                IntegerValue::Signed(-29),
            ),
        ),
    ]);

    for (target, expected_registers) in [
        (
            NativeTarget::linux_x64(),
            [MachineRegister::X86Rdi, MachineRegister::X86Rsi],
        ),
        (
            NativeTarget::linux_arm64(),
            [MachineRegister::Aarch64X(0), MachineRegister::Aarch64X(1)],
        ),
    ] {
        let plan = entry_plan(target, &[i16_type, i64_type]);
        let arguments = lower_normalized_foreign_scalar_arguments(
            boundary,
            &declaration,
            &[first, second],
            &plan,
            &constants,
        )
        .expect("two evaluated register literal arguments");
        assert_eq!(arguments.len(), 2);
        for (index, (argument, expected_register)) in
            arguments.iter().zip(expected_registers).enumerate()
        {
            assert_eq!(argument.source_value, [first, second][index]);
            assert_eq!(argument.scalar_type, [i16_type, i64_type][index]);
            assert_eq!(argument.parameter_index, index as u32);
            assert_eq!(argument.placement, plan.call.parameters[index]);
            assert!(matches!(
                argument.placement.locations.as_slice(),
                [ValueLocation::Register { register, .. }] if *register == expected_register
            ));
        }
        assert_eq!(arguments[0].immediate, IntegerValue::Unsigned(513));
        assert_eq!(arguments[1].immediate, IntegerValue::Signed(-29));

        let mut stack_plan = plan;
        stack_plan.call.parameters[1].locations = vec![ValueLocation::Stack {
            stack_byte_offset: 0,
            value_byte_offset: 0,
            byte_size: 8,
            alignment: 8,
        }];
        assert!(
            lower_normalized_foreign_scalar_arguments(
                boundary,
                &declaration,
                &[first, second],
                &stack_plan,
                &constants,
            )
            .is_err()
        );
    }
}

#[test]
fn zero_argument_leaf_stays_valid_and_scalar_mutations_fail_closed() {
    let boundary = BoundaryMachineId::new(51).expect("boundary");
    let source = ValueId::new(52).expect("source");
    let constant = OperationId::new(53).expect("constant");
    let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    let zero_plan = omega_calling_conventions::evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::native_for_target(NativeTarget::linux_x64()),
        &CallSignature::default(),
    )
    .expect("zero-argument plan")
    .plan()
    .clone();
    assert_eq!(
        lower_normalized_foreign_scalar_arguments(
            boundary,
            &declaration(boundary, Vec::new()),
            &[],
            &zero_plan,
            &BTreeMap::new(),
        ),
        Ok(Vec::new())
    );

    let one_parameter_declaration = declaration(boundary, vec![ScalarType::Integer(i32_type)]);
    let plan = entry_plan(NativeTarget::linux_x64(), &[i32_type]);
    let constants = BTreeMap::from([(source, (constant, i32_type, IntegerValue::Signed(9)))]);
    for (arguments, constants) in [
        (Vec::new(), constants.clone()),
        (vec![source], BTreeMap::new()),
        (
            vec![source],
            BTreeMap::from([(source, (constant, i32_type, IntegerValue::Unsigned(9)))]),
        ),
    ] {
        assert!(matches!(
            lower_normalized_foreign_scalar_arguments(
                boundary,
                &one_parameter_declaration,
                &arguments,
                &plan,
                &constants,
            ),
            Err(LoweringError::BoundaryRealizationMismatch(actual)) if actual == boundary
        ));
    }

    let mut stack_plan = plan.clone();
    stack_plan.call.parameters[0].locations = vec![ValueLocation::Stack {
        stack_byte_offset: 0,
        value_byte_offset: 0,
        byte_size: 4,
        alignment: 4,
    }];
    let mut result_plan = plan.clone();
    result_plan.call.result = Some(plan.call.parameters[0].clone());
    for invalid in [stack_plan, result_plan] {
        assert!(
            lower_normalized_foreign_scalar_arguments(
                boundary,
                &one_parameter_declaration,
                &[source],
                &invalid,
                &constants,
            )
            .is_err()
        );
    }

    let five_parameter_declaration = declaration(boundary, vec![ScalarType::Integer(i32_type); 5]);
    for (target, expected_fifth_register) in [
        (NativeTarget::linux_x64(), MachineRegister::X86R8),
        (NativeTarget::linux_arm64(), MachineRegister::Aarch64X(4)),
    ] {
        let five_plan = entry_plan(target, &[i32_type; 5]);
        let five_arguments = lower_normalized_foreign_scalar_arguments(
            boundary,
            &five_parameter_declaration,
            &[source; 5],
            &five_plan,
            &constants,
        )
        .expect("five register-resident literal arguments");
        assert_eq!(
            five_arguments
                .iter()
                .map(|argument| argument.parameter_index)
                .collect::<Vec<_>>(),
            vec![0, 1, 2, 3, 4]
        );
        assert!(matches!(
            five_arguments[4].placement.locations.as_slice(),
            [ValueLocation::Register { register, .. }] if *register == expected_fifth_register
        ));
    }

    let six_parameter_declaration = declaration(boundary, vec![ScalarType::Integer(i32_type); 6]);
    let six_plan = entry_plan(NativeTarget::linux_x64(), &[i32_type; 6]);
    assert!(
        lower_normalized_foreign_scalar_arguments(
            boundary,
            &six_parameter_declaration,
            &[source; 6],
            &six_plan,
            &constants,
        )
        .is_err()
    );
}
