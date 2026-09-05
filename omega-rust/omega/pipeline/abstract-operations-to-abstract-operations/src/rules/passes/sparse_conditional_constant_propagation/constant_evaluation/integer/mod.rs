//! Optimizer module role: stage group. Integer constant-evaluation taxonomy.
//!
//! `binary` maps all 22 arithmetic, shift, quotient, and bitwise rule entrances;
//! `exact_integer_cast_constants` owns the proof-certified cast entrance;
//! `unary` maps widening and bitwise-not to their exact entrances; `facts` is
//! the shared read-only scalar-constant/type lookup seam.

mod binary;
mod exact_integer_cast_constants;
mod facts;
mod unary;

pub use binary::*;
pub use exact_integer_cast_constants::ExactIntegerCastConstantsRule;
pub use unary::*;

pub(super) use facts::integer_constant;
pub(in crate::rules::passes::sparse_conditional_constant_propagation) use facts::integer_value_type;
