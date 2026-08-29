//! Canonical source-closure encoding by semantic responsibility.
//!
//! [`selection`] encodes root and dependency requests, [`source`] encodes
//! package/source identity, and [`framing`] owns primitive bounded bytes and
//! the domain-separated fingerprint.

mod framing;
mod selection;
mod source;

pub(super) use framing::{encode_hex, fingerprint, Decoder};
pub(super) use selection::{
    decode_dependency_selection, decode_package_navigation, decode_root_selection, encode_subject,
};
pub(super) use source::decode_source_identity;
