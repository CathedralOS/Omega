//!
//! `selected_forms.rs` encodes a selected form, `branch_forms.rs` the branch
//! forms and `movn_materialization.rs` immediates; `decoding.rs` reads the
//! words back and `encoding_types.rs` holds the carriers.

pub(crate) mod atomic;
#[cfg(test)]
mod boolean_materialization_tests;
mod branch_forms;
#[cfg(test)]
mod byte_literal_address_tests;
#[cfg(test)]
mod byte_view_address_tests;
mod compare_byte_sequences;
mod copy_bytes;
pub(crate) mod crash;
mod decoding;
mod encoded_scalar;
mod encoding_types;
mod float_bits;
pub(crate) mod float_result_policy;
pub(crate) mod floating_control;
mod scan_byte_sequence;
pub(crate) mod trapping_forms;
mod zero_bytes;
pub use floating_control::{
    encode_aarch64_selected_floating_control_form};
pub(crate) mod hosted_exit_process;
pub(crate) mod hosted_read_byte;
pub(crate) mod hosted_write_byte;
pub(crate) mod ieee_float_arithmetic;
#[cfg(test)]
mod integer_normalization_tests;
mod jump;
mod memory;
mod movn_materialization;
mod normalized_foreign_call;
mod scalar_call;
mod selected_forms;
pub(crate) mod system_call;
#[cfg(test)]
mod tests;

pub use branch_forms::{
    AARCH64_CONDITIONAL_BRANCH_BYTE_REACH,
    encode_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form,
    encode_aarch64_selected_i64_less_than_branch_form,
    encode_aarch64_selected_i64_less_than_widened_branch_form,
    encode_aarch64_selected_nonzero_branch_form,
    encode_aarch64_selected_nonzero_widened_branch_form,
    encode_aarch64_selected_u64_less_than_branch_form,
    encode_aarch64_selected_u64_less_than_widened_branch_form,
    validate_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form,
    validate_aarch64_selected_i64_less_than_branch_form,
    validate_aarch64_selected_i64_less_than_widened_branch_form,
    validate_aarch64_selected_nonzero_branch_form,
    validate_aarch64_selected_nonzero_widened_branch_form,
    validate_aarch64_selected_u64_less_than_branch_form,
    validate_aarch64_selected_u64_less_than_widened_branch_form};
pub use encoding_types::{ Aarch64SelectedFormEncodingError,
    ValidatedAarch64SelectedFormEncoding};
pub(crate) use encoding_types::{
    Aarch64MovkPatch, Aarch64MovnSeed,
    Aarch64SelectedFormFootprint, Aarch64ShortestMovnMaterializationRecipe};
pub use hosted_write_byte::{
    encode_aarch64_selected_hosted_write_byte_form,
    validate_aarch64_selected_hosted_write_byte_form};
pub use jump::{encode_aarch64_selected_jump_form, validate_aarch64_selected_jump_form};
pub use memory::{encode_aarch64_selected_memory_form, validate_aarch64_selected_memory_form};

#[cfg(test)]
pub(crate) use movn_materialization::{
    aarch64_shortest_movn_materialization_recipe, encode_aarch64_shortest_movn_materialization,
    validate_aarch64_shortest_movn_materialization};
pub use normalized_foreign_call::{ Aarch64CallbackAddressMaterialization,
    Aarch64CallbackMaterializationDestination, Aarch64NormalizedForeignCallFixup,
    Aarch64NormalizedForeignCallFixupKind, Aarch64NormalizedForeignCallFixupState,
    Aarch64NormalizedForeignCallTemplateError,
    encode_aarch64_selected_normalized_foreign_call_template,
    validate_aarch64_selected_normalized_foreign_call_template};
pub use scalar_call::{ Aarch64ScalarCallFixup, Aarch64ScalarCallFixupKind,
    Aarch64ScalarCallFixupState, Aarch64ScalarCallTemplateError, encode_aarch64_selected_dynamic_call_template,
    encode_aarch64_selected_scalar_call_template, validate_aarch64_selected_scalar_call_template};

pub use selected_forms::{
    AARCH64_BYTE_LITERAL_ADDRESS_PAGE_FIELD_OFFSET,
    AARCH64_BYTE_LITERAL_ADDRESS_PAGE_OFFSET_FIELD_OFFSET, encode_aarch64_selected_form,
    validate_aarch64_selected_form_encoding};

/// The nzcv flag register units every flag-consuming form references.
fn nzcv_flag_units(
    physical: &register_model::ValidatedPhysicalRegisterModel,
) -> Result<Vec<register_model::RegisterUnitId>, Aarch64SelectedFormEncodingError> {
    Ok(physical
        .model()
        .view_named("nzcv")
        .ok_or(Aarch64SelectedFormEncodingError::EncodedFormMismatch)?
        .units
        .clone())
}

fn decode_all<D>(
    bytes: &[u8],
    decode_word: impl Fn(u32, usize) -> Result<D, Aarch64SelectedFormEncodingError>,
) -> Result<Vec<D>, Aarch64SelectedFormEncodingError> {
    let (words, tail) = bytes.as_chunks::<4>();
    if !tail.is_empty() {
        return Err(Aarch64SelectedFormEncodingError::MalformedEncoding);
    }
    words
        .iter()
        .enumerate()
        .map(|(position, word)| decode_word(u32::from_le_bytes(*word), position))
        .collect()
}
