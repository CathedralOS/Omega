//! Why an escape sequence in a quoted literal was refused.

/// Why an escape sequence is not one of the seven the spec allows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum QuotedEscapeError {
    /// The source ended right after the backslash.
    EndOfSource,

    /// `\u`: quoted literals are bytes, and the spec rejects codepoint escapes.
    Codepoint,

    /// `\x` not followed by two hexadecimal digits.
    InvalidHex,

    /// A backslash followed by anything else.
    Unknown,
}
