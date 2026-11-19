//! Operands carried unchanged from abstract operations into target and machine records.
//! Each operand describes a value, call, block binding, integer operation or storage access.

#![forbid(unsafe_code)]

pub mod access_lifecycles;
pub mod access_references;
pub mod calls;
pub mod integer_arithmetic;
pub mod structural_stores;
pub mod values;
