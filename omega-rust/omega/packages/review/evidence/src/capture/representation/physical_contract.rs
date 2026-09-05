//! Translation of retained physical representation contracts into review evidence.

use crate::record::{
    PackageReviewBoundaryCallingPolicy, PackageReviewBoundaryShape,
    PackageReviewBoundaryShapeClass, PackageReviewBoundaryShapeField,
    PackageReviewBoundaryShapeGraph, PackageReviewBoundaryValueClass,
    PackageReviewBoundaryValueLocation, PackageReviewBoundaryValuePlacement,
    PackageReviewBoundaryValueShape, PackageReviewIndirectPointerLocation,
    PackageReviewMachineRegister, PackageReviewOpaqueRepresentationApplicationOrigin,
    PackageReviewOpaqueRepresentationCopyDisposition,
    PackageReviewOpaqueRepresentationLifecycleDisposition, PackageReviewRepresentationArchitecture,
    PackageReviewRepresentationObjectFormat, PackageReviewRepresentationTarget,
    PackageReviewRepresentationTargetProfile, PackageReviewSystemVEightbyteClass,
};
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;

pub(crate) fn project_representation_origin(
    origin: omega_representation_planning::OpaqueRepresentationApplicationOrigin,
) -> PackageReviewOpaqueRepresentationApplicationOrigin {
    match origin {
        omega_representation_planning::OpaqueRepresentationApplicationOrigin::NamedConformance => {
            PackageReviewOpaqueRepresentationApplicationOrigin::NamedConformance
        }
    }
}

pub(crate) fn project_representation_lifecycle(
    lifecycle: omega_representation_planning::OpaqueRepresentationLifecycleDisposition,
) -> PackageReviewOpaqueRepresentationLifecycleDisposition {
    match lifecycle {
        omega_representation_planning::OpaqueRepresentationLifecycleDisposition::Inert => {
            PackageReviewOpaqueRepresentationLifecycleDisposition::Inert
        }
    }
}

pub(crate) fn project_representation_copy_disposition(
    disposition: omega_representation_planning::OpaqueRepresentationCopyDisposition,
) -> PackageReviewOpaqueRepresentationCopyDisposition {
    match disposition {
        omega_representation_planning::OpaqueRepresentationCopyDisposition::PlacementOnly => {
            PackageReviewOpaqueRepresentationCopyDisposition::PlacementOnly
        }
        omega_representation_planning::OpaqueRepresentationCopyDisposition::CheckedSemanticCopy => {
            PackageReviewOpaqueRepresentationCopyDisposition::CheckedSemanticCopy
        }
    }
}

pub(crate) fn project_representation_target(
    compilation: &CheckedCompilation,
) -> Result<PackageReviewRepresentationTarget, Vec<Diagnostic>> {
    let profile = compilation.selected_target_profile().ok_or_else(|| {
        vec![Diagnostic::error(
            "representation demand requires a selected target profile",
        )]
    })?;
    let native = compilation.selected_native_target().ok_or_else(|| {
        vec![Diagnostic::error(
            "representation demand requires a selected native target",
        )]
    })?;
    if profile.native_target() != native {
        return Err(vec![Diagnostic::error(
            "representation demand target profile disagrees with its native target",
        )]);
    }
    Ok(PackageReviewRepresentationTarget {
        profile: match profile {
            omega_target::TargetProfile::LinuxArm64 => {
                PackageReviewRepresentationTargetProfile::LinuxArm64
            }
            omega_target::TargetProfile::LinuxX64 => {
                PackageReviewRepresentationTargetProfile::LinuxX64
            }
            omega_target::TargetProfile::MacosArm64 => {
                PackageReviewRepresentationTargetProfile::MacosArm64
            }
            omega_target::TargetProfile::WindowsX64 => {
                PackageReviewRepresentationTargetProfile::WindowsX64
            }
            omega_target::TargetProfile::UefiX64 => {
                PackageReviewRepresentationTargetProfile::UefiX64
            }
            omega_target::TargetProfile::CrossPlatformCli => {
                PackageReviewRepresentationTargetProfile::CrossPlatformCli
            }
            omega_target::TargetProfile::LocalUnchecked => {
                PackageReviewRepresentationTargetProfile::LocalUnchecked
            }
        },
        architecture: match native.architecture {
            omega_target::Architecture::Aarch64 => PackageReviewRepresentationArchitecture::Aarch64,
            omega_target::Architecture::X86_64 => PackageReviewRepresentationArchitecture::X86_64,
        },
        object_format: match native.object_format {
            omega_target::ObjectFormat::Elf => PackageReviewRepresentationObjectFormat::Elf,
            omega_target::ObjectFormat::MachO => PackageReviewRepresentationObjectFormat::MachO,
            omega_target::ObjectFormat::Coff => PackageReviewRepresentationObjectFormat::Coff,
        },
        pointer_size: u16::try_from(native.pointer_size).map_err(|_| {
            vec![Diagnostic::error(
                "representation demand pointer size exceeds canonical evidence",
            )]
        })?,
        pointer_alignment: u16::try_from(native.pointer_alignment).map_err(|_| {
            vec![Diagnostic::error(
                "representation demand pointer alignment exceeds canonical evidence",
            )]
        })?,
    })
}

pub(crate) fn project_boundary_shape_graph(
    signature: &omega_provider_planning::calling_policy_plans::MaterializedBoundarySignature,
) -> PackageReviewBoundaryShapeGraph {
    PackageReviewBoundaryShapeGraph {
        shapes: signature
            .shapes()
            .iter()
            .map(|shape| PackageReviewBoundaryShape {
                class: match shape.class() {
                    omega_provider_planning::calling_policy_plans::BoundaryValueClass::Integer => {
                        PackageReviewBoundaryShapeClass::Integer
                    }
                    omega_provider_planning::calling_policy_plans::BoundaryValueClass::Float => {
                        PackageReviewBoundaryShapeClass::Float
                    }
                    omega_provider_planning::calling_policy_plans::BoundaryValueClass::Reference => {
                        PackageReviewBoundaryShapeClass::Reference
                    }
                    omega_provider_planning::calling_policy_plans::BoundaryValueClass::FixedArray {
                        element,
                        length,
                    } => PackageReviewBoundaryShapeClass::FixedArray { element, length },
                    omega_provider_planning::calling_policy_plans::BoundaryValueClass::Record {
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

pub(crate) fn project_calling_policy(
    policy: omega_calling_conventions::CallingPolicy,
) -> PackageReviewBoundaryCallingPolicy {
    match policy {
        omega_calling_conventions::CallingPolicy::MicrosoftX64 => {
            PackageReviewBoundaryCallingPolicy::MicrosoftX64
        }
        omega_calling_conventions::CallingPolicy::SystemVAMD64 => {
            PackageReviewBoundaryCallingPolicy::SystemVAMD64
        }
        omega_calling_conventions::CallingPolicy::Aapcs64 => {
            PackageReviewBoundaryCallingPolicy::Aapcs64
        }
        omega_calling_conventions::CallingPolicy::LinuxSyscallX86_64 => {
            PackageReviewBoundaryCallingPolicy::LinuxSyscallX86_64
        }
        omega_calling_conventions::CallingPolicy::LinuxSyscallAarch64 => {
            PackageReviewBoundaryCallingPolicy::LinuxSyscallAarch64
        }
    }
}

pub(crate) fn project_value_placement(
    placement: &omega_calling_conventions::ValuePlacement,
) -> PackageReviewBoundaryValuePlacement {
    PackageReviewBoundaryValuePlacement {
        shape: PackageReviewBoundaryValueShape {
            class: match placement.shape.class {
                omega_calling_conventions::ValueClass::Integer => {
                    PackageReviewBoundaryValueClass::Integer
                }
                omega_calling_conventions::ValueClass::Float => {
                    PackageReviewBoundaryValueClass::Float
                }
                omega_calling_conventions::ValueClass::BorrowedReference => {
                    PackageReviewBoundaryValueClass::BorrowedReference
                }
                omega_calling_conventions::ValueClass::HomogeneousFloatAggregate { members } => {
                    PackageReviewBoundaryValueClass::HomogeneousFloatAggregate { members }
                }
                omega_calling_conventions::ValueClass::SystemVAggregate { first, second } => {
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
                omega_calling_conventions::ValueLocation::Register {
                    register,
                    value_byte_offset,
                    byte_size,
                } => PackageReviewBoundaryValueLocation::Register {
                    register: project_machine_register(register),
                    value_byte_offset,
                    byte_size,
                },
                omega_calling_conventions::ValueLocation::Stack {
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
                omega_calling_conventions::ValueLocation::Indirect {
                    pointer,
                    copy_stack_byte_offset,
                    byte_size,
                    alignment,
                } => PackageReviewBoundaryValueLocation::Indirect {
                    pointer: match pointer {
                        omega_calling_conventions::IndirectPointerLocation::Register(register) => {
                            PackageReviewIndirectPointerLocation::Register(
                                project_machine_register(register),
                            )
                        }
                        omega_calling_conventions::IndirectPointerLocation::Stack {
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

fn project_system_v_class(
    class: omega_calling_conventions::SystemVEightbyteClass,
) -> PackageReviewSystemVEightbyteClass {
    match class {
        omega_calling_conventions::SystemVEightbyteClass::Integer => {
            PackageReviewSystemVEightbyteClass::Integer
        }
        omega_calling_conventions::SystemVEightbyteClass::Sse => {
            PackageReviewSystemVEightbyteClass::Sse
        }
    }
}

pub(crate) fn project_machine_register(
    register: omega_calling_conventions::MachineRegister,
) -> PackageReviewMachineRegister {
    match register {
        omega_calling_conventions::MachineRegister::X86Rax => PackageReviewMachineRegister::X86Rax,
        omega_calling_conventions::MachineRegister::X86Rcx => PackageReviewMachineRegister::X86Rcx,
        omega_calling_conventions::MachineRegister::X86Rdx => PackageReviewMachineRegister::X86Rdx,
        omega_calling_conventions::MachineRegister::X86Rbx => PackageReviewMachineRegister::X86Rbx,
        omega_calling_conventions::MachineRegister::X86Rsp => PackageReviewMachineRegister::X86Rsp,
        omega_calling_conventions::MachineRegister::X86Rbp => PackageReviewMachineRegister::X86Rbp,
        omega_calling_conventions::MachineRegister::X86Rsi => PackageReviewMachineRegister::X86Rsi,
        omega_calling_conventions::MachineRegister::X86Rdi => PackageReviewMachineRegister::X86Rdi,
        omega_calling_conventions::MachineRegister::X86R8 => PackageReviewMachineRegister::X86R8,
        omega_calling_conventions::MachineRegister::X86R9 => PackageReviewMachineRegister::X86R9,
        omega_calling_conventions::MachineRegister::X86R10 => PackageReviewMachineRegister::X86R10,
        omega_calling_conventions::MachineRegister::X86R11 => PackageReviewMachineRegister::X86R11,
        omega_calling_conventions::MachineRegister::X86R12 => PackageReviewMachineRegister::X86R12,
        omega_calling_conventions::MachineRegister::X86R13 => PackageReviewMachineRegister::X86R13,
        omega_calling_conventions::MachineRegister::X86R14 => PackageReviewMachineRegister::X86R14,
        omega_calling_conventions::MachineRegister::X86R15 => PackageReviewMachineRegister::X86R15,
        omega_calling_conventions::MachineRegister::X86Xmm(index) => {
            PackageReviewMachineRegister::X86Xmm(index)
        }
        omega_calling_conventions::MachineRegister::Aarch64X(index) => {
            PackageReviewMachineRegister::Aarch64X(index)
        }
        omega_calling_conventions::MachineRegister::Aarch64V(index) => {
            PackageReviewMachineRegister::Aarch64V(index)
        }
    }
}
