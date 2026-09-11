//! Canonical target-register identity primitives shared by selected families.

use crate::selection::shared::*;

pub(super) fn encode_constraint_key(bytes: &mut Vec<u8>, key: RegisterConstraintKey) {
    bytes.push(match key.family {
        register_model::RegisterConstraintFamily::Call => 0,
        register_model::RegisterConstraintFamily::Return => 1,
        register_model::RegisterConstraintFamily::SystemCall => 2,
        register_model::RegisterConstraintFamily::InlineAssembly => 3,
        register_model::RegisterConstraintFamily::Instruction => 4,
    });
    bytes.extend_from_slice(&key.variant.to_le_bytes());
}
