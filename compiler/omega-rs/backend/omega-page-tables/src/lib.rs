//! Target page-table materializers over normalized Extent mapping plans.
//!
//! This crate is provider-side engineering, not a source-language escape
//! hatch. It consumes only inert projections from [`omega_extents::PageTableDraft`]
//! and returns bytes. Installation authority and translation receipts remain
//! separate.

mod x86_64;

pub use x86_64::*;
