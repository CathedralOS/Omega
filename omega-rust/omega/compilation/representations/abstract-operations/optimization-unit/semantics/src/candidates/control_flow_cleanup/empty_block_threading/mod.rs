//! Optimizer module role: stage group. Empty-block threading validation.

mod linear;
mod path_qualified;

pub(crate) use linear::validate_linear_empty_block_candidate;
pub(crate) use path_qualified::validate_path_qualified_empty_block_candidate;
