//! Canonical source-closure encoding by semantic responsibility.
//!
//! [`selection`] encodes root and dependency requests, [`source`] encodes
//! package/source identity, and [`framing`] owns primitive bounded bytes and
//! the domain-separated fingerprint.

mod framing;
mod projection;
mod selection;
mod source;

pub(super) use framing::{Decoder, encode_hex, fingerprint};
pub(super) use projection::{decode_dependency_projection, decode_target_profile};
pub(super) use selection::{
    decode_dependency_selection, decode_package_navigation, decode_root_selection, encode_subject,
};
pub(super) use source::decode_source_identity;
