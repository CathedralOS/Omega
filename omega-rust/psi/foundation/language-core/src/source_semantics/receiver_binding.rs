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
//!
//! Rendered place paths (`self.field`, `self.items[0]`) are a second
//! vocabulary built on the same spelling: the receiver, the member separator,
//! then the path below the receiver. Stages that hold only the rendered text
//! build it with [`receiver_place_label`] and take it apart with
//! [`receiver_place_field`] or [`is_receiver_rooted`], so the `self.` prefix
//! is spelled here beside the receiver and nowhere else.

/// The spelling of the receiver binding.
pub const SELF_RECEIVER: &str = "self";

/// The separator between a rendered place path's root and its first member
/// (`self.field`). The typed-tree member renderer and the facts place
/// renderer spell their `.` from here, so a rendered member expression and
/// the canonical label of the place it names agree by construction.
pub const PLACE_MEMBER_SEPARATOR: char = '.';

/// Whether `name` spells the receiver binding of an attached machine or state.
///
/// This is a spelling test, not a resolution: a shadowing local cannot be
/// named `self`, so a name that spells the receiver is the receiver.
pub fn is_self_receiver(name: &str) -> bool {
    name == SELF_RECEIVER
}

/// Render the place reached from the receiver through `field`: `self.field`.
///
/// `field` is the rendered path below the receiver, so it may carry deeper
/// segments of its own (`items[0]`, `a.b`).
pub fn receiver_place_label(field: &str) -> String {
    let mut label = String::with_capacity(SELF_RECEIVER.len() + 1 + field.len());
    label.push_str(SELF_RECEIVER);
    label.push(PLACE_MEMBER_SEPARATOR);
    label.push_str(field);
    label
}

/// The rendered path below the receiver when `label` renders a place reached
/// from it (`self.a.b` gives `a.b`); `None` for the bare receiver, for a
/// place rooted elsewhere, and for a name that merely begins with the
/// receiver's letters (`selfish.a`).
pub fn receiver_place_field(label: &str) -> Option<&str> {
    label
        .strip_prefix(SELF_RECEIVER)?
        .strip_prefix(PLACE_MEMBER_SEPARATOR)
}

/// Whether `label` renders the receiver itself or a place reached from it.
pub fn is_receiver_rooted(label: &str) -> bool {
    is_self_receiver(label) || receiver_place_field(label).is_some()
}

#[cfg(test)]
mod tests {
    use super::{is_receiver_rooted, is_self_receiver, receiver_place_field, receiver_place_label};

    #[test]
    fn receiver_place_label_renders_the_member_path_below_the_receiver() {
        assert_eq!(receiver_place_label("count"), "self.count");
        assert_eq!(receiver_place_label("items[0]"), "self.items[0]");
        assert_eq!(receiver_place_label("a.b"), "self.a.b");
    }

    #[test]
    fn receiver_place_field_takes_apart_only_receiver_rooted_labels() {
        assert_eq!(receiver_place_field("self.count"), Some("count"));
        assert_eq!(receiver_place_field("self.a.b"), Some("a.b"));
        assert_eq!(receiver_place_field("self.items[0]"), Some("items[0]"));
        assert_eq!(receiver_place_field("self"), None);
        assert_eq!(receiver_place_field("selfish.a"), None);
        assert_eq!(receiver_place_field("target.count"), None);
        assert_eq!(receiver_place_field(""), None);
    }

    #[test]
    fn receiver_rooted_covers_the_receiver_and_places_below_it() {
        assert!(is_self_receiver("self"));
        assert!(is_receiver_rooted("self"));
        assert!(is_receiver_rooted("self.count"));
        assert!(!is_receiver_rooted("selfish"));
        assert!(!is_receiver_rooted("target.count"));
    }
}
