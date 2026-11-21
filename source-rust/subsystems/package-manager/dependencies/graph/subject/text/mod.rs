/*
    Source-closure text records use one purpose-tagged format.

    Fixed-order LF-terminated records start with `omega-source-closure <ver>`,
    then `target`, `root` (role/request/selected source), `packages N` (each
    source, navigation, `authored N` product requests, and `authored-build M`
    build requests), `edges N` (requester key, purpose, ordinal, request,
    resolved alias, selected source), `catalog-dependencies N`, and
    `end`. Provider graphs contain no nested catalog rows; their observations
    are flattened at the outer level, keeping recovery bounded to two levels.

    String fields are quoted byte strings (`\"`, `\\`, and lowercase `\xhh` for
    non-printable bytes) so raw caller path spelling is preserved without
    assuming UTF-8, while semantic string fields still require UTF-8. Only
    canonical re-encoding is accepted, and both text and reconstructed binary
    independently obey the same record-byte limit.
*/

/*
    Canonical source-closure text representation belongs to the source-closure
    subject boundary (`resolution/graph/subject/text/mod.rs`).
*/

mod framing;
mod record;
mod requests;

mod source;
mod values;

use super::{
    CanonicalSourceClosureSubjectError as Error, CanonicalSourceClosureSubjectLimits as Limits,
};
