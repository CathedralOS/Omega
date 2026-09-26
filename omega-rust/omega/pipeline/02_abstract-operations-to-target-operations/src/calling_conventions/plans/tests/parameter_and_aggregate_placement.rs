use super::integer_signature;
use crate::calling_conventions::plans::{
    CallSignature, CallingPolicy, ConcreteVariadicCallSignature, EntryControl,
    IndirectPointerLocation, MachineRegister, ValueLocation, ValueShape, evaluate_call_plan,
    evaluate_darwin_aapcs64_variadic_call_plan, validate_call_plan,
};

#[test]
fn microsoft_x64_places_four_registers_then_shadow_relative_stack() {
    let plan = evaluate_call_plan(CallingPolicy::MicrosoftX64, &integer_signature(6))
        .expect("MS x64 plan");
    assert!(matches!(
        plan.parameters[0].locations[0],
        ValueLocation::Register {
            register: MachineRegister::X86Rcx,
            ..
        }
    ));
    assert!(matches!(
        plan.parameters[4].locations[0],
        ValueLocation::Stack {
            stack_byte_offset: 32,
            ..
        }
    ));
    assert_eq!(plan.shadow_bytes, 32);
}

#[test]
fn zero_sized_parameters_retain_identity_without_consuming_abi_locations() {
    let empty = ValueShape::integer(0, 1);
    let scalar = ValueShape::integer(8, 8);
    for policy in [
        CallingPolicy::MicrosoftX64,
        CallingPolicy::SystemVAMD64,
        CallingPolicy::Aapcs64,
    ] {
        let plan = evaluate_call_plan(
            policy,
            &CallSignature {
                parameters: vec![empty, scalar, empty],
                result: None,
            },
        )
        .expect("canonical empty values are erased from ABI placement");
        assert_eq!(plan.parameters.len(), 3);
        assert_eq!(plan.parameters[0].shape, empty);
        assert!(plan.parameters[0].locations.is_empty());
        assert_eq!(plan.parameters[1].shape, scalar);
        assert!(!plan.parameters[1].locations.is_empty());
        assert_eq!(plan.parameters[2].shape, empty);
        assert!(plan.parameters[2].locations.is_empty());
        validate_call_plan(
            &plan,
            &CallSignature {
                parameters: vec![empty, scalar, empty],
                result: None,
            },
        )
        .unwrap();
    }
}

#[test]
fn microsoft_x64_indirect_result_uses_rcx_and_shifts_parameters() {
    let signature = CallSignature {
        parameters: vec![ValueShape::integer(8, 8); 4],
        result: Some(ValueShape::integer(16, 8)),
    };
    let plan = evaluate_call_plan(CallingPolicy::MicrosoftX64, &signature)
        .expect("MS x64 indirect-result plan");

    assert!(matches!(
        plan.result.expect("indirect result").locations.as_slice(),
        [ValueLocation::Indirect {
            pointer: IndirectPointerLocation::Register(MachineRegister::X86Rcx),
            copy_stack_byte_offset: None,
            byte_size: 16,
            alignment: 8,
        }]
    ));
    assert!(matches!(
        plan.parameters[0].locations.as_slice(),
        [ValueLocation::Register {
            register: MachineRegister::X86Rdx,
            ..
        }]
    ));
    assert!(matches!(
        plan.parameters[3].locations.as_slice(),
        [ValueLocation::Stack {
            stack_byte_offset: 32,
            ..
        }]
    ));
}

#[test]
fn microsoft_x64_indirect_parameters_use_positional_pointer_slots() {
    let signature = CallSignature {
        parameters: vec![
            ValueShape::integer(16, 8),
            ValueShape::integer(8, 8),
            ValueShape::integer(8, 8),
            ValueShape::integer(8, 8),
            ValueShape::integer(16, 8),
        ],
        result: None,
    };
    let plan = evaluate_call_plan(CallingPolicy::MicrosoftX64, &signature)
        .expect("MS x64 indirect-parameter plan");

    assert!(matches!(
        plan.parameters[0].locations.as_slice(),
        [ValueLocation::Indirect {
            pointer: IndirectPointerLocation::Register(MachineRegister::X86Rcx),
            copy_stack_byte_offset: Some(48),
            ..
        }]
    ));
    assert!(matches!(
        plan.parameters[4].locations.as_slice(),
        [ValueLocation::Indirect {
            pointer: IndirectPointerLocation::Stack {
                stack_byte_offset: 32,
                alignment: 8,
            },
            copy_stack_byte_offset: Some(64),
            ..
        }]
    ));
}

#[test]
fn system_v_and_aapcs_use_independent_float_register_banks() {
    let signature = CallSignature {
        parameters: vec![
            ValueShape::integer(8, 8),
            ValueShape::float(8),
            ValueShape::integer(8, 8),
            ValueShape::float(8),
        ],
        result: None,
    };
    let sysv = evaluate_call_plan(CallingPolicy::SystemVAMD64, &signature).expect("SysV");
    let aapcs = evaluate_call_plan(CallingPolicy::Aapcs64, &signature).expect("AAPCS");
    assert!(matches!(
        sysv.parameters[1].locations[0],
        ValueLocation::Register {
            register: MachineRegister::X86Xmm(0),
            ..
        }
    ));
    assert!(matches!(
        aapcs.parameters[3].locations[0],
        ValueLocation::Register {
            register: MachineRegister::Aarch64V(1),
            ..
        }
    ));
}

#[test]
fn darwin_aapcs64_variadic_scalars_start_on_the_stack() {
    let word = ValueShape::integer(8, 8);
    let mode = ValueShape::integer(4, 4);
    let signature = ConcreteVariadicCallSignature {
        fixed_parameters: vec![word, ValueShape::integer(4, 4)],
        variadic_parameters: vec![mode],
        result: Some(ValueShape::integer(4, 4)),
    };
    let plan =
        evaluate_darwin_aapcs64_variadic_call_plan(&signature).expect("Darwin arm64 variadic plan");

    assert!(matches!(
        plan.parameters[0].locations.as_slice(),
        [ValueLocation::Register {
            register: MachineRegister::Aarch64X(0),
            ..
        }]
    ));
    assert!(matches!(
        plan.parameters[1].locations.as_slice(),
        [ValueLocation::Register {
            register: MachineRegister::Aarch64X(1),
            ..
        }]
    ));
    assert_eq!(
        plan.parameters[2].locations,
        vec![ValueLocation::Stack {
            stack_byte_offset: 0,
            value_byte_offset: 0,
            byte_size: 4,
            alignment: 8,
        }]
    );
    assert!(matches!(
        plan.result.expect("result placement").locations.as_slice(),
        [ValueLocation::Register {
            register: MachineRegister::Aarch64X(0),
            ..
        }]
    ));
}

#[test]
fn system_v_small_integer_aggregates_use_consecutive_registers_and_results() {
    let signature = CallSignature {
        parameters: vec![ValueShape::integer(8, 8), ValueShape::integer(16, 8)],
        result: Some(ValueShape::integer(12, 8)),
    };
    let plan = evaluate_call_plan(CallingPolicy::SystemVAMD64, &signature)
        .expect("SysV small aggregate plan");

    assert_eq!(
        plan.parameters[1].locations,
        vec![
            ValueLocation::Register {
                register: MachineRegister::X86Rsi,
                value_byte_offset: 0,
                byte_size: 8,
            },
            ValueLocation::Register {
                register: MachineRegister::X86Rdx,
                value_byte_offset: 8,
                byte_size: 8,
            },
        ]
    );
    assert_eq!(
        plan.result.expect("aggregate result").locations,
        vec![
            ValueLocation::Register {
                register: MachineRegister::X86Rax,
                value_byte_offset: 0,
                byte_size: 8,
            },
            ValueLocation::Register {
                register: MachineRegister::X86Rdx,
                value_byte_offset: 8,
                byte_size: 4,
            },
        ]
    );
}

#[test]
fn system_v_register_exhausted_aggregate_moves_wholly_to_stack_and_rolls_back() {
    let signature = CallSignature {
        parameters: vec![ValueShape::integer(8, 8); 5]
            .into_iter()
            .chain([ValueShape::integer(16, 8), ValueShape::integer(8, 8)])
            .collect(),
        result: None,
    };
    let plan = evaluate_call_plan(CallingPolicy::SystemVAMD64, &signature)
        .expect("SysV exhausted aggregate plan");

    assert_eq!(
        plan.parameters[5].locations,
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
    assert_eq!(
        plan.parameters[6].locations,
        vec![ValueLocation::Register {
            register: MachineRegister::X86R9,
            value_byte_offset: 0,
            byte_size: 8,
        }]
    );
}

#[test]
fn aapcs_hfa_is_one_value_split_across_vector_registers() {
    let signature = CallSignature {
        parameters: vec![ValueShape::homogeneous_float_aggregate(8, 4)],
        result: None,
    };
    let plan = evaluate_call_plan(CallingPolicy::Aapcs64, &signature).expect("AAPCS HFA");
    assert_eq!(plan.parameters[0].locations.len(), 4);
    assert!(matches!(
        plan.parameters[0].locations[3],
        ValueLocation::Register {
            register: MachineRegister::Aarch64V(3),
            value_byte_offset: 24,
            ..
        }
    ));
}

#[test]
fn aapcs_small_integer_aggregates_use_consecutive_x_registers() {
    let signature = CallSignature {
        parameters: vec![ValueShape::integer(8, 8), ValueShape::integer(16, 16)],
        result: Some(ValueShape::integer(12, 8)),
    };
    let plan =
        evaluate_call_plan(CallingPolicy::Aapcs64, &signature).expect("AAPCS small aggregate plan");

    assert_eq!(
        plan.parameters[1].locations,
        vec![
            ValueLocation::Register {
                register: MachineRegister::Aarch64X(2),
                value_byte_offset: 0,
                byte_size: 8,
            },
            ValueLocation::Register {
                register: MachineRegister::Aarch64X(3),
                value_byte_offset: 8,
                byte_size: 8,
            },
        ]
    );
    assert_eq!(
        plan.result.expect("aggregate result").locations,
        vec![
            ValueLocation::Register {
                register: MachineRegister::Aarch64X(0),
                value_byte_offset: 0,
                byte_size: 8,
            },
            ValueLocation::Register {
                register: MachineRegister::Aarch64X(1),
                value_byte_offset: 8,
                byte_size: 4,
            },
        ]
    );
}

#[test]
fn aapcs_small_aggregate_moves_wholly_to_stack_when_x_registers_run_out() {
    let signature = CallSignature {
        parameters: vec![ValueShape::integer(8, 8); 7]
            .into_iter()
            .chain([ValueShape::integer(16, 8), ValueShape::integer(8, 8)])
            .collect(),
        result: None,
    };
    let plan = evaluate_call_plan(CallingPolicy::Aapcs64, &signature)
        .expect("AAPCS exhausted aggregate plan");

    assert_eq!(
        plan.parameters[7].locations,
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
        plan.parameters[8].locations.as_slice(),
        [ValueLocation::Stack {
            stack_byte_offset: 16,
            ..
        }]
    ));
}

#[test]
fn aapcs_large_aggregates_use_caller_copies_and_indirect_results() {
    let signature = CallSignature {
        parameters: vec![ValueShape::integer(24, 8), ValueShape::integer(8, 8)],
        result: Some(ValueShape::integer(32, 16)),
    };
    let plan =
        evaluate_call_plan(CallingPolicy::Aapcs64, &signature).expect("AAPCS large aggregate plan");

    assert_eq!(
        plan.parameters[0].locations,
        vec![ValueLocation::Indirect {
            pointer: IndirectPointerLocation::Register(MachineRegister::Aarch64X(0)),
            copy_stack_byte_offset: Some(0),
            byte_size: 24,
            alignment: 8,
        }]
    );
    assert!(matches!(
        plan.parameters[1].locations.as_slice(),
        [ValueLocation::Register {
            register: MachineRegister::Aarch64X(1),
            ..
        }]
    ));
    assert_eq!(
        plan.result.expect("indirect aggregate result").locations,
        vec![ValueLocation::Indirect {
            pointer: IndirectPointerLocation::Register(MachineRegister::Aarch64X(8)),
            copy_stack_byte_offset: None,
            byte_size: 32,
            alignment: 16,
        }]
    );
}

#[test]
fn aapcs_large_aggregate_pointer_uses_stack_before_its_copy() {
    let signature = CallSignature {
        parameters: vec![ValueShape::integer(8, 8); 8]
            .into_iter()
            .chain([ValueShape::integer(24, 16)])
            .collect(),
        result: None,
    };
    let plan = evaluate_call_plan(CallingPolicy::Aapcs64, &signature)
        .expect("AAPCS stack-indirect aggregate plan");

    assert_eq!(
        plan.parameters[8].locations,
        vec![ValueLocation::Indirect {
            pointer: IndirectPointerLocation::Stack {
                stack_byte_offset: 0,
                alignment: 8,
            },
            copy_stack_byte_offset: Some(16),
            byte_size: 24,
            alignment: 16,
        }]
    );
}

#[test]
fn linux_syscall_pins_non_c_call_registers() {
    let plan = evaluate_call_plan(CallingPolicy::LinuxSyscallX86_64, &integer_signature(6))
        .expect("Linux syscall");
    assert!(matches!(
        plan.parameters[3].locations[0],
        ValueLocation::Register {
            register: MachineRegister::X86R10,
            ..
        }
    ));
    assert!(matches!(
        plan.entry_control,
        EntryControl::SupervisorCall {
            number_register: MachineRegister::X86Rax,
            immediate: 0,
        }
    ));

    let aarch64 = evaluate_call_plan(CallingPolicy::LinuxSyscallAarch64, &integer_signature(6))
        .expect("AArch64 Linux syscall");
    assert!(
        aarch64
            .ordinary_clobbers
            .contains(MachineRegister::Aarch64X(8)),
        "the number register is part of the realized syscall clobber set"
    );
}
