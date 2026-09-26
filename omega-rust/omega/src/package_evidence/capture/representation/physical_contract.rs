//! Translation of retained physical representation contracts into review evidence.

use crate::package_evidence::capture::PackageReviewInput;
use crate::package_evidence::record::{
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
use diagnostics::Diagnostic;

pub(crate) fn project_representation_origin(
    origin: crate::representation_planning::OpaqueRepresentationApplicationOrigin,
) -> PackageReviewOpaqueRepresentationApplicationOrigin {
    match origin {
        crate::representation_planning::OpaqueRepresentationApplicationOrigin::NamedConformance => {
            PackageReviewOpaqueRepresentationApplicationOrigin::NamedConformance
        }
    }
}

pub(crate) fn project_representation_lifecycle(
    lifecycle: crate::representation_planning::OpaqueRepresentationLifecycleDisposition,
) -> PackageReviewOpaqueRepresentationLifecycleDisposition {
    match lifecycle {
        crate::representation_planning::OpaqueRepresentationLifecycleDisposition::Inert => {
            PackageReviewOpaqueRepresentationLifecycleDisposition::Inert
        }
    }
}

pub(crate) fn project_representation_copy_disposition(
    disposition: crate::representation_planning::OpaqueRepresentationCopyDisposition,
) -> PackageReviewOpaqueRepresentationCopyDisposition {
    match disposition {
        crate::representation_planning::OpaqueRepresentationCopyDisposition::PlacementOnly => {
            PackageReviewOpaqueRepresentationCopyDisposition::PlacementOnly
        }
        crate::representation_planning::OpaqueRepresentationCopyDisposition::CheckedSemanticCopy => {
            PackageReviewOpaqueRepresentationCopyDisposition::CheckedSemanticCopy
        }
    }
}

pub(crate) fn project_representation_target(
    compilation: &PackageReviewInput<'_>,
) -> Result<PackageReviewRepresentationTarget, Vec<Diagnostic>> {
    let profile = compilation
        .custody
        .selected_target_profile()
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "representation demand requires a selected target profile",
            )]
        })?;
    let native = compilation
        .custody
        .selected_native_target()
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "representation demand requires a selected native target",
            )]
        })?;
    // A recognized profile can lack a Rust native realization (the
    // bootstrap Alpha target); such a custody is rejected rather than
    // interrogating `native_target()`.
    let Some(profile_native) = profile.native_realization() else {
        return Err(vec![Diagnostic::error(format!(
            "native realization for target profile `{}` is not implemented",
            profile.target_name()
        ))]);
    };
    if profile_native != native {
        return Err(vec![Diagnostic::error(
            "representation demand target profile disagrees with its native target",
        )]);
    }
    Ok(PackageReviewRepresentationTarget {
        profile: match profile {
            target::TargetProfile::LinuxArm64 => {
                PackageReviewRepresentationTargetProfile::LinuxArm64
            }
            target::TargetProfile::LinuxX64 => PackageReviewRepresentationTargetProfile::LinuxX64,
            target::TargetProfile::MacosArm64 => {
                PackageReviewRepresentationTargetProfile::MacosArm64
            }
            target::TargetProfile::MacosX64 => PackageReviewRepresentationTargetProfile::MacosX64,
            target::TargetProfile::WindowsX64 => {
                PackageReviewRepresentationTargetProfile::WindowsX64
            }
            target::TargetProfile::UefiX64 => PackageReviewRepresentationTargetProfile::UefiX64,
            target::TargetProfile::CrossPlatformCli => {
                PackageReviewRepresentationTargetProfile::CrossPlatformCli
            }
            target::TargetProfile::LocalUnchecked => {
                PackageReviewRepresentationTargetProfile::LocalUnchecked
            }
            target::TargetProfile::AlphaBootstrap => {
                unreachable!("alpha_bootstrap is rejected before realization capture")
            }
        },
        architecture: match native.architecture {
            target::Architecture::Aarch64 => PackageReviewRepresentationArchitecture::Aarch64,
            target::Architecture::X86_64 => PackageReviewRepresentationArchitecture::X86_64,
        },
        object_format: match native.object_format {
            target::ObjectFormat::Elf => PackageReviewRepresentationObjectFormat::Elf,
            target::ObjectFormat::MachO => PackageReviewRepresentationObjectFormat::MachO,
            target::ObjectFormat::Coff => PackageReviewRepresentationObjectFormat::Coff,
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
    signature: &crate::provider_planning::calling_policy_plans::MaterializedBoundarySignature,
) -> PackageReviewBoundaryShapeGraph {
    PackageReviewBoundaryShapeGraph {
        shapes: signature
            .shapes()
            .iter()
            .map(|shape| PackageReviewBoundaryShape {
                class: match shape.class() {
                    crate::provider_planning::calling_policy_plans::BoundaryValueClass::Integer => {
                        PackageReviewBoundaryShapeClass::Integer
                    }
                    crate::provider_planning::calling_policy_plans::BoundaryValueClass::Float => {
                        PackageReviewBoundaryShapeClass::Float
                    }
                    crate::provider_planning::calling_policy_plans::BoundaryValueClass::Reference => {
                        PackageReviewBoundaryShapeClass::Reference
                    }
                    crate::provider_planning::calling_policy_plans::BoundaryValueClass::FixedArray {
                        element,
                        length,
                    } => PackageReviewBoundaryShapeClass::FixedArray { element, length },
                    crate::provider_planning::calling_policy_plans::BoundaryValueClass::Record {
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
    policy: abstract_operations_to_target_operations::calling_conventions::CallingPolicy,
) -> PackageReviewBoundaryCallingPolicy {
    match policy {
        abstract_operations_to_target_operations::calling_conventions::CallingPolicy::MicrosoftX64 => {
            PackageReviewBoundaryCallingPolicy::MicrosoftX64
        }
        abstract_operations_to_target_operations::calling_conventions::CallingPolicy::SystemVAMD64 => {
            PackageReviewBoundaryCallingPolicy::SystemVAMD64
        }
        abstract_operations_to_target_operations::calling_conventions::CallingPolicy::Aapcs64 => PackageReviewBoundaryCallingPolicy::Aapcs64,
        abstract_operations_to_target_operations::calling_conventions::CallingPolicy::LinuxSyscallX86_64 => {
            PackageReviewBoundaryCallingPolicy::LinuxSyscallX86_64
        }
        abstract_operations_to_target_operations::calling_conventions::CallingPolicy::LinuxSyscallAarch64 => {
            PackageReviewBoundaryCallingPolicy::LinuxSyscallAarch64
        }
    }
}

pub(crate) fn project_value_placement(
    placement: &abstract_operations_to_target_operations::calling_conventions::ValuePlacement,
) -> PackageReviewBoundaryValuePlacement {
    PackageReviewBoundaryValuePlacement {
        shape: PackageReviewBoundaryValueShape {
            class: match placement.shape.class {
                abstract_operations_to_target_operations::calling_conventions::ValueClass::Integer => {
                    PackageReviewBoundaryValueClass::Integer
                }
                abstract_operations_to_target_operations::calling_conventions::ValueClass::Float => PackageReviewBoundaryValueClass::Float,
                abstract_operations_to_target_operations::calling_conventions::ValueClass::BorrowedReference => {
                    PackageReviewBoundaryValueClass::BorrowedReference
                }
                abstract_operations_to_target_operations::calling_conventions::ValueClass::HomogeneousFloatAggregate { members } => {
                    PackageReviewBoundaryValueClass::HomogeneousFloatAggregate { members }
                }
                abstract_operations_to_target_operations::calling_conventions::ValueClass::SystemVAggregate { first, second } => {
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
                abstract_operations_to_target_operations::calling_conventions::ValueLocation::Register {
                    register,
                    value_byte_offset,
                    byte_size,
                } => PackageReviewBoundaryValueLocation::Register {
                    register: project_machine_register(register),
                    value_byte_offset,
                    byte_size,
                },
                abstract_operations_to_target_operations::calling_conventions::ValueLocation::Stack {
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
                abstract_operations_to_target_operations::calling_conventions::ValueLocation::Indirect {
                    pointer,
                    copy_stack_byte_offset,
                    byte_size,
                    alignment,
                } => PackageReviewBoundaryValueLocation::Indirect {
                    pointer: match pointer {
                        abstract_operations_to_target_operations::calling_conventions::IndirectPointerLocation::Register(register) => {
                            PackageReviewIndirectPointerLocation::Register(
                                project_machine_register(register),
                            )
                        }
                        abstract_operations_to_target_operations::calling_conventions::IndirectPointerLocation::Stack {
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
    class: abstract_operations_to_target_operations::calling_conventions::SystemVEightbyteClass,
) -> PackageReviewSystemVEightbyteClass {
    match class {
        abstract_operations_to_target_operations::calling_conventions::SystemVEightbyteClass::Integer => {
            PackageReviewSystemVEightbyteClass::Integer
        }
        abstract_operations_to_target_operations::calling_conventions::SystemVEightbyteClass::Sse => PackageReviewSystemVEightbyteClass::Sse,
    }
}

pub(crate) fn project_machine_register(
    register: abstract_operations_to_target_operations::calling_conventions::MachineRegister,
) -> PackageReviewMachineRegister {
    match register {
        abstract_operations_to_target_operations::calling_conventions::MachineRegister::X86Rax => PackageReviewMachineRegister::X86Rax,
        abstract_operations_to_target_operations::calling_conventions::MachineRegister::X86Rcx => PackageReviewMachineRegister::X86Rcx,
        abstract_operations_to_target_operations::calling_conventions::MachineRegister::X86Rdx => PackageReviewMachineRegister::X86Rdx,
        abstract_operations_to_target_operations::calling_conventions::MachineRegister::X86Rbx => PackageReviewMachineRegister::X86Rbx,
        abstract_operations_to_target_operations::calling_conventions::MachineRegister::X86Rsp => PackageReviewMachineRegister::X86Rsp,
        abstract_operations_to_target_operations::calling_conventions::MachineRegister::X86Rbp => PackageReviewMachineRegister::X86Rbp,
        abstract_operations_to_target_operations::calling_conventions::MachineRegister::X86Rsi => PackageReviewMachineRegister::X86Rsi,
        abstract_operations_to_target_operations::calling_conventions::MachineRegister::X86Rdi => PackageReviewMachineRegister::X86Rdi,
        abstract_operations_to_target_operations::calling_conventions::MachineRegister::X86R8 => PackageReviewMachineRegister::X86R8,
        abstract_operations_to_target_operations::calling_conventions::MachineRegister::X86R9 => PackageReviewMachineRegister::X86R9,
        abstract_operations_to_target_operations::calling_conventions::MachineRegister::X86R10 => PackageReviewMachineRegister::X86R10,
        abstract_operations_to_target_operations::calling_conventions::MachineRegister::X86R11 => PackageReviewMachineRegister::X86R11,
        abstract_operations_to_target_operations::calling_conventions::MachineRegister::X86R12 => PackageReviewMachineRegister::X86R12,
        abstract_operations_to_target_operations::calling_conventions::MachineRegister::X86R13 => PackageReviewMachineRegister::X86R13,
        abstract_operations_to_target_operations::calling_conventions::MachineRegister::X86R14 => PackageReviewMachineRegister::X86R14,
        abstract_operations_to_target_operations::calling_conventions::MachineRegister::X86R15 => PackageReviewMachineRegister::X86R15,
        abstract_operations_to_target_operations::calling_conventions::MachineRegister::X86Xmm(index) => {
            PackageReviewMachineRegister::X86Xmm(index)
        }
        abstract_operations_to_target_operations::calling_conventions::MachineRegister::Aarch64X(index) => {
            PackageReviewMachineRegister::Aarch64X(index)
        }
        abstract_operations_to_target_operations::calling_conventions::MachineRegister::Aarch64V(index) => {
            PackageReviewMachineRegister::Aarch64V(index)
        }
    }
}
