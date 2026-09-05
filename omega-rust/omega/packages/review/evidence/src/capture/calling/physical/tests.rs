use super::*;
use crate::record::{
    PackageReviewBoundaryCallingPolicy as Policy, PackageReviewBoundaryValueClass as Class,
    PackageReviewBoundaryValueLocation as Location,
    PackageReviewIndirectPointerLocation as Pointer, PackageReviewMachineRegister as Register,
    PackageReviewSystemVEightbyteClass as Eightbyte,
};
use calling_conventions::{
    CallSignature, CallbackMaterialization, CallingPolicy, IndirectPointerLocation,
    MachineRegister, NativeParameterId, NativePlace, RegisterSet, StaticMachineBinderId,
    SystemVEightbyteClass, ValueClass, ValueLocation, ValuePlacement, ValueShape,
    evaluate_ordinary_boundary_entry_plan,
};

// Every projection fixture also exercises its independent bounded codec.
fn project(plan: &BoundaryEntryPlan) -> PackagePolicyPhysicalCallingContract {
    let component = super::project(plan);
    let bytes = component.canonical_bytes().unwrap();
    let recovered = PackagePolicyPhysicalCallingContract::recover_canonical(
        &bytes,
        crate::encoding::PackagePolicyRecoveryLimits::default(),
    )
    .unwrap();
    assert_eq!(recovered, component);
    assert_eq!(recovered.canonical_bytes().unwrap(), bytes);
    component
}

fn fixture() -> BoundaryEntryPlan {
    evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::SystemVAMD64,
        &CallSignature {
            parameters: vec![ValueShape::integer(8, 8)],
            result: None,
        },
    )
    .unwrap()
    .plan()
    .clone()
}

#[test]
fn validated_capture_retains_each_physical_field_without_mutating_the_plan() {
    let validated = evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::MicrosoftX64,
        &CallSignature {
            parameters: vec![ValueShape::integer(8, 8), ValueShape::float(8)],
            result: Some(ValueShape::integer(8, 8)),
        },
    )
    .unwrap();
    let original = validated.clone();
    let projected = PackagePolicyPhysicalCallingContract::from_validated_plan(&validated);
    assert_eq!(projected, project(validated.plan()));
    assert_eq!(validated, original);
    assert_eq!(projected.parameters().len(), 2);
    assert!(projected.result().is_some());
    assert_eq!(projected.stack_alignment(), 16);
    assert_eq!(projected.shadow_bytes(), 32);
    assert_eq!(
        projected.entry_control(),
        PackagePolicyEntryControl::CallReturn
    );
    assert_eq!(projected.policy(), Policy::MicrosoftX64);
    assert_eq!(
        projected.state().initial_regime(),
        PackagePolicyMachineRegime::X86Long64
    );

    let baseline = project(&fixture());
    let mutations: &[fn(&mut BoundaryEntryPlan)] = &[
        |plan| plan.call.policy = CallingPolicy::MicrosoftX64,
        |plan| plan.call.parameters[0].shape.byte_size = 4,
        |plan| plan.call.parameters[0].shape.alignment = 4,
        |plan| plan.call.parameters[0].locations.clear(),
        |plan| plan.call.result = Some(plan.call.parameters[0].clone()),
        |plan| plan.call.ordinary_clobbers = RegisterSet::default(),
        |plan| plan.call.stack_alignment = 32,
        |plan| plan.call.shadow_bytes = 24,
        |plan| plan.call.entry_control = EntryControl::InterruptReturn,
        |plan| plan.state.initial_regime = MachineRegime::Aarch64A64 { exception_level: 2 },
        |plan| plan.state.interrupted_state = MachineStateSet::new([MachineState::DebugState]),
        |plan| plan.state.saved_state = MachineStateSet::new([MachineState::DebugState]),
        |plan| plan.state.restored_state = MachineStateSet::new([MachineState::DebugState]),
        |plan| {
            plan.state.permitted_transitive_use = MachineStateSet::new([MachineState::DebugState])
        },
        |plan| plan.state.stack = EntryStack::Dedicated { class: 17 },
        |plan| plan.state.preemption = Preemption::Nestable { maximum_depth: 23 },
    ];
    for (index, mutate) in mutations.iter().enumerate() {
        let mut plan = fixture();
        mutate(&mut plan);
        assert_ne!(project(&plan), baseline, "physical field mutation {index}");
    }
}

#[test]
fn all_policy_control_regime_stack_and_preemption_variants_retain_payloads() {
    let mut plan = fixture();
    for (source, expected) in [
        (CallingPolicy::MicrosoftX64, Policy::MicrosoftX64),
        (CallingPolicy::SystemVAMD64, Policy::SystemVAMD64),
        (CallingPolicy::Aapcs64, Policy::Aapcs64),
        (
            CallingPolicy::LinuxSyscallX86_64,
            Policy::LinuxSyscallX86_64,
        ),
        (
            CallingPolicy::LinuxSyscallAarch64,
            Policy::LinuxSyscallAarch64,
        ),
    ] {
        plan.call.policy = source;
        assert_eq!(project(&plan).policy(), expected);
    }
    for (source, expected) in [
        (
            EntryControl::CallReturn,
            PackagePolicyEntryControl::CallReturn,
        ),
        (
            EntryControl::InterruptReturn,
            PackagePolicyEntryControl::InterruptReturn,
        ),
        (
            EntryControl::SupervisorCall {
                number_register: MachineRegister::Aarch64X(8),
                immediate: 65535,
            },
            PackagePolicyEntryControl::SupervisorCall {
                number_register: Register::Aarch64X(8),
                immediate: 65535,
            },
        ),
    ] {
        plan.call.entry_control = source;
        assert_eq!(project(&plan).entry_control(), expected);
    }
    for exception_level in 0..=3 {
        plan.state.initial_regime = MachineRegime::Aarch64A64 { exception_level };
        assert_eq!(
            project(&plan).state().initial_regime(),
            PackagePolicyMachineRegime::Aarch64A64 { exception_level }
        );
    }
    for (source, expected) in [
        (
            EntryStack::Interrupted,
            PackagePolicyEntryStack::Interrupted,
        ),
        (
            EntryStack::Dedicated { class: 65535 },
            PackagePolicyEntryStack::Dedicated { class: 65535 },
        ),
        (
            EntryStack::ProviderSelected,
            PackagePolicyEntryStack::ProviderSelected,
        ),
    ] {
        plan.state.stack = source;
        assert_eq!(project(&plan).state().stack(), expected);
    }
    for (source, expected) in [
        (
            Preemption::NotApplicable,
            PackagePolicyPreemption::NotApplicable,
        ),
        (Preemption::Masked, PackagePolicyPreemption::Masked),
        (
            Preemption::Nestable {
                maximum_depth: 65535,
            },
            PackagePolicyPreemption::Nestable {
                maximum_depth: 65535,
            },
        ),
        (
            Preemption::ProviderDefined,
            PackagePolicyPreemption::ProviderDefined,
        ),
    ] {
        plan.state.preemption = source;
        assert_eq!(project(&plan).state().preemption(), expected);
    }
}

#[test]
fn every_machine_state_subset_remains_distinct_in_each_independent_set() {
    let classes = [
        (
            MachineState::GeneralRegisters,
            PackagePolicyMachineState::GeneralRegisters,
        ),
        (
            MachineState::VectorRegisters,
            PackagePolicyMachineState::VectorRegisters,
        ),
        (MachineState::Flags, PackagePolicyMachineState::Flags),
        (
            MachineState::InstructionPointer,
            PackagePolicyMachineState::InstructionPointer,
        ),
        (
            MachineState::StackPointer,
            PackagePolicyMachineState::StackPointer,
        ),
        (
            MachineState::SegmentState,
            PackagePolicyMachineState::SegmentState,
        ),
        (
            MachineState::ControlState,
            PackagePolicyMachineState::ControlState,
        ),
        (
            MachineState::DebugState,
            PackagePolicyMachineState::DebugState,
        ),
        (
            MachineState::ExtendedState,
            PackagePolicyMachineState::ExtendedState,
        ),
    ];
    for bits in 0..512 {
        let selected: Vec<_> = classes
            .iter()
            .enumerate()
            .filter(|(ordinal, _)| bits & (1 << ordinal) != 0)
            .map(|(_, pair)| *pair)
            .collect();
        let native = MachineStateSet::new(selected.iter().map(|pair| pair.0));
        let expected: Vec<_> = selected.iter().map(|pair| pair.1).collect();
        assert_eq!(state_set(native).as_slice(), expected);
        let reordered = PackagePolicyMachineStateSet::new(
            expected
                .iter()
                .rev()
                .copied()
                .chain(expected.iter().copied()),
        );
        assert_eq!(reordered.as_slice(), expected);
        for ordinal in 0..4 {
            let mut plan = fixture();
            plan.state.interrupted_state = MachineStateSet::empty();
            plan.state.saved_state = MachineStateSet::empty();
            plan.state.restored_state = MachineStateSet::empty();
            plan.state.permitted_transitive_use = MachineStateSet::empty();
            match ordinal {
                0 => plan.state.interrupted_state = native,
                1 => plan.state.saved_state = native,
                2 => plan.state.restored_state = native,
                _ => plan.state.permitted_transitive_use = native,
            }
            let captured = project(&plan);
            let state = captured.state();
            let sets = [
                state.interrupted_state(),
                state.saved_state(),
                state.restored_state(),
                state.permitted_transitive_use(),
            ];
            for (index, set) in sets.iter().enumerate() {
                assert_eq!(
                    set.as_slice(),
                    if index == ordinal {
                        expected.as_slice()
                    } else {
                        &[]
                    }
                );
            }
        }
    }
}

#[test]
fn all_shape_and_location_variants_preserve_values_and_fragment_order() {
    let classes = [
        (ValueClass::Integer, Class::Integer),
        (ValueClass::Float, Class::Float),
        (ValueClass::BorrowedReference, Class::BorrowedReference),
        (
            ValueClass::HomogeneousFloatAggregate { members: 3 },
            Class::HomogeneousFloatAggregate { members: 3 },
        ),
        (
            ValueClass::SystemVAggregate {
                first: SystemVEightbyteClass::Integer,
                second: SystemVEightbyteClass::Sse,
            },
            Class::SystemVAggregate {
                first: Eightbyte::Integer,
                second: Eightbyte::Sse,
            },
        ),
        (
            ValueClass::SystemVAggregate {
                first: SystemVEightbyteClass::Sse,
                second: SystemVEightbyteClass::Integer,
            },
            Class::SystemVAggregate {
                first: Eightbyte::Sse,
                second: Eightbyte::Integer,
            },
        ),
    ];
    let mut plan = fixture();
    plan.call.parameters = classes
        .iter()
        .map(|(class, _)| ValuePlacement {
            shape: ValueShape {
                class: *class,
                byte_size: 24,
                alignment: 8,
            },
            locations: vec![
                ValueLocation::Register {
                    register: MachineRegister::X86Rdi,
                    value_byte_offset: 8,
                    byte_size: 4,
                },
                ValueLocation::Stack {
                    stack_byte_offset: 72,
                    value_byte_offset: 12,
                    byte_size: 12,
                    alignment: 4,
                },
                ValueLocation::Indirect {
                    pointer: IndirectPointerLocation::Register(MachineRegister::X86Rax),
                    copy_stack_byte_offset: Some(96),
                    byte_size: 24,
                    alignment: 8,
                },
                ValueLocation::Indirect {
                    pointer: IndirectPointerLocation::Stack {
                        stack_byte_offset: 128,
                        alignment: 16,
                    },
                    copy_stack_byte_offset: None,
                    byte_size: 24,
                    alignment: 8,
                },
            ],
        })
        .collect();
    plan.call.result = Some(plan.call.parameters[2].clone());
    let projected = project(&plan);
    let expected_locations = [
        Location::Register {
            register: Register::X86Rdi,
            value_byte_offset: 8,
            byte_size: 4,
        },
        Location::Stack {
            stack_byte_offset: 72,
            value_byte_offset: 12,
            byte_size: 12,
            alignment: 4,
        },
        Location::Indirect {
            pointer: Pointer::Register(Register::X86Rax),
            copy_stack_byte_offset: Some(96),
            byte_size: 24,
            alignment: 8,
        },
        Location::Indirect {
            pointer: Pointer::Stack {
                stack_byte_offset: 128,
                alignment: 16,
            },
            copy_stack_byte_offset: None,
            byte_size: 24,
            alignment: 8,
        },
    ];
    for (parameter, (_, class)) in projected.parameters().iter().zip(classes) {
        assert_eq!(parameter.shape().class(), class);
        assert_eq!(parameter.shape().byte_size(), 24);
        assert_eq!(parameter.shape().alignment(), 8);
        assert_eq!(parameter.locations(), expected_locations);
    }
    assert_eq!(projected.result(), Some(&projected.parameters()[2]));
    plan.call.parameters.swap(0, 1);
    assert_ne!(project(&plan), projected);
    plan.call.parameters.swap(0, 1);
    plan.call.parameters[0].locations.swap(0, 1);
    assert_ne!(project(&plan), projected);
}

#[test]
fn register_sets_are_canonical_and_cover_every_register_variant() {
    let registers = [
        (MachineRegister::X86Rax, Register::X86Rax),
        (MachineRegister::X86Rcx, Register::X86Rcx),
        (MachineRegister::X86Rdx, Register::X86Rdx),
        (MachineRegister::X86Rbx, Register::X86Rbx),
        (MachineRegister::X86Rsp, Register::X86Rsp),
        (MachineRegister::X86Rbp, Register::X86Rbp),
        (MachineRegister::X86Rsi, Register::X86Rsi),
        (MachineRegister::X86Rdi, Register::X86Rdi),
        (MachineRegister::X86R8, Register::X86R8),
        (MachineRegister::X86R9, Register::X86R9),
        (MachineRegister::X86R10, Register::X86R10),
        (MachineRegister::X86R11, Register::X86R11),
        (MachineRegister::X86R12, Register::X86R12),
        (MachineRegister::X86R13, Register::X86R13),
        (MachineRegister::X86R14, Register::X86R14),
        (MachineRegister::X86R15, Register::X86R15),
        (MachineRegister::X86Xmm(15), Register::X86Xmm(15)),
        (MachineRegister::Aarch64X(30), Register::Aarch64X(30)),
        (MachineRegister::Aarch64V(31), Register::Aarch64V(31)),
    ];
    let mut plan = fixture();
    plan.call.ordinary_clobbers = RegisterSet::new(registers.iter().map(|pair| pair.0));
    let projected = project(&plan);
    assert_eq!(
        projected.ordinary_clobbers(),
        registers.iter().map(|pair| pair.1).collect::<Vec<_>>()
    );
    plan.call.ordinary_clobbers = RegisterSet::new(
        registers
            .iter()
            .rev()
            .chain(registers.iter())
            .map(|pair| pair.0),
    );
    assert_eq!(project(&plan), projected);
}

#[test]
fn callback_materializations_are_not_misrepresented_as_physical_policy() {
    let mut plan = fixture();
    let physical = project(&plan);
    plan.call
        .callback_materializations
        .push(CallbackMaterialization {
            binder: StaticMachineBinderId::new(7).unwrap(),
            destination: NativePlace::Parameter(NativeParameterId::new(11).unwrap()),
        });
    assert_eq!(
        project(&plan),
        physical,
        "callback semantics belong to the complete wrapper, never private IDs in this physical facet"
    );
}
