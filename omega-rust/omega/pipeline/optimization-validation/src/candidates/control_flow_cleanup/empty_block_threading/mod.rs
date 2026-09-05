//! Optimizer module role: stage group. Empty-block threading validation.

use super::*;

mod linear;
mod path_qualified;

pub use linear::validate_linear_empty_block_candidate;
pub use path_qualified::validate_path_qualified_empty_block_candidate;
