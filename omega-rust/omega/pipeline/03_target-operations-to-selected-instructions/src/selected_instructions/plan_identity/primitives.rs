//! Canonical target-register identity primitives shared by selected families.

use crate::register_model::RegisterConstraintKey;

pub(super) fn encode_constraint_key(bytes: &mut Vec<u8>, key: RegisterConstraintKey) {
    bytes.push(match key.family {
        crate::register_model::RegisterConstraintFamily::Call => 0,
        crate::register_model::RegisterConstraintFamily::Return => 1,
        crate::register_model::RegisterConstraintFamily::SystemCall => 2,
        crate::register_model::RegisterConstraintFamily::InlineAssembly => 3,
        crate::register_model::RegisterConstraintFamily::Instruction => 4,
    });
    bytes.extend_from_slice(&key.variant.to_le_bytes());
}
