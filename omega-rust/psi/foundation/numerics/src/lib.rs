#![forbid(unsafe_code)]

//! Target-neutral exact numerics and source-literal payloads for Psi.
//!
//! Arbitrary-precision integers, exact arithmetic under the integer policy,
//! float semantics with their projection onto bounded carriers, and literal
//! payload parsing. Ranges and overflow are decided here once so every later
//! stage agrees with the numeric-values specification the README links.

pub mod arithmetic;
pub mod bignum;
pub mod float_projection;
pub mod float_semantics;
pub mod float_semantics_catalog;
pub mod integer_policy;
pub mod literals;
