//! Translation of retained physical representation contracts into review evidence.

use crate::capture::EvidenceOption;
use crate::capture::PackageReviewInput;
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
use calling_conventions::CallingPolicy;
use calling_conventions::MachineRegister;
use calling_conventions::SystemVEightbyteClass;
use calling_conventions::ValueClass;
use diagnostics::Diagnostic;
use provider_planning::calling_policy_plans::BoundaryValueClass;
use target::TargetProfile;

pub(crate) fn project_representation_origin(
    origin: representation_planning::OpaqueRepresentationApplicationOrigin,
) -> PackageReviewOpaqueRepresentationApplicationOrigin {
    match origin {
        representation_planning::OpaqueRepresentationApplicationOrigin::NamedConformance => {
            PackageReviewOpaqueRepresentationApplicationOrigin::NamedConformance
        }
    }
}

pub(crate) fn project_representation_lifecycle(
    lifecycle: representation_planning::OpaqueRepresentationLifecycleDisposition,
) -> PackageReviewOpaqueRepresentationLifecycleDisposition {
    match lifecycle {
        representation_planning::OpaqueRepresentationLifecycleDisposition::Inert => {
            PackageReviewOpaqueRepresentationLifecycleDisposition::Inert
        }
    }
}

pub(crate) fn project_representation_copy_disposition(
    disposition: representation_planning::OpaqueRepresentationCopyDisposition,
) -> PackageReviewOpaqueRepresentationCopyDisposition {
    match disposition {
        representation_planning::OpaqueRepresentationCopyDisposition::PlacementOnly => {
            PackageReviewOpaqueRepresentationCopyDisposition::PlacementOnly
        }
        representation_planning::OpaqueRepresentationCopyDisposition::CheckedSemanticCopy => {
            PackageReviewOpaqueRepresentationCopyDisposition::CheckedSemanticCopy
        }
    }
}

pub(crate) fn project_representation_target(
    compilation: &PackageReviewInput<'_>,
) -> Result<PackageReviewRepresentationTarget, Vec<Diagnostic>> {
    let profile = compilation
        .custody
        .execution()
        .selected_target_profile
        .or_evidence("representation demand requires a selected target profile")?;
    let native = compilation
        .custody
        .execution()
        .selected_native_target()
        .or_evidence("representation demand requires a selected native target")?;
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
            TargetProfile::LinuxArm64 => PackageReviewRepresentationTargetProfile::LinuxArm64,
            TargetProfile::LinuxX64 => PackageReviewRepresentationTargetProfile::LinuxX64,
            TargetProfile::MacosArm64 => PackageReviewRepresentationTargetProfile::MacosArm64,
            TargetProfile::MacosX64 => PackageReviewRepresentationTargetProfile::MacosX64,
            TargetProfile::WindowsX64 => PackageReviewRepresentationTargetProfile::WindowsX64,
            TargetProfile::UefiX64 => PackageReviewRepresentationTargetProfile::UefiX64,
            TargetProfile::CrossPlatformCli => {
                PackageReviewRepresentationTargetProfile::CrossPlatformCli
            }
            TargetProfile::LocalUnchecked => {
                PackageReviewRepresentationTargetProfile::LocalUnchecked
            }
            TargetProfile::AlphaBootstrap => {
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
