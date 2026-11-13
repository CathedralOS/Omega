//! Pure projection of admitted ABI geometry into inert review vocabulary.
use crate::record::*;
use calling_conventions::{CallingPolicy, MachineRegister, SystemVEightbyteClass, ValueClass};
use provider_planning::calling_policy_plans::BoundaryValueClass;

pub(crate) fn project_boundary_shape_graph(
    signature: &provider_planning::calling_policy_plans::MaterializedBoundarySignature,
) -> PackageReviewBoundaryShapeGraph {
    PackageReviewBoundaryShapeGraph {
        shapes: signature
            .shapes()
            .iter()
            .map(|shape| PackageReviewBoundaryShape {
                class: match shape.class() {
                    BoundaryValueClass::Integer => PackageReviewBoundaryShapeClass::Integer,
                    BoundaryValueClass::Float => PackageReviewBoundaryShapeClass::Float,
                    BoundaryValueClass::Reference => PackageReviewBoundaryShapeClass::Reference,
                    BoundaryValueClass::FixedArray { element, length } => {
                        PackageReviewBoundaryShapeClass::FixedArray { element, length }
                    }
                    BoundaryValueClass::Record {
                        first_field,
                        field_count,
                    } => PackageReviewBoundaryShapeClass::Record {
                        first_field,
                        field_count,
                    },
                },
                byte_size: shape.byte_size(),
                alignment: shape.alignment(),
            })
            .collect(),
        fields: signature
            .fields()
            .iter()
            .map(|field| PackageReviewBoundaryShapeField {
                shape: field.shape(),
                byte_offset: field.byte_offset(),
            })
            .collect(),
        parameters: signature.parameters().to_vec(),
        result: signature.result(),
    }
}

pub(crate) fn project_calling_policy(policy: CallingPolicy) -> PackageReviewBoundaryCallingPolicy {
    match policy {
        CallingPolicy::MicrosoftX64 => PackageReviewBoundaryCallingPolicy::MicrosoftX64,
        CallingPolicy::SystemVAMD64 => PackageReviewBoundaryCallingPolicy::SystemVAMD64,
        CallingPolicy::Aapcs64 => PackageReviewBoundaryCallingPolicy::Aapcs64,
        CallingPolicy::LinuxSyscallX86_64 => PackageReviewBoundaryCallingPolicy::LinuxSyscallX86_64,
        CallingPolicy::LinuxSyscallAarch64 => {
            PackageReviewBoundaryCallingPolicy::LinuxSyscallAarch64
        }
    }
}

pub(crate) fn project_value_placement(
    placement: &calling_conventions::ValuePlacement,
) -> PackageReviewBoundaryValuePlacement {
    PackageReviewBoundaryValuePlacement {
        shape: PackageReviewBoundaryValueShape {
            class: match placement.shape.class {
                ValueClass::Integer => PackageReviewBoundaryValueClass::Integer,
                ValueClass::Float => PackageReviewBoundaryValueClass::Float,
                ValueClass::BorrowedReference => PackageReviewBoundaryValueClass::BorrowedReference,
                ValueClass::BorrowedScalarReference => {
                    PackageReviewBoundaryValueClass::BorrowedScalarReference
                }
                ValueClass::HomogeneousFloatAggregate { members } => {
                    PackageReviewBoundaryValueClass::HomogeneousFloatAggregate { members }
                }
                ValueClass::SystemVAggregate { first, second } => {
                    PackageReviewBoundaryValueClass::SystemVAggregate {
                        first: project_system_v_class(first),
                        second: project_system_v_class(second),
                    }
                }
            },
            byte_size: placement.shape.byte_size,
            alignment: placement.shape.alignment,
        },
        locations: placement
            .locations
            .iter()
            .map(|location| match *location {
                calling_conventions::ValueLocation::Register {
                    register,
                    value_byte_offset,
                    byte_size,
                } => PackageReviewBoundaryValueLocation::Register {
                    register: project_machine_register(register),
                    value_byte_offset,
                    byte_size,
                },
                calling_conventions::ValueLocation::Stack {
                    stack_byte_offset,
                    value_byte_offset,
                    byte_size,
                    alignment,
                } => PackageReviewBoundaryValueLocation::Stack {
                    stack_byte_offset,
                    value_byte_offset,
                    byte_size,
                    alignment,
                },
                calling_conventions::ValueLocation::Indirect {
                    pointer,
                    copy_stack_byte_offset,
                    byte_size,
                    alignment,
                } => PackageReviewBoundaryValueLocation::Indirect {
                    pointer: match pointer {
                        calling_conventions::IndirectPointerLocation::Register(register) => {
                            PackageReviewIndirectPointerLocation::Register(
                                project_machine_register(register),
                            )
                        }
                        calling_conventions::IndirectPointerLocation::Stack {
                            stack_byte_offset,
                            alignment,
                        } => PackageReviewIndirectPointerLocation::Stack {
                            stack_byte_offset,
                            alignment,
                        },
                    },
                    copy_stack_byte_offset,
                    byte_size,
                    alignment,
                },
            })
            .collect(),
    }
}

fn project_system_v_class(class: SystemVEightbyteClass) -> PackageReviewSystemVEightbyteClass {
    match class {
        SystemVEightbyteClass::Integer => PackageReviewSystemVEightbyteClass::Integer,
        SystemVEightbyteClass::Sse => PackageReviewSystemVEightbyteClass::Sse,
    }
}

pub(crate) fn project_machine_register(register: MachineRegister) -> PackageReviewMachineRegister {
    match register {
        MachineRegister::X86Rax => PackageReviewMachineRegister::X86Rax,
        MachineRegister::X86Rcx => PackageReviewMachineRegister::X86Rcx,
        MachineRegister::X86Rdx => PackageReviewMachineRegister::X86Rdx,
        MachineRegister::X86Rbx => PackageReviewMachineRegister::X86Rbx,
        MachineRegister::X86Rsp => PackageReviewMachineRegister::X86Rsp,
        MachineRegister::X86Rbp => PackageReviewMachineRegister::X86Rbp,
        MachineRegister::X86Rsi => PackageReviewMachineRegister::X86Rsi,
        MachineRegister::X86Rdi => PackageReviewMachineRegister::X86Rdi,
        MachineRegister::X86R8 => PackageReviewMachineRegister::X86R8,
        MachineRegister::X86R9 => PackageReviewMachineRegister::X86R9,
        MachineRegister::X86R10 => PackageReviewMachineRegister::X86R10,
        MachineRegister::X86R11 => PackageReviewMachineRegister::X86R11,
        MachineRegister::X86R12 => PackageReviewMachineRegister::X86R12,
        MachineRegister::X86R13 => PackageReviewMachineRegister::X86R13,
        MachineRegister::X86R14 => PackageReviewMachineRegister::X86R14,
        MachineRegister::X86R15 => PackageReviewMachineRegister::X86R15,
        MachineRegister::X86Xmm(index) => PackageReviewMachineRegister::X86Xmm(index),
        MachineRegister::Aarch64X(index) => PackageReviewMachineRegister::Aarch64X(index),
        MachineRegister::Aarch64V(index) => PackageReviewMachineRegister::Aarch64V(index),
    }
}
