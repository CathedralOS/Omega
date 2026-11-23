//! Whether the machine header being parsed may have a body.

/// Whether a machine header may be followed by a body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BodyRule {
    /// A body, or `;` for a declaration with none.
    Optional,

    /// Always `;`: a requirement inside a trait.
    Refused,
}
