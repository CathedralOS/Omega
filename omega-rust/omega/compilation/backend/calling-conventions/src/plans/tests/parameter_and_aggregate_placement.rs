/*
We test parameter and aggregate placement across Microsoft x64, System V AMD64,
AAPCS64, and Linux syscall conventions, covering shadow space, zero-sized
parameter erasure, indirect results and caller-owned copies, independent
integer/float register banks, and register-exhaustion stack spillover.
*/

use super::{call_signature, integer_signature};
use crate::plans::{
    CallingPolicy, EntryControl, IndirectPointerLocation, MachineRegister, ValueLocation,
    ValueShape, evaluate_call_plan, validate_call_plan,
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
        let plan = evaluate_call_plan(policy, &call_signature(vec![empty, scalar, empty], None))
            .expect("canonical empty values are erased from ABI placement");
        assert_eq!(plan.parameters.len(), 3);
        assert_eq!(plan.parameters[0].shape, empty);
        assert!(plan.parameters[0].locations.is_empty());
        assert_eq!(plan.parameters[1].shape, scalar);
        assert!(!plan.parameters[1].locations.is_empty());
        assert_eq!(plan.parameters[2].shape, empty);
        assert!(plan.parameters[2].locations.is_empty());
        validate_call_plan(&plan, &call_signature(vec![empty, scalar, empty], None)).unwrap();
    }
}

#[test]
fn microsoft_x64_indirect_result_uses_rcx_and_shifts_parameters() {
    let signature = call_signature(
        vec![ValueShape::integer(8, 8); 4],
        Some(ValueShape::integer(16, 8)),
    );
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
    let signature = call_signature(
        vec![
            ValueShape::integer(16, 8),
            ValueShape::integer(8, 8),
            ValueShape::integer(8, 8),
            ValueShape::integer(8, 8),
            ValueShape::integer(16, 8),
        ],
        None,
    );
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
    let signature = call_signature(
        vec![
            ValueShape::integer(8, 8),
            ValueShape::float(8),
            ValueShape::integer(8, 8),
            ValueShape::float(8),
        ],
        None,
    );
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
fn system_v_small_integer_aggregates_use_consecutive_registers_and_results() {
    let signature = call_signature(
        vec![ValueShape::integer(8, 8), ValueShape::integer(16, 8)],
        Some(ValueShape::integer(12, 8)),
    );
    let plan = evaluate_call_plan(CallingPolicy::SystemVAMD64, &signature)
        .expect("SysV small aggregate plan");

    assert_eq!(
        plan.parameters[1].locations,
        vec![
            ValueLocation::register(MachineRegister::X86Rsi, 0, 8),
            ValueLocation::register(MachineRegister::X86Rdx, 8, 8),
        ]
    );
    assert_eq!(
        plan.result.expect("aggregate result").locations,
        vec![
            ValueLocation::register(MachineRegister::X86Rax, 0, 8),
            ValueLocation::register(MachineRegister::X86Rdx, 8, 4),
        ]
    );
}

#[test]
fn system_v_register_exhausted_aggregate_moves_wholly_to_stack_and_rolls_back() {
    let signature = call_signature(
        vec![ValueShape::integer(8, 8); 5]
            .into_iter()
            .chain([ValueShape::integer(16, 8), ValueShape::integer(8, 8)])
            .collect(),
        None,
    );
    let plan = evaluate_call_plan(CallingPolicy::SystemVAMD64, &signature)
        .expect("SysV exhausted aggregate plan");

    assert_eq!(
        plan.parameters[5].locations,
        vec![
            ValueLocation::stack(0, 0, 8, 8),
            ValueLocation::stack(8, 8, 8, 8),
        ]
    );
    assert_eq!(
        plan.parameters[6].locations,
        vec![ValueLocation::register(MachineRegister::X86R9, 0, 8)]
    );
}

#[test]
fn aapcs_hfa_is_one_value_split_across_vector_registers() {
    let signature = call_signature(vec![ValueShape::homogeneous_float_aggregate(8, 4)], None);
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

/// AAPCS64 C.3: a float record that misses the vector registers closes them,
/// so the `f64` after it goes to the stack (as C passes it), not to `v7`.
#[test]
fn aapcs_hfa_that_misses_vector_registers_closes_them_for_later_floats() {
    let signature = call_signature(
        vec![ValueShape::float(8); 7]
            .into_iter()
            .chain([
                ValueShape::homogeneous_float_aggregate(8, 2),
                ValueShape::float(8),
            ])
            .collect(),
        None,
    );
    let plan = evaluate_call_plan(CallingPolicy::Aapcs64, &signature).expect("AAPCS HFA overflow");

    assert_eq!(
        plan.parameters[7].locations,
        vec![ValueLocation::stack(0, 0, 16, 8)]
    );
    assert_eq!(
        plan.parameters[8].locations,
        vec![ValueLocation::stack(16, 0, 8, 8)]
    );
}

/// The twin: with only seven floats before it, the last `f64` still takes `v7`.
#[test]
fn aapcs_float_after_seven_floats_takes_the_last_vector_register() {
    let signature = call_signature(vec![ValueShape::float(8); 8], None);
    let plan = evaluate_call_plan(CallingPolicy::Aapcs64, &signature).expect("AAPCS floats");

    assert_eq!(
        plan.parameters[7].locations,
        vec![ValueLocation::register(MachineRegister::Aarch64V(7), 0, 8)]
    );
}

/// AAPCS64 C.3: a stacked three-`f32` record takes 16 bytes, so the `u32`
/// after it sits at sp+16, not sp+12.
#[test]
fn aapcs_stacked_hfa_size_rounds_up_to_eight_bytes() {
    let signature = call_signature(
        vec![ValueShape::integer(8, 8); 8]
            .into_iter()
            .chain(vec![ValueShape::float(8); 8])
            .chain([
                ValueShape::homogeneous_float_aggregate(4, 3),
                ValueShape::integer(4, 4),
            ])
            .collect(),
        None,
    );
    let plan = evaluate_call_plan(CallingPolicy::Aapcs64, &signature).expect("AAPCS stacked HFA");

    assert_eq!(
        plan.parameters[16].locations,
        vec![ValueLocation::stack(0, 0, 12, 4)]
    );
    assert_eq!(
        plan.parameters[17].locations,
        vec![ValueLocation::stack(16, 0, 4, 4)]
    );
}

#[test]
fn aapcs_small_integer_aggregates_use_consecutive_x_registers() {
    let signature = call_signature(
        vec![ValueShape::integer(8, 8), ValueShape::integer(16, 16)],
        Some(ValueShape::integer(12, 8)),
    );
    let plan =
        evaluate_call_plan(CallingPolicy::Aapcs64, &signature).expect("AAPCS small aggregate plan");

    assert_eq!(
        plan.parameters[1].locations,
        vec![
            ValueLocation::register(MachineRegister::Aarch64X(2), 0, 8),
            ValueLocation::register(MachineRegister::Aarch64X(3), 8, 8),
        ]
    );
    assert_eq!(
        plan.result.expect("aggregate result").locations,
        vec![
            ValueLocation::register(MachineRegister::Aarch64X(0), 0, 8),
            ValueLocation::register(MachineRegister::Aarch64X(1), 8, 4),
        ]
    );
}

#[test]
fn darwin_aligned_arguments_start_in_the_next_integer_register_without_a_gap() {
    for preceding in [1, 3, 5] {
        let mut parameters = vec![ValueShape::integer(8, 8); preceding];
        parameters.extend([ValueShape::integer(16, 16), ValueShape::integer(8, 8)]);
        let signature = call_signature(parameters, None);
        for (target, first) in [
            (target::NativeTarget::macos_arm64(), preceding as u8),
            (target::NativeTarget::linux_arm64(), preceding as u8 + 1),
        ] {
            let plan = evaluate_call_plan(CallingPolicy::native_for_target(target), &signature)
                .expect("aligned argument plan");
            validate_call_plan(&plan, &signature).expect("valid aligned argument plan");
            assert_eq!(
                plan.parameters[preceding].locations,
                [
                    ValueLocation::register(MachineRegister::Aarch64X(first), 0, 8),
                    ValueLocation::register(MachineRegister::Aarch64X(first + 1), 8, 8),
                ]
            );
            assert_eq!(
                plan.parameters[preceding + 1].locations,
                [if first + 2 < 8 {
                    ValueLocation::register(MachineRegister::Aarch64X(first + 2), 0, 8)
                } else {
                    ValueLocation::stack(0, 0, 8, 8)
                }]
            );
        }
    }
}

#[test]
fn arm64_aligned_arguments_fit_the_last_pair_or_spill_wholly_and_close_the_bank() {
    for preceding in [6, 7] {
        let mut parameters = vec![ValueShape::integer(8, 8); preceding];
        parameters.extend([ValueShape::integer(16, 16), ValueShape::integer(8, 8)]);
        let signature = call_signature(parameters, None);
        for policy in [CallingPolicy::DarwinAapcs64, CallingPolicy::Aapcs64] {
            let plan = evaluate_call_plan(policy, &signature).expect("register exhaustion plan");
            validate_call_plan(&plan, &signature).expect("valid register exhaustion plan");
            let expected = if preceding == 6 {
                vec![
                    ValueLocation::register(MachineRegister::Aarch64X(6), 0, 8),
                    ValueLocation::register(MachineRegister::Aarch64X(7), 8, 8),
                ]
            } else {
                vec![
                    ValueLocation::stack(0, 0, 8, 8),
                    ValueLocation::stack(8, 8, 8, 8),
                ]
            };
            assert_eq!(plan.parameters[preceding].locations, expected);
            assert_eq!(
                plan.parameters[preceding + 1].locations,
                [ValueLocation::stack(
                    if preceding == 6 { 0 } else { 16 },
                    0,
                    8,
                    8
                )]
            );
        }
    }
}

#[test]
fn aapcs_small_aggregate_moves_wholly_to_stack_when_x_registers_run_out() {
    let signature = call_signature(
        vec![ValueShape::integer(8, 8); 7]
            .into_iter()
            .chain([ValueShape::integer(16, 8), ValueShape::integer(8, 8)])
            .collect(),
        None,
    );
    let plan = evaluate_call_plan(CallingPolicy::Aapcs64, &signature)
        .expect("AAPCS exhausted aggregate plan");

    assert_eq!(
        plan.parameters[7].locations,
        vec![
            ValueLocation::stack(0, 0, 8, 8),
            ValueLocation::stack(8, 8, 8, 8),
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
    let signature = call_signature(
        vec![ValueShape::integer(24, 8), ValueShape::integer(8, 8)],
        Some(ValueShape::integer(32, 16)),
    );
    let plan =
        evaluate_call_plan(CallingPolicy::Aapcs64, &signature).expect("AAPCS large aggregate plan");

    assert_eq!(
        plan.parameters[0].locations,
        vec![ValueLocation::indirect(
            IndirectPointerLocation::Register(MachineRegister::Aarch64X(0)),
            Some(0),
            24,
            8
        )]
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
        vec![ValueLocation::indirect(
            IndirectPointerLocation::Register(MachineRegister::Aarch64X(8)),
            None,
            32,
            16
        )]
    );
}

#[test]
fn aapcs_large_aggregate_pointer_uses_stack_before_its_copy() {
    let signature = call_signature(
        vec![ValueShape::integer(8, 8); 8]
            .into_iter()
            .chain([ValueShape::integer(24, 16)])
            .collect(),
        None,
    );
    let plan = evaluate_call_plan(CallingPolicy::Aapcs64, &signature)
        .expect("AAPCS stack-indirect aggregate plan");

    assert_eq!(
        plan.parameters[8].locations,
        vec![ValueLocation::indirect(
            IndirectPointerLocation::Stack {
                stack_byte_offset: 0,
                alignment: 8,
            },
            Some(16),
            24,
            16
        )]
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
