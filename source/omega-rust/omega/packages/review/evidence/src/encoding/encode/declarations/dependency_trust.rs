use super::super::PackageReviewEncodingError;
use super::super::encoder::Encoder;
use super::super::values::identity::encode_nominal;
use crate::record::{
    PackageReviewBoundaryCallingPolicy, PackageReviewBoundaryShapeClass,
    PackageReviewBoundaryShapeGraph, PackageReviewBoundaryValueClass,
    PackageReviewBoundaryValueLocation, PackageReviewBoundaryValuePlacement,
    PackageReviewDangerousAuthority, PackageReviewDangerousAuthorityClass,
    PackageReviewDangerousAuthoritySlack, PackageReviewIndirectPointerLocation,
    PackageReviewMachineRegister, PackageReviewOpaqueRepresentationCopyDisposition,
    PackageReviewOpaqueRepresentationMovementRole, PackageReviewOpaqueRepresentationOccurrence,
    PackageReviewOpaqueRepresentationPathElement, PackageReviewRepresentationArchitecture,
    PackageReviewRepresentationObjectFormat, PackageReviewRepresentationTarget,
    PackageReviewRepresentationTargetProfile, PackageReviewRepresentationTcb,
    PackageReviewRepresentationTcbKind, PackageReviewSemanticDependency,
    PackageReviewSemanticDependencyExposure, PackageReviewSemanticDependencyKind,
    PackageReviewSystemVEightbyteClass,
};

pub(crate) fn encode_semantic_dependency_key(
    encoder: &mut Encoder,
    dependency: &PackageReviewSemanticDependency,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &dependency.consumer)?;
    encode_nominal(encoder, &dependency.dependency)?;
    encoder.byte(semantic_dependency_kind_tag(dependency.kind));
    Ok(())
}

pub(crate) fn encode_semantic_dependency(
    encoder: &mut Encoder,
    dependency: &PackageReviewSemanticDependency,
) -> Result<(), PackageReviewEncodingError> {
    encode_semantic_dependency_key(encoder, dependency)?;
    encoder.byte(match dependency.exposure {
        PackageReviewSemanticDependencyExposure::PrivateImplementation => 0,
        PackageReviewSemanticDependencyExposure::PublicInterface => 1,
    });
    Ok(())
}

pub(crate) const fn semantic_dependency_kind_tag(kind: PackageReviewSemanticDependencyKind) -> u8 {
    match kind {
        PackageReviewSemanticDependencyKind::NominalIdentity => 0,
        PackageReviewSemanticDependencyKind::Layout => 1,
        PackageReviewSemanticDependencyKind::OwnershipBehavior => 2,
        PackageReviewSemanticDependencyKind::AutomaticCleanup => 3,
        PackageReviewSemanticDependencyKind::AutomaticCleanupMachine => 4,
    }
}

pub(crate) fn encode_representation_tcb_key(
    encoder: &mut Encoder,
    row: &PackageReviewRepresentationTcb,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &row.declaration)?;
    match &row.kind {
        PackageReviewRepresentationTcbKind::Unbound => encoder.byte(0),
        PackageReviewRepresentationTcbKind::ProducerAvailability { conformance, .. } => {
            encoder.byte(1);
            encode_nominal(encoder, conformance)?;
        }
        PackageReviewRepresentationTcbKind::SelectedCopyReceipt { .. } => encoder.byte(2),
        PackageReviewRepresentationTcbKind::ConsumerDemand {
            boundary_trait,
            boundary_arguments,
            requirement,
            requirement_identity,
            ..
        } => {
            encoder.byte(3);
            encode_nominal(encoder, boundary_trait)?;
            encoder.sequence(boundary_arguments, super::data::encode_type_identity)?;
            encode_nominal(encoder, requirement)?;
            encoder.string(requirement_identity)?;
        }
    }
    Ok(())
}

pub(crate) fn encode_representation_tcb(
    encoder: &mut Encoder,
    row: &PackageReviewRepresentationTcb,
) -> Result<(), PackageReviewEncodingError> {
    encode_representation_tcb_key(encoder, row)?;
    match &row.kind {
        PackageReviewRepresentationTcbKind::Unbound => {}
        PackageReviewRepresentationTcbKind::ProducerAvailability { carrier, .. } => {
            encode_nominal(encoder, carrier)?;
        }
        PackageReviewRepresentationTcbKind::SelectedCopyReceipt {
            conformance,
            carrier,
            representation_schema_version,
            origin,
            lifecycle,
            copy_disposition,
            conformance_application_commitment,
            selected_application_commitment,
        } => {
            encode_nominal(encoder, conformance)?;
            encode_nominal(encoder, carrier)?;
            encoder.u16(*representation_schema_version);
            encoder.byte(match origin {
                crate::record::PackageReviewOpaqueRepresentationApplicationOrigin::NamedConformance => 1,
            });
            encoder.byte(match lifecycle {
                crate::record::PackageReviewOpaqueRepresentationLifecycleDisposition::Inert => 1,
            });
            encoder.byte(match copy_disposition {
                crate::record::PackageReviewOpaqueRepresentationCopyDisposition::PlacementOnly => 1,
                crate::record::PackageReviewOpaqueRepresentationCopyDisposition::CheckedSemanticCopy => 2,
            });
            encoder.fixed_bytes(conformance_application_commitment);
            encoder.fixed_bytes(selected_application_commitment);
        }
        PackageReviewRepresentationTcbKind::ConsumerDemand {
            target,
            conformance,
            carrier,
            representation_schema_version,
            origin,
            lifecycle,
            copy_disposition,
            shape_graph,
            occurrences,
            calling_policy,
            conformance_application_commitment,
            selected_application_commitment,
            boundary_plan_commitment,
            ..
        } => {
            encode_representation_target(encoder, *target);
            encode_nominal(encoder, conformance)?;
            encode_nominal(encoder, carrier)?;
            encoder.u16(*representation_schema_version);
            encoder.byte(match origin {
                crate::record::PackageReviewOpaqueRepresentationApplicationOrigin::NamedConformance => 1,
            });
            encoder.byte(match lifecycle {
                crate::record::PackageReviewOpaqueRepresentationLifecycleDisposition::Inert => 1,
            });
            encoder.byte(copy_disposition_tag(*copy_disposition));
            encode_boundary_shape_graph(encoder, shape_graph)?;
            encoder.sequence(occurrences, encode_opaque_occurrence)?;
            encoder.byte(calling_policy_tag(*calling_policy));
            encoder.fixed_bytes(conformance_application_commitment);
            encoder.fixed_bytes(selected_application_commitment);
            encoder.fixed_bytes(boundary_plan_commitment);
        }
    }
    Ok(())
}

const fn copy_disposition_tag(disposition: PackageReviewOpaqueRepresentationCopyDisposition) -> u8 {
    match disposition {
        PackageReviewOpaqueRepresentationCopyDisposition::PlacementOnly => 1,
        PackageReviewOpaqueRepresentationCopyDisposition::CheckedSemanticCopy => 2,
    }
}

fn encode_representation_target(encoder: &mut Encoder, target: PackageReviewRepresentationTarget) {
    encoder.byte(match target.profile() {
        PackageReviewRepresentationTargetProfile::LinuxArm64 => 0,
        PackageReviewRepresentationTargetProfile::LinuxX64 => 1,
        PackageReviewRepresentationTargetProfile::MacosArm64 => 2,
        PackageReviewRepresentationTargetProfile::WindowsX64 => 3,
        PackageReviewRepresentationTargetProfile::UefiX64 => 4,
        PackageReviewRepresentationTargetProfile::CrossPlatformCli => 5,
        PackageReviewRepresentationTargetProfile::LocalUnchecked => 6,
    });
    encoder.byte(match target.architecture() {
        PackageReviewRepresentationArchitecture::Aarch64 => 0,
        PackageReviewRepresentationArchitecture::X86_64 => 1,
    });
    encoder.byte(match target.object_format() {
        PackageReviewRepresentationObjectFormat::Elf => 0,
        PackageReviewRepresentationObjectFormat::MachO => 1,
        PackageReviewRepresentationObjectFormat::Coff => 2,
    });
    encoder.u16(target.pointer_size());
    encoder.u16(target.pointer_alignment());
}

fn encode_boundary_shape_graph(
    encoder: &mut Encoder,
    graph: &PackageReviewBoundaryShapeGraph,
) -> Result<(), PackageReviewEncodingError> {
    encoder.sequence(graph.shapes(), |encoder, shape| {
        match shape.class() {
            PackageReviewBoundaryShapeClass::Integer => encoder.byte(0),
            PackageReviewBoundaryShapeClass::Float => encoder.byte(1),
            PackageReviewBoundaryShapeClass::Reference => encoder.byte(2),
            PackageReviewBoundaryShapeClass::FixedArray { element, length } => {
                encoder.byte(3);
                encoder.u16(element);
                encoder.u16(length);
            }
            PackageReviewBoundaryShapeClass::Record {
                first_field,
                field_count,
            } => {
                encoder.byte(4);
                encoder.u16(first_field);
                encoder.u16(field_count);
            }
        }
        encoder.u16(shape.byte_size());
        encoder.u16(shape.alignment());
        Ok(())
    })?;
    encoder.sequence(graph.fields(), |encoder, field| {
        encoder.u16(field.shape());
        encoder.u16(field.byte_offset());
        Ok(())
    })?;
    encoder.sequence(graph.parameters(), |encoder, root| {
        encoder.u16(*root);
        Ok(())
    })?;
    encoder.option(graph.result().as_ref(), |encoder, root| {
        encoder.u16(*root);
        Ok(())
    })
}

fn encode_opaque_occurrence(
    encoder: &mut Encoder,
    occurrence: &PackageReviewOpaqueRepresentationOccurrence,
) -> Result<(), PackageReviewEncodingError> {
    encoder.u16(occurrence.carrier_shape_root());
    match occurrence.role() {
        PackageReviewOpaqueRepresentationMovementRole::Parameter {
            formal_ordinal,
            native_ordinal,
        } => {
            encoder.byte(0);
            encoder.u32(formal_ordinal);
            encoder.u32(native_ordinal);
        }
        PackageReviewOpaqueRepresentationMovementRole::Result => encoder.byte(1),
    }
    encoder.sequence(occurrence.path(), |encoder, element| {
        match element {
            PackageReviewOpaqueRepresentationPathElement::FixedArrayElement => encoder.byte(0),
            PackageReviewOpaqueRepresentationPathElement::RecordField { ordinal } => {
                encoder.byte(1);
                encoder.u16(*ordinal);
            }
        }
        Ok(())
    })?;
    encode_value_placement(encoder, occurrence.placement())
}

fn encode_value_placement(
    encoder: &mut Encoder,
    placement: &PackageReviewBoundaryValuePlacement,
) -> Result<(), PackageReviewEncodingError> {
    let shape = placement.shape();
    match shape.class() {
        PackageReviewBoundaryValueClass::Integer => encoder.byte(0),
        PackageReviewBoundaryValueClass::Float => encoder.byte(1),
        PackageReviewBoundaryValueClass::HomogeneousFloatAggregate { members } => {
            encoder.byte(2);
            encoder.byte(members);
        }
        PackageReviewBoundaryValueClass::SystemVAggregate { first, second } => {
            encoder.byte(3);
            encoder.byte(system_v_class_tag(first));
            encoder.byte(system_v_class_tag(second));
        }
    }
    encoder.u16(shape.byte_size());
    encoder.u16(shape.alignment());
    encoder.sequence(placement.locations(), encode_value_location)
}

fn encode_value_location(
    encoder: &mut Encoder,
    location: &PackageReviewBoundaryValueLocation,
) -> Result<(), PackageReviewEncodingError> {
    match *location {
        PackageReviewBoundaryValueLocation::Register {
            register,
            value_byte_offset,
            byte_size,
        } => {
            encoder.byte(0);
            encode_machine_register(encoder, register);
            encoder.u16(value_byte_offset);
            encoder.u16(byte_size);
        }
        PackageReviewBoundaryValueLocation::Stack {
            stack_byte_offset,
            value_byte_offset,
            byte_size,
            alignment,
        } => {
            encoder.byte(1);
            encoder.u32(stack_byte_offset);
            encoder.u16(value_byte_offset);
            encoder.u16(byte_size);
            encoder.u16(alignment);
        }
        PackageReviewBoundaryValueLocation::Indirect {
            pointer,
            copy_stack_byte_offset,
            byte_size,
            alignment,
        } => {
            encoder.byte(2);
            match pointer {
                PackageReviewIndirectPointerLocation::Register(register) => {
                    encoder.byte(0);
                    encode_machine_register(encoder, register);
                }
                PackageReviewIndirectPointerLocation::Stack {
                    stack_byte_offset,
                    alignment,
                } => {
                    encoder.byte(1);
                    encoder.u32(stack_byte_offset);
                    encoder.u16(alignment);
                }
            }
            encoder.option(copy_stack_byte_offset.as_ref(), |encoder, offset| {
                encoder.u32(*offset);
                Ok(())
            })?;
            encoder.u16(byte_size);
            encoder.u16(alignment);
        }
    }
    Ok(())
}

const fn system_v_class_tag(class: PackageReviewSystemVEightbyteClass) -> u8 {
    match class {
        PackageReviewSystemVEightbyteClass::Integer => 0,
        PackageReviewSystemVEightbyteClass::Sse => 1,
    }
}

fn encode_machine_register(encoder: &mut Encoder, register: PackageReviewMachineRegister) {
    match register {
        PackageReviewMachineRegister::X86Rax => encoder.byte(0),
        PackageReviewMachineRegister::X86Rcx => encoder.byte(1),
        PackageReviewMachineRegister::X86Rdx => encoder.byte(2),
        PackageReviewMachineRegister::X86Rbx => encoder.byte(3),
        PackageReviewMachineRegister::X86Rsp => encoder.byte(4),
        PackageReviewMachineRegister::X86Rbp => encoder.byte(5),
        PackageReviewMachineRegister::X86Rsi => encoder.byte(6),
        PackageReviewMachineRegister::X86Rdi => encoder.byte(7),
        PackageReviewMachineRegister::X86R8 => encoder.byte(8),
        PackageReviewMachineRegister::X86R9 => encoder.byte(9),
        PackageReviewMachineRegister::X86R10 => encoder.byte(10),
        PackageReviewMachineRegister::X86R11 => encoder.byte(11),
        PackageReviewMachineRegister::X86R12 => encoder.byte(12),
        PackageReviewMachineRegister::X86R13 => encoder.byte(13),
        PackageReviewMachineRegister::X86R14 => encoder.byte(14),
        PackageReviewMachineRegister::X86R15 => encoder.byte(15),
        PackageReviewMachineRegister::X86Xmm(index) => {
            encoder.byte(16);
            encoder.byte(index);
        }
        PackageReviewMachineRegister::Aarch64X(index) => {
            encoder.byte(17);
            encoder.byte(index);
        }
        PackageReviewMachineRegister::Aarch64V(index) => {
            encoder.byte(18);
            encoder.byte(index);
        }
    }
}

const fn calling_policy_tag(policy: PackageReviewBoundaryCallingPolicy) -> u8 {
    match policy {
        PackageReviewBoundaryCallingPolicy::MicrosoftX64 => 0,
        PackageReviewBoundaryCallingPolicy::SystemVAMD64 => 1,
        PackageReviewBoundaryCallingPolicy::Aapcs64 => 2,
        PackageReviewBoundaryCallingPolicy::LinuxSyscallX86_64 => 3,
        PackageReviewBoundaryCallingPolicy::LinuxSyscallAarch64 => 4,
    }
}

pub(crate) fn encode_dangerous_authority(
    encoder: &mut Encoder,
    authority: &PackageReviewDangerousAuthority,
) -> Result<(), PackageReviewEncodingError> {
    encoder.byte(match authority.class {
        PackageReviewDangerousAuthorityClass::Filesystem => 0,
        PackageReviewDangerousAuthorityClass::MachineControl => 1,
        PackageReviewDangerousAuthorityClass::PortIo => 2,
        PackageReviewDangerousAuthorityClass::InterruptControl => 3,
        PackageReviewDangerousAuthorityClass::InterruptEntry => 4,
        PackageReviewDangerousAuthorityClass::RootMemory => 5,
        PackageReviewDangerousAuthorityClass::Process => 6,
    });
    encode_nominal(encoder, &authority.service)
}

pub(crate) fn encode_dangerous_authority_slack(
    encoder: &mut Encoder,
    slack: &PackageReviewDangerousAuthoritySlack,
) -> Result<(), PackageReviewEncodingError> {
    encoder.byte(match slack.class {
        PackageReviewDangerousAuthorityClass::Filesystem => 0,
        PackageReviewDangerousAuthorityClass::MachineControl => 1,
        PackageReviewDangerousAuthorityClass::PortIo => 2,
        PackageReviewDangerousAuthorityClass::InterruptControl => 3,
        PackageReviewDangerousAuthorityClass::InterruptEntry => 4,
        PackageReviewDangerousAuthorityClass::RootMemory => 5,
        PackageReviewDangerousAuthorityClass::Process => 6,
    });
    encode_nominal(encoder, &slack.callable)?;
    encode_nominal(encoder, &slack.service)
}
