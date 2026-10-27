use super::encode_value_placement;
use crate::encoding::{PackageReviewEncodingError, encode::encoder::Encoder};
use crate::record::{
    PackageReviewBoundaryShapeClass, PackageReviewBoundaryShapeGraph,
    PackageReviewOpaqueRepresentationMovementRole, PackageReviewOpaqueRepresentationOccurrence,
    PackageReviewOpaqueRepresentationPathElement, PackageReviewRepresentationArchitecture,
    PackageReviewRepresentationObjectFormat, PackageReviewRepresentationTarget,
    PackageReviewRepresentationTargetProfile,
};

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
