use register_model::{RegisterConstraintFamily, RegisterConstraintKey, RegisterUnitId};
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
    bytes.extend_from_slice(&(target.pointer_size as u64).to_le_bytes());
    bytes.extend_from_slice(&(target.pointer_alignment as u64).to_le_bytes());
}

pub(super) fn encode_constraint_key(bytes: &mut Vec<u8>, key: RegisterConstraintKey) {
    bytes.push(match key.family {
        RegisterConstraintFamily::Call => 0,
        RegisterConstraintFamily::Return => 1,
        RegisterConstraintFamily::SystemCall => 2,
        RegisterConstraintFamily::InlineAssembly => 3,
        RegisterConstraintFamily::Instruction => 4,
    });
    bytes.extend_from_slice(&key.variant.to_le_bytes());
}

pub(super) fn encode_units(bytes: &mut Vec<u8>, units: &[RegisterUnitId]) {
    encode_len(bytes, units.len());
    for unit in units {
        bytes.extend_from_slice(&unit.0.to_le_bytes());
    }
}

pub(super) fn encode_ids(bytes: &mut Vec<u8>, values: impl ExactSizeIterator<Item = u64>) {
    encode_len(bytes, values.len());
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
}

pub(super) fn encode_u16s(bytes: &mut Vec<u8>, values: &[u16]) {
    encode_len(bytes, values.len());
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
}

pub(super) fn encode_len(bytes: &mut Vec<u8>, length: usize) {
    bytes.extend_from_slice(&(length as u64).to_le_bytes());
}
