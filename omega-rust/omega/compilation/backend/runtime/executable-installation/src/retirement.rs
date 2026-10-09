//! Retirement authorities, receipts and retired installations — the endings
//! of an installed realization.
//!
//! `uninstall.rs` is the drain-or-quarantine join, `replacement.rs` the
//! patch-then-drain join, and `quarantine.rs` the trapping reservation a
//! drain that cannot complete parks in.

pub(crate) mod quarantine;
pub(crate) mod replacement;
pub(crate) mod uninstall;

