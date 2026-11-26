// Shared semantic dependency, authority, target geometry, placement, and machine-register encoding.

use crate::encoding::PackageReviewEncodingError;
use crate::encoding::encode::encoder::Encoder;
use crate::encoding::encode::values::identity::{encode_nominal, encode_service};
use crate::record::{
    PackageReviewBoundaryCallingPolicy, PackageReviewBoundaryShapeClass,
    PackageReviewBoundaryShapeGraph, PackageReviewBoundaryValueClass,
    PackageReviewBoundaryValueLocation, PackageReviewBoundaryValuePlacement,
    PackageReviewDangerousAuthority, PackageReviewDangerousAuthorityClass,
    PackageReviewDangerousAuthoritySlack, PackageReviewIndirectPointerLocation,
    PackageReviewMachineRegister, PackageReviewOpaqueRepresentationMovementRole,
    PackageReviewOpaqueRepresentationOccurrence, PackageReviewOpaqueRepresentationPathElement,
    PackageReviewRepresentationArchitecture, PackageReviewRepresentationObjectFormat,
    PackageReviewRepresentationTarget, PackageReviewRepresentationTargetProfile,
    PackageReviewSemanticDependencyKind, PackageReviewSystemVEightbyteClass,
};

// Canonical encoding for semantic dependency tags, dangerous authority classes, and slack rows.

pub(crate) const fn semantic_dependency_kind_tag(kind: PackageReviewSemanticDependencyKind) -> u8 {
    match kind {
        PackageReviewSemanticDependencyKind::NominalIdentity => 0,
        PackageReviewSemanticDependencyKind::Layout => 1,
        PackageReviewSemanticDependencyKind::OwnershipBehavior => 2,
        PackageReviewSemanticDependencyKind::AutomaticCleanup => 3,
        PackageReviewSemanticDependencyKind::AutomaticCleanupMachine => 4,
    }
}

pub(in crate::encoding::encode::declarations::dependency_trust) fn encode_authority_class(
    encoder: &mut Encoder,
    class: PackageReviewDangerousAuthorityClass,
) {
    match class {
        PackageReviewDangerousAuthorityClass::Filesystem => encoder.tag("filesystem", 0),
        PackageReviewDangerousAuthorityClass::MachineControl => encoder.tag("machine_control", 1),
        PackageReviewDangerousAuthorityClass::PortIo => encoder.tag("port_io", 2),
        PackageReviewDangerousAuthorityClass::InterruptControl => {
            encoder.tag("interrupt_control", 3)
        }
        PackageReviewDangerousAuthorityClass::InterruptEntry => encoder.tag("interrupt_entry", 4),
        PackageReviewDangerousAuthorityClass::RootMemory => encoder.tag("root_memory", 5),
        PackageReviewDangerousAuthorityClass::Process => encoder.tag("process", 6),
        PackageReviewDangerousAuthorityClass::ConsumerAudit => encoder.tag("consumer_audit", 7),
        PackageReviewDangerousAuthorityClass::Network => encoder.tag("network", 8),
    }
}

pub(crate) fn encode_dangerous_authority(
    encoder: &mut Encoder,
    authority: &PackageReviewDangerousAuthority,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field_by("class", authority.class, encode_authority_class)?;
    encoder.field_by("service", &authority.service, encode_service)
}

pub(crate) fn encode_dangerous_authority_slack(
    encoder: &mut Encoder,
    slack: &PackageReviewDangerousAuthoritySlack,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field_by("class", slack.class, encode_authority_class)?;
    encoder.field_by("callable", &slack.callable, encode_nominal)?;
    encoder.field("service", |encoder| encode_service(encoder, &slack.service))
}

// Canonical encoding for representation target profiles, boundary shape graphs, and opaque occurrences.

pub(crate) fn encode_representation_target(
    encoder: &mut Encoder,
    target: PackageReviewRepresentationTarget,
) {
    let _ = encoder.field("profile", |encoder| {
        match target.profile() {
            PackageReviewRepresentationTargetProfile::LinuxArm64 => encoder.tag("linux_arm64", 0),
            PackageReviewRepresentationTargetProfile::LinuxX64 => encoder.tag("linux_x86_64", 1),
            PackageReviewRepresentationTargetProfile::MacosArm64 => encoder.tag("macos_arm64", 2),
            PackageReviewRepresentationTargetProfile::MacosX64 => encoder.tag("macos_x86_64", 7),
            PackageReviewRepresentationTargetProfile::WindowsX64 => {
                encoder.tag("windows_x86_64", 3)
            }
            PackageReviewRepresentationTargetProfile::WindowsArm64 => {
                encoder.tag("windows_arm64", 8)
            }
            PackageReviewRepresentationTargetProfile::UefiX64 => encoder.tag("uefi_x86_64", 4),
            PackageReviewRepresentationTargetProfile::CrossPlatformCli => {
                encoder.tag("cross_platform_cli", 5)
            }
            PackageReviewRepresentationTargetProfile::LocalUnchecked => {
                encoder.tag("local_unchecked", 6)
            }
        };
        Ok(())
    });
    let _ = encoder.field("architecture", |encoder| {
        match target.architecture() {
            PackageReviewRepresentationArchitecture::Aarch64 => encoder.tag("aarch64", 0),
            PackageReviewRepresentationArchitecture::X86_64 => encoder.tag("x86_64", 1),
        };
        Ok(())
    });
    let _ = encoder.field("object_format", |encoder| {
        match target.object_format() {
            PackageReviewRepresentationObjectFormat::Elf => encoder.tag("elf", 0),
            PackageReviewRepresentationObjectFormat::MachO => encoder.tag("mach_o", 1),
            PackageReviewRepresentationObjectFormat::Coff => encoder.tag("coff", 2),
        };
        Ok(())
    });
    let _ = encoder.u16_field("pointer_size", target.pointer_size());
    let _ = encoder.u16_field("pointer_alignment", target.pointer_alignment());
}

pub(crate) fn encode_boundary_shape_graph(
    encoder: &mut Encoder,
    graph: &PackageReviewBoundaryShapeGraph,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("shapes", |encoder| {
        encoder.sequence(graph.shapes(), |encoder, shape| {
            encoder.field("class", |encoder| {
                match shape.class() {
                    PackageReviewBoundaryShapeClass::Integer => encoder.tag("integer", 0),
                    PackageReviewBoundaryShapeClass::Float => encoder.tag("float", 1),
                    PackageReviewBoundaryShapeClass::Reference => encoder.tag("reference", 2),
                    PackageReviewBoundaryShapeClass::FixedArray { element, length } => {
                        encoder.tag("fixed_array", 3);
                        encoder.u16_field("element", element)?;
                        encoder.u16_field("length", length)?;
                    }
                    PackageReviewBoundaryShapeClass::Record {
                        first_field,
                        field_count,
                    } => {
                        encoder.tag("record", 4);
                        encoder.u16_field("first_field", first_field)?;
                        encoder.u16_field("field_count", field_count)?;
                    }
                };
                Ok(())
            })?;
            encoder.u16_field("byte_size", shape.byte_size())?;
            encoder.u16_field("alignment", shape.alignment())?;
            Ok(())
        })
    })?;
    encoder.field("fields", |encoder| {
        encoder.sequence(graph.fields(), |encoder, field| {
            encoder.u16_field("shape", field.shape())?;
            encoder.u16_field("byte_offset", field.byte_offset())?;
            Ok(())
        })
    })?;
    encoder.field("parameters", |encoder| {
        encoder.sequence(graph.parameters(), |encoder, root| {
            encoder.u16_field("root", *root)?;
            Ok(())
        })
    })?;
    encoder.field("result", |encoder| {
        encoder.option(graph.result().as_ref(), |encoder, root| {
            encoder.u16_field("root", *root)?;
            Ok(())
        })
    })
}

pub(crate) fn encode_opaque_occurrence(
    encoder: &mut Encoder,
    occurrence: &PackageReviewOpaqueRepresentationOccurrence,
) -> Result<(), PackageReviewEncodingError> {
    encoder.u16_field("carrier_shape_root", occurrence.carrier_shape_root())?;
    encoder.field("role", |encoder| {
        match occurrence.role() {
            PackageReviewOpaqueRepresentationMovementRole::Parameter {
                formal_ordinal,
                native_ordinal,
            } => {
                encoder.tag("parameter", 0);
                encoder.u32_field("formal_ordinal", formal_ordinal)?;
                encoder.u32_field("native_ordinal", native_ordinal)?;
            }
            PackageReviewOpaqueRepresentationMovementRole::Result => encoder.tag("result", 1),
        };
        Ok(())
    })?;
    encoder.field("path", |encoder| {
        encoder.sequence(occurrence.path(), |encoder, element| {
            encoder.field("element", |encoder| {
                match element {
                    PackageReviewOpaqueRepresentationPathElement::FixedArrayElement => {
                        encoder.tag("fixed_array_element", 0)
                    }
                    PackageReviewOpaqueRepresentationPathElement::RecordField { ordinal } => {
                        encoder.tag("record_field", 1);
                        encoder.u16_field("ordinal", *ordinal)?;
                    }
                };
                Ok(())
            })?;
            Ok(())
        })
    })?;
    encoder.field_by("placement", occurrence.placement(), encode_value_placement)
}

// Canonical encoding for boundary value placements, register/stack/indirect locations, and System V classes.

pub(crate) fn encode_value_placement(
    encoder: &mut Encoder,
    placement: &PackageReviewBoundaryValuePlacement,
) -> Result<(), PackageReviewEncodingError> {
    let shape = placement.shape();
    encoder.field("class", |encoder| {
        match shape.class() {
            PackageReviewBoundaryValueClass::Integer => encoder.tag("integer", 0),
            PackageReviewBoundaryValueClass::IntegerAggregate => {
                encoder.tag("integer_aggregate", 6)
            }
            PackageReviewBoundaryValueClass::Float => encoder.tag("float", 1),
            PackageReviewBoundaryValueClass::BorrowedReference => {
                encoder.tag("borrowed_reference", 4)
            }
            PackageReviewBoundaryValueClass::BorrowedScalarReference => {
                encoder.tag("borrowed_scalar_reference", 5)
            }
            PackageReviewBoundaryValueClass::HomogeneousFloatAggregate { members } => {
                encoder.tag("homogeneous_float_aggregate", 2);
                encoder.byte_field("members", members)?;
            }
            PackageReviewBoundaryValueClass::SystemVAggregate { first, second } => {
                encoder.tag("system_v_aggregate", 3);
                encoder.field_by("first", first, encode_system_v_class)?;
                encoder.field_by("second", second, encode_system_v_class)?;
            }
        };
        Ok(())
    })?;
    encoder.u32_field("byte_size", shape.byte_size())?;
    encoder.u16_field("alignment", shape.alignment())?;
    encoder.sequence_field("locations", placement.locations(), encode_value_location)
}

pub(in crate::encoding::encode::declarations::dependency_trust) fn encode_value_location(
    encoder: &mut Encoder,
    location: &PackageReviewBoundaryValueLocation,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("location", |encoder| {
        match *location {
            PackageReviewBoundaryValueLocation::Register {
                register,
                value_byte_offset,
                byte_size,
            } => {
                encoder.tag("register", 0);
                encoder.field_by("register", register, encode_machine_register)?;
                encoder.u32_field("value_byte_offset", value_byte_offset)?;
                encoder.u32_field("byte_size", byte_size)?;
            }
            PackageReviewBoundaryValueLocation::Stack {
                stack_byte_offset,
                value_byte_offset,
                byte_size,
                alignment,
            } => {
                encoder.tag("stack", 1);
                encoder.u32_field("stack_byte_offset", stack_byte_offset)?;
                encoder.u32_field("value_byte_offset", value_byte_offset)?;
                encoder.u32_field("byte_size", byte_size)?;
                encoder.u16_field("alignment", alignment)?;
            }
            PackageReviewBoundaryValueLocation::Indirect {
                pointer,
                copy_stack_byte_offset,
                byte_size,
                alignment,
            } => {
                encoder.tag("indirect", 2);
                encoder.field("pointer", |encoder| {
                    match pointer {
                        PackageReviewIndirectPointerLocation::Register(register) => {
                            encoder.tag("register", 0);
                            encoder.field_by("register", register, encode_machine_register)?;
                        }
                        PackageReviewIndirectPointerLocation::Stack {
                            stack_byte_offset,
                            alignment,
                        } => {
                            encoder.tag("stack", 1);
                            encoder.u32_field("stack_byte_offset", stack_byte_offset)?;
                            encoder.u16_field("alignment", alignment)?;
                        }
                    };
                    Ok(())
                })?;
                encoder.field("copy_stack_byte_offset", |encoder| {
                    encoder.option(copy_stack_byte_offset.as_ref(), |encoder, offset| {
                        encoder.u32_field("offset", *offset)?;
                        Ok(())
                    })
                })?;
                encoder.u32_field("byte_size", byte_size)?;
                encoder.u16_field("alignment", alignment)?;
            }
        };
        Ok(())
    })?;
    Ok(())
}

pub(in crate::encoding::encode::declarations::dependency_trust) fn encode_system_v_class(
    encoder: &mut Encoder,
    class: PackageReviewSystemVEightbyteClass,
) {
    match class {
        PackageReviewSystemVEightbyteClass::Integer => encoder.tag("integer", 0),
        PackageReviewSystemVEightbyteClass::Sse => encoder.tag("sse", 1),
    }
}

// Canonical encoding for machine registers and boundary calling-policy tags.

pub(crate) fn encode_machine_register(
    encoder: &mut Encoder,
    register: PackageReviewMachineRegister,
) {
    match register {
        PackageReviewMachineRegister::X86Rax => encoder.tag("x86_rax", 0),
        PackageReviewMachineRegister::X86Rcx => encoder.tag("x86_rcx", 1),
        PackageReviewMachineRegister::X86Rdx => encoder.tag("x86_rdx", 2),
        PackageReviewMachineRegister::X86Rbx => encoder.tag("x86_rbx", 3),
        PackageReviewMachineRegister::X86Rsp => encoder.tag("x86_rsp", 4),
        PackageReviewMachineRegister::X86Rbp => encoder.tag("x86_rbp", 5),
        PackageReviewMachineRegister::X86Rsi => encoder.tag("x86_rsi", 6),
        PackageReviewMachineRegister::X86Rdi => encoder.tag("x86_rdi", 7),
        PackageReviewMachineRegister::X86R8 => encoder.tag("x86_r8", 8),
        PackageReviewMachineRegister::X86R9 => encoder.tag("x86_r9", 9),
        PackageReviewMachineRegister::X86R10 => encoder.tag("x86_r10", 10),
        PackageReviewMachineRegister::X86R11 => encoder.tag("x86_r11", 11),
        PackageReviewMachineRegister::X86R12 => encoder.tag("x86_r12", 12),
        PackageReviewMachineRegister::X86R13 => encoder.tag("x86_r13", 13),
        PackageReviewMachineRegister::X86R14 => encoder.tag("x86_r14", 14),
        PackageReviewMachineRegister::X86R15 => encoder.tag("x86_r15", 15),
        PackageReviewMachineRegister::X86Xmm(index) => {
            encoder.tag("x86_xmm", 16);
            let _ = encoder.byte_field("index", index);
        }
        PackageReviewMachineRegister::Aarch64X(index) => {
            encoder.tag("aarch64_x", 17);
            let _ = encoder.byte_field("index", index);
        }
        PackageReviewMachineRegister::Aarch64V(index) => {
            encoder.tag("aarch64_v", 18);
            let _ = encoder.byte_field("index", index);
        }
    }
}

pub(crate) const fn calling_policy_tag(policy: PackageReviewBoundaryCallingPolicy) -> u8 {
    match policy {
        PackageReviewBoundaryCallingPolicy::MicrosoftX64 => 0,
        PackageReviewBoundaryCallingPolicy::SystemVAMD64 => 1,
        PackageReviewBoundaryCallingPolicy::Aapcs64 => 2,
        PackageReviewBoundaryCallingPolicy::LinuxSyscallX86_64 => 3,
        PackageReviewBoundaryCallingPolicy::LinuxSyscallAarch64 => 4,
        PackageReviewBoundaryCallingPolicy::DarwinAapcs64 => 5,
    }
}
