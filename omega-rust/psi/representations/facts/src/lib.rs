#![forbid(unsafe_code)]

//! Durable target-neutral facts and their current storage.
//!
//! Start at `fact_plan`: a `FactPlan` holds the facts checking established for
//! one program, organized as contexts, places, evidence, and write frames, and
//! the resolution helpers that map a place back to its member symbol. Later
//! stages read facts by handle; none of them re-derives a fact from source.

pub mod fact_plan;

pub use fact_plan::contexts::view::*;
pub use fact_plan::contexts::*;
pub use fact_plan::evidence::*;
pub(crate) use fact_plan::places::resolution::canonical_place_label;
pub use fact_plan::places::resolution::{
    effective_member_symbol, payload_variant_for_field, resolve_place_member_symbol,
};
pub use fact_plan::places::write_frame::*;
pub use fact_plan::places::*;
pub use fact_plan::*;
