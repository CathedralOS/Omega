//! Lowering one scalar expression: state and unit scalars, returns, operand
//! landing, integer binary, bitwise and cast construction, and literal
//! landing and retagging.
//!
//! Indexed observations use one ordered projection plan for members and
//! selectors. A canonical place describes geometry but cannot retain an
//! executable selector's occurrence; read plans therefore keep the full
//! scalar operand and its exact indexing handle, independently of root location.
//!
//! Subtraction selects its result from operand roles, not storage context:
//! address offsets return addresses, while address differences return counts.
//! Anonymous offsets land as u64; named operands retain their source carriers.
//! Later Terminal reconstruction independently checks those signatures and
//! proves the mathematical difference fits its result before native lowering.

pub(crate) use validation::integer_widen_is_total;

pub(in crate::values::scalar) use element_selection::*;
pub(crate) use entry_lowering::*;
pub(crate) use integers::*;
pub(crate) use recast_reads::*;
mod element_selection;
mod entry_lowering;
mod integers;
mod recast_reads;
