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
            PackageReviewRepresentationTargetProfile::LinuxX64 => encoder.tag("linux_x64", 1),
            PackageReviewRepresentationTargetProfile::MacosArm64 => encoder.tag("macos_arm64", 2),
            PackageReviewRepresentationTargetProfile::WindowsX64 => encoder.tag("windows_x64", 3),
            PackageReviewRepresentationTargetProfile::UefiX64 => encoder.tag("uefi_x64", 4),
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
    let _ = encoder.field("pointer_size", |encoder| {
        encoder.u16(target.pointer_size());
        Ok(())
    });
    let _ = encoder.field("pointer_alignment", |encoder| {
        encoder.u16(target.pointer_alignment());
        Ok(())
    });
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
                        encoder.field("element", |encoder| {
                            encoder.u16(element);
                            Ok(())
                        })?;
                        encoder.field("length", |encoder| {
                            encoder.u16(length);
                            Ok(())
                        })?;
                    }
                    PackageReviewBoundaryShapeClass::Record {
                        first_field,
                        field_count,
                    } => {
                        encoder.tag("record", 4);
                        encoder.field("first_field", |encoder| {
                            encoder.u16(first_field);
                            Ok(())
                        })?;
                        encoder.field("field_count", |encoder| {
                            encoder.u16(field_count);
                            Ok(())
                        })?;
                    }
                };
                Ok(())
            })?;
            encoder.field("byte_size", |encoder| {
                encoder.u16(shape.byte_size());
                Ok(())
            })?;
            encoder.field("alignment", |encoder| {
                encoder.u16(shape.alignment());
                Ok(())
            })?;
            Ok(())
        })
    })?;
    encoder.field("fields", |encoder| {
        encoder.sequence(graph.fields(), |encoder, field| {
            encoder.field("shape", |encoder| {
                encoder.u16(field.shape());
                Ok(())
            })?;
            encoder.field("byte_offset", |encoder| {
                encoder.u16(field.byte_offset());
                Ok(())
            })?;
            Ok(())
        })
    })?;
    encoder.field("parameters", |encoder| {
        encoder.sequence(graph.parameters(), |encoder, root| {
            encoder.field("root", |encoder| {
                encoder.u16(*root);
                Ok(())
            })?;
            Ok(())
        })
    })?;
    encoder.field("result", |encoder| {
        encoder.option(graph.result().as_ref(), |encoder, root| {
            encoder.field("root", |encoder| {
                encoder.u16(*root);
                Ok(())
            })?;
            Ok(())
        })
    })
}

pub(crate) fn encode_opaque_occurrence(
    encoder: &mut Encoder,
    occurrence: &PackageReviewOpaqueRepresentationOccurrence,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("carrier_shape_root", |encoder| {
        encoder.u16(occurrence.carrier_shape_root());
        Ok(())
    })?;
    encoder.field("role", |encoder| {
        match occurrence.role() {
            PackageReviewOpaqueRepresentationMovementRole::Parameter {
                formal_ordinal,
                native_ordinal,
            } => {
                encoder.tag("parameter", 0);
                encoder.field("formal_ordinal", |encoder| {
                    encoder.u32(formal_ordinal);
                    Ok(())
                })?;
                encoder.field("native_ordinal", |encoder| {
                    encoder.u32(native_ordinal);
                    Ok(())
                })?;
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
                        encoder.field("ordinal", |encoder| {
                            encoder.u16(*ordinal);
                            Ok(())
                        })?;
                    }
                };
                Ok(())
            })?;
            Ok(())
        })
    })?;
    encoder.field("placement", |encoder| {
        encode_value_placement(encoder, occurrence.placement())
    })
}
