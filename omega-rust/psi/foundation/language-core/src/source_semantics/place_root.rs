//! Rendered place paths and the binding each one is rooted at.
//!
//! A rendered place is a root name, the member separator, then the path below
//! the root: `main.start`, `self.items[0]`, `holder.child.value`. Stages that
//! hold only the rendered text build one with [`place_label`] and take one
//! apart with [`place_field`] or [`is_rooted_at`], so the separator is spelled
//! here and nowhere else.
//!
//! The root is supplied by the caller rather than fixed here, because which
//! binding holds a machine's data record is a property of the declaration, not
//! of this vocabulary. Under the receiver spelling the root is always `self`;
//! the same declaration written statically roots its places at an ordinary
//! parameter name (`main`). `TypedTrees::state_data_binding` answers which
//! name that is, and these functions take whatever it answers, so one
//! rootedness rule serves both spellings.

/// The separator between a rendered place path's root and its first member
/// (`main.field`). The typed-tree member renderer and the facts place
/// renderer spell their `.` from here, so a rendered member expression and
/// the canonical label of the place it names agree by construction.
pub const PLACE_MEMBER_SEPARATOR: char = '.';

/// Render the place reached from `root` through `field`: `main.field`.
///
/// `field` is the rendered path below the root, so it may carry deeper
/// segments of its own (`items[0]`, `a.b`).
pub fn place_label(root: &str, field: &str) -> String {
    let mut label = String::with_capacity(root.len() + 1 + field.len());
    label.push_str(root);
    label.push(PLACE_MEMBER_SEPARATOR);
    label.push_str(field);
    label
}

/// The rendered path below `root` when `label` renders a place reached from
/// it (`main.a.b` gives `a.b`); `None` for the bare root, for a place rooted
/// elsewhere, and for a name that merely begins with the root's letters
/// (`mainline.a` is not rooted at `main`).
pub fn place_field<'label>(root: &str, label: &'label str) -> Option<&'label str> {
    label
        .strip_prefix(root)?
        .strip_prefix(PLACE_MEMBER_SEPARATOR)
}

/// Whether `label` renders `root` itself or a place reached from it.
pub fn is_rooted_at(root: &str, label: &str) -> bool {
    label == root || place_field(root, label).is_some()
}

#[cfg(test)]
mod tests {
    use super::{is_rooted_at, place_field, place_label};

    #[test]
    fn place_label_renders_the_member_path_below_the_root() {
        assert_eq!(place_label("self", "count"), "self.count");
        assert_eq!(place_label("main", "count"), "main.count");
        assert_eq!(place_label("main", "items[0]"), "main.items[0]");
        assert_eq!(place_label("holder", "a.b"), "holder.a.b");
    }

    #[test]
    fn place_field_takes_apart_only_labels_rooted_at_the_given_name() {
        assert_eq!(place_field("self", "self.count"), Some("count"));
        assert_eq!(place_field("main", "main.a.b"), Some("a.b"));
        assert_eq!(place_field("main", "main.items[0]"), Some("items[0]"));
        assert_eq!(place_field("main", "main"), None);
        assert_eq!(place_field("main", "mainline.a"), None);
        assert_eq!(place_field("main", "other.count"), None);
        assert_eq!(place_field("main", ""), None);
        // The receiver spelling is one root among others, not a special case.
        assert_eq!(place_field("main", "self.count"), None);
    }

    #[test]
    fn rootedness_covers_the_root_and_places_below_it() {
        assert!(is_rooted_at("main", "main"));
        assert!(is_rooted_at("main", "main.count"));
        assert!(!is_rooted_at("main", "mainline"));
        assert!(!is_rooted_at("main", "other.count"));
        assert!(is_rooted_at("self", "self.count"));
        assert!(!is_rooted_at("self", "main.count"));
    }
}
