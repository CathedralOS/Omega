//! Interpret admitted layout records to read, stage and write storage bytes.
//! Symbolic writes resolve only their supplied data or entry coordinates. This
//! service never chooses a layout, activates source or generates executable code.
#![forbid(unsafe_code)]

mod materialization;
mod symbolic_materialization;
pub use materialization::field_codec::{
    decode_aggregate_field, decode_aggregate_field_into, stage_scalar_field, update_scalar_field,
};
pub use materialization::stored_integer_writes::apply_write;
pub use materialization::{
    decode_scalar_layout, materialize_aggregate_layout_into, materialize_resolved_into,
    materialize_scalar_layout_into, stage_aggregate_layout,
};
pub use symbolic_materialization::{
    derive_symbolic_materialization, derive_symbolic_materialization_with_inner_layouts,
};

#[cfg(test)]
use layout_plans::test_support;

#[cfg(test)]
mod contract_tests;
