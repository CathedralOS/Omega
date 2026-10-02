//! The `self` keyword that the receiver spelling binds a machine's data to.
//!
//! `self` names the receiver an attached machine or state body operates on:
//! the parameter marked `is_self` on the declaration. This module is the only
//! place that spelling lives. Producers that must emit it (the keyword table,
//! identifier construction, diagnostic text) use [`SELF_RECEIVER`]; every
//! stage that asks "is this name the receiver keyword?" calls
//! [`is_self_receiver`] or the thin wrapper each stage's identifier type
//! provides.
//!
//! This is a question about one keyword, not about which binding holds a
//! machine's data: under the static-machine contract that binding is an
//! ordinary parameter with an authored name. Stages that ask what a rendered
//! place is rooted at use [`crate::place_root`], which takes the root name
//! rather than assuming this one.

/// The spelling of the receiver keyword.
pub const SELF_RECEIVER: &str = "self";

/// Whether `name` spells the receiver keyword.
///
/// This is a spelling test, not a resolution: a shadowing local cannot be
/// named `self`, so a name that spells the keyword is the receiver.
pub fn is_self_receiver(name: &str) -> bool {
    name == SELF_RECEIVER
}

#[cfg(test)]
mod tests {
    use super::is_self_receiver;

    #[test]
    fn only_the_bare_keyword_spells_the_receiver() {
        assert!(is_self_receiver("self"));
        assert!(!is_self_receiver("selfish"));
        assert!(!is_self_receiver("main"));
        assert!(!is_self_receiver(""));
    }
}
