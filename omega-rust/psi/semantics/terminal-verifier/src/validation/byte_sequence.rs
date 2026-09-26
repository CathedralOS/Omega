//! Byte-sequence operations: reading, writing, measuring and slicing a
//! byte sequence, each validated against its retained evidence.

pub(crate) mod length;
pub(crate) mod read;
pub(crate) mod subslice;
pub(crate) mod write;
