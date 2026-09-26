use crate::plans::{
    CallSignature, CallingPolicy, IndirectPointerLocation, MachineRegister, SystemVEightbyteClass,
    ValueLocation, ValueShape, evaluate_call_plan, validate_call_plan,
};

#[test]
fn system_v_memory_class_uses_stack_values_and_a_hidden_result_pointer() {
    let signature = CallSignature {
        parameters: vec![ValueShape::integer(24, 8), ValueShape::integer(8, 8)],
        result: Some(ValueShape::integer(24, 8)),
    };
    let plan = evaluate_call_plan(CallingPolicy::SystemVAMD64, &signature)
        .expect("SysV MEMORY-class plan");

    assert_eq!(
        plan.parameters[0].locations,
        vec![
            ValueLocation::Stack {
                stack_byte_offset: 0,
                value_byte_offset: 0,
                byte_size: 8,
                alignment: 8,
            },
            ValueLocation::Stack {
                stack_byte_offset: 8,
                value_byte_offset: 8,
                byte_size: 8,
                alignment: 8,
            },
            ValueLocation::Stack {
                stack_byte_offset: 16,
                value_byte_offset: 16,
                byte_size: 8,
                alignment: 8,
            },
        ]
    );
    assert!(matches!(
        plan.parameters[1].locations.as_slice(),
        [ValueLocation::Register {
            register: MachineRegister::X86Rsi,
            ..
        }]
    ));
    assert!(matches!(
        plan.result.expect("indirect result").locations.as_slice(),
        [ValueLocation::Indirect {
            pointer: IndirectPointerLocation::Register(MachineRegister::X86Rdi),
            copy_stack_byte_offset: None,
            byte_size: 24,
            alignment: 8,
        }]
    ));
}

#[test]
fn system_v_two_f64_record_uses_sse_fragments_or_whole_stack_rollback() {
    let pair = ValueShape::homogeneous_float_aggregate(8, 2);
    let signature = CallSignature {
        parameters: vec![ValueShape::float(8); 8]
            .into_iter()
            .chain([pair])
            .collect(),
        result: Some(pair),
    };
    let plan = evaluate_call_plan(CallingPolicy::SystemVAMD64, &signature)
        .expect("SysV two-f64 record plan");

    assert_eq!(
        plan.parameters[8].locations,
        vec![
            ValueLocation::Stack {
                stack_byte_offset: 0,
                value_byte_offset: 0,
                byte_size: 8,
                alignment: 8,
            },
            ValueLocation::Stack {
                stack_byte_offset: 8,
                value_byte_offset: 8,
                byte_size: 8,
                alignment: 8,
            },
        ]
    );
    assert!(matches!(
        plan.result.expect("SSE result").locations.as_slice(),
        [
            ValueLocation::Register {
                register: MachineRegister::X86Xmm(0),
                ..
            },
            ValueLocation::Register {
                register: MachineRegister::X86Xmm(1),
                ..
            }
        ]
    ));
}

#[test]
fn system_v_three_f32_record_packs_into_two_sse_eightbytes() {
    let triple = ValueShape::homogeneous_float_aggregate(4, 3);
    let plan = evaluate_call_plan(
        CallingPolicy::SystemVAMD64,
        &CallSignature {
            parameters: vec![triple],
            result: Some(triple),
        },
    )
    .expect("SysV three-f32 record plan");

    let expected = vec![
        ValueLocation::Register {
            register: MachineRegister::X86Xmm(0),
            value_byte_offset: 0,
            byte_size: 8,
        },
        ValueLocation::Register {
            register: MachineRegister::X86Xmm(1),
            value_byte_offset: 8,
            byte_size: 4,
        },
    ];
    assert_eq!(plan.parameters[0].locations, expected);
    assert_eq!(plan.result.expect("packed SSE result").locations, expected);
}

#[test]
fn system_v_mixed_record_uses_independent_register_banks() {
    let integer_sse = ValueShape::system_v_aggregate(
        16,
        8,
        SystemVEightbyteClass::Integer,
        SystemVEightbyteClass::Sse,
    );
    let sse_integer = ValueShape::system_v_aggregate(
        16,
        8,
        SystemVEightbyteClass::Sse,
        SystemVEightbyteClass::Integer,
    );
    let plan = evaluate_call_plan(
        CallingPolicy::SystemVAMD64,
        &CallSignature {
            parameters: vec![integer_sse],
            result: Some(sse_integer),
        },
    )
    .expect("mixed SysV aggregate plan");

    assert_eq!(
        plan.parameters[0].locations,
        vec![
            ValueLocation::Register {
                register: MachineRegister::X86Rdi,
                value_byte_offset: 0,
                byte_size: 8,
            },
            ValueLocation::Register {
                register: MachineRegister::X86Xmm(0),
                value_byte_offset: 8,
                byte_size: 8,
            },
        ]
    );
    assert_eq!(
        plan.result.expect("mixed result").locations,
        vec![
            ValueLocation::Register {
                register: MachineRegister::X86Xmm(0),
                value_byte_offset: 0,
                byte_size: 8,
            },
            ValueLocation::Register {
                register: MachineRegister::X86Rax,
                value_byte_offset: 8,
                byte_size: 8,
            },
        ]
    );
}

#[test]
fn system_v_mixed_record_rolls_back_both_banks() {
    let mixed = ValueShape::system_v_aggregate(
        16,
        8,
        SystemVEightbyteClass::Integer,
        SystemVEightbyteClass::Sse,
    );
    let mut parameters = vec![ValueShape::float(8); 8];
    parameters.extend([mixed, ValueShape::integer(8, 8)]);
    let plan = evaluate_call_plan(
        CallingPolicy::SystemVAMD64,
        &CallSignature {
            parameters,
            result: None,
        },
    )
    .expect("mixed SysV rollback plan");

    assert!(matches!(
        plan.parameters[8].locations.as_slice(),
        [
            ValueLocation::Stack {
                stack_byte_offset: 0,
                value_byte_offset: 0,
                byte_size: 8,
                ..
            },
            ValueLocation::Stack {
                stack_byte_offset: 8,
                value_byte_offset: 8,
                byte_size: 8,
                ..
            }
        ]
    ));
    assert!(matches!(
        plan.parameters[9].locations.as_slice(),
        [ValueLocation::Register {
            register: MachineRegister::X86Rdi,
            ..
        }]
    ));

    let mut parameters = vec![ValueShape::integer(8, 8); 6];
    parameters.extend([mixed, ValueShape::float(8)]);
    let plan = evaluate_call_plan(
        CallingPolicy::SystemVAMD64,
        &CallSignature {
            parameters,
            result: None,
        },
    )
    .expect("inverse mixed SysV rollback plan");
    assert!(
        plan.parameters[6]
            .locations
            .iter()
            .all(|location| matches!(location, ValueLocation::Stack { .. }))
    );
    assert!(matches!(
        plan.parameters[7].locations.as_slice(),
        [ValueLocation::Register {
            register: MachineRegister::X86Xmm(0),
            ..
        }]
    ));
}

#[test]
fn system_v_classified_record_rejects_all_integer_eightbytes() {
    let malformed = ValueShape::system_v_aggregate(
        16,
        8,
        SystemVEightbyteClass::Integer,
        SystemVEightbyteClass::Integer,
    );
    let error = evaluate_call_plan(
        CallingPolicy::SystemVAMD64,
        &CallSignature {
            parameters: vec![malformed],
            result: None,
        },
    )
    .expect_err("equal classes must use an existing normalized aggregate class");
    assert!(error.0.contains("at least one SSE eightbyte"));
}

#[test]
fn borrowed_references_retain_original_storage_pointers_without_copies() {
    let borrowed = ValueShape::borrowed_reference(16, 8);
    for (policy, register) in [
        (CallingPolicy::MicrosoftX64, MachineRegister::X86Rcx),
        (CallingPolicy::SystemVAMD64, MachineRegister::X86Rdi),
        (CallingPolicy::Aapcs64, MachineRegister::Aarch64X(0)),
    ] {
        let plan = evaluate_call_plan(
            policy,
            &CallSignature {
                parameters: vec![borrowed],
                result: Some(ValueShape::integer(4, 4)),
            },
        )
        .expect("borrowed-reference plan");
        assert_eq!(
            plan.parameters[0].locations,
            vec![ValueLocation::Indirect {
                pointer: IndirectPointerLocation::Register(register),
                copy_stack_byte_offset: None,
                byte_size: 16,
                alignment: 8,
            }]
        );
        validate_call_plan(
            &plan,
            &CallSignature {
                parameters: vec![borrowed],
                result: Some(ValueShape::integer(4, 4)),
            },
        )
        .expect("borrowed-reference plan validates");
    }
}

#[test]
fn zero_byte_referents_retain_pointer_abi_and_reject_erased_pointer_evidence() {
    for policy in [
        CallingPolicy::MicrosoftX64,
        CallingPolicy::SystemVAMD64,
        CallingPolicy::Aapcs64,
    ] {
        let signature = CallSignature {
            parameters: vec![
                ValueShape::integer(0, 1),
                ValueShape::borrowed_reference(0, 1),
                ValueShape::integer(8, 8),
            ],
            result: None,
        };
        let plan =
            evaluate_call_plan(policy, &signature).expect("empty referent still needs pointer");
        assert!(plan.parameters[0].locations.is_empty());
        assert!(matches!(
            plan.parameters[1].locations.as_slice(),
            [ValueLocation::Indirect {
                copy_stack_byte_offset: None,
                byte_size: 0,
                alignment: 1,
                ..
            }]
        ));
        assert!(!plan.parameters[2].locations.is_empty());
        let nonempty = evaluate_call_plan(
            policy,
            &CallSignature {
                parameters: vec![
                    ValueShape::borrowed_reference(4, 4),
                    ValueShape::integer(8, 8),
                ],
                result: None,
            },
        )
        .unwrap();
        assert_eq!(
            plan.parameters[2], nonempty.parameters[1],
            "empty referent consumes the same pointer slot"
        );
        let mut missing_pointer = plan.clone();
        missing_pointer.parameters[1].locations.clear();
        assert!(validate_call_plan(&missing_pointer, &signature).is_err());
        let mut copied = plan.clone();
        let ValueLocation::Indirect {
            copy_stack_byte_offset,
            ..
        } = &mut copied.parameters[1].locations[0]
        else {
            panic!("pointer placement")
        };
        *copy_stack_byte_offset = Some(0);
        assert!(validate_call_plan(&copied, &signature).is_err());
        for invalid in [ValueShape::integer(0, 8), ValueShape::float(0)] {
            assert!(
                evaluate_call_plan(
                    policy,
                    &CallSignature {
                        parameters: vec![invalid],
                        result: None
                    }
                )
                .is_err()
            );
        }
    }
}

#[test]
fn borrowed_reference_stack_pointer_is_not_a_referent_copy() {
    let mut parameters = vec![ValueShape::integer(8, 8); 6];
    parameters.push(ValueShape::borrowed_reference(16, 8));
    let plan = evaluate_call_plan(
        CallingPolicy::SystemVAMD64,
        &CallSignature {
            parameters,
            result: None,
        },
    )
    .expect("stacked borrowed-reference plan");
    assert_eq!(
        plan.parameters[6].locations,
        vec![ValueLocation::Indirect {
            pointer: IndirectPointerLocation::Stack {
                stack_byte_offset: 0,
                alignment: 8,
            },
            copy_stack_byte_offset: None,
            byte_size: 16,
            alignment: 8,
        }]
    );
}

#[test]
fn borrowed_references_are_not_results() {
    let error = evaluate_call_plan(
        CallingPolicy::SystemVAMD64,
        &CallSignature {
            parameters: Vec::new(),
            result: Some(ValueShape::borrowed_reference(8, 8)),
        },
    )
    .expect_err("borrowed-reference result must be rejected");
    assert!(error.0.contains("parameter-only"));
}
