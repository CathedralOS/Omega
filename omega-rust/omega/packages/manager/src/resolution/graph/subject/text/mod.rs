//! Canonical source-closure text, version 1; independent of binary version 6.
//!
//! Fixed-order LF-terminated records start with `omega-source-closure 1`, then
//! `target`, `root` (role/request/selected source), `packages N` (each source,
//! navigation, and `authored N` requests), `edges N` (requester key, ordinal,
//! request, resolved alias, selected source), and `end`. Keys expose name and
//! lineage; resolutions expose commit/tree/content. No field is an encoded
//! binary subject or admission result. Navigation remains separate from identity.
//!
//! String fields are quoted byte strings: printable ASCII is literal except
//! `\"` and `\\`; other bytes use lowercase `\xhh`. This preserves raw caller
//! path spelling without assuming UTF-8. Semantic string fields still require
//! UTF-8. Decimal counts have no leading zeros. Only canonical re-encoding is
//! accepted, including field order, spacing, escapes, and the final newline.
//! Text and reconstructed binary independently obey the same record-byte limit;
//! existing identity/request/count limits apply before semantic construction.

mod framing;
mod record;
mod requests;
mod source;
mod values;

use super::{
    CanonicalSourceClosureSubjectError as Error, CanonicalSourceClosureSubjectLimits as Limits,
};
