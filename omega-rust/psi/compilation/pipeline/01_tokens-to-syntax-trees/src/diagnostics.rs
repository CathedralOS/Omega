//! The parser's failure surface: `parse_error` is the public [`ParseError`]
//! and `render_diagnostic` renders the expected-token and end-of-input
//! diagnostics the grammar raises.
//!
//! [`ParseError`]: parse_error::ParseError

pub mod parse_error;
pub(crate) mod render_diagnostic;
