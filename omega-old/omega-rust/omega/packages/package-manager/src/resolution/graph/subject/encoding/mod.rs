/*
    We organize canonical source-closure binary encoding by semantic
    responsibility: `selection` encodes root and dependency requests, `source`
    encodes package and source identity, and `framing` owns primitive bounded
    bytes and the domain-separated fingerprint.
*/

/*
    Canonical subject binary encoding belongs to the source-closure subject
    boundary (`resolution/graph/subject/encoding/mod.rs`).
*/

pub(crate) mod framing;
mod selection;
mod source;

pub(super) use framing::{encode_hex, fingerprint};
pub(super) use selection::encode_subject;
