//! Token input: `token_cursor` is the parser's cursor over the token stream,
//! `paths` joins identifier paths, `literals` reads integer and float
//! literal text with its suffixes, and `delimited` finds top-level
//! punctuation inside a delimited span.

mod delimited;
mod literals;
pub(crate) mod paths;
pub(crate) mod token_cursor;
