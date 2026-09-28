//! An array constructor belongs to a declaration/return or one exact call
//! operand. Call coordinates retain authored occurrences; constructors never
//! invent call ordinals. The containing plan supplies state and statement.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedArrayConstructionSource {
    Statement,
    CallArgument {
        call_ordinal: u32,
        /// Authored formal position, not the filtered structural argument index.
        parameter_position: u32,
    },
}
