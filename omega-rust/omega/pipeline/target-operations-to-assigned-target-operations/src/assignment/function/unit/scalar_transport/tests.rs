use super::*;
use calling_conventions::{
    CallSignature, CallingPolicy, ValuePlacement, ValueShape, evaluate_call_plan,
};
use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, OperationId, ScalarType, ValueId,
};
use target_operations::MachineRegister;

fn empty_call(target: NativeTarget) -> CallPlan {
    evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: Vec::new(),
            result: None,
        },
    )
    .unwrap()
}

fn parameter(
    parameter_index: u32,
    source: MachineRegister,
    destination: MachineRegister,
) -> AssignedUnitScalarCallArgument {
    AssignedUnitScalarCallArgument {
        parameter_index,
        source: AssignedUnitScalarArgumentSource::Parameter {
            parameter_index,
            source_value: ValueId::new(u64::from(parameter_index) + 1).unwrap(),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
            location: AssignedScalarLocation::Register(source),
        },
        destination: AssignedCallDestination::Register(destination),
    }
}

#[test]
fn empty_calls_retain_exact_target_stack_congruences() {
    for (target, expected) in [
        (NativeTarget::linux_x64(), 8),
        (NativeTarget::windows_x64(), 40),
        (NativeTarget::linux_arm64(), 0),
    ] {
        let transport = assign(&empty_call(target), &[], target, CallTransportKind::Mixed).unwrap();
        assert_eq!(transport.call_stack_bytes, expected);
        assert!(transport.snapshot_slots.is_empty());
    }
}

#[test]
fn scalar_result_coff_minimum_is_distinct_from_mixed_transport() {
    let target = NativeTarget::windows_x64();
    let mut call = empty_call(target);
    // Isolate the transport policy from the separately validated ABI plan.
    call.shadow_bytes = 0;
    assert_eq!(
        assign(&call, &[], target, CallTransportKind::ScalarResult)
            .unwrap()
            .call_stack_bytes,
        40
    );
    assert_eq!(
        assign(&call, &[], target, CallTransportKind::Mixed)
            .unwrap()
            .call_stack_bytes,
        8
    );
}

#[test]
fn unchanged_register_handoff_needs_no_snapshots() {
    let target = NativeTarget::linux_x64();
    let arguments = [
        parameter(0, MachineRegister::X86Rdi, MachineRegister::X86Rdi),
        parameter(1, MachineRegister::X86Rsi, MachineRegister::X86Rsi),
    ];
    let transport = assign(
        &empty_call(target),
        &arguments,
        target,
        CallTransportKind::Mixed,
    )
    .unwrap();
    assert_eq!(transport.call_stack_bytes, 8);
    assert!(transport.snapshot_slots.is_empty());
}

#[test]
fn swapped_sources_have_unique_first_seen_snapshot_slots() {
    for (target, first, second, expected_stack) in [
        (
            NativeTarget::linux_x64(),
            MachineRegister::X86Rsi,
            MachineRegister::X86Rdi,
            24,
        ),
        (
            NativeTarget::linux_arm64(),
            MachineRegister::Aarch64X(1),
            MachineRegister::Aarch64X(0),
            16,
        ),
    ] {
        let arguments = [
            parameter(0, first, second),
            parameter(1, second, first),
            parameter(2, first, first),
        ];
        let transport = assign(
            &empty_call(target),
            &arguments,
            target,
            CallTransportKind::Mixed,
        )
        .unwrap();
        assert_eq!(transport.snapshot_slots, [(first, 0), (second, 8)]);
        assert_eq!(transport.call_stack_bytes, expected_stack);
    }
}

#[test]
fn immediate_overwriting_a_later_parameter_requires_preservation() {
    let target = NativeTarget::linux_x64();
    let mut immediate = parameter(0, MachineRegister::X86Rdi, MachineRegister::X86Rsi);
    immediate.source = AssignedUnitScalarArgumentSource::IntegerImmediate {
        defining_operation: OperationId::new(1).unwrap(),
        source_value: ValueId::new(1).unwrap(),
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        value: IntegerValue::Unsigned(7),
    };
    let arguments = [
        immediate,
        parameter(1, MachineRegister::X86Rsi, MachineRegister::X86Rdi),
    ];
    let transport = assign(
        &empty_call(target),
        &arguments,
        target,
        CallTransportKind::Mixed,
    )
    .unwrap();
    assert_eq!(transport.snapshot_slots, [(MachineRegister::X86Rsi, 0)]);
    assert_eq!(transport.call_stack_bytes, 8);
}

#[test]
fn stack_arguments_precede_aligned_snapshot_storage() {
    let target = NativeTarget::linux_x64();
    let mut call = empty_call(target);
    call.parameters.push(ValuePlacement {
        shape: ValueShape::integer(1, 1),
        locations: vec![ValueLocation::Stack {
            stack_byte_offset: 8,
            value_byte_offset: 0,
            byte_size: 1,
            alignment: 8,
        }],
    });
    let arguments = [
        parameter(0, MachineRegister::X86Rsi, MachineRegister::X86Rdi),
        parameter(1, MachineRegister::X86Rdi, MachineRegister::X86Rsi),
    ];
    let transport = assign(&call, &arguments, target, CallTransportKind::Mixed).unwrap();
    assert_eq!(
        transport.snapshot_slots,
        [(MachineRegister::X86Rsi, 16), (MachineRegister::X86Rdi, 24)]
    );
    assert_eq!(transport.call_stack_bytes, 40);
}

#[test]
fn only_aarch64_reserves_the_indirect_aggregate_copy_extent() {
    for (target, expected) in [
        (NativeTarget::linux_x64(), 24),
        (NativeTarget::linux_arm64(), 64),
    ] {
        let mut call = empty_call(target);
        call.parameters.push(ValuePlacement {
            shape: ValueShape::integer(17, 8),
            locations: vec![ValueLocation::Indirect {
                pointer: IndirectPointerLocation::Stack {
                    stack_byte_offset: 8,
                    alignment: 8,
                },
                copy_stack_byte_offset: Some(32),
                byte_size: 17,
                alignment: 8,
            }],
        });
        let transport = assign(&call, &[], target, CallTransportKind::Mixed).unwrap();
        assert_eq!(transport.call_stack_bytes, expected);
        assert!(transport.snapshot_slots.is_empty());
    }
}

#[test]
fn stack_extent_and_final_padding_overflows_reject() {
    for (target, offset, width) in [
        (NativeTarget::linux_x64(), u32::MAX, 8),
        (NativeTarget::linux_x64(), u32::MAX - 8, 8),
        (NativeTarget::linux_arm64(), u32::MAX - 8, 8),
    ] {
        let mut call = empty_call(target);
        call.parameters.push(ValuePlacement {
            shape: ValueShape::integer(width, 8),
            locations: vec![ValueLocation::Stack {
                stack_byte_offset: offset,
                value_byte_offset: 0,
                byte_size: width,
                alignment: 8,
            }],
        });
        assert_eq!(
            assign(&call, &[], target, CallTransportKind::Mixed),
            Err(AssignmentError::UnitScalarFrameNotEncodable)
        );
    }
}

#[test]
fn aarch64_indirect_copy_overflow_rejects() {
    let target = NativeTarget::linux_arm64();
    let mut call = empty_call(target);
    call.parameters.push(ValuePlacement {
        shape: ValueShape::integer(17, 8),
        locations: vec![ValueLocation::Indirect {
            pointer: IndirectPointerLocation::Register(MachineRegister::Aarch64X(0)),
            copy_stack_byte_offset: Some(u32::MAX - 15),
            byte_size: 17,
            alignment: 8,
        }],
    });
    assert_eq!(
        assign(&call, &[], target, CallTransportKind::Mixed),
        Err(AssignmentError::UnitScalarFrameNotEncodable)
    );
}
