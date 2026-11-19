use crate::register_model::units_of;
use register_model::RegisterConstraintKey;
use selected_instructions::ControlRegister;
use selected_instructions::GlobalDescriptorTableOperation as DescriptorOperation;
use selected_instructions::InterruptControlKind;
use selected_instructions::MachineAlternativeFamily as Family;
use selected_instructions::MachineBarrier;
use selected_instructions::MachineEffectDeclaration;
use selected_instructions::MachineEncodedEffects;
use selected_instructions::MachineEncodedMemoryEffect;
use selected_instructions::MachineEncodedStackEffect;
use selected_instructions::MachineEncodedTrapBehavior;
use selected_instructions::MachineMemoryEffect;
use selected_instructions::MachineSemanticKind as Semantic;
use selected_instructions::MachineSizeKnowledge;
use selected_instructions::MachineTrapBehavior;
use selected_instructions::MemoryFenceKind;
use selected_instructions::RegisterOperation as Operation;
use selected_instructions::RegisterTargetApplicability;
use selected_instructions::SegmentRegister;
use selected_instructions::SegmentReload;
use selected_instructions::SystemRegister;

pub fn semantic(operation: Operation) -> Option<Semantic> {
    Some(match operation {
        Operation::SystemRead(SystemRegister::Control(_)) => Semantic::ControlRegisterRead,
        Operation::SystemWrite(SystemRegister::Control(_)) => Semantic::ControlRegisterWrite,
        Operation::GlobalDescriptorTable(DescriptorOperation::Load) => {
            Semantic::GlobalDescriptorTableLoad
        }
        Operation::GlobalDescriptorTable(DescriptorOperation::Store) => {
            Semantic::GlobalDescriptorTableStore
        }
        Operation::TaskRegisterActivate => Semantic::TaskRegisterActivate,
        Operation::TaskRegisterRead => Semantic::TaskRegisterRead,
        Operation::SegmentReload(SegmentReload::Code) => Semantic::CodeSegmentReload,
        Operation::SegmentReload(SegmentReload::Data) => Semantic::DataSegmentReload,
        Operation::SegmentRead(_) => Semantic::SegmentSelectorRead,
        Operation::PortRead => Semantic::PortRegisterRead,
        Operation::PortWrite => Semantic::PortRegisterWrite,
        Operation::ModelSpecificRead => Semantic::ModelSpecificRead,
        Operation::ModelSpecificWrite => Semantic::ModelSpecificWrite,
        Operation::MemoryFence(kind) if kind.target() == RegisterTargetApplicability::X86_64 => {
            Semantic::MemoryFence
        }
        Operation::ProcessorHalt => Semantic::ProcessorHalt,
        Operation::InterruptControl(_) => Semantic::InterruptControl,
        Operation::SystemRead(SystemRegister::Rflags) => Semantic::SystemRegisterRead,
        Operation::SystemWrite(SystemRegister::Rflags) => Semantic::SystemRegisterWrite,
        _ => return None,
    })
}

pub fn operation(semantic: Semantic) -> Option<Operation> {
    Some(match semantic {
        Semantic::ControlRegisterRead => {
            Operation::SystemRead(SystemRegister::Control(ControlRegister::Cr0))
        }
        Semantic::ControlRegisterWrite => {
            Operation::SystemWrite(SystemRegister::Control(ControlRegister::Cr0))
        }
        Semantic::GlobalDescriptorTableLoad => {
            Operation::GlobalDescriptorTable(DescriptorOperation::Load)
        }
        Semantic::GlobalDescriptorTableStore => {
            Operation::GlobalDescriptorTable(DescriptorOperation::Store)
        }
        Semantic::TaskRegisterActivate => Operation::TaskRegisterActivate,
        Semantic::TaskRegisterRead => Operation::TaskRegisterRead,
        Semantic::CodeSegmentReload => Operation::SegmentReload(SegmentReload::Code),
        Semantic::DataSegmentReload => Operation::SegmentReload(SegmentReload::Data),
        Semantic::SegmentSelectorRead => Operation::SegmentRead(SegmentRegister::Code),
        Semantic::PortRegisterRead => Operation::PortRead,
        Semantic::PortRegisterWrite => Operation::PortWrite,
        Semantic::ModelSpecificRead => Operation::ModelSpecificRead,
        Semantic::ModelSpecificWrite => Operation::ModelSpecificWrite,
        Semantic::MemoryFence => Operation::MemoryFence(MemoryFenceKind::Full),
        Semantic::ProcessorHalt => Operation::ProcessorHalt,
        Semantic::InterruptControl => Operation::InterruptControl(InterruptControlKind::Disable),
        Semantic::SystemRegisterRead => Operation::SystemRead(SystemRegister::Rflags),
        Semantic::SystemRegisterWrite => Operation::SystemWrite(SystemRegister::Rflags),
        _ => return None,
    })
}

pub fn family(operation: Operation) -> Option<Family> {
    semantic(operation).map(Family::from)
}

pub fn scratch(operation: Operation) -> &'static [&'static str] {
    match operation {
        Operation::SystemRead(SystemRegister::Control(_)) => &["r10", "r15"],
        Operation::SystemWrite(SystemRegister::Control(_)) => &["rax", "r10", "r11", "r15"],
        Operation::GlobalDescriptorTable(_) => &["r10"],
        Operation::TaskRegisterActivate => &["r9", "r10", "r11", "condition_flags"],
        Operation::SegmentReload(SegmentReload::Code) => &["r9", "r10", "condition_flags"],
        Operation::SegmentReload(SegmentReload::Data) => {
            &["rcx", "rdx", "r8", "r9", "r10", "r11", "condition_flags"]
        }
        Operation::PortRead => &["rax", "rdx", "r10"],
        Operation::SystemRead(SystemRegister::Rflags)
        | Operation::SystemWrite(SystemRegister::Rflags) => &["r10", "r15"],
        Operation::PortWrite => &["rax", "rdx", "r10", "r11"],
        Operation::ModelSpecificRead | Operation::ModelSpecificWrite => {
            &["rax", "rcx", "rdx", "r10", "r11"]
        }
        _ => &[],
    }
}

pub fn effects(operation: Operation) -> MachineEncodedEffects {
    if matches!(operation, Operation::MemoryFence(_)) {
        return MachineEncodedEffects::catalog_fence_v1();
    }
    let physical = crate::x86_64_physical_register_model_ref();
    let mut encoded = MachineEncodedEffects {
        implicit_unit_clobbers: units_of(scratch(operation)),
        trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ..MachineEncodedEffects::fallthrough_v1(
            (0..operation.input_carriers().len() as u16).collect(),
            if operation.produces_value() {
                vec![operation.input_carriers().len() as u16]
            } else {
                vec![]
            },
        )
    };
    if operation == Operation::SystemRead(SystemRegister::Rflags)
        || operation == Operation::SystemWrite(SystemRegister::Rflags)
    {
        let stack_pointer = physical
            .view_named("rsp")
            .expect("canonical stack pointer")
            .id;
        let units = |name| {
            physical
                .view_named(name)
                .expect("canonical flags register")
                .units
                .iter()
                .copied()
        };
        encoded.implicit_unit_uses = if operation.produces_value() {
            register_model::sorted_units(["rflags", "rsp"].into_iter().flat_map(units))
        } else {
            register_model::sorted_units(["rsp"].into_iter().flat_map(units))
        };
        if !operation.produces_value() {
            encoded.implicit_unit_defs = register_model::sorted_units(units("rflags"));
        }
        encoded.memory = MachineEncodedMemoryEffect::TemporaryStackStorageV1 {
            stack_pointer,
            byte_count: 8,
        };
        encoded.stack = MachineEncodedStackEffect::TemporaryBytesV1 {
            stack_pointer,
            byte_count: 8,
        };
    }
    if operation == Operation::TaskRegisterActivate {
        let stack_pointer = physical
            .view_named("rsp")
            .expect("canonical stack pointer")
            .id;
        encoded.implicit_unit_uses = units_of(&["rsp", "control_flags"]);
        encoded.memory = MachineEncodedMemoryEffect::TaskRegisterActivationV1 {
            table_pointer_operand: 0,
            task_pointer_operand: 1,
            selector_operand: 2,
            scratch_pointer_operand: 3,
            table_byte_count_operand: 4,
            stack_pointer,
        };
        encoded.stack = MachineEncodedStackEffect::TemporaryBytesV1 {
            stack_pointer,
            byte_count: 8,
        };
    }
    if let Operation::SegmentReload(kind) = operation {
        let stack_pointer = physical
            .view_named("rsp")
            .expect("canonical stack pointer")
            .id;
        encoded.implicit_unit_uses = units_of(&["rsp", "control_flags"]);
        encoded.memory = MachineEncodedMemoryEffect::SegmentDescriptorReloadV1 {
            table_pointer_operand: 0,
            selector_operand: 1,
            scratch_pointer_operand: 2,
            table_byte_count_operand: 3,
            stack_pointer,
        };
        encoded.stack = MachineEncodedStackEffect::TemporaryBytesV1 {
            stack_pointer,
            byte_count: match kind {
                SegmentReload::Code => 16,
                SegmentReload::Data => 8,
            },
        };
    }
    match operation {
        Operation::GlobalDescriptorTable(DescriptorOperation::Load) => {
            encoded.memory = MachineEncodedMemoryEffect::ReadPointerV1 {
                pointer_operand: 0,
                byte_count: 10,
            };
        }
        Operation::GlobalDescriptorTable(DescriptorOperation::Store) => {
            encoded.memory = MachineEncodedMemoryEffect::WriteFixedPointerV1 {
                pointer_operand: 0,
                byte_count: 10,
            };
        }
        _ => {}
    }
    if matches!(operation, Operation::InterruptControl(_)) {
        let flags = physical
            .view_named("control_flags")
            .expect("canonical flags register");
        encoded.implicit_unit_uses = register_model::sorted_units(flags.units.iter().copied());
        encoded.implicit_unit_defs = encoded.implicit_unit_uses.clone();
    }
    encoded
}

pub fn declaration(
    operation: Operation,
    constraint: RegisterConstraintKey,
) -> MachineEffectDeclaration {
    if matches!(operation, Operation::MemoryFence(_)) {
        return super::memory_fence::declaration(constraint);
    }
    MachineEffectDeclaration::single(
        semantic(operation).expect("x86 register operation"),
        constraint,
        if matches!(
            operation,
            Operation::SystemRead(SystemRegister::Rflags)
                | Operation::SystemWrite(SystemRegister::Rflags)
        ) {
            MachineMemoryEffect::TemporaryStackStorageV1
        } else if matches!(operation, Operation::SegmentReload(_)) {
            MachineMemoryEffect::SegmentDescriptorReloadV1
        } else if operation == Operation::TaskRegisterActivate {
            MachineMemoryEffect::TaskRegisterActivationV1
        } else if operation == Operation::GlobalDescriptorTable(DescriptorOperation::Load) {
            MachineMemoryEffect::ReadPointerV1
        } else if operation == Operation::GlobalDescriptorTable(DescriptorOperation::Store) {
            MachineMemoryEffect::WritePointerV1
        } else {
            MachineMemoryEffect::NoneV1
        },
        MachineTrapBehavior::MayArchitecturalFaultV1,
        MachineBarrier::ExternalEffect,
        family(operation).expect("x86 register family"),
        if matches!(
            operation,
            Operation::ProcessorHalt | Operation::InterruptControl(_)
        ) {
            MachineSizeKnowledge::ExactBytes(1)
        } else {
            crate::encoder_resolved(
                1,
                Some(
                    if matches!(
                        operation,
                        Operation::TaskRegisterActivate | Operation::SegmentReload(_)
                    ) {
                        512
                    } else {
                        40
                    },
                ),
            )
        },
        effects(operation),
    )
}
