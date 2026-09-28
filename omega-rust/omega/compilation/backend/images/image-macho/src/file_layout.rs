//! Where every byte goes: the fixed arm64 geometry, alignment rounding,
//! the one-pass offset and address plan, and the little- and big-endian
//! appenders.

pub(crate) mod bytes;
pub(crate) mod constants;
pub(crate) mod layout;
pub(crate) mod plan;
