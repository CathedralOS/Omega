//! Canonical source-closure text, version 2; independent of binary version 7.
//!
//! Fixed-order LF-terminated records start with `omega-source-closure 2`, then
//! `target`, `root` (role/request/selected source), `packages N` (each source,
//! navigation, `authored N` product requests, and `authored-build M` build
//! requests), `edges N` (requester key, purpose, ordinal, request, resolved
//! alias, selected source), and `end`. Keys expose name and lineage;
//! resolutions expose commit/tree/content. No field is an encoded binary
//! subject or admission result. Navigation remains separate from identity.
//!
//! Legacy `omega-source-closure 1` records remain readable as the versioned
//! lock migration: they carry no `authored-build` sections or `purpose` rows
//! and decode as product-only projections and edges. A recovered version-1
//! subject canonically re-encodes as version 2 text (and version-7 binary),
//! so retained locks are upgraded on their next write without broadening the
//! legacy product semantics they recorded.
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
