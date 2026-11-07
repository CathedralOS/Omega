/*
Content-verified compilation reuse keys, checked-source metadata caching, and
compiled-review cache digests.

We group the mechanisms that let compilation and package review reuse earlier
work without weakening verification. `compilation_reuse` computes exact source
equivalence keys (`SourceAssemblyInputs`, `ProductSourceInputs`) for parse and
frontend checkpoints. `checked_source_cache` stores and replays verified
`CanonicalFilesystemMetadataIndex` records keyed by canonical root path and a
two-pass host stat fingerprint. `review_cache_digests` and
`review_record_encoding` compute content digests and canonical wire tags for the
persisted compiled-review cache.
*/

/*
Every cache key or stored index in this subsystem is derived data only, never an
admission shortcut. Replaying a cached metadata index requires two identical
stat traversals and full record re-validation, and reusing a frontend or review
checkpoint still requires the caller to verify live package closure, target
custody, and source consumption commitments.
*/

/*
See `checked_source_cache.rs` for the `OMGCSRC\x01` record store,
`compilation_reuse.rs` for frontend input equivalence, and
`review_cache_digests.rs` / `review_record_encoding.rs` for persisted review
cache keys and tags.
*/

pub(crate) mod checked_source_cache;
pub(crate) mod compilation_reuse;
pub(crate) mod review_cache_digests;
pub mod review_record_encoding;
