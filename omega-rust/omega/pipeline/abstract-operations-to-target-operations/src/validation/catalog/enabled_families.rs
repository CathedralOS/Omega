//! Sole ordered enable/disable inventory for exact abstract-to-target function families.

use super::dispatch;
use super::model::TranslationFamilyDescriptor;

pub(super) const ENABLED_TRANSLATION_FAMILIES: &[TranslationFamilyDescriptor] = &[
    dispatch::terminal::UNIT_RETURN,
    dispatch::terminal::PORT_WRITE_UNIT_RETURN,
    dispatch::terminal::UNIT_CALL_RETURN,
    dispatch::terminal::BYTE_SEQUENCE_LITERAL_UNIT_RETURN,
    dispatch::terminal::INTEGER_LITERAL_UNIT_RETURN,
    dispatch::terminal::INTEGER_LITERAL_SEQUENCE_UNIT_RETURN,
    dispatch::terminal::IEEE_FLOAT_LITERAL_UNIT_RETURN,
    dispatch::terminal::IEEE_FLOAT_LITERAL_SEQUENCE_UNIT_RETURN,
    dispatch::terminal::INTEGER_IEEE_FLOAT_LITERAL_SEQUENCE_UNIT_RETURN,
    dispatch::terminal::NEAREST_IEEE_FLOAT_FUSED_MULTIPLY_ADD_UNIT_RETURN,
    dispatch::terminal::TRIVIAL_AFFINE_LOCAL_UNIT_RETURN,
    dispatch::structural::CALLER,
    dispatch::structural::CALLEE,
];
