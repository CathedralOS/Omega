//! Side-local custody facade for an exact integer-cast chain.

mod chain;
mod completion;
mod literal;

pub(super) use completion::prove_from_root;
pub(super) use literal::remap_integer_literal;
