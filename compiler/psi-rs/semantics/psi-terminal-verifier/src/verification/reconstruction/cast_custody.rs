//! Side-local reconstruction facade for exact integer-cast chain custody.

mod chain;
mod completion;
mod literal;

pub(super) use completion::retained_from_root;
pub(super) use literal::remap_integer_literal;
