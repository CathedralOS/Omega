//! Reusing earlier work without trusting it: `compilation_reuse` decides exact
//! input equivalence, `checked_source_cache` keeps content-verified checked
//! source metadata, and `review_cache_digests` and `review_record_encoding`
//! key and encode the persisted compiled-review cache.

pub(crate) mod checked_source_cache;
pub(crate) mod compilation_reuse;
pub(crate) mod review_cache_digests;
pub mod review_record_encoding;
