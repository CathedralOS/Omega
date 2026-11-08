//! Canonical source-closure encoding by semantic responsibility.
//!
//! [`selection`] encodes root and dependency requests, [`source`] encodes
//! package/source identity, and [`framing`] owns primitive bounded bytes and
//! the domain-separated fingerprint.

pub(crate) mod framing;
mod selection;
mod source;

pub(super) use framing::{encode_hex, fingerprint};
pub(super) use selection::encode_subject;
