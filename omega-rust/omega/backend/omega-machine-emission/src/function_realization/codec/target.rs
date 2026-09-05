use super::super::prelude::*;
use super::cursor::Cursor;
use super::error::FunctionRelativeOptimizationRealizationManifestDecodeError;

pub(super) fn encode_target(bytes: &mut Vec<u8>, target: NativeTarget) {
    bytes.push(match target.architecture {
        Architecture::Aarch64 => 1,
        Architecture::X86_64 => 2,
    });
    bytes.push(match target.object_format {
        ObjectFormat::Elf => 1,
        ObjectFormat::MachO => 2,
        ObjectFormat::Coff => 3,
    });
    encode_usize(bytes, target.pointer_size);
    encode_usize(bytes, target.pointer_alignment);
}

pub(super) fn decode_target(
    cursor: &mut Cursor<'_>,
) -> Result<NativeTarget, FunctionRelativeOptimizationRealizationManifestDecodeError> {
    let architecture = match cursor.byte()? {
        1 => Architecture::Aarch64,
        2 => Architecture::X86_64,
        tag => {
            return Err(
                FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownArchitecture(
                    tag,
                ),
            );
        }
    };
    let object_format = match cursor.byte()? {
        1 => ObjectFormat::Elf,
        2 => ObjectFormat::MachO,
        3 => ObjectFormat::Coff,
        tag => {
            return Err(
                FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownObjectFormat(
                    tag,
                ),
            );
        }
    };
    let pointer_size = usize::try_from(u64::from_le_bytes(cursor.array()?)).map_err(|_| {
        FunctionRelativeOptimizationRealizationManifestDecodeError::TargetLayoutOverflow
    })?;
    let pointer_alignment = usize::try_from(u64::from_le_bytes(cursor.array()?)).map_err(|_| {
        FunctionRelativeOptimizationRealizationManifestDecodeError::TargetLayoutOverflow
    })?;
    Ok(NativeTarget {
        architecture,
        object_format,
        pointer_size,
        pointer_alignment,
    })
}

pub(super) const fn architecture_name(architecture: Architecture) -> &'static str {
    match architecture {
        Architecture::Aarch64 => "aarch64",
        Architecture::X86_64 => "x86_64",
    }
}

pub(super) const fn object_format_name(object_format: ObjectFormat) -> &'static str {
    match object_format {
        ObjectFormat::Elf => "elf",
        ObjectFormat::MachO => "macho",
        ObjectFormat::Coff => "coff",
    }
}

fn encode_usize(bytes: &mut Vec<u8>, value: usize) {
    bytes.extend_from_slice(
        &u64::try_from(value)
            .expect("function-relative realization value fits u64")
            .to_le_bytes(),
    );
}
