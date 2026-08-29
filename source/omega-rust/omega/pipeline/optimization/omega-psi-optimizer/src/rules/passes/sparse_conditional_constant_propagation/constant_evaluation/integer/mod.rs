//! Integer constant-evaluation taxonomy.
//!
//! `binary` owns arithmetic, shifts, and bitwise binary rules; `cast` owns the
//! proof-certified exact cast; `unary` owns widening and bitwise-not; `facts`
//! is the shared read-only scalar-constant/type lookup seam.

mod binary;
mod cast;
mod facts;
mod unary;

pub use binary::*;
pub use cast::*;
pub use unary::*;

pub(super) use facts::integer_constant;
pub(in crate::rules::passes::sparse_conditional_constant_propagation) use facts::integer_value_type;
