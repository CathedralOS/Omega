//! Where an expression with no `;` after it may stand as a result.

/// Where an expression with no `;` after it is accepted as a result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TailRule {
    /// Only as the last executable thing in a body. This is the language's rule.
    LastInBody,

    /// Anywhere. Only fragments use this: the spec lists bare expressions one per line to show
    /// forms, and each ends where the next token cannot continue it.
    Anywhere,
}
