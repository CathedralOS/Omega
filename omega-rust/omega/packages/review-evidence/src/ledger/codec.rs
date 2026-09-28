//! The ordinary package obligation ledger's byte encoding, its decoding,
//! and the fingerprint over the encoded bytes.

mod decoding;
mod encoding;

pub use decoding::decode_ordinary_package_obligation_ledger;
pub use encoding::{
    encode_ordinary_package_obligation_ledger, ordinary_package_obligation_ledger_fingerprint,
};
