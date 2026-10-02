//! Byte-sequence operations: reading, writing, measuring and slicing a
//! byte sequence, each validated against its retained evidence.

pub(crate) mod concatenation;
pub(crate) mod length;
pub(crate) mod predicate;
pub(crate) mod read;
pub(crate) mod subslice;
pub(crate) mod write;

pub(crate) mod store;
