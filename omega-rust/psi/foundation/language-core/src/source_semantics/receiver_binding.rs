//! The receiver binding of an attached machine or state.
//!
//! `self` names the receiver an attached machine or state body operates on:
//! the data whose fields the body reads and writes, the parameter marked
//! `is_self` on the declaration, and the head of a `self.field` place path.
//! This module is the only place that spelling lives. Producers that must emit
//! the spelling (the keyword table, identifier construction, diagnostic text)
//! use [`SELF_RECEIVER`]; every stage that asks "is this the receiver?" of a
//! name, parameter, or path head calls [`is_self_receiver`] or the thin
//! wrapper each stage's identifier type provides.

/// The spelling of the receiver binding.
pub const SELF_RECEIVER: &str = "self";

/// Whether `name` spells the receiver binding of an attached machine or state.
///
/// This is a spelling test, not a resolution: a shadowing local cannot be
/// named `self`, so a name that spells the receiver is the receiver.
pub fn is_self_receiver(name: &str) -> bool {
    name == SELF_RECEIVER
}
