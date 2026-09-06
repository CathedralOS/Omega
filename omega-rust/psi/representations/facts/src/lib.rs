#![forbid(unsafe_code)]

//! Durable target-neutral facts and their current storage.

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
