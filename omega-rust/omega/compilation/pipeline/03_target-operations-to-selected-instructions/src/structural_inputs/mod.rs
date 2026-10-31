//! Input-only reconstruction shared by legalization and selection: structural
//! storage and borrowed-pointer geometry, structural parameter shape with
//! incoming ABI storage. None of it
//! confers ownership or access authority.

pub(crate) mod structural_reference_input;
pub(crate) mod structural_unit_input;

// The frequent structural-reference queries are named once here: their callers
// ask for the shape or the index correspondence, not for the module layout.
pub(crate) use structural_reference_input::{parameter_shape, runtime_indices_match, shape};
