//! Canonical encoding primitives for restart-stable review baselines.

mod bounds;
mod framing;
mod integrity;
mod package_sources;
mod review_records;

pub(super) use bounds::{clone_baseline_bytes, ensure_bounded_string};
pub(super) use framing::{Decoder, Encoder};
pub(super) use integrity::{capsule_checksum, replay_parent_binding};
pub(super) use package_sources::{
    decode_package_key, decode_resolution, encode_package_key, encode_resolution,
    validate_package_key_bounds,
};
pub(super) use review_records::{
    decode_replay_record_option, encode_replay_record_option, validate_recovery_row,
};
