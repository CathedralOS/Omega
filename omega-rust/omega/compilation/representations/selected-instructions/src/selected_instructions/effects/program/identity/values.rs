use register_model::RegisterUnitId;
use target::{Architecture, NativeTarget, ObjectFormat};

pub(super) fn encode_target(bytes: &mut Vec<u8>, target: NativeTarget) {
    bytes.push(match target.architecture {
        Architecture::Aarch64 => 0,
        Architecture::X86_64 => 1,
    });
    bytes.push(match target.object_format {
        ObjectFormat::Elf => 0,
        ObjectFormat::MachO => 1,
        ObjectFormat::Coff => 2,
    });
    crate::codec_bytes::push_encoded(bytes, (target.pointer_size as u64).to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, (target.pointer_alignment as u64).to_le_bytes());
}

pub(super) fn encode_units(bytes: &mut Vec<u8>, units: &[RegisterUnitId]) {
    encode_len(bytes, units.len());
    for unit in units {
        crate::codec_bytes::push_encoded(bytes, unit.0.to_le_bytes());
    }
}

pub fn encode_ids(bytes: &mut Vec<u8>, values: impl ExactSizeIterator<Item = u64>) {
    encode_len(bytes, values.len());
    for value in values {
        crate::codec_bytes::push_encoded(bytes, value.to_le_bytes());
    }
}

pub(super) fn encode_len(bytes: &mut Vec<u8>, length: usize) {
    crate::codec_bytes::push_encoded(bytes, (length as u64).to_le_bytes());
}
