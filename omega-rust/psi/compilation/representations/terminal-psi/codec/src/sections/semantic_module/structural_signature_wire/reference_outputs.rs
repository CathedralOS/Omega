//! Provider completion contracts participate in the canonical boundary bytes.
//! Omitting a lifetime anchor changes the portable declaration's identity.

use super::super::structural_place_wire::{decode_structural_path, encode_structural_path};
use super::{
    CodecError, Reader, Writer, decode_counted, decode_structural_access, encode_structural_access,
};
use terminal_psi::{
    BoundaryLifetimeDependency, BoundaryReferenceDestination, BoundaryReferenceOutput,
};

pub(super) fn encode(
    writer: &mut Writer,
    outputs: &[BoundaryReferenceOutput],
) -> Result<(), CodecError> {
    writer.len("boundary reference outputs", outputs.len())?;
    for output in outputs {
        match output.destination {
            BoundaryReferenceDestination::Parameter(position) => {
                writer.u8(0);
                writer.u32(position);
            }
        }
        encode_structural_path(writer, "boundary reference output path", &output.path)?;
        encode_structural_access(writer, output.access);
        writer.len(
            "boundary reference lifetime dependencies",
            output.dependencies.len(),
        )?;
        for dependency in &output.dependencies {
            match dependency {
                BoundaryLifetimeDependency::Referent {
                    argument_index,
                    access,
                } => {
                    writer.u8(0);
                    writer.u32(*argument_index);
                    encode_structural_access(writer, *access);
                }
                BoundaryLifetimeDependency::Stored {
                    argument_index,
                    path,
                    access,
                } => {
                    writer.u8(1);
                    writer.u32(*argument_index);
                    encode_structural_access(writer, *access);
                    encode_structural_path(writer, "boundary reference lifetime path", path)?;
                }
            }
        }
    }
    Ok(())
}

pub(super) fn decode(reader: &mut Reader<'_>) -> Result<Vec<BoundaryReferenceOutput>, CodecError> {
    decode_counted(reader, |reader| {
        let destination = match reader.u8()? {
            0 => BoundaryReferenceDestination::Parameter(reader.u32()?),
            tag => return Err(CodecError::InvalidTag("BoundaryReferenceDestination", tag)),
        };
        let path = decode_structural_path(reader)?;
        let access = decode_structural_access(reader)?;
        let dependencies = decode_counted(reader, |reader| match reader.u8()? {
            0 => Ok(BoundaryLifetimeDependency::Referent {
                argument_index: reader.u32()?,
                access: decode_structural_access(reader)?,
            }),
            1 => Ok(BoundaryLifetimeDependency::Stored {
                argument_index: reader.u32()?,
                access: decode_structural_access(reader)?,
                path: decode_structural_path(reader)?,
            }),
            tag => Err(CodecError::InvalidTag("BoundaryLifetimeDependency", tag)),
        })?;
        Ok(BoundaryReferenceOutput {
            destination,
            path,
            access,
            dependencies,
        })
    })
}
