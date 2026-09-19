//!
//! `selected_forms.rs` encodes a selected form, `branch_forms.rs` the branch
//! forms and `movn_materialization.rs` immediates; `decoding.rs` reads the
//! words back and `encoding_types.rs` holds the carriers.

#[cfg(test)]
mod boolean_materialization_tests;
mod branch_forms;
#[cfg(test)]
mod byte_view_address_tests;
mod copy_bytes;
mod decoding;
mod encoding_types;
mod float_bits;
pub(crate) mod hosted_exit_process;
pub(crate) mod hosted_read_byte;
pub(crate) mod hosted_write_byte;
#[cfg(test)]
mod integer_normalization_tests;
mod jump;
mod memory;
mod movn_materialization;
mod normalized_foreign_call;
mod scalar_call;
mod selected_forms;
#[cfg(test)]
mod tests;

pub use branch_forms::{
    encode_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form,
    encode_aarch64_selected_i64_less_than_branch_form, encode_aarch64_selected_nonzero_branch_form,
    encode_aarch64_selected_u64_less_than_branch_form,
    validate_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form,
    validate_aarch64_selected_i64_less_than_branch_form,
    validate_aarch64_selected_nonzero_branch_form,
    validate_aarch64_selected_u64_less_than_branch_form,
};
pub use encoding_types::{
    Aarch64MovkPatch, Aarch64MovnSeed, Aarch64SelectedFormEncodingError,
    Aarch64SelectedFormFootprint, Aarch64ShortestMovnMaterializationRecipe,
    ValidatedAarch64SelectedFormEncoding,
};
pub use hosted_write_byte::{
    encode_aarch64_selected_hosted_write_byte_form,
    validate_aarch64_selected_hosted_write_byte_form,
};
pub use jump::*;
pub use memory::*;
pub use movn_materialization::{
    aarch64_shortest_movn_materialization_recipe, encode_aarch64_shortest_movn_materialization,
    validate_aarch64_shortest_movn_materialization,
};
pub use normalized_foreign_call::*;
pub use scalar_call::*;
pub use selected_forms::{encode_aarch64_selected_form, validate_aarch64_selected_form_encoding};
