use super::{Error, Reader};
use crate::record::{
    PackageReviewBoundaryShape, PackageReviewBoundaryShapeClass, PackageReviewBoundaryShapeField,
    PackageReviewBoundaryShapeGraph, PackageReviewRepresentationArchitecture,
    PackageReviewRepresentationObjectFormat, PackageReviewRepresentationTarget,
    PackageReviewRepresentationTargetProfile,
};

pub(super) fn target(reader: &mut Reader<'_>) -> Result<PackageReviewRepresentationTarget, Error> {
    Ok(PackageReviewRepresentationTarget {
        profile: match reader.byte()? {
            0 => PackageReviewRepresentationTargetProfile::LinuxArm64,
            1 => PackageReviewRepresentationTargetProfile::LinuxX64,
            2 => PackageReviewRepresentationTargetProfile::MacosArm64,
            3 => PackageReviewRepresentationTargetProfile::WindowsX64,
            4 => PackageReviewRepresentationTargetProfile::UefiX64,
            5 => PackageReviewRepresentationTargetProfile::CrossPlatformCli,
            6 => PackageReviewRepresentationTargetProfile::LocalUnchecked,
            _ => return Err(Error::InvalidTag),
        },
        architecture: match reader.byte()? {
            0 => PackageReviewRepresentationArchitecture::Aarch64,
            1 => PackageReviewRepresentationArchitecture::X86_64,
            _ => return Err(Error::InvalidTag),
        },
        object_format: match reader.byte()? {
            0 => PackageReviewRepresentationObjectFormat::Elf,
            1 => PackageReviewRepresentationObjectFormat::MachO,
            2 => PackageReviewRepresentationObjectFormat::Coff,
            _ => return Err(Error::InvalidTag),
        },
        pointer_size: reader.u16()?,
        pointer_alignment: reader.u16()?,
    })
}

pub(super) fn graph(reader: &mut Reader<'_>) -> Result<PackageReviewBoundaryShapeGraph, Error> {
    Ok(PackageReviewBoundaryShapeGraph {
        shapes: reader.sequence(5, |reader| {
            Ok(PackageReviewBoundaryShape {
                class: match reader.byte()? {
                    0 => PackageReviewBoundaryShapeClass::Integer,
                    1 => PackageReviewBoundaryShapeClass::Float,
                    2 => PackageReviewBoundaryShapeClass::Reference,
                    3 => PackageReviewBoundaryShapeClass::FixedArray {
                        element: reader.u16()?,
                        length: reader.u16()?,
                    },
                    4 => PackageReviewBoundaryShapeClass::Record {
                        first_field: reader.u16()?,
                        field_count: reader.u16()?,
                    },
                    _ => return Err(Error::InvalidTag),
                },
                byte_size: reader.u16()?,
                alignment: reader.u16()?,
            })
        })?,
        fields: reader.sequence(4, |reader| {
            Ok(PackageReviewBoundaryShapeField {
                shape: reader.u16()?,
                byte_offset: reader.u16()?,
            })
        })?,
        parameters: reader.sequence(2, Reader::u16)?,
        result: reader.option(Reader::u16)?,
    })
}
